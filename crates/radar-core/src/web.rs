//! Descubrimiento y confirmación de webs con procedencia y presupuesto explícitos.
use crate::{
    eligibility,
    model::*,
    public_http,
    store::{self, Store},
};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub const REGISTRY_URL:&str="https://datosabiertos.navarra.es/api/3/action/datastore_search?resource_id=0a197290-2e3b-4b0c-b383-4f1ec46d9468&limit=10000";
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS web_estado(negocio_id INTEGER PRIMARY KEY REFERENCES negocios(id) ON DELETE CASCADE,estado TEXT NOT NULL DEFAULT 'pendiente',url_confirmada TEXT,origen TEXT,confirmado_en TEXT,ultima_busqueda TEXT,consulta TEXT,aviso TEXT,generacion INTEGER NOT NULL DEFAULT 1);
CREATE TABLE IF NOT EXISTS web_candidatas(id INTEGER PRIMARY KEY,negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,url TEXT NOT NULL,titulo TEXT,tipo TEXT NOT NULL DEFAULT 'resultado_web',proveedor TEXT NOT NULL,consulta TEXT,posicion INTEGER,fecha TEXT NOT NULL,estado TEXT NOT NULL DEFAULT 'sugerida',evidencias TEXT NOT NULL DEFAULT '{}',UNIQUE(negocio_id,url));
CREATE TABLE IF NOT EXISTS web_search_cache(consulta TEXT PRIMARY KEY,resultados TEXT NOT NULL,creado TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS web_search_budget(periodo TEXT PRIMARY KEY,creditos INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS web_search_daily_budget(dia TEXT PRIMARY KEY,creditos INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS auditorias_tecnicas(id TEXT PRIMARY KEY,job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,url TEXT NOT NULL,motor TEXT NOT NULL,dispositivo TEXT NOT NULL,version TEXT NOT NULL DEFAULT '',estado TEXT NOT NULL DEFAULT 'pendiente',fecha TEXT NOT NULL,resultado TEXT NOT NULL DEFAULT '{}',archivos TEXT NOT NULL DEFAULT '{}',error TEXT,UNIQUE(job_id,motor,dispositivo));
CREATE INDEX IF NOT EXISTS idx_tecnica_negocio ON auditorias_tecnicas(negocio_id,fecha DESC);
CREATE INDEX IF NOT EXISTS idx_tecnica_negocio_url_fecha ON auditorias_tecnicas(negocio_id,url,fecha DESC);
CREATE TABLE IF NOT EXISTS auditoria_hallazgo_revision(negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,report_id TEXT NOT NULL REFERENCES auditorias_tecnicas(id) ON DELETE CASCADE,finding_key TEXT NOT NULL,estado TEXT NOT NULL,motivo TEXT,actualizado TEXT NOT NULL,PRIMARY KEY(report_id,finding_key));
CREATE INDEX IF NOT EXISTS idx_hallazgo_revision_negocio ON auditoria_hallazgo_revision(negocio_id,actualizado DESC);
CREATE TABLE IF NOT EXISTS diagnostico_visitas(negocio_id INTEGER PRIMARY KEY REFERENCES negocios(id) ON DELETE CASCADE,estado TEXT NOT NULL DEFAULT '{}',actualizado TEXT NOT NULL);
"#;
pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    conn.execute(
        "INSERT OR IGNORE INTO web_estado(negocio_id,estado) SELECT id,'pendiente' FROM negocios",
        [],
    )?;
    // Migrate source and automatic URLs into suggestions. Only an explicit
    // user correction remains a confirmed own website.
    let mut rows=conn.prepare("SELECT n.id,COALESCE(n.web,w.url_confirmada) FROM negocios n JOIN web_estado w ON w.negocio_id=n.id WHERE COALESCE(n.web,w.url_confirmada) IS NOT NULL AND (w.estado NOT IN ('confirmada','verificada_automatica') OR COALESCE(w.origen,'') NOT IN ('correccion_usuario','confirmacion_usuario','verificacion_automatica'))")?;
    let values = rows
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, url) in values {
        if public_http::normalize(&url).is_ok() {
            add_candidate(
                conn,
                id,
                &url,
                None,
                "migracion_fuente",
                None,
                None,
                &json!({"migracion":"web_no_confirmada"}),
            )?;
        }
        conn.execute("UPDATE web_estado SET estado='pendiente',url_confirmada=NULL,origen='migracion_fuente' WHERE negocio_id=?1",[id])?;
        conn.execute("UPDATE negocios SET web=NULL WHERE id=?1", [id])?;
    }
    let done: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=6)",
        [],
        |r| r.get(0),
    )?;
    if !done {
        let tx = conn.unchecked_transaction()?;
        tx.execute("UPDATE oportunidades_servicio SET score=MIN(score,40),motivo='Web no localizada: encaje provisional pendiente de contrastar.' WHERE servicio IN ('creacion_web','landing') AND negocio_id IN (SELECT id FROM negocios WHERE web IS NULL)",[])?;
        tx.execute(
            "INSERT INTO schema_migrations(version,applied_at) VALUES(6,?1)",
            [store::ahora()],
        )?;
        tx.commit()?;
    }
    Ok(())
}
pub fn search_query(n: &Negocio, address: Option<&str>, city: Option<&str>) -> String {
    let clean = |s: &str| s.replace('"', " ").trim().to_string();
    let mut parts = vec![format!("\"{}\"", clean(&n.nombre))];
    if let Some(a) = address
        .or(n.direccion.as_deref())
        .filter(|s| !s.trim().is_empty())
    {
        parts.push(format!("\"{}\"", clean(a)));
    }
    let city = city.unwrap_or(&n.municipio);
    if !city.trim().is_empty() {
        parts.push(clean(city));
    }
    parts.join(" ")
}
pub fn configured() -> bool {
    std::env::var("TAVILY_API_KEY").is_ok_and(|s| !s.trim().is_empty())
}
pub fn remaining(conn: &Connection) -> Result<i64> {
    let period = chrono::Utc::now().format("%Y-%m").to_string();
    let used = conn
        .query_row(
            "SELECT creditos FROM web_search_budget WHERE periodo=?1",
            [period],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok((1000 - used).max(0))
}
pub fn reserve_search_credit(conn: &Connection) -> Result<bool> {
    let month = chrono::Utc::now().format("%Y-%m").to_string();
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let tx = conn.unchecked_transaction()?;
    tx.execute("INSERT OR IGNORE INTO web_search_budget(periodo) VALUES(?1)", [&month])?;
    tx.execute("INSERT OR IGNORE INTO web_search_daily_budget(dia) VALUES(?1)", [&day])?;
    let monthly = tx.execute("UPDATE web_search_budget SET creditos=creditos+1 WHERE periodo=?1 AND creditos<1000", [&month])?;
    let daily = tx.execute("UPDATE web_search_daily_budget SET creditos=creditos+1 WHERE dia=?1 AND creditos<35", [&day])?;
    if monthly == 0 || daily == 0 {
        tx.rollback()?;
        return Ok(false);
    }
    tx.commit()?;
    Ok(true)
}

pub fn next_automatic_discovery(conn: &Connection) -> Result<Option<(i64, bool)>> {
    let candidato_revisable = "(json_extract(c.evidencias,'$.verificacion.revisada_en') IS NULL OR (json_extract(c.evidencias,'$.verificacion.resultado')='no_disponible' AND datetime(json_extract(c.evidencias,'$.verificacion.revisada_en'))<datetime('now','-1 day')) OR (json_extract(c.evidencias,'$.verificacion.resultado') NOT IN ('no_disponible','bloqueada_por_robots') AND datetime(json_extract(c.evidencias,'$.verificacion.revisada_en'))<datetime('now','-90 days')))";
    let query = format!("SELECT n.id,
           NOT EXISTS(SELECT 1 FROM web_candidatas c WHERE c.negocio_id=n.id AND c.estado='sugerida' AND c.tipo NOT IN ('red_social','directorio') AND {candidato_revisable})
         FROM negocios n LEFT JOIN web_estado w ON w.negocio_id=n.id
         WHERE n.admisible=1 AND n.web IS NULL
           AND NOT EXISTS(SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=n.id)
           AND NOT EXISTS(SELECT 1 FROM web_estado a WHERE a.negocio_id=n.id AND a.estado='verificada_automatica' AND a.url_confirmada IS NOT NULL)
           AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.tipo='discovery' AND j.estado IN ('pendiente','en_curso') AND CAST(json_extract(j.params,'$.negocio_id') AS INTEGER)=n.id)
           AND (EXISTS(SELECT 1 FROM web_candidatas c WHERE c.negocio_id=n.id AND c.estado='sugerida' AND c.tipo NOT IN ('red_social','directorio') AND {candidato_revisable})
             OR ((COALESCE(w.ultima_busqueda,'')='' OR datetime(w.ultima_busqueda)<datetime('now','-90 days'))
               AND COALESCE((SELECT creditos FROM web_search_budget WHERE periodo=strftime('%Y-%m','now')),0)<1000
               AND COALESCE((SELECT creditos FROM web_search_daily_budget WHERE dia=date('now')),0)<35))
         ORDER BY CASE WHEN EXISTS(SELECT 1 FROM web_candidatas c WHERE c.negocio_id=n.id AND c.estado='sugerida' AND c.tipo NOT IN ('red_social','directorio') AND {candidato_revisable}) THEN 0 ELSE 1 END,
           CASE n.fuente WHEN 'overpass' THEN 0 WHEN 'industria' THEN 1 WHEN 'turismo' THEN 2 WHEN 'cooperativas' THEN 3 ELSE 4 END,
           COALESCE(w.ultima_busqueda,''),n.id LIMIT 1");
    let row = conn.query_row(
        &query,
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?),),
    ).optional()?;
    Ok(row)
}
pub fn info(conn: &Connection, id: i64) -> Result<WebInfo> {
    let n = store::get_negocio(conn, id)?.context("Negocio no encontrado")?;
    let state=conn.query_row("SELECT estado,url_confirmada,origen,ultima_busqueda,consulta,aviso FROM web_estado WHERE negocio_id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?))).optional()?;
    let (estado, url_confirmada, origen, ultima_busqueda, query, aviso) =
        state.unwrap_or_else(|| ("pendiente".into(), None, None, None, None, None));
    let mut stmt=conn.prepare("SELECT id,url,titulo,tipo,proveedor,consulta,posicion,fecha,estado,evidencias FROM web_candidatas WHERE negocio_id=?1 AND estado NOT IN ('descartada','confirmada') ORDER BY CASE WHEN posicion=1 THEN 0 ELSE 1 END,fecha DESC,id DESC")?;
    let rows = stmt.query_map([id], |r| {
        Ok(WebCandidata {
            id: r.get(0)?,
            url: r.get(1)?,
            titulo: r.get(2)?,
            tipo: r.get(3)?,
            proveedor: r.get(4)?,
            consulta: r.get(5)?,
            posicion: r.get(6)?,
            fecha: r.get(7)?,
            estado: r.get(8)?,
            evidencias: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or(json!({})),
        })
    })?;
    Ok(WebInfo {
        estado,
        url_confirmada,
        origen,
        ultima_busqueda,
        consulta: query.unwrap_or_else(|| search_query(&n, None, None)),
        aviso,
        buscador_configurado: configured(),
        creditos_restantes: remaining(conn)?,
        candidatas: rows.collect::<rusqlite::Result<Vec<_>>>()?,
    })
}
pub fn add_candidate(
    conn: &Connection,
    id: i64,
    url: &str,
    title: Option<&str>,
    provider: &str,
    query: Option<&str>,
    position: Option<i64>,
    evidence: &Value,
) -> Result<()> {
    let url = match public_http::normalize(url) {
        Ok(u) => u.to_string(),
        Err(_) => return Ok(()),
    };
    let kind = if crate::scoring::es_red_social(&url) {
        "red_social"
    } else if evidence["directorio"].as_bool() == Some(true) {
        "directorio"
    } else {
        "resultado_web"
    };
    conn.execute("INSERT INTO web_candidatas(negocio_id,url,titulo,tipo,proveedor,consulta,posicion,fecha,evidencias) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(negocio_id,url) DO UPDATE SET titulo=COALESCE(excluded.titulo,titulo),consulta=COALESCE(excluded.consulta,consulta),posicion=COALESCE(excluded.posicion,posicion),evidencias=excluded.evidencias",params![id,url,title,kind,provider,query,position,store::ahora(),evidence.to_string()])?;
    Ok(())
}
pub fn confirmed(
    conn: &Connection,
    id: i64,
    url: &str,
    origin: &str,
    city: Option<&str>,
    address: Option<&str>,
) -> Result<()> {
    anyhow::ensure!(
        matches!(origin, "correccion_usuario" | "confirmacion_usuario"),
        "La web solo se confirma mediante una acción explícita del usuario"
    );
    let url = public_http::normalize(url)?.to_string();
    anyhow::ensure!(
        !crate::scoring::es_red_social(&url),
        "Una red social no se puede confirmar como web propia"
    );
    let tx = conn.unchecked_transaction()?;
    let changed=tx.execute("UPDATE negocios SET web=?2,municipio=COALESCE(?3,municipio),direccion=COALESCE(?4,direccion) WHERE id=?1 AND admisible=1",params![id,url,city.filter(|s|!s.trim().is_empty()),address.filter(|s|!s.trim().is_empty())])?;
    anyhow::ensure!(changed > 0, "Negocio no encontrado");
    tx.execute("INSERT INTO web_estado(negocio_id,estado,url_confirmada,origen,confirmado_en) VALUES(?1,'confirmada',?2,?3,?4) ON CONFLICT(negocio_id) DO UPDATE SET generacion=generacion+CASE WHEN url_confirmada!=excluded.url_confirmada OR url_confirmada IS NULL THEN 1 ELSE 0 END,estado='confirmada',url_confirmada=excluded.url_confirmada,origen=excluded.origen,confirmado_en=excluded.confirmado_en,aviso=NULL",params![id,url,origin,store::ahora()])?;
    add_candidate(
        &tx,
        id,
        &url,
        None,
        origin,
        None,
        None,
        &json!({"confirmacion":origin}),
    )?;
    tx.execute(
        "UPDATE web_candidatas SET estado='confirmada' WHERE negocio_id=?1 AND url=?2",
        params![id, url],
    )?;
    tx.execute(
        "DELETE FROM oportunidades_servicio WHERE negocio_id=?1",
        [id],
    )?;
    // No reutilizar una auditoría histórica de otra URL al valorar la nueva web.
    let a = Auditoria {
        id: 0,
        negocio_id: id,
        fecha: store::ahora(),
        estado_web: EstadoWeb::WebMejorable,
        score: 20,
        prioridad: "C".into(),
        senales: json!({"web":url,"sin_auditar":true,"web_confirmada":origin}),
    };
    store::insert_auditoria(&tx, &a)?;
    store::guardar_oportunidades_auditoria(&tx, &a)?;
    tx.commit()?;
    Ok(())
}

pub fn verified_automatically(conn: &Connection, id: i64, raw_url: &str, evidence: &Value) -> Result<()> {
    let url = public_http::normalize(raw_url)?.to_string();
    anyhow::ensure!(!crate::scoring::es_red_social(&url), "Una red social no se puede verificar como web propia");
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute("UPDATE negocios SET web=?2 WHERE id=?1 AND admisible=1", params![id,url])?;
    anyhow::ensure!(changed > 0, "Negocio no encontrado o no admisible");
    tx.execute("INSERT INTO web_estado(negocio_id,estado,url_confirmada,origen,confirmado_en) VALUES(?1,'verificada_automatica',?2,'verificacion_automatica',?3) ON CONFLICT(negocio_id) DO UPDATE SET generacion=generacion+CASE WHEN url_confirmada!=excluded.url_confirmada OR url_confirmada IS NULL THEN 1 ELSE 0 END,estado='verificada_automatica',url_confirmada=excluded.url_confirmada,origen='verificacion_automatica',confirmado_en=excluded.confirmado_en,aviso=NULL", params![id,url,store::ahora()])?;
    tx.execute("UPDATE web_candidatas SET estado='confirmada',evidencias=?3 WHERE negocio_id=?1 AND url=?2", params![id,url,evidence.to_string()])?;
    tx.commit()?;
    Ok(())
}
pub fn discard(conn: &Connection, id: i64, candidate: i64) -> Result<bool> {
    Ok(conn.execute(
        "UPDATE web_candidatas SET estado='descartada' WHERE id=?1 AND negocio_id=?2",
        params![candidate, id],
    )? > 0)
}

fn save_candidate_verification(conn: &Connection, candidate: &WebCandidata, mut verification: Value) -> Result<()> {
    let mut evidence = candidate.evidencias.clone();
    if let Some(object) = evidence.as_object_mut() {
        object.insert("verificacion".into(), verification.take());
    } else {
        evidence = json!({"origen":candidate.proveedor,"verificacion":verification});
    }
    conn.execute("UPDATE web_candidatas SET evidencias=?2 WHERE id=?1", params![candidate.id,evidence.to_string()])?;
    Ok(())
}

pub fn organic(results: &Value) -> Option<(usize, &Value)> {
    results.as_array()?.iter().enumerate().find(|(_, r)| {
        let kind = r["type"].as_str().unwrap_or("").to_ascii_lowercase();
        let marker = ["label", "sponsorship", "ad_label"]
            .iter()
            .filter_map(|k| r[*k].as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        let title = r["title"].as_str().unwrap_or("").to_ascii_lowercase();
        ![
            "is_ad",
            "is_sponsored",
            "sponsored",
            "promoted",
            "paid",
            "is_paid",
        ]
        .iter()
        .any(|k| r[*k].as_bool() == Some(true))
            && ![
                "ad",
                "advertisement",
                "advertising",
                "promotion",
                "sponsored",
                "sponsored_result",
                "paid",
                "promoted",
            ]
            .contains(&kind.as_str())
            && ![
                "sponsored",
                "advertisement",
                "paid result",
                "anuncio",
                "patrocinado",
            ]
            .iter()
            .any(|m| marker.contains(m) || title.starts_with(m))
            && r["url"].as_str().is_some()
    })
}
fn organic_result(result: &Value) -> bool {
    organic(&Value::Array(vec![result.clone()])).is_some()
}
fn field(row: &Value, key: &str) -> Option<String> {
    row[key]
        .as_str()
        .map(str::trim)
        .filter(|v| {
            !v.is_empty() && !eligibility::normalize(v).contains("noseconsiente") && *v != "-"
        })
        .map(str::to_owned)
}
async fn registry(store: &Store) -> Result<Value> {
    static CACHE: std::sync::OnceLock<tokio::sync::Mutex<(i64, Value)>> =
        std::sync::OnceLock::new();
    let mut local = CACHE
        .get_or_init(|| tokio::sync::Mutex::new((0, Value::Null)))
        .lock()
        .await;
    if local.0 > chrono::Utc::now().timestamp() {
        return Ok(local.1.clone());
    }

    let cached = {
        let c = store.lock().unwrap();
        c.query_row(
            "SELECT value FROM meta WHERE key='turismo_registry_cache'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
    };
    if let Some(c) = cached {
        if let Ok(v) = serde_json::from_str::<Value>(&c) {
            if v["expires"].as_i64().unwrap_or(0) > chrono::Utc::now().timestamp() {
                *local = (v["expires"].as_i64().unwrap(), v["records"].clone());
                return Ok(local.1.clone());
            }
        }
    }
    let page = public_http::fetch(REGISTRY_URL, 20).await?;
    anyhow::ensure!(
        page.status == 200,
        "Registro de Turismo HTTP {}",
        page.status
    );
    let data: Value = serde_json::from_str(&page.body)?;
    let records = data["result"]["records"].clone();
    anyhow::ensure!(records.is_array(), "Registro de Turismo sin registros");
    let c = store.lock().unwrap();
    store::meta_set(
        &c,
        "turismo_registry_cache",
        &json!({"records":records,"expires":chrono::Utc::now().timestamp()+86400}).to_string(),
    )?;
    *local = (chrono::Utc::now().timestamp() + 86400, records.clone());
    Ok(records)
}
pub async fn discover(store: &Store, id: i64, allow_search: bool) -> Result<Value> {
    let n = {
        let c = store.lock().unwrap();
        store::get_negocio(&c, id)?.context("Negocio no encontrado")?
    };
    let confirmed_url = {
        let c = store.lock().unwrap();
        c.query_row("SELECT url_confirmada FROM web_estado WHERE negocio_id=?1 AND ((estado='confirmada' AND origen IN ('correccion_usuario','confirmacion_usuario')) OR (estado='verificada_automatica' AND origen='verificacion_automatica'))",[id],|r|r.get::<_,Option<String>>(0)).optional()?.flatten()
    };
    if confirmed_url.is_some() {
        return Ok(json!({"estado":"ya_tiene_web","negocio_id":id}));
    }
    let official = registry(store).await;
    let mut address = n.direccion.clone();
    let mut city = Some(n.municipio.clone());
    let mut official_notice = None;
    if let Ok(rows) = official {
        let matches = rows
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                field(r, "NOMBRE").is_some_and(|name| {
                    eligibility::normalize(&name) == eligibility::normalize(&n.nombre)
                })
            })
            .collect::<Vec<_>>();
        let matching = matches
            .iter()
            .copied()
            .filter(|r| {
                field(r, "MUNICIPIO")
                    .or_else(|| field(r, "LOCALIDAD"))
                    .is_some_and(|c| {
                        eligibility::normalize(&c) == eligibility::normalize(&n.municipio)
                    })
            })
            .collect::<Vec<_>>();
        let one = if matching.len() == 1 {
            Some(matching[0])
        } else if matches.len() == 1 && n.municipio == "Navarra" {
            Some(matches[0])
        } else {
            None
        };
        if let Some(r) = one {
            address = address.or(field(r, "DIRECCION"));
            city = field(r, "LOCALIDAD")
                .or_else(|| field(r, "MUNICIPIO"))
                .or(city);
            if let Some(url) = field(r, "WEB") {
                let c = store.lock().unwrap();
                add_candidate(
                    &c,
                    id,
                    &url,
                    field(r, "NOMBRE").as_deref(),
                    "Registro de Turismo de Navarra",
                    None,
                    None,
                    &json!({"fuente":REGISTRY_URL,"direccion":address,"localidad":city}),
                )?;
            }
        }
    } else if let Err(err) = official {
        official_notice = Some(format!("Registro oficial no disponible: {err:#}"));
    }
    let query = search_query(&n, address.as_deref(), city.as_deref());
    let mut notice = official_notice;
    let mut searched = false;
    let key = std::env::var("TAVILY_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let key = if allow_search { key } else { None };
    if let Some(key) = key {
        let cached = {
            let c = store.lock().unwrap();
            c.query_row("SELECT resultados FROM web_search_cache WHERE consulta=?1 AND datetime(creado)>datetime('now','-30 days')",[&query],|r|r.get::<_,String>(0)).optional()?
        };
        let results = if let Some(c) = cached {
            searched = true;
            serde_json::from_str(&c)?
        } else {
            let reserved = {
                let c = store.lock().unwrap();
                reserve_search_credit(&c)?
            };
            if !reserved {
                notice = Some("Límite gratuito diario o mensual alcanzado; búsqueda pausada.".into());
                json!([])
            } else {
                searched = true;
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(25))
                    .build()?;
                let response=client.post("https://api.tavily.com/search").bearer_auth(&key).json(&json!({"query":query,"search_depth":"basic","auto_parameters":false,"include_answer":false,"include_raw_content":false,"include_images":false,"max_results":5,"include_usage":true})).send().await;
                match response {
                    Ok(r) if r.status().is_success() => {
                        let v: Value = r.json().await?;
                        let extra = v["usage"]["credits"]
                            .as_i64()
                            .unwrap_or(1)
                            .saturating_sub(1);
                        let c = store.lock().unwrap();
                        if extra > 0 {
                            c.execute("UPDATE web_search_budget SET creditos=MIN(1000,creditos+?2) WHERE periodo=?1",params![chrono::Utc::now().format("%Y-%m").to_string(),extra])?;
                        }
                        let results = v["results"].clone();
                        c.execute("INSERT INTO web_search_cache(consulta,resultados,creado) VALUES(?1,?2,?3) ON CONFLICT(consulta) DO UPDATE SET resultados=excluded.resultados,creado=excluded.creado",params![query,results.to_string(),store::ahora()])?;
                        searched = true;
                        results
                    }
                    Ok(r) => {
                        notice = Some(format!(
                            "Buscador no disponible (HTTP {}).",
                            r.status().as_u16()
                        ));
                        json!([])
                    }
                    Err(_) => {
                        notice=Some("Buscador temporalmente no disponible; se conservan las sugerencias anteriores.".into());
                        json!([])
                    }
                }
            }
        };
        if let Some(rows) = results.as_array() {
            let c = store.lock().unwrap();
            for (position, r) in rows.iter().enumerate().filter(|(_, r)| organic_result(r)).take(5) {
                if let Some(url) = r["url"].as_str() {
                    add_candidate(&c,id,url,r["title"].as_str(),"Tavily",Some(&query),Some(position as i64+1),&json!({"tipo_resultado":"organico","identidad_confirmada":false}))?;
                }
            }
        }
    } else if allow_search {
        notice=Some("Búsqueda automática global sin configurar. Puedes buscar en el navegador o introducir una URL.".into());
    }
    // Contrastar candidatos públicos, sin convertir su posición de búsqueda en identidad.
    let candidates = {
        let c = store.lock().unwrap();
        info(&c, id)?.candidatas
    };
    for candidate in candidates.iter().take(3) {
        {
            let c = store.lock().unwrap();
            save_candidate_verification(&c, candidate, json!({"revisada_en":store::ahora(),"resultado":"no_disponible"}))?;
        }
        if candidate.tipo == "red_social" {
            continue;
        }
        match public_http::robots_allowed(&candidate.url).await {
            Ok(true) => {}
            Ok(false) => {
                let c = store.lock().unwrap();
                save_candidate_verification(&c, candidate, json!({"revisada_en":store::ahora(),"resultado":"bloqueada_por_robots"}))?;
                continue;
            }
            Err(_) => {
                let c = store.lock().unwrap();
                save_candidate_verification(&c, candidate, json!({"revisada_en":store::ahora(),"resultado":"no_disponible"}))?;
                continue;
            }
        }
        if let Ok(p) = public_http::fetch(&candidate.url, 12).await {
            if (200..300).contains(&p.status) {
                let html = scraper::Html::parse_document(&p.body);
                let name = eligibility::normalize(&html.root_element().text().collect::<String>());
                let phone = n
                    .telefono
                    .as_deref()
                    .map(|s| s.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
                    .filter(|s| s.len() >= 8);
                let body_digits = p
                    .body
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>();
                let has_phone = phone.as_deref().is_some_and(|v| {
                    body_digits.contains(v)
                        || (v.len() > 9 && body_digits.contains(&v[v.len() - 9..]))
                });
                let has_address = address.as_deref().is_some_and(|a| {
                    eligibility::normalize(a).len() > 8 && name.contains(&eligibility::normalize(a))
                });
                let has_name = eligibility::normalize(&n.nombre).len() > 3
                    && name.contains(&eligibility::normalize(&n.nombre));
                let structured = structured_identity(&p.body, &n.nombre, &p.url);
                let is_directory = candidate.tipo == "directorio" || p.body.contains("\"@type\":\"ItemList\"")
                    || p.body.contains("\"@type\": \"ItemList\"");
                let c = store.lock().unwrap();
                let strong = has_name && (has_phone || has_address) && structured && !is_directory;
                let verification = json!({"revisada_en":store::ahora(),"resultado":if strong{"verificada_automatica"}else if is_directory{"directorio"}else{"sin_pruebas_suficientes"},"nombre_coincide":has_name,"telefono_coincide":has_phone,"direccion_coincide":has_address,"datos_estructurados":structured,"directorio":is_directory,"url_final":p.url});
                let mut evidence = candidate.evidencias.clone();
                evidence["verificacion"] = verification.clone();
                c.execute("UPDATE web_candidatas SET evidencias=?2,tipo=CASE WHEN ?3 THEN 'directorio' ELSE tipo END WHERE id=?1",params![candidate.id,evidence.to_string(),is_directory])?;
                if strong {
                    verified_automatically(&c,id,&candidate.url,&evidence)?;
                    return Ok(json!({"estado":"verificada_automatica","negocio_id":id,"url":candidate.url,"evidencias":verification}));
                }
            }
        }
    }
    let c = store.lock().unwrap();
    let count: i64 = c.query_row(
        "SELECT COUNT(*) FROM web_candidatas WHERE negocio_id=?1 AND estado='sugerida'",
        [id],
        |r| r.get(0),
    )?;
    let status = if count > 0 {
        "sugerida"
    } else if searched {
        "no_localizada"
    } else {
        "pendiente"
    };
    let searched_at = searched.then(store::ahora);
    c.execute("INSERT INTO web_estado(negocio_id,estado,ultima_busqueda,consulta,aviso) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(negocio_id) DO UPDATE SET estado=CASE WHEN url_confirmada IS NOT NULL THEN estado ELSE excluded.estado END,ultima_busqueda=COALESCE(excluded.ultima_busqueda,ultima_busqueda),consulta=excluded.consulta,aviso=excluded.aviso",params![id,status,searched_at,query,notice])?;
    let score = if status == "no_localizada" { 60 } else { 40 };
    c.execute("UPDATE oportunidades_servicio SET score=?2,motivo=?3,fecha=?4 WHERE negocio_id=?1 AND servicio IN ('creacion_web','landing')",params![id,score,"Web no localizada: encaje provisional; confirmar con el negocio.",store::ahora()])?;
    Ok(json!({"estado":status,"candidatas":count,"consulta":query,"aviso":notice}))
}

fn structured_identity(html: &str, name: &str, url: &str) -> bool {
    fn visit(v: &Value, name: &str, domain: &Option<String>) -> bool {
        match v {
            Value::Array(a) => a.iter().any(|v| visit(v, name, domain)),
            Value::Object(o) => {
                let business_type = o.get("@type").is_some_and(|t| {
                    let text = t.to_string().to_lowercase();
                    [
                        "organization",
                        "localbusiness",
                        "restaurant",
                        "store",
                        "professionalservice",
                        "hotel",
                        "lodgingbusiness",
                        "medicalbusiness",
                        "foodestablishment",
                        "sportsactivitylocation",
                    ]
                    .iter()
                    .any(|v| text.contains(v))
                });
                let own = business_type
                    && o.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|n| eligibility::normalize(n) == eligibility::normalize(name))
                    && o.get("url")
                        .and_then(Value::as_str)
                        .and_then(eligibility::domain)
                        .as_ref()
                        == domain.as_ref()
                    && domain.is_some();
                own || o.values().any(|v| {
                    if v.is_object() || v.is_array() {
                        visit(v, name, domain)
                    } else {
                        false
                    }
                })
            }
            _ => false,
        }
    }
    let doc = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse("script[type='application/ld+json']").unwrap();
    doc.select(&sel)
        .filter_map(|s| serde_json::from_str::<Value>(&s.text().collect::<String>()).ok())
        .any(|v| visit(&v, name, &eligibility::domain(url)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orden_organico_no_es_confirmacion() {
        let r = json!([
            {"url":"https://ad.test","is_ad":true},
            {"url":"https://paid.test","type":"Advertisement"},
            {"url":"https://sponsored.test","label":"Anuncio patrocinado"},
            {"url":"https://directory.test","title":"Directorio"},
            {"url":"https://own.test"}
        ]);
        assert_eq!(organic(&r).unwrap().0, 3);
    }
    #[test]
    fn campos_reservados_no_son_urls() {
        assert_eq!(
            field(&json!({"WEB":"No se consiente la publicación"}), "WEB"),
            None
        );
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    fn db() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(store::SCHEMA).unwrap();
        init(&c).unwrap();
        c.execute("INSERT INTO negocios(nombre,municipio,fuente,fuente_id,primera_vista) VALUES('Restaurante de prueba','Navarra','test','test-1','2026-01-01')",[]).unwrap();
        c
    }
    #[test]
    fn sugerencia_no_modifica_url_y_correccion_sobrevive_scan() {
        let c = db();
        add_candidate(
            &c,
            1,
            "https://sugerida.example",
            None,
            "Tavily",
            Some("consulta"),
            Some(1),
            &json!({}),
        )
        .unwrap();
        assert!(store::get_negocio(&c, 1).unwrap().unwrap().web.is_none());
        confirmed(
            &c,
            1,
            "https://confirmada.example/",
            "correccion_usuario",
            Some("Mutilva / Mutiloa"),
            Some("Camino Labiano 47"),
        )
        .unwrap();
        let mut n = store::get_negocio(&c, 1).unwrap().unwrap();
        n.web = None;
        n.direccion = None;
        n.municipio = "Navarra".into();
        store::upsert_negocio(&c, &n).unwrap();
        let n = store::get_negocio(&c, 1).unwrap().unwrap();
        assert_eq!(n.web.as_deref(), Some("https://confirmada.example/"));
        assert_eq!(n.municipio, "Mutilva / Mutiloa");
        let before = technical_generation(&c);
        let a = crate::technical::enqueue(&c, 1).unwrap();
        let b = crate::technical::enqueue(&c, 1).unwrap();
        assert_eq!(a.id, b.id);
        confirmed(
            &c,
            1,
            "https://otra.example/",
            "correccion_usuario",
            None,
            None,
        )
        .unwrap();
        assert!(technical_generation(&c) > before);
        assert_ne!(crate::technical::enqueue(&c, 1).unwrap().id, a.id);
    }
    #[test]
    fn init_migra_urls_heredadas_y_conserva_confirmacion_manual() {
        let c = db();
        c.execute(
            "UPDATE negocios SET web='https://importada.example/' WHERE id=1",
            [],
        )
        .unwrap();
        init(&c).unwrap();
        let n = store::get_negocio(&c, 1).unwrap().unwrap();
        assert!(n.web.is_none());
        let suggested = info(&c, 1).unwrap();
        assert!(suggested.url_confirmada.is_none());
        assert!(suggested
            .candidatas
            .iter()
            .any(|candidate| candidate.url == "https://importada.example/"));
        confirmed(
            &c,
            1,
            "https://manual.example/",
            "correccion_usuario",
            None,
            None,
        )
        .unwrap();
        init(&c).unwrap();
        assert_eq!(
            store::get_negocio(&c, 1).unwrap().unwrap().web.as_deref(),
            Some("https://manual.example/")
        );
    }
    fn technical_generation(c: &Connection) -> i64 {
        c.query_row(
            "SELECT generacion FROM web_estado WHERE negocio_id=1",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }
    #[test]
    fn homonimo_y_directorio_no_son_identidad_verificada() {
        assert!(!structured_identity(
            r#"<script type="application/ld+json">{"name":"Otro restaurante","url":"https://example.com"}</script>"#,
            "Restaurante de prueba",
            "https://example.com"
        ));
        assert!(!structured_identity(
            r#"<script type="application/ld+json">{"name":"Restaurante de prueba","url":"https://otra.com"}</script>"#,
            "Restaurante de prueba",
            "https://example.com"
        ));
    }
    #[test]
    fn consulta_domicilio_y_fallback() {
        let c = db();
        let mut n = store::get_negocio(&c, 1).unwrap().unwrap();
        n.direccion = Some("Camino Labiano 47".into());
        assert!(search_query(&n, None, Some("Mutilva")).contains("\"Camino Labiano 47\""));
        n.direccion = None;
        assert_eq!(
            search_query(&n, None, None),
            "\"Restaurante de prueba\" Navarra"
        );
    }
    #[test]
    fn busqueda_no_localizada_solo_incrementa_encaje_provisional() {
        for completed in [false, true] {
            let s = json!({"busqueda_completada":completed});
            let ops = crate::scoring::puntuar_servicios(1, EstadoWeb::SinWeb, &s, "2026-11-04");
            for op in ops
                .iter()
                .filter(|o| matches!(o.servicio, ServicioWeb::CreacionWeb | ServicioWeb::Landing))
            {
                assert!(op.score <= if completed { 60 } else { 40 });
            }
        }
    }
    #[test]
    fn presupuesto_mensual_sin_sobrepasar_mil() {
        let c = db();
        let p = chrono::Utc::now().format("%Y-%m").to_string();
        c.execute("INSERT INTO web_search_budget VALUES(?1,1000)", [&p])
            .unwrap();
        assert_eq!(remaining(&c).unwrap(), 0);
        assert_eq!(c.execute("UPDATE web_search_budget SET creditos=creditos+1 WHERE periodo=?1 AND creditos<1000",[p]).unwrap(),0);
    }
    #[test]
    fn checkpoints_por_motor_y_dispositivo_no_se_mezclan() {
        let c = db();
        confirmed(
            &c,
            1,
            "https://example.com/",
            "correccion_usuario",
            None,
            None,
        )
        .unwrap();
        let j = crate::technical::enqueue(&c, 1).unwrap();
        for (engine, device) in [
            ("lighthouse", "mobile"),
            ("lighthouse", "desktop"),
            ("siteone", "mobile"),
            ("siteone", "desktop"),
        ] {
            crate::technical::save_result(
                &c,
                &j,
                engine,
                device,
                &json!({"status":"completo","normalized":{"device":device},"files":{}}),
            )
            .unwrap();
        }
        assert_eq!(crate::technical::reports(&c, 1).unwrap().len(), 4);
        crate::technical::save_result(
            &c,
            &j,
            "siteone",
            "mobile",
            &json!({"status":"parcial","normalized":{},"files":{}}),
        )
        .unwrap();
        assert_eq!(crate::technical::reports(&c, 1).unwrap().len(), 4);
    }
    #[test]
    fn revision_humana_se_guarda_por_informe_y_hallazgo() {
        let c = db();
        confirmed(
            &c,
            1,
            "https://example.com/",
            "correccion_usuario",
            None,
            None,
        )
        .unwrap();
        let job = crate::technical::enqueue(&c, 1).unwrap();
        crate::technical::save_result(
            &c,
            &job,
            "siteone",
            "mobile",
            &json!({"status":"completo","normalized":{},"files":{}}),
        )
        .unwrap();
        let request = ActualizarRevisionHallazgo {
            report_id: format!("{}-siteone-mobile", job.id),
            finding_key: "axe|https://example.com/|main".into(),
            estado: "confirmado".into(),
            motivo: None,
        };
        crate::technical::update_finding_review(&c, 1, &request).unwrap();
        let updated = ActualizarRevisionHallazgo {
            estado: "descartado".into(),
            motivo: Some("Falso positivo revisado".into()),
            ..request.clone()
        };
        crate::technical::update_finding_review(&c, 1, &updated).unwrap();
        let reviews = crate::technical::finding_reviews(&c, 1).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].estado, "descartado");
        assert_eq!(
            reviews[0].motivo.as_deref(),
            Some("Falso positivo revisado")
        );
        let invalid = ActualizarRevisionHallazgo {
            estado: "aprobado".into(),
            ..request
        };
        assert!(crate::technical::update_finding_review(&c, 1, &invalid).is_err());
    }
}
