//! Fuente OpenStreetMap / Overpass API: descubre negocios por municipio y sector (diseño §5).

use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::model::Negocio;
use crate::store::{self, Store};

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";
const UA: &str = "RadarNavarra/0.1 (+prospeccion local B2B; contacto: radar@localhost)";

/// Pares (clave OSM, regex de valores). Vacío = cualquier POI con nombre.
fn filtros_sector(sector: Option<&str>) -> Vec<(&'static str, &'static str)> {
    match sector.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("hosteleria") => vec![
            ("amenity", r"^(restaurant|bar|cafe|pub|fast_food|ice_cream|nightclub)$"),
            ("tourism", r"^(hotel|hostel|guest_house|motel|apartment|chalet)$"),
        ],
        Some("comercio") => vec![("shop", "")],
        Some("servicios") => vec![
            ("shop", r"^(hairdresser|beauty|car_repair|laundry|tailor|shoe_repair|funeral_directors|pet_grooming)$"),
            ("amenity", r"^(car_wash|clinic|dentist|pharmacy|veterinary)$"),
        ],
        Some("profesionales") => vec![("office", "")],
        Some("industria") => vec![("craft", "")],
        _ => vec![],
    }
}

fn query(municipio: &str, sector: Option<&str>, fuzzy: bool) -> String {
    let nombre = if fuzzy {
        format!(r#"["name"~"^{}(\\s*/\\s*.*)?$"]"#, regex_escape(municipio))
    } else {
        format!(r#"["name"="{}"]"#, municipio.replace('"', "\\\""))
    };
    let mut lineas = String::new();
    let filtros = filtros_sector(sector);
    if filtros.is_empty() {
        lineas.push_str("  nwr[\"name\"](area.a);\n");
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
    name: Option<String>,
    amenity: Option<String>,
    shop: Option<String>,
    office: Option<String>,
    tourism: Option<String>,
    craft: Option<String>,
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
    if let Some(a) = &tags.amenity {
        if ["restaurant", "bar", "cafe", "pub", "fast_food", "ice_cream", "nightclub"].contains(&a.as_str()) {
            return Some("hosteleria".into());
        }
        if ["car_wash", "clinic", "dentist", "pharmacy", "veterinary"].contains(&a.as_str()) {
            return Some("servicios".into());
        }
    }
    if let Some(t) = &tags.tourism {
        if ["hotel", "hostel", "guest_house", "motel", "apartment", "chalet"].contains(&t.as_str()) {
            return Some("hosteleria".into());
        }
    }
    if let Some(s) = &tags.shop {
        if ["hairdresser", "beauty", "car_repair", "laundry", "tailor", "shoe_repair", "funeral_directors", "pet_grooming"]
            .contains(&s.as_str())
        {
            return Some("servicios".into());
        }
        return Some("comercio".into());
    }
    if tags.office.is_some() {
        return Some("profesionales".into());
    }
    if tags.craft.is_some() {
        return Some("industria".into());
    }
    None
}

fn categoria_de(tags: &Tags) -> Option<String> {
    tags.amenity
        .clone()
        .or(tags.shop.clone())
        .or(tags.office.clone())
        .or(tags.tourism.clone())
        .or(tags.craft.clone())
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
    let web = tags
        .contact_website
        .clone()
        .or(tags.website.clone())
        .or(tags.url.clone())
        .map(|w| normalizar_web(&w));
    let ahora = store::ahora();
    Some(Negocio {
        id: 0,
        nombre,
        categoria: categoria_de(tags),
        sector: sector_de(tags),
        direccion,
        municipio: tags.addr_city.clone().unwrap_or_else(|| municipio.to_string()),
        lat,
        lon,
        telefono: tags.contact_phone.clone().or(tags.phone.clone()),
        email: tags.contact_email.clone().or(tags.email.clone()),
        web,
        fuente: "overpass".into(),
        fuente_id: format!("osm-{}", el.id),
        primera_vista: ahora.clone(),
        ultima_verificacion: Some(ahora),
    })
}

/// Ejecuta el scan de un municipio (y sector opcional) contra Overpass e inserta en SQLite.
/// Devuelve el número de negocios procesados.
pub async fn scan_municipio(store: &Store, municipio: &str, sector: Option<&str>) -> Result<usize> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .user_agent(UA)
        .build()?;

    let resp = client
        .post(OVERPASS_URL)
        .form(&[("data", query(municipio, sector, false))])
        .send()
        .await
        .context("Overpass (consulta exacta)")?;
    let mut data: OverpassResponse = resp.json().await.context("parseando respuesta Overpass")?;
    if data.elements.is_empty() {
        let resp = client
            .post(OVERPASS_URL)
            .form(&[("data", query(municipio, sector, true))])
            .send()
            .await
            .context("Overpass (consulta fuzzy)")?;
        data = resp.json().await.context("parseando respuesta Overpass (fuzzy)")?;
    }

    let mut count = 0;
    let conn = store.lock().unwrap();
    for el in &data.elements {
        if let Some(tags) = &el.tags {
            if let Some(neg) = a_negocio(el, tags, municipio) {
                store::upsert_negocio(&conn, &neg)?;
                count += 1;
            }
        }
    }
    Ok(count)
}
