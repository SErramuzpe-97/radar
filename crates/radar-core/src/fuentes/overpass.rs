//! Fuente OpenStreetMap / Overpass API: descubre negocios por municipio y sector (diseño §5).

use std::time::Duration;
use std::{collections::BTreeMap, fs::File, io::BufReader, path::Path, time::SystemTime};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::eligibility::{self, Evidence};
use crate::model::Negocio;
use crate::store::{self, Store};
use osmpbfreader::{OsmId, OsmObj, OsmPbfReader};

const DEFAULT_OVERPASS_URL: &str = "https://ovp.dresel.systems/api/interpreter";
const UA: &str =
    "RadarNavarra/0.1 (+https://radar-navarra.duckdns.org; contact: hello@carlotacarrillo.eu)";

/// Pares (clave OSM, regex de valores). Vacío = cualquier POI con nombre.
fn filtros_sector(sector: Option<&str>) -> Vec<(&'static str, &'static str)> {
    match sector.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("hosteleria") => vec![
            (
                "amenity",
                r"^(restaurant|bar|cafe|pub|fast_food|ice_cream|nightclub)$",
            ),
            (
                "tourism",
                r"^(hotel|hostel|guest_house|motel|apartment|chalet)$",
            ),
        ],
        Some("comercio") => vec![("shop", "")],
        Some("servicios") => vec![
            (
                "shop",
                r"^(hairdresser|beauty|car_repair|laundry|tailor|shoe_repair|funeral_directors|pet_grooming|pest_control)$",
            ),
            (
                "amenity",
                r"^(car_wash|clinic|dentist|pharmacy|veterinary|kindergarten|childcare|funeral_hall|animal_boarding)$",
            ),
            (
                "craft",
                r"^(plumber|electrician|locksmith|pest_control|drain|drain_cleaner|car_repair)$",
            ),
            ("education", r"^(preschool|kindergarten|farm|agricultural)$"),
            ("attraction", r"^(farm|animal|educational)$"),
            (
                "service",
                r"^(drain_cleaning|sewer_cleaning|pest_control|dog_walking|pet_sitting|animal_care)$",
            ),
            (
                "tourism",
                r"^(attraction|museum|gallery|zoo|theme_park|visitor_centre)$",
            ),
            ("leisure", r"^(petting_zoo)$"),
            ("historic", r"^(castle|monument|archaeological_site|ruins)$"),
            ("school", r"^(kindergarten|preschool|nursery)$"),
        ],
        Some("profesionales") => vec![("office", "")],
        Some("industria") => vec![("craft", ""), ("tourism", r"^(winery)$")],
        _ => vec![],
    }
}

fn query(municipio: &str, sector: Option<&str>, fuzzy: bool) -> String {
    let alias = aliases_municipio(municipio).join("|");
    let alias = regex_escape(&alias).replace("\\|", "|");
    let nombre = if fuzzy {
        format!(r#"["name"~"^({})(\\s*/\\s*.*)?$"]"#, alias)
    } else {
        format!(r#"["name"~"^({})$"]"#, alias)
    };
    let mut lineas = String::new();
    let filtros = filtros_sector(sector);
    if filtros.is_empty() {
        for key in [
            "shop",
            "office",
            "craft",
            "amenity",
            "tourism",
            "leisure",
            "education",
            "attraction",
            "service",
            "historic",
            "school",
        ] {
            lineas.push_str(&format!("  nwr[\"name\"][\"{key}\"](area.a);\n"));
        }
    } else {
        for (k, re) in filtros {
            if re.is_empty() {
                lineas.push_str(&format!("  nwr[\"name\"][\"{k}\"](area.a);\n"));
            } else {
                lineas.push_str(&format!("  nwr[\"name\"][\"{k}\"~\"{re}\"](area.a);\n"));
            }
        }
    }
    format!(
        "[out:json][timeout:120];\narea[\"boundary\"=\"administrative\"][\"admin_level\"=\"8\"]{nombre}->.a;\n(\n{lineas});\nout center tags;"
    )
}

fn aliases_municipio(municipio: &str) -> Vec<&str> {
    match municipio.trim().to_lowercase().as_str() {
        "pamplona" | "iruña" | "iruna" => vec!["Pamplona", "Iruña", "Pamplona/Iruña"],
        "estella" | "estella-lizarra" | "lizarra" => vec!["Estella-Lizarra", "Estella"],
        "cizur" | "cizur mayor" | "zizur mayor" | "zizur nagusia" => {
            vec!["Cizur", "Zizur Mayor", "Zizur Nagusia"]
        }
        "noain" | "noáin" | "noain (valle de elorz)" => vec![
            "Noáin",
            "Noain",
            "Noáin (Valle de Elorz)",
            "Noain (Elortzibar)",
        ],
        "barañain" | "barañáin" => vec!["Barañáin", "Barañain"],
        "puente la reina" | "puente la reina/gares" | "gares" => vec!["Puente la Reina", "Gares"],
        "aoiz" | "agoitz" => vec!["Aoiz", "Agoitz"],
        "irurtzun" => vec!["Irurtzun"],
        _ => vec![municipio.trim()],
    }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if "\\.^$|()[]{}*+?\"".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    #[serde(default)]
    elements: Vec<Element>,
}

#[derive(Debug, Deserialize)]
struct Element {
    id: i64,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    center: Option<Center>,
    #[serde(default)]
    tags: Option<Tags>,
}

#[derive(Debug, Deserialize)]
struct Center {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct Tags {
    brand: Option<String>,
    operator: Option<String>,
    #[serde(rename = "brand:wikidata")]
    brand_wikidata: Option<String>,
    #[serde(rename = "operator:wikidata")]
    operator_wikidata: Option<String>,
    disused: Option<String>,
    abandoned: Option<String>,
    #[serde(rename = "disused:shop")]
    disused_shop: Option<String>,
    #[serde(rename = "abandoned:shop")]
    abandoned_shop: Option<String>,
    #[serde(rename = "end_date")]
    end_date: Option<String>,
    name: Option<String>,
    amenity: Option<String>,
    shop: Option<String>,
    office: Option<String>,
    tourism: Option<String>,
    craft: Option<String>,
    leisure: Option<String>,
    education: Option<String>,
    school: Option<String>,
    historic: Option<String>,
    attraction: Option<String>,
    fee: Option<String>,
    charge: Option<String>,
    tours: Option<String>,
    opening_hours: Option<String>,
    service: Option<String>,
    #[serde(rename = "addr:street")]
    addr_street: Option<String>,
    #[serde(rename = "addr:housenumber")]
    addr_housenumber: Option<String>,
    #[serde(rename = "addr:city")]
    addr_city: Option<String>,
    #[serde(rename = "contact:phone")]
    contact_phone: Option<String>,
    phone: Option<String>,
    #[serde(rename = "contact:email")]
    contact_email: Option<String>,
    email: Option<String>,
    #[serde(rename = "contact:website")]
    contact_website: Option<String>,
    website: Option<String>,
    url: Option<String>,
}

fn sector_de(tags: &Tags) -> Option<String> {
    if tags
        .office
        .as_deref()
        .is_some_and(|v| ["ngo", "association"].contains(&v))
        && (tags.opening_hours.is_some()
            || tags.contact_phone.is_some()
            || tags.phone.is_some()
            || tags.contact_email.is_some()
            || tags.email.is_some()
            || tags.contact_website.is_some()
            || tags.website.is_some()
            || tags.url.is_some())
    {
        return Some("servicios".into());
    }
    if let Some(a) = &tags.amenity {
        if [
            "restaurant",
            "bar",
            "cafe",
            "pub",
            "fast_food",
            "ice_cream",
            "nightclub",
        ]
        .contains(&a.as_str())
        {
            return Some("hosteleria".into());
        }
        if [
            "car_wash",
            "clinic",
            "dentist",
            "pharmacy",
            "veterinary",
            "kindergarten",
            "childcare",
            "funeral_hall",
            "animal_boarding",
        ]
        .contains(&a.as_str())
        {
            return Some("servicios".into());
        }
    }
    if let Some(t) = &tags.tourism {
        if [
            "hotel",
            "hostel",
            "guest_house",
            "motel",
            "apartment",
            "chalet",
        ]
        .contains(&t.as_str())
        {
            return Some("hosteleria".into());
        }
        if ["winery"].contains(&t.as_str()) {
            return Some("industria".into());
        }
        if [
            "attraction",
            "museum",
            "gallery",
            "zoo",
            "theme_park",
            "visitor_centre",
        ]
        .contains(&t.as_str())
            && tiene_visita_comercial(tags)
        {
            return Some("servicios".into());
        }
    }
    if tags.craft.as_deref().is_some_and(|v| {
        [
            "plumber",
            "electrician",
            "locksmith",
            "pest_control",
            "drain",
            "drain_cleaner",
            "car_repair",
        ]
        .contains(&v)
    }) {
        return Some("servicios".into());
    }
    if tags.service.as_deref().is_some_and(|v| {
        [
            "drain_cleaning",
            "sewer_cleaning",
            "pest_control",
            "dog_walking",
            "pet_sitting",
            "animal_care",
        ]
        .contains(&v)
    }) {
        return Some("servicios".into());
    }
    if tags
        .education
        .as_deref()
        .is_some_and(|v| ["preschool", "kindergarten", "farm", "agricultural"].contains(&v))
        || tags
            .school
            .as_deref()
            .is_some_and(|v| ["preschool", "kindergarten", "nursery"].contains(&v))
        || tags
            .attraction
            .as_deref()
            .is_some_and(|v| ["farm", "animal", "educational"].contains(&v))
    {
        return Some("servicios".into());
    }
    if tags
        .historic
        .as_deref()
        .is_some_and(|v| ["castle", "monument", "archaeological_site", "ruins"].contains(&v))
        && tiene_visita_comercial(tags)
    {
        return Some("servicios".into());
    }
    if tags.leisure.as_deref() == Some("petting_zoo") && tiene_visita_comercial(tags) {
        return Some("servicios".into());
    }
    if let Some(s) = &tags.shop {
        if [
            "no",
            "vacant",
            "closed",
            "disused",
            "abandoned",
            "construction",
            "empty",
            "mall",
        ]
        .contains(&s.as_str())
        {
            return None;
        }
        if [
            "hairdresser",
            "beauty",
            "car_repair",
            "laundry",
            "tailor",
            "shoe_repair",
            "funeral_directors",
            "pet_grooming",
            "pest_control",
        ]
        .contains(&s.as_str())
        {
            return Some("servicios".into());
        }
        return Some("comercio".into());
    }
    if tags.office.as_deref().is_some_and(|v| {
        ![
            "yes",
            "no",
            "government",
            "ngo",
            "political_party",
            "association",
            "religion",
            "diplomatic",
            "educational_institution",
        ]
        .contains(&v)
    }) {
        return Some("profesionales".into());
    }
    if tags
        .craft
        .as_deref()
        .is_some_and(|v| !["yes", "no", "vacant", "disused"].contains(&v))
    {
        return Some("industria".into());
    }
    None
}

fn tiene_visita_comercial(tags: &Tags) -> bool {
    tags.fee
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
        || tags.charge.as_deref().is_some_and(|v| {
            !["", "0", "free", "gratis", "gratuito"]
                .contains(&v.trim().to_ascii_lowercase().as_str())
        })
        || tags
            .tours
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("yes"))
}

fn categoria_de(tags: &Tags) -> Option<String> {
    tags.amenity
        .clone()
        .or(tags.shop.clone())
        .or(tags.office.clone())
        .or(tags.tourism.clone())
        .or(tags.craft.clone())
        .or(tags.education.clone())
        .or(tags.historic.clone())
        .or(tags.school.clone())
        .or(tags.leisure.clone())
}

fn rubro_de(tags: &Tags) -> Option<&'static str> {
    if tags
        .office
        .as_deref()
        .is_some_and(|v| ["ngo", "association"].contains(&v))
    {
        return Some("asociacion_ong");
    }
    if tags
        .education
        .as_deref()
        .is_some_and(|v| ["farm", "agricultural"].contains(&v))
        || tags
            .attraction
            .as_deref()
            .is_some_and(|v| ["farm", "animal", "educational"].contains(&v))
        || tags.leisure.as_deref() == Some("petting_zoo")
    {
        return Some("granja_escuela");
    }
    if let Some(craft) = tags.craft.as_deref() {
        match craft {
            "winery" => return Some("bodega"),
            "plumber" => return Some("fontaneria"),
            "electrician" => return Some("electricidad"),
            "locksmith" => return Some("cerrajeria"),
            "pest_control" => return Some("control_plagas"),
            "drain" | "drain_cleaner" => return Some("desatascos"),
            "car_repair" => return Some("automocion"),
            _ => {}
        }
    }
    if tags
        .service
        .as_deref()
        .is_some_and(|v| ["drain_cleaning", "sewer_cleaning"].contains(&v))
    {
        return Some("desatascos");
    }
    if tags
        .service
        .as_deref()
        .is_some_and(|v| ["dog_walking", "pet_sitting", "animal_care"].contains(&v))
    {
        return Some("cuidado_mascotas");
    }
    if tags.shop.as_deref() == Some("car_repair") {
        return Some("automocion");
    }
    crate::rubros::desde_categoria(
        categoria_de(tags).as_deref().or(tags.tourism.as_deref()),
        sector_de(tags).as_deref(),
    )
}

fn evidencia_rubro(tags: &Tags) -> Option<serde_json::Value> {
    let web = tags.contact_website.is_some() || tags.website.is_some() || tags.url.is_some();
    let mut evidencia = crate::rubros::evidencia_visita(
        tags.tourism
            .as_deref()
            .or(tags.historic.as_deref())
            .or(tags.leisure.as_deref()),
        tags.tours.as_deref(),
        tags.fee.as_deref(),
        tags.charge.as_deref(),
        tags.opening_hours.as_deref(),
        web,
    );
    if rubro_de(tags) == Some("restaurante") {
        let mut e = evidencia.take().unwrap_or_else(|| serde_json::json!({}));
        e["perfil_web"] = serde_json::json!({"sugerencia":"carta_menu","motivo":"priorizar carta, menú y reservas"});
        evidencia = Some(e);
    }
    if let Some(office) = tags
        .office
        .as_deref()
        .filter(|v| ["ngo", "association"].contains(v))
    {
        let mut e = evidencia.take().unwrap_or_else(|| serde_json::json!({}));
        e["tipo_entidad"] = serde_json::json!(if office == "ngo" { "ong" } else { "asociacion" });
        evidencia = Some(e);
    }
    evidencia
}

fn normalizar_web(web: &str) -> String {
    let w = web.trim();
    if w.starts_with("http://") || w.starts_with("https://") {
        w.to_string()
    } else {
        format!("https://{w}")
    }
}

fn a_negocio(el: &Element, tags: &Tags, municipio: &str) -> Option<Negocio> {
    let nombre = tags.name.clone()?;
    let (lat, lon) = match (&el.lat, &el.lon, &el.center) {
        (Some(la), Some(lo), _) => (Some(*la), Some(*lo)),
        (_, _, Some(c)) => (Some(c.lat), Some(c.lon)),
        _ => (None, None),
    };
    let direccion = match (&tags.addr_street, &tags.addr_housenumber) {
        (Some(calle), Some(num)) => Some(format!("{calle} {num}")),
        (Some(calle), None) => Some(calle.clone()),
        _ => None,
    };
    let urls = tags
        .contact_website
        .iter()
        .chain(tags.website.iter())
        .chain(tags.url.iter())
        .flat_map(|s| s.split(';'))
        .map(normalizar_web)
        .collect::<Vec<_>>();
    let web = urls
        .iter()
        .find(|u| !crate::scoring::es_red_social(u))
        .or_else(|| urls.first())
        .cloned();
    let ahora = store::ahora();
    Some(Negocio {
        id: 0,
        nombre,
        categoria: categoria_de(tags),
        sector: sector_de(tags),
        rubro: rubro_de(tags).map(str::to_owned),
        evidencia_rubro: evidencia_rubro(tags),
        direccion,
        municipio: tags
            .addr_city
            .clone()
            .unwrap_or_else(|| municipio.to_string()),
        lat,
        lon,
        telefono: tags.contact_phone.clone().or(tags.phone.clone()),
        email: tags.contact_email.clone().or(tags.email.clone()),
        web: web.clone(),
        estado_web_lista: if web.is_some() {
            "confirmada"
        } else {
            "sin_localizar"
        }
        .into(),
        estado_auditoria_tecnica: "pendiente".into(),
        fuente: "overpass".into(),
        fuente_id: format!(
            "osm-{}/{}",
            if el.kind.is_empty() {
                "unknown"
            } else {
                &el.kind
            },
            el.id
        ),
        primera_vista: ahora.clone(),
        ultima_verificacion: Some(ahora),
        score_potencial: None,
        prioridad_comercial: None,
        estado_pipeline: None,
        oportunidades: Vec::new(),
        descartado: false,
    })
}

/// Ejecuta el scan de un municipio (y sector opcional) contra Overpass e inserta en SQLite.
/// Devuelve el número de negocios procesados.
pub async fn scan_municipio(store: &Store, municipio: &str, sector: Option<&str>) -> Result<usize> {
    if municipio.trim().is_empty() {
        bail!("el municipio no puede estar vacío");
    }
    if let Some(sector) = sector {
        let sector = sector.to_ascii_lowercase();
        if ![
            "hosteleria",
            "comercio",
            "servicios",
            "profesionales",
            "industria",
            "todos",
        ]
        .contains(&sector.as_str())
        {
            bail!("sector desconocido: {sector}; usa hosteleria, comercio, servicios, profesionales, industria o todos");
        }
    }

    // El endpoint principal devuelve HTTP 406 desde algunas redes. La instancia europea
    // publica un extracto diario y es suficiente para el ámbito de Radar (Navarra).
    let endpoint =
        std::env::var("RADAR_OVERPASS_URL").unwrap_or_else(|_| DEFAULT_OVERPASS_URL.to_string());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .user_agent(UA)
        .build()?;

    let mut data = consultar(
        &client,
        &endpoint,
        query(municipio, sector, false),
        "exacta",
    )
    .await?;
    if data.elements.is_empty() {
        data = consultar(
            &client,
            &endpoint,
            query(municipio, sector, true),
            "ampliada",
        )
        .await?;
    }

    let candidates = data
        .elements
        .iter()
        .filter_map(|el| {
            let tags = el.tags.as_ref()?;
            let n = a_negocio(el, tags, municipio)?;
            let url = format!("https://www.openstreetmap.org/{}/{}", el.kind, el.id);
            Some((n, evidencia_osm(tags), url))
        })
        .collect::<Vec<_>>();
    let evidencias = candidates
        .iter()
        .map(|(_, e, _)| e.clone())
        .collect::<Vec<_>>();
    let classifier = eligibility::resolve(store, &evidencias).await?;
    let conn = store.lock().unwrap();
    let mut count = 0;
    for (n, e, url) in candidates {
        if eligibility::record(&conn, &n, &e, &classifier, &url)?.admitted() {
            let id = store::upsert_negocio(&conn, &n)?;
            for u in &e.web_urls {
                crate::web::add_candidate(
                    &conn,
                    id,
                    u,
                    None,
                    "OSM",
                    None,
                    None,
                    &json!({"fuente":url}),
                )?;
            }
            count += 1;
        }
    }
    Ok(count)
}

/// Busca negocios de un sector en toda la Comunidad Foral de Navarra.
pub async fn scan_navarra(store: &Store, cache_path: &str, sector: Option<&str>) -> Result<usize> {
    let resumen = scan_navarra_resumen(store, cache_path, sector).await?;
    Ok(resumen.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize)
}

pub async fn scan_navarra_resumen(
    store: &Store,
    cache_path: &str,
    sector: Option<&str>,
) -> Result<serde_json::Value> {
    if let Some(s) = sector {
        if ![
            "hosteleria",
            "comercio",
            "servicios",
            "profesionales",
            "industria",
            "todos",
        ]
        .contains(&s.to_ascii_lowercase().as_str())
        {
            bail!("sector desconocido: {s}");
        }
    }
    let path = Path::new(cache_path);
    let ttl_hours = std::env::var("RADAR_OSM_REFRESH_HOURS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(24)
        .clamp(1, 168);
    let vigente = path
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| SystemTime::now().duration_since(m).ok())
        .is_some_and(|age| age.as_secs() < ttl_hours * 3600);
    if !vigente {
        if let Err(e) = descargar_extracto(path).await {
            if !path.exists() {
                return Err(e);
            }
            eprintln!("No se pudo actualizar el extracto OSM; se usará la copia local: {e:#}");
        }
    }
    let path = path.to_path_buf();
    let store_original = store;
    let store = store.clone();
    let sector = sector.map(str::to_owned);
    let candidatos =
        tokio::task::spawn_blocking(move || preparar_extracto(&store, &path, sector.as_deref()))
            .await
            .context("preparando extracto OSM")??;
    let evidencias = candidatos
        .iter()
        .map(|(_, e, _)| e.clone())
        .collect::<Vec<_>>();
    let classifier = eligibility::resolve(store_original, &evidencias).await?;
    let conn = store_original.lock().unwrap();
    let tx = conn.unchecked_transaction()?;
    let fuente_id:i64=tx.query_row("SELECT id FROM fuentes WHERE url LIKE '%geofabrik.de/europe/spain/navarra-latest.osm.pbf' ORDER BY id LIMIT 1",[],|r|r.get(0))?;
    let mut sectores = serde_json::Map::new();
    let mut rubros = serde_json::Map::new();
    let mut motivos = serde_json::Map::new();
    let mut admitidos = 0;
    for (n, e, url) in candidatos {
        let d = eligibility::record(&tx, &n, &e, &classifier, &url)?;
        let reason = motivos.entry(d.reason.clone()).or_insert(json!(0));
        *reason = json!(reason.as_u64().unwrap_or(0) + 1);
        let entry=sectores.entry(n.sector.clone().unwrap_or_else(||"sin_sector".into())).or_insert(json!({"examinados":0,"admitidos":0,"excluidos":0,"pendientes":0,"nuevos":0,"actualizados":0,"vinculados":0}));
        entry["examinados"] = json!(entry["examinados"].as_u64().unwrap_or(0) + 1);
        let rubro_entry = rubros.entry(n.rubro.clone().unwrap_or_else(||"sin_rubro".into())).or_insert(json!({"examinados":0,"admitidos":0,"excluidos":0,"pendientes":0,"nuevos":0,"actualizados":0,"vinculados":0}));
        rubro_entry["examinados"] = json!(rubro_entry["examinados"].as_u64().unwrap_or(0) + 1);
        if !d.admitted() {
            let key = if d.status == "excluido" {
                "excluidos"
            } else {
                "pendientes"
            };
            entry[key] = json!(entry[key].as_u64().unwrap_or(0) + 1);
            rubro_entry[key] = json!(rubro_entry[key].as_u64().unwrap_or(0) + 1);
            continue;
        }
        let existed: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM negocio_fuentes WHERE fuente_id=?1 AND external_id=?2) OR EXISTS(SELECT 1 FROM negocios WHERE fuente=?3 AND fuente_id=?2)",
            rusqlite::params![fuente_id, n.fuente_id, n.fuente],
            |r| r.get(0),
        )?;
        let (id, merged) = store::upsert_negocio_fuente(&tx, &n, fuente_id, &n.fuente_id, &url)?;
        for u in &e.web_urls {
            crate::web::add_candidate(&tx, id, u, None, "OSM", None, None, &json!({"fuente":url}))?;
        }
        entry["admitidos"] = json!(entry["admitidos"].as_u64().unwrap_or(0) + 1);
        rubro_entry["admitidos"] = json!(rubro_entry["admitidos"].as_u64().unwrap_or(0) + 1);
        let key = if existed {
            "actualizados"
        } else if merged {
            "vinculados"
        } else {
            "nuevos"
        };
        entry[key] = json!(entry[key].as_u64().unwrap_or(0) + 1);
        rubro_entry[key] = json!(rubro_entry[key].as_u64().unwrap_or(0) + 1);
        admitidos += 1;
    }
    tx.commit()?;
    Ok(
        json!({"total":admitidos,"por_sector":sectores,"por_rubro":rubros,"motivos":motivos,"politica":eligibility::POLICY_VERSION,"metodo":"evidencias_comerciales_y_organizacion"}),
    )
}

async fn descargar_extracto(path: &Path) -> Result<()> {
    const URL: &str = "https://download.geofabrik.de/europe/spain/navarra-latest.osm.pbf";
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("creando caché OSM {}", parent.display()))?;
    let tmp = path.with_extension("pbf.part");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent(UA)
        .build()?;
    let response = client
        .get(URL)
        .send()
        .await
        .context("descargando extracto OSM de Navarra")?
        .error_for_status()
        .context("respuesta HTTP al descargar extracto OSM")?;
    let bytes = response.bytes().await.context("leyendo extracto OSM")?;
    if bytes.len() < 1_000_000 {
        bail!(
            "el extracto OSM descargado es demasiado pequeño ({} bytes)",
            bytes.len()
        );
    }
    std::fs::write(&tmp, &bytes)
        .with_context(|| format!("escribiendo caché temporal {}", tmp.display()))?;
    if let Err(e) = File::open(&tmp).and_then(|f| {
        OsmPbfReader::new(f)
            .iter()
            .next()
            .ok_or_else(|| std::io::Error::other("PBF vacío"))?
            .map(|_| ())
            .map_err(|e| std::io::Error::other(e.to_string()))
    }) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).context("validando extracto OSM PBF");
    }
    std::fs::rename(&tmp, path)
        .with_context(|| format!("publicando caché OSM {}", path.display()))?;
    Ok(())
}

fn evidencia_osm(tags: &Tags) -> Evidence {
    let nonprofit_profile = tags
        .office
        .as_deref()
        .is_some_and(|v| ["ngo", "association"].contains(&v));
    Evidence {
        commercial: sector_de(tags).is_some() && !nonprofit_profile,
        nonprofit_activity: nonprofit_profile && sector_de(tags).is_some(),
        in_navarra: true,
        inactive: tags.disused.as_deref().is_some_and(|v| v != "no")
            || tags.abandoned.as_deref().is_some_and(|v| v != "no")
            || tags.disused_shop.is_some()
            || tags.abandoned_shop.is_some()
            || tags.end_date.is_some(),
        web_urls: tags
            .contact_website
            .iter()
            .chain(tags.website.iter())
            .chain(tags.url.iter())
            .flat_map(|s| s.split(';'))
            .map(normalizar_web)
            .collect(),
        category: categoria_de(tags).unwrap_or_default(),
        brand: tags.brand.clone(),
        operator: tags.operator.clone(),
        entities: tags
            .brand_wikidata
            .iter()
            .chain(tags.operator_wikidata.iter())
            .flat_map(|s| s.split(';').map(str::trim).map(str::to_owned))
            .collect(),
        ..Default::default()
    }
}

fn preparar_extracto(
    store: &Store,
    path: &Path,
    sector: Option<&str>,
) -> Result<Vec<(Negocio, Evidence, String)>> {
    let mut pbf = OsmPbfReader::new(BufReader::new(File::open(path)?));
    let mut municipalities = std::collections::BTreeSet::new();
    for obj in pbf.iter() {
        let obj = obj?;
        let t = obj.tags();
        if t.get("boundary")
            .is_some_and(|v| v.as_str() == "administrative")
            && t.get("admin_level").is_some_and(|v| v.as_str() == "8")
        {
            for k in ["name", "name:es", "name:eu", "official_name", "alt_name"] {
                if let Some(n) = t.get(k) {
                    municipalities.insert(eligibility::normalize(n));
                    for n in n.split([';', '/']) {
                        municipalities.insert(eligibility::normalize(n));
                    }
                }
            }
        }
    }
    {
        let conn = store.lock().unwrap();
        for n in municipalities {
            conn.execute(
                "INSERT OR IGNORE INTO municipios_navarra(nombre) VALUES(?1)",
                [n],
            )?;
        }
    }
    let mut pbf = OsmPbfReader::new(BufReader::new(File::open(path)?));
    let objetos = pbf.get_objs_and_deps(|obj| {
        obj.tags().contains_key("name")
            && [
                "shop",
                "office",
                "craft",
                "amenity",
                "tourism",
                "leisure",
                "education",
                "attraction",
                "service",
                "historic",
                "school",
            ]
            .iter()
            .any(|k| obj.tags().contains_key(*k))
    })?;
    let mut out = Vec::new();
    for (id, obj) in &objetos {
        if obj.tags().get("name").is_none() {
            continue;
        }
        if ![
            "shop",
            "office",
            "craft",
            "amenity",
            "tourism",
            "leisure",
            "education",
            "attraction",
            "service",
            "historic",
            "school",
        ]
        .iter()
        .any(|k| obj.tags().contains_key(*k))
        {
            continue;
        }
        let tags = tags_osm(obj.tags());
        if let Some(f) = sector.filter(|s| *s != "todos") {
            if sector_de(&tags).as_deref() != Some(f) {
                continue;
            }
        }
        let (lat, lon) = coordenadas(*id, &objetos);
        let kind = match id {
            OsmId::Node(_) => "node",
            OsmId::Way(_) => "way",
            OsmId::Relation(_) => "relation",
        }
        .to_string();
        let el = Element {
            id: id.inner_id(),
            kind: kind.clone(),
            lat,
            lon,
            center: None,
            tags: None,
        };
        if let Some(n) = a_negocio(&el, &tags, "Navarra") {
            let url = format!("https://www.openstreetmap.org/{kind}/{}", id.inner_id());
            out.push((n, evidencia_osm(&tags), url));
        }
    }
    Ok(out)
}

fn tags_osm(raw: &osmpbfreader::Tags) -> Tags {
    let get = |key: &str| raw.get(key).map(ToString::to_string);
    Tags {
        brand: get("brand"),
        operator: get("operator"),
        brand_wikidata: get("brand:wikidata"),
        operator_wikidata: get("operator:wikidata"),
        disused: get("disused"),
        abandoned: get("abandoned"),
        disused_shop: get("disused:shop"),
        abandoned_shop: get("abandoned:shop"),
        end_date: get("end_date"),
        name: get("name"),
        amenity: get("amenity"),
        shop: get("shop"),
        office: get("office"),
        tourism: get("tourism"),
        craft: get("craft"),
        leisure: get("leisure"),
        education: get("education"),
        school: get("school"),
        historic: get("historic"),
        attraction: get("attraction"),
        fee: get("fee"),
        charge: get("charge"),
        tours: get("tours"),
        opening_hours: get("opening_hours"),
        service: get("service"),
        addr_street: get("addr:street"),
        addr_housenumber: get("addr:housenumber"),
        addr_city: get("addr:city"),
        contact_phone: get("contact:phone"),
        phone: get("phone"),
        contact_email: get("contact:email"),
        email: get("email"),
        contact_website: get("contact:website"),
        website: get("website"),
        url: get("url"),
    }
}

fn coordenadas(id: OsmId, objetos: &BTreeMap<OsmId, OsmObj>) -> (Option<f64>, Option<f64>) {
    let mut points = Vec::new();
    fn agregar(
        id: OsmId,
        objetos: &BTreeMap<OsmId, OsmObj>,
        points: &mut Vec<(f64, f64)>,
        depth: usize,
    ) {
        if depth > 4 {
            return;
        }
        match objetos.get(&id) {
            Some(OsmObj::Node(n)) => points.push((n.lat(), n.lon())),
            Some(OsmObj::Way(w)) => {
                for node in &w.nodes {
                    agregar((*node).into(), objetos, points, depth + 1);
                }
            }
            Some(OsmObj::Relation(r)) => {
                for member in &r.refs {
                    agregar(member.member, objetos, points, depth + 1);
                }
            }
            None => {}
        }
    }
    agregar(id, objetos, &mut points, 0);
    if points.is_empty() {
        return (None, None);
    }
    let len = points.len() as f64;
    (
        Some(points.iter().map(|p| p.0).sum::<f64>() / len),
        Some(points.iter().map(|p| p.1).sum::<f64>() / len),
    )
}

async fn consultar(
    client: &reqwest::Client,
    endpoint: &str,
    consulta: String,
    tipo: &str,
) -> Result<OverpassResponse> {
    let response = client
        .post(endpoint)
        .form(&[("data", consulta)])
        .send()
        .await
        .with_context(|| format!("conectando con Overpass (consulta {tipo}) en {endpoint}"))?;
    let status = response.status();
    if !status.is_success() {
        let detalle = response.text().await.unwrap_or_default();
        let detalle = detalle.chars().take(400).collect::<String>();
        bail!("Overpass respondió HTTP {status} (consulta {tipo}): {detalle}");
    }
    response
        .json()
        .await
        .with_context(|| format!("parseando respuesta Overpass (consulta {tipo})"))
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    #[test]
    fn lugar_o_equipo_no_se_convierte_en_comercio_por_tener_nombre() {
        for raw in [
            r#"{"name":"Ascensor del barrio","highway":"elevator"}"#,
            r#"{"name":"Municipio / Herria","boundary":"administrative"}"#,
            r#"{"name":"Entrada parking","amenity":"parking_entrance"}"#,
            r#"{"name":"Oficina postal","amenity":"post_office"}"#,
            r#"{"name":"Oficina municipal","office":"government"}"#,
            r#"{"name":"Local vacío","shop":"vacant"}"#,
        ] {
            let tags: Tags = serde_json::from_str(raw).unwrap();
            assert!(!evidencia_osm(&tags).commercial, "{raw}");
        }
        for raw in [
            r#"{"name":"Taller Uno","craft":"carpenter"}"#,
            r#"{"name":"Café independiente","amenity":"cafe"}"#,
            r#"{"name":"Despacho Uno","office":"lawyer"}"#,
            r#"{"name":"Tienda Uno","shop":"bakery"}"#,
        ] {
            let tags: Tags = serde_json::from_str(raw).unwrap();
            assert!(evidencia_osm(&tags).commercial);
        }
    }
    #[test]
    fn rubros_nuevos_conservan_sector_general_y_senales_verificables() {
        for (raw, sector, rubro) in [
            (
                r#"{"name":"Bodega","craft":"winery"}"#,
                "industria",
                "bodega",
            ),
            (
                r#"{"name":"Flor","shop":"florist"}"#,
                "comercio",
                "floristeria",
            ),
            (
                r#"{"name":"Guardería","amenity":"kindergarten"}"#,
                "servicios",
                "guarderia",
            ),
            (
                r#"{"name":"Vet","amenity":"veterinary"}"#,
                "servicios",
                "veterinaria",
            ),
            (
                r#"{"name":"Fontanero","craft":"plumber"}"#,
                "servicios",
                "fontaneria",
            ),
            (
                r#"{"name":"Museo","tourism":"museum","fee":"yes","opening_hours":"Mo-Su 10:00-18:00"}"#,
                "servicios",
                "turismo_cultural",
            ),
            (
                r#"{"name":"Castillo","historic":"castle","fee":"yes"}"#,
                "servicios",
                "turismo_cultural",
            ),
            (
                r#"{"name":"Granja","attraction":"animal","tourism":"attraction","fee":"yes"}"#,
                "servicios",
                "granja_escuela",
            ),
            (
                r#"{"name":"Guardería","school":"nursery"}"#,
                "servicios",
                "guarderia",
            ),
            (
                r#"{"name":"Plagas","shop":"pest_control"}"#,
                "servicios",
                "control_plagas",
            ),
            (
                r#"{"name":"Paseos","service":"dog_walking"}"#,
                "servicios",
                "cuidado_mascotas",
            ),
            (
                r#"{"name":"Visitas a bodega","tourism":"winery"}"#,
                "industria",
                "bodega",
            ),
        ] {
            let tags: Tags = serde_json::from_str(raw).unwrap();
            assert_eq!(sector_de(&tags).as_deref(), Some(sector), "{raw}");
            assert_eq!(rubro_de(&tags), Some(rubro), "{raw}");
        }
        let gratuito: Tags =
            serde_json::from_str(r#"{"name":"Museo gratuito","tourism":"museum","fee":"no"}"#)
                .unwrap();
        assert_eq!(
            sector_de(&gratuito),
            None,
            "no debe inferirse actividad comercial por ser un museo"
        );
        let e = evidencia_rubro(&gratuito).unwrap();
        assert_eq!(e["tarifa"]["estado"], "declarada_gratuita");
        assert_eq!(e["presencia_web"]["estado"], "no_registrada_en_osm");
        let q = query("Tudela", Some("servicios"), false);
        for tag in [
            "education",
            "school",
            "historic",
            "service",
            "leisure",
            "tourism",
        ] {
            assert!(
                q.contains(&format!("[\"{tag}\"")),
                "no se consulta {tag}: {q}"
            );
        }
    }
    #[test]
    fn asociaciones_ong_requieren_actividad_publica_y_se_clasifican_aparte() {
        let active: Tags = serde_json::from_str(r#"{"name":"Asociación local","office":"association","contact:website":"https://ejemplo.test"}"#).unwrap();
        assert_eq!(sector_de(&active).as_deref(), Some("servicios"));
        assert_eq!(rubro_de(&active), Some("asociacion_ong"));
        let evidence = evidencia_osm(&active);
        assert!(!evidence.commercial);
        assert!(evidence.nonprofit_activity);
        let inactive: Tags =
            serde_json::from_str(r#"{"name":"Sin contacto","office":"ngo"}"#).unwrap();
        assert_eq!(sector_de(&inactive), None);
        assert!(!evidencia_osm(&inactive).nonprofit_activity);
    }
    #[test]
    fn query_todos_exige_categoria_y_conserva_la_identidad_del_objeto() {
        let q = query("Pamplona", None, false);
        assert!(!q.contains("nwr[\"name\"](area.a)"));
        let tags: Tags =
            serde_json::from_str(r#"{"name":"Taller Uno","craft":"carpenter"}"#).unwrap();
        let a = Element {
            id: 123,
            kind: "node".into(),
            lat: None,
            lon: None,
            center: None,
            tags: None,
        };
        let b = Element {
            id: 123,
            kind: "way".into(),
            lat: None,
            lon: None,
            center: None,
            tags: None,
        };
        assert_ne!(
            a_negocio(&a, &tags, "Navarra").unwrap().fuente_id,
            a_negocio(&b, &tags, "Navarra").unwrap().fuente_id
        );
    }
}
