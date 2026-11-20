//! Comparación SEO local con candidatos revisables, crawl público seguro y evidencia versionada.
use crate::{eligibility, model::*, public_http, store, technical, web};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use scraper::{Html, Selector};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

const MAX_COMPETITORS: usize = 5;
const MAX_CANDIDATES: usize = 30;
const MAX_PAGES_PER_SITE: usize = 4;
const CACHE_DAYS: i64 = 30;
const METHOD_VERSION: &str = "seo-signals-v1";

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS seo_comparaciones(
 id TEXT PRIMARY KEY,
 negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,
 url_propia TEXT NOT NULL,
 huella TEXT NOT NULL,
 estado TEXT NOT NULL,
 sugerencias TEXT NOT NULL DEFAULT '[]',
 seleccion TEXT NOT NULL DEFAULT '[]',
 resultado TEXT NOT NULL DEFAULT '{}',
 creado TEXT NOT NULL,
 actualizado TEXT NOT NULL,
 job_id TEXT,
 error TEXT
);
CREATE INDEX IF NOT EXISTS idx_seo_comparaciones_negocio ON seo_comparaciones(negocio_id,creado DESC);
CREATE TABLE IF NOT EXISTS seo_sitios_cache(url TEXT PRIMARY KEY,consultado TEXT NOT NULL,metricas TEXT NOT NULL);
"#;

pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(7,?1)",
        [store::ahora()],
    )?;
    Ok(())
}

pub fn enabled() -> bool {
    !matches!(
        std::env::var("RADAR_SEO_COMPARE_ENABLED").ok().as_deref(),
        Some("0" | "false" | "off")
    )
}

fn parse_analysis(row: &rusqlite::Row<'_>) -> rusqlite::Result<SeoComparacion> {
    Ok(SeoComparacion {
        id: row.get(0)?,
        negocio_id: row.get(1)?,
        url_propia: row.get(2)?,
        estado: row.get(3)?,
        sugerencias: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
        seleccion: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
        resultado: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_else(|_| json!({})),
        creado: row.get(7)?,
        actualizado: row.get(8)?,
        job_id: row.get(9)?,
        error: row.get(10)?,
    })
}

pub fn get(
    conn: &Connection,
    business_id: i64,
    analysis_id: &str,
) -> Result<Option<SeoComparacion>> {
    conn.query_row("SELECT id,negocio_id,url_propia,estado,sugerencias,seleccion,resultado,creado,actualizado,job_id,error FROM seo_comparaciones WHERE negocio_id=?1 AND id=?2",params![business_id,analysis_id],parse_analysis).optional().map_err(Into::into)
}

pub fn list(conn: &Connection, business_id: i64) -> Result<Vec<SeoComparacion>> {
    let mut stmt=conn.prepare("SELECT id,negocio_id,url_propia,estado,sugerencias,seleccion,resultado,creado,actualizado,job_id,error FROM seo_comparaciones WHERE negocio_id=?1 ORDER BY creado DESC LIMIT 10")?;
    let rows = stmt
        .query_map([business_id], parse_analysis)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into);
    rows
}

fn propio(
    conn: &Connection,
    id: i64,
) -> Result<(
    String,
    String,
    String,
    Option<String>,
    Option<f64>,
    Option<f64>,
)> {
    let n = store::get_negocio(conn, id)?.context("Negocio no encontrado")?;
    let url=conn.query_row("SELECT url_confirmada FROM web_estado WHERE negocio_id=?1 AND ((estado='confirmada' AND origen IN ('correccion_usuario','confirmacion_usuario')) OR (estado='verificada_automatica' AND origen='verificacion_automatica'))",[id],|r|r.get::<_,Option<String>>(0)).optional()?.flatten().context("Confirma una web propia antes de comparar SEO")?;
    Ok((
        n.nombre,
        n.municipio,
        url,
        n.rubro.or(n.categoria),
        n.lat,
        n.lon,
    ))
}

fn distancia_km(a_lat: f64, a_lon: f64, b_lat: f64, b_lon: f64) -> f64 {
    let rad = std::f64::consts::PI / 180.0;
    let dlat = (b_lat - a_lat) * rad;
    let dlon = (b_lon - a_lon) * rad;
    let x = (dlat / 2.0).sin().powi(2)
        + a_lat.to_radians().cos() * b_lat.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    6371.0 * 2.0 * x.sqrt().atan2((1.0 - x).sqrt())
}

fn candidatos(
    conn: &Connection,
    id: i64,
    municipio: &str,
    rubro: Option<&str>,
    lat: Option<f64>,
    lon: Option<f64>,
    own_url: &str,
) -> Result<Vec<SeoCompetidor>> {
    let mut stmt=conn.prepare("SELECT n.id,n.nombre,n.municipio,n.rubro,n.categoria,n.lat,n.lon,n.web FROM negocios n JOIN web_estado w ON w.negocio_id=n.id WHERE n.id<>?1 AND n.admisible=1 AND n.web IS NOT NULL AND w.url_confirmada=n.web AND ((w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario')) OR (w.estado='verificada_automatica' AND w.origen='verificacion_automatica')) AND NOT EXISTS(SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=n.id) AND (n.rubro=?2 OR (n.rubro IS NULL AND n.sector=(SELECT sector FROM negocios WHERE id=?1)) OR n.municipio=?3) ORDER BY CASE WHEN n.municipio=?3 THEN 0 ELSE 1 END,n.id LIMIT 300")?;
    let rows = stmt
        .query_map(params![id, rubro, municipio], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let own_domain = eligibility::domain(own_url);
    let mut entries = Vec::new();
    for (bid, name, city, cat, legacy, clat, clon, url) in rows {
        let Some(domain) = eligibility::domain(&url) else {
            continue;
        };
        if Some(domain.clone()) == own_domain {
            continue;
        }
        let candidate_rubro = cat.or(legacy);
        let same_category = rubro.is_some_and(|r| candidate_rubro.as_deref() == Some(r));
        let distance = lat
            .zip(lon)
            .zip(clat.zip(clon))
            .map(|((a, b), (c, d))| distancia_km(a, b, c, d));
        let same_city = eligibility::normalize(&city) == eligibility::normalize(municipio);
        if !same_city && !distance.is_some_and(|d| d <= 30.0) {
            continue;
        }
        entries.push((
            same_category,
            same_city,
            distance.unwrap_or(999.0),
            SeoCompetidor {
                negocio_id: Some(bid),
                nombre: name,
                municipio: Some(city),
                url,
                puntuacion_ia: None,
                fuentes: vec![],
            },
        ));
    }
    entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.total_cmp(&b.2))
    });
    Ok(entries
        .into_iter()
        .take(MAX_CANDIDATES)
        .map(|x| x.3)
        .collect())
}

pub async fn sugerir(store_ref: &Arc<Mutex<Connection>>, id: i64) -> Result<SeoComparacion> {
    let (name, city, url, rubro, hash, candidates) = {
        let c = store_ref.lock().unwrap();
        let (name, city, url, rubro, lat, lon) = propio(&c, id)?;
        let hash = web::huella_evaluacion(&c, id)?;
        let candidates = candidatos(&c, id, &city, rubro.as_deref(), lat, lon, &url)?;
        (name, city, url, rubro, hash, candidates)
    };
    let analysis_id = uuid::Uuid::new_v4().to_string();
    {
        let c = store_ref.lock().unwrap();
        c.execute("INSERT INTO seo_comparaciones(id,negocio_id,url_propia,huella,estado,resultado,creado,actualizado) VALUES(?1,?2,?3,?4,'sugiriendo',?5,?6,?6)",params![analysis_id,id,url,hash,json!({"huella_entrada":hash}).to_string(),store::ahora()])?;
    }
    if candidates.is_empty() {
        let c = store_ref.lock().unwrap();
        c.execute("UPDATE seo_comparaciones SET estado='seleccion',sugerencias='[]',actualizado=?2 WHERE id=?1",params![analysis_id,store::ahora()])?;
        return get(&c, id, &analysis_id)?.context("No se pudo guardar el análisis");
    }
    {
        let c = store_ref.lock().unwrap();
        if !web::reservar_credito_ia(&c)? {
            c.execute("UPDATE seo_comparaciones SET estado='error',error='Cuota gratuita diaria de IA agotada',actualizado=?2 WHERE id=?1",params![analysis_id,store::ahora()])?;
            bail!("Cuota gratuita diaria de IA agotada; inténtalo mañana")
        }
    }
    let options=candidates.iter().map(|candidate|json!({"id":candidate.negocio_id,"nombre":candidate.nombre,"municipio":candidate.municipio,"url":candidate.url})).collect::<Vec<_>>();
    let ai = match crate::openrouter::score_competitors(&name, &city, rubro.as_deref(), &options)
        .await
    {
        Ok(result) => result,
        Err(error) => {
            let c = store_ref.lock().unwrap();
            c.execute(
                "UPDATE seo_comparaciones SET estado='error',error=?2,actualizado=?3 WHERE id=?1",
                params![analysis_id, error.to_string(), store::ahora()],
            )?;
            return Err(error);
        }
    };
    let refs = crate::openrouter::citations(&ai)
        .into_iter()
        .map(|(url, title)| json!({"url":url,"title":title}))
        .collect::<Vec<_>>();
    let answers = &ai["answers"];
    let mut suggestions = candidates;
    for candidate in &mut suggestions {
        if let Some(bid) = candidate.negocio_id {
            let answer = &answers[format!("competitor_{bid}")];
            let choice = answer["choice"].as_str().unwrap_or("not_comparable");
            let activity = answers[format!("activity_{bid}")]["choice"]
                .as_str()
                .unwrap_or("unknown");
            let organization = answers[format!("organization_{bid}")]["choice"]
                .as_str()
                .unwrap_or("unknown");
            if activity != "active" || organization != "independent" {
                candidate.puntuacion_ia = Some(0.0);
                continue;
            }
            candidate.puntuacion_ia = Some(match choice {
                "strong" => 3.0,
                "some" => 2.0,
                _ => 0.0,
            });
            candidate.fuentes = refs.clone();
        }
    }
    suggestions.retain(|candidate| candidate.puntuacion_ia.unwrap_or(0.0) > 0.0);
    suggestions.sort_by(|a, b| {
        b.puntuacion_ia
            .unwrap_or(0.0)
            .total_cmp(&a.puntuacion_ia.unwrap_or(0.0))
    });
    suggestions.truncate(MAX_COMPETITORS);
    let suggestions_json = serde_json::to_value(&suggestions)?;
    let c = store_ref.lock().unwrap();
    if web::huella_evaluacion(&c, id)? != hash {
        c.execute("UPDATE seo_comparaciones SET estado='obsoleta',sugerencias=?2,resultado=?3,actualizado=?4 WHERE id=?1",params![analysis_id,suggestions_json.to_string(),json!({"huella_entrada":hash,"fuentes":refs,"modelo":ai["model"]}).to_string(),store::ahora()])?;
    } else {
        c.execute("UPDATE seo_comparaciones SET estado='seleccion',sugerencias=?2,resultado=?3,actualizado=?4 WHERE id=?1",params![analysis_id,suggestions_json.to_string(),json!({"huella_entrada":hash,"fuentes":refs,"modelo":ai["model"],"uso":ai["usage"]}).to_string(),store::ahora()])?;
    }
    get(&c, id, &analysis_id)?.context("No se pudo recuperar la comparación sugerida")
}

pub fn guardar_seleccion(
    conn: &Connection,
    id: i64,
    analysis_id: &str,
    competitors: &[SeoCompetidor],
) -> Result<SeoComparacion> {
    anyhow::ensure!(
        !competitors.is_empty() && competitors.len() <= MAX_COMPETITORS,
        "Selecciona entre uno y cinco competidores"
    );
    let analysis = get(conn, id, analysis_id)?.context("Análisis no encontrado")?;
    anyhow::ensure!(
        analysis.estado == "seleccion"
            || analysis.estado == "terminado"
            || analysis.estado == "error",
        "El análisis no admite selección en este estado"
    );
    let mut domains = HashSet::new();
    let own = eligibility::domain(&analysis.url_propia);
    let mut selected = Vec::new();
    for candidate in competitors {
        let normalized = public_http::normalize(&candidate.url)?.to_string();
        let domain = eligibility::domain(&normalized).context("Dominio no válido")?;
        anyhow::ensure!(
            Some(domain.clone()) != own,
            "El dominio propio no puede ser competidor"
        );
        anyhow::ensure!(
            domains.insert(domain),
            "No repitas dominios en la selección"
        );
        anyhow::ensure!(
            !candidate.nombre.trim().is_empty() && candidate.nombre.trim().len() <= 160,
            "El nombre es obligatorio y no puede superar 160 caracteres"
        );
        anyhow::ensure!(
            candidate
                .municipio
                .as_ref()
                .is_none_or(|city| city.len() <= 160),
            "Localidad demasiado larga"
        );
        let (name, municipality, score, sources) = if let Some(bid) = candidate.negocio_id {
            let original = analysis
                .sugerencias
                .iter()
                .find(|suggestion| {
                    suggestion.negocio_id == Some(bid) && suggestion.url == normalized
                })
                .context("El negocio no está entre las sugerencias de este análisis")?;
            (
                original.nombre.clone(),
                original.municipio.clone(),
                original.puntuacion_ia,
                original.fuentes.clone(),
            )
        } else {
            (
                candidate.nombre.trim().to_string(),
                candidate.municipio.clone(),
                None,
                Vec::new(),
            )
        };
        selected.push(SeoCompetidor {
            url: normalized,
            nombre: name,
            municipio: municipality,
            negocio_id: candidate.negocio_id,
            puntuacion_ia: score,
            fuentes: sources,
        });
    }
    let result = json!({"metodo":METHOD_VERSION,"huella_entrada":analysis.resultado["huella_entrada"],"sitios":[],"comparativas":[],"informe":null});
    conn.execute("UPDATE seo_comparaciones SET seleccion=?3,estado='pendiente',error=NULL,resultado=?4,actualizado=?5 WHERE id=?1 AND negocio_id=?2",params![analysis_id,id,serde_json::to_string(&selected)?,result.to_string(),store::ahora()])?;
    get(conn, id, analysis_id)?.context("No se pudo guardar la selección")
}

pub fn enlazar_job(conn: &Connection, id: &str, job_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE seo_comparaciones SET job_id=?2,estado='pendiente',actualizado=?3 WHERE id=?1",
        params![id, job_id, store::ahora()],
    )?;
    Ok(())
}

fn store_site_progress(
    conn: &Connection,
    id: &str,
    state: &str,
    result: &Value,
    error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE seo_comparaciones SET estado=?2,resultado=?3,error=?4,actualizado=?5 WHERE id=?1",
        params![id, state, result.to_string(), error, store::ahora()],
    )?;
    Ok(())
}

fn html_signals(url: &str, page: &public_http::Page) -> Value {
    let document = Html::parse_document(&page.body);
    let text_sel = Selector::parse("body").unwrap();
    let text = document
        .select(&text_sel)
        .next()
        .map(|n| n.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let words = text
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| w.len() > 2 && !STOP.contains(&w.as_str()))
        .collect::<Vec<_>>();
    let mut terms: BTreeMap<String, usize> = BTreeMap::new();
    for word in words {
        *terms.entry(word).or_default() += 1;
    }
    let top_terms = terms.into_iter().collect::<Vec<_>>();
    let mut top_terms = top_terms;
    top_terms.sort_by(|a, b| b.1.cmp(&a.1));
    top_terms.truncate(30);
    let get_text = |selector: &str| {
        Selector::parse(selector).ok().and_then(|s| {
            document
                .select(&s)
                .next()
                .map(|n| n.text().collect::<Vec<_>>().join(" ").trim().to_string())
        })
    };
    let title = get_text("title");
    let description = Selector::parse("meta[name='description']")
        .ok()
        .and_then(|s| {
            document
                .select(&s)
                .next()
                .and_then(|n| n.value().attr("content").map(str::to_string))
        });
    let canonical = Selector::parse("link[rel='canonical']").ok().and_then(|s| {
        document
            .select(&s)
            .next()
            .and_then(|n| n.value().attr("href").map(str::to_string))
    });
    let robots = Selector::parse("meta[name='robots']")
        .ok()
        .and_then(|s| {
            document
                .select(&s)
                .next()
                .and_then(|n| n.value().attr("content").map(str::to_lowercase))
        })
        .unwrap_or_default();
    let h1 = Selector::parse("h1")
        .ok()
        .map(|s| {
            document
                .select(&s)
                .map(|n| n.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty())
                .take(4)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let headings = (1..=3)
        .map(|level| {
            let sel = Selector::parse(&format!("h{level}")).unwrap();
            json!({"nivel":level,"cantidad":document.select(&sel).count()})
        })
        .collect::<Vec<_>>();
    let links = Selector::parse("a[href]")
        .ok()
        .map(|s| {
            document
                .select(&s)
                .filter_map(|n| n.value().attr("href"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let own_domain = eligibility::domain(url);
    let mut internal = HashSet::new();
    let mut external = HashSet::new();
    let mut selected = Vec::new();
    for href in links {
        let Ok(base) = public_http::normalize(&page.url) else {
            continue;
        };
        let Ok(link) = base.join(href) else { continue };
        if !["http", "https"].contains(&link.scheme()) || link.query().is_some() {
            continue;
        }
        let link = public_http::normalize(link.as_str()).ok();
        let Some(link) = link else { continue };
        let Some(root) = eligibility::domain(link.as_str()) else {
            continue;
        };
        if Some(root.clone()) == own_domain {
            internal.insert(link.to_string());
            let path = link.path().to_lowercase();
            if selected.len() < 40
                && [
                    "servicio",
                    "producto",
                    "contact",
                    "sobre",
                    "about",
                    "tarifa",
                    "precio",
                    "categoria",
                    "servicios",
                    "producto",
                    "tienda",
                ]
                .iter()
                .any(|word| path.contains(word))
            {
                selected.push(link.to_string());
            }
        } else {
            external.insert(root);
        }
    }
    let mut schema = HashSet::new();
    if let Ok(sel) = Selector::parse("script[type='application/ld+json']") {
        for script in document.select(&sel).take(10) {
            if let Ok(value) = serde_json::from_str::<Value>(&script.text().collect::<String>()) {
                fn types(v: &Value, set: &mut HashSet<String>) {
                    match v {
                        Value::Object(o) => {
                            if let Some(t) = o.get("@type") {
                                if let Some(s) = t.as_str() {
                                    set.insert(s.to_string());
                                } else if let Some(a) = t.as_array() {
                                    for x in a {
                                        if let Some(s) = x.as_str() {
                                            set.insert(s.to_string());
                                        }
                                    }
                                }
                            }
                            for v in o.values() {
                                types(v, set)
                            }
                        }
                        Value::Array(a) => {
                            for v in a {
                                types(v, set)
                            }
                        }
                        _ => {}
                    }
                }
                types(&value, &mut schema);
            }
        }
    }
    let body_hash = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(page.body.as_bytes()))
    };
    json!({"url":page.url,"status":page.status,"elapsed_ms":page.elapsed_ms,"truncado":page.truncated,"titulo":title,"titulo_len":title.as_ref().map(|s|s.chars().count()),"descripcion":description,"descripcion_len":description.as_ref().map(|s|s.chars().count()),"canonical":canonical,"noindex":robots.contains("noindex"),"h1":h1,"encabezados":headings,"palabras":text.split_whitespace().count(),"terminos":top_terms,"enlaces_internos":internal.len(),"enlaces_externos":external.len(),"schema_types":schema,"body_sha256":body_hash,"links_selected":selected})
}

const STOP: &[&str] = &[
    "para", "con", "por", "las", "los", "una", "uno", "del", "que", "the", "and", "for", "with",
    "from", "this", "that", "www", "com", "http", "https", "navarra", "empresa", "empresas",
];

async fn crawl_site(url: &str) -> Result<Value> {
    let url = public_http::normalize(url)?.to_string();
    if !public_http::robots_allowed(&url).await? {
        return Ok(
            json!({"url":url,"estado":"bloqueado_robots","paginas":[],"cobertura":{"visitadas":0,"omitidas":1}}),
        );
    }
    let first = public_http::fetch(&url, 12).await?;
    let mut pages = vec![html_signals(&url, &first)];
    let mut links = pages[0]["links_selected"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    links.sort();
    links.dedup();
    for link in links.into_iter().take(MAX_PAGES_PER_SITE - 1) {
        if public_http::robots_allowed(&link).await.unwrap_or(false) {
            if let Ok(page) = public_http::fetch(&link, 10).await {
                if page.status < 500 {
                    pages.push(html_signals(&link, &page));
                }
            }
        }
    }
    let robots = public_http::fetch(&format!("{}/robots.txt", url.trim_end_matches('/')), 8)
        .await
        .ok();
    let sitemap_url = robots
        .as_ref()
        .and_then(|p| {
            p.body.lines().find_map(|line| {
                line.strip_prefix("Sitemap:")
                    .or_else(|| line.strip_prefix("SITEMAP:"))
                    .map(str::trim)
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| format!("{}/sitemap.xml", url.trim_end_matches('/')));
    let sitemap = public_http::fetch(&sitemap_url, 8).await.ok();
    let mut word_terms = BTreeMap::<String, usize>::new();
    let mut schema_types = HashSet::new();
    let mut title_missing = 0;
    let mut description_missing = 0;
    let mut h1_missing = 0;
    let mut words = 0;
    let mut internal = 0;
    let mut elapsed = 0;
    let mut indexable = 0;
    for p in &pages {
        if p["titulo"].is_null() {
            title_missing += 1;
        }
        if p["descripcion"].is_null() {
            description_missing += 1;
        }
        if p["h1"].as_array().is_none_or(Vec::is_empty) {
            h1_missing += 1;
        }
        if p["status"]
            .as_u64()
            .is_some_and(|x| (200..400).contains(&x))
            && !p["noindex"].as_bool().unwrap_or(false)
        {
            indexable += 1;
        }
        words += p["palabras"].as_u64().unwrap_or(0);
        internal += p["enlaces_internos"].as_u64().unwrap_or(0);
        elapsed += p["elapsed_ms"].as_i64().unwrap_or(0);
        if let Some(ts) = p["terminos"].as_array() {
            for item in ts {
                if let Some(pair) = item.as_array() {
                    if let (Some(t), Some(c)) = (
                        pair.first().and_then(Value::as_str),
                        pair.get(1).and_then(Value::as_u64),
                    ) {
                        *word_terms.entry(t.to_string()).or_default() += c as usize;
                    }
                }
            }
        }
        if let Some(types) = p["schema_types"].as_array() {
            for t in types {
                if let Some(t) = t.as_str() {
                    schema_types.insert(t.to_string());
                }
            }
        }
    }
    let top_terms = word_terms.into_iter().collect::<Vec<_>>();
    let mut top_terms = top_terms;
    top_terms.sort_by(|a, b| b.1.cmp(&a.1));
    top_terms.truncate(40);
    Ok(
        json!({"url":url,"estado":"medido","consultado":store::ahora(),"paginas":pages,"cobertura":{"visitadas":pages.len(),"maximo":MAX_PAGES_PER_SITE,"bloqueadas":0},"metricas":{"paginas_indexables":indexable,"titulos_ausentes":title_missing,"descripciones_ausentes":description_missing,"h1_ausentes":h1_missing,"palabras_totales":words,"enlaces_internos":internal,"schema_types":schema_types,"terminos":top_terms,"latencia_media_ms":if pages.is_empty(){0}else{elapsed/pages.len() as i64},"sitemap_http":sitemap.as_ref().map(|p|p.status),"sitemap_detectado":sitemap.as_ref().is_some_and(|p|(200..300).contains(&p.status))}}),
    )
}

pub async fn ejecutar(
    store_ref: &Arc<Mutex<Connection>>,
    analysis_id: &str,
    mut job: Job,
    update: impl Fn(&mut Job),
) -> Result<()> {
    let analysis = {
        let c = store_ref.lock().unwrap();
        get_by_id(&c, analysis_id)?.context("Comparación SEO no encontrada")?
    };
    let selected = analysis.seleccion.clone();
    anyhow::ensure!(!selected.is_empty(), "No hay competidores seleccionados");
    job.estado = EstadoJob::EnCurso;
    job.total = (selected.len() + 1) as i64;
    job.hechos = 0;
    update(&mut job);
    {
        let c = store_ref.lock().unwrap();
        store_site_progress(&c, analysis_id, "en_curso", &analysis.resultado, None)?;
    }
    let current = {
        let c = store_ref.lock().unwrap();
        web::huella_evaluacion(&c, analysis.negocio_id)?
    };
    if current != analysis.resultado["huella_entrada"].as_str().unwrap_or("") {
        bail!("La ficha cambió desde que se eligieron competidores")
    }
    let audit_state = {
        let c = store_ref.lock().unwrap();
        let reports=technical::reports(&c,analysis.negocio_id)?.into_iter().filter(|report|report.url==analysis.url_propia&&matches!(report.estado.as_str(),"terminado"|"parcial")&&chrono::DateTime::parse_from_rfc3339(&report.fecha).ok().is_some_and(|date|date>chrono::Utc::now()-chrono::Duration::days(30))).map(|report|json!({"motor":report.motor,"dispositivo":report.dispositivo,"version":report.version,"fecha":report.fecha,"estado":report.estado,"resultado":report.resultado})).collect::<Vec<_>>();
        let recent = !reports.is_empty();
        let job = if recent {
            None
        } else {
            Some(technical::enqueue(&c, analysis.negocio_id)?)
        };
        json!({"reciente_reutilizable":recent,"informes_reutilizados":reports,"job_refresco":job.map(|j|j.id)})
    };
    let mut sites = analysis.resultado["sitios"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut all = vec![(
        "propia".to_string(),
        "Web propia".to_string(),
        analysis.url_propia.clone(),
    )];
    all.extend(selected.iter().enumerate().map(|(i, c)| {
        (
            format!("competidor_{}", i + 1),
            c.nombre.clone(),
            c.url.clone(),
        )
    }));
    for (idx, (key, name, url)) in all.into_iter().enumerate() {
        let saved = sites
            .iter()
            .find(|site| site["key"] == key)
            .map(|site| site["datos"].clone())
            .filter(|data| data["estado"] == "medido");
        let cached: Option<String> = if saved.is_some() {
            None
        } else {
            let c = store_ref.lock().unwrap();
            c.query_row("SELECT metricas FROM seo_sitios_cache WHERE url=?1 AND datetime(consultado)>datetime('now',?2)",params![url,format!("-{} days",CACHE_DAYS)],|r|r.get(0)).optional()?
        };
        let cached = cached.filter(|raw| {
            serde_json::from_str::<Value>(raw)
                .ok()
                .is_some_and(|value| value["metodo"] == METHOD_VERSION)
        });
        let mut data = if let Some(saved) = saved {
            saved
        } else if let Some(cached) = cached {
            serde_json::from_str(&cached)?
        } else {
            match crawl_site(&url).await {
                Ok(mut data) => {
                    data["metodo"] = json!(METHOD_VERSION);
                    let c = store_ref.lock().unwrap();
                    c.execute("INSERT INTO seo_sitios_cache(url,consultado,metricas) VALUES(?1,?2,?3) ON CONFLICT(url) DO UPDATE SET consultado=excluded.consultado,metricas=excluded.metricas",params![url,store::ahora(),data.to_string()])?;
                    data
                }
                Err(error) => {
                    json!({"url":url,"estado":"error","error":error.to_string(),"paginas":[],"cobertura":{"visitadas":0,"omitidas":1}})
                }
            }
        };
        if let Some(object) = data.as_object_mut() {
            object.insert("metodo".into(), json!(METHOD_VERSION));
        }
        if let Some(slot) = sites.iter_mut().find(|s| s["key"] == key) {
            *slot = json!({"key":key,"nombre":name,"url":url,"datos":data});
        } else {
            sites.push(json!({"key":key,"nombre":name,"url":url,"datos":data}));
        }
        let result = json!({"metodo":METHOD_VERSION,"huella_entrada":current,"auditoria_propia":audit_state,"sitios":sites,"comparativas":[],"informe":null});
        {
            let c = store_ref.lock().unwrap();
            store_site_progress(&c, analysis_id, "en_curso", &result, None)?;
        }
        job.hechos = (idx + 1) as i64;
        job.resumen = Some(json!({"sitios_listos":job.hechos,"total":job.total}));
        update(&mut job);
    }
    let mut result = json!({"metodo":METHOD_VERSION,"huella_entrada":current,"auditoria_propia":audit_state,"sitios":sites,"comparativas":comparisons(&sites),"informe":null});
    {
        let c = store_ref.lock().unwrap();
        if !web::reservar_credito_ia(&c)? {
            store_site_progress(
                &c,
                analysis_id,
                "error",
                &result,
                Some("Cuota gratuita diaria agotada antes de sintetizar el informe"),
            )?;
            bail!("Cuota gratuita diaria agotada; reintenta mañana")
        }
    }
    let business_name = {
        let c = store_ref.lock().unwrap();
        store::get_negocio(&c, analysis.negocio_id)?
            .context("Negocio no encontrado")?
            .nombre
    };
    let report =
        crate::openrouter::seo_report(&business_name, &analysis.url_propia, &result).await?;
    result["informe"] = json!({"texto":report.0,"modelo":report.1,"generado":store::ahora()});
    let c = store_ref.lock().unwrap();
    if web::huella_evaluacion(&c, analysis.negocio_id)? != current {
        store_site_progress(
            &c,
            analysis_id,
            "obsoleta",
            &result,
            Some("La ficha cambió durante el análisis; resultado no aplicado"),
        )?;
        bail!("La ficha cambió durante el análisis")
    }
    store_site_progress(&c, analysis_id, "terminado", &result, None)?;
    job.hechos = job.total;
    job.estado = EstadoJob::Terminado;
    job.resumen = Some(
        json!({"competidores":selected.len(),"paginas":sites.iter().map(|s|s["datos"]["cobertura"]["visitadas"].as_u64().unwrap_or(0)).sum::<u64>()}),
    );
    update(&mut job);
    Ok(())
}

fn get_by_id(conn: &Connection, id: &str) -> Result<Option<SeoComparacion>> {
    conn.query_row("SELECT id,negocio_id,url_propia,estado,sugerencias,seleccion,resultado,creado,actualizado,job_id,error FROM seo_comparaciones WHERE id=?1",[id],parse_analysis).optional().map_err(Into::into)
}

fn comparisons(sites: &[Value]) -> Value {
    let own = sites
        .iter()
        .find(|s| s["key"] == "propia")
        .map(|s| &s["datos"]["metricas"]);
    let Some(own) = own else { return json!([]) };
    let fields = [
        ("Páginas indexables", "paginas_indexables"),
        ("Títulos ausentes", "titulos_ausentes"),
        ("Descripciones ausentes", "descripciones_ausentes"),
        ("H1 ausentes", "h1_ausentes"),
        ("Palabras analizadas", "palabras_totales"),
        ("Enlaces internos", "enlaces_internos"),
        ("Latencia media ms", "latencia_media_ms"),
    ];
    sites.iter().filter(|s|s["key"]!="propia").map(|site|{let other=&site["datos"]["metricas"];json!({"competidor":site["nombre"],"url":site["url"],"metricas":fields.iter().map(|(label,key)|json!({"nombre":label,"propia":own[*key],"competidor":other[*key]})).collect::<Vec<_>>(),"tipos_schema":{"propia":own["schema_types"],"competidor":other["schema_types"]},"cobertura":{"propia":site_coverage(own),"competidor":site_coverage(other)}})}).collect::<Vec<_>>().into()
}
fn site_coverage(metrics: &Value) -> Value {
    json!({"paginas_indexables":metrics["paginas_indexables"],"titulos_ausentes":metrics["titulos_ausentes"],"descripciones_ausentes":metrics["descripciones_ausentes"],"h1_ausentes":metrics["h1_ausentes"]})
}
