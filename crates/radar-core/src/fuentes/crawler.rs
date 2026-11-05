//! Rastreo responsable de fuentes web públicas registradas por el usuario.

use std::{
    collections::{HashSet, VecDeque},
    net::IpAddr,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use robotstxt::DefaultMatcher;
use scraper::{Html, Selector};
use serde_json::Value;
use sha2::Digest;

use crate::eligibility::{self, Evidence};
use crate::{
    model::{Fuente, Negocio},
    store::{self, Store},
};

const UA: &str = "RadarNavarra/0.1 (+https://radar-navarra.duckdns.org; crawler responsable)";
const MAX_HTML_BYTES: usize = 2 * 1024 * 1024;

#[derive(Default)]
struct ExtractedBusiness {
    name: String,
    category: Option<String>,
    street: Option<String>,
    city: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    website: Option<String>,
    region: Option<String>,
    postal: Option<String>,
    brand: Option<String>,
    operator: Option<String>,
    parent: bool,
    entities: Vec<String>,
}

pub async fn rastrear_fuente(store: &Store, fuente_id: i64) -> Result<Value> {
    let fuente = {
        let conn = store.lock().unwrap();
        store::fuente(&conn, fuente_id)?.context("fuente no encontrada")?
    };
    validar_fuente(&fuente)?;
    let result = rastrear_inner(store, &fuente).await;
    let conn = store.lock().unwrap();
    store::actualizar_ejecucion_fuente(
        &conn,
        fuente.id,
        result.as_ref().err().map(|e| e.to_string()).as_deref(),
    )?;
    result
}

fn validar_fuente(fuente: &Fuente) -> Result<()> {
    if !fuente.habilitada {
        bail!("la fuente está deshabilitada");
    }
    if fuente
        .revisada_en
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
        || fuente
            .condiciones
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        || fuente.atribucion.as_deref().unwrap_or("").trim().is_empty()
    {
        bail!("la fuente debe tener revisión de condiciones y atribución antes de rastrearse");
    }
    if !["semilla", "dataset", "busqueda"].contains(&fuente.tipo.as_str()) {
        bail!("tipo de fuente desconocido: {}", fuente.tipo);
    }
    let parsed = reqwest::Url::parse(&fuente.url).context("URL de fuente no válida")?;
    if !["https", "http"].contains(&parsed.scheme()) || parsed.host_str().is_none() {
        bail!("solo se permiten URLs HTTP(S) públicas");
    }
    if fuente.tipo == "busqueda" && !fuente.url.contains("{query}") {
        bail!("la URL del proveedor de búsqueda debe contener {{query}}");
    }
    Ok(())
}

async fn rastrear_inner(store: &Store, fuente: &Fuente) -> Result<Value> {
    let seed = reqwest::Url::parse(&fuente.url)?;
    validar_host_publico(&seed).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(UA)
        .build()?;
    let robots_url = seed.join("/robots.txt")?;
    let robots_resp = client
        .get(robots_url)
        .send()
        .await
        .context("consultando robots.txt")?;
    let robots_body = match robots_resp.status().as_u16() {
        404 => String::new(),
        200..=299 => robots_resp.text().await.context("leyendo robots.txt")?,
        status => bail!("robots.txt respondió HTTP {status}; se detiene el rastreo por seguridad"),
    };
    let delay_ms = std::env::var("RADAR_CRAWL_DELAY_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1500)
        .clamp(500, 30000);

    if fuente.tipo == "busqueda" {
        return buscar_dominios(store, &client, fuente, &robots_body, delay_ms).await;
    }

    let mut cola = VecDeque::from([(seed.clone(), 0_i64)]);
    let mut vistos = HashSet::new();
    let mut paginas = 0_u64;
    let mut leads = 0_u64;
    let host = seed.host_str().unwrap_or_default().to_ascii_lowercase();
    let max_paginas = fuente.max_paginas_dia.clamp(1, 1000) as u64;
    while let Some((url, depth)) = cola.pop_front() {
        if paginas >= max_paginas {
            break;
        }
        let canonical = canonical_url(&url);
        if !vistos.insert(canonical.clone()) {
            continue;
        }
        let permitido = {
            let mut matcher = DefaultMatcher::default();
            matcher.one_agent_allowed_by_robots(&robots_body, "RadarNavarra", url.as_str())
        };
        if !permitido {
            let conn = store.lock().unwrap();
            store::registrar_pagina(
                &conn,
                fuente.id,
                &canonical,
                depth,
                None,
                Some("bloqueada por robots.txt"),
            )?;
            continue;
        }
        {
            let conn = store.lock().unwrap();
            if store::pagina_rastreada(&conn, fuente.id, &canonical)? {
                continue;
            }
            if store::paginas_hoy(&conn, fuente.id)? >= fuente.max_paginas_dia {
                break;
            }
        }
        let response = client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("descargando {canonical}"))?;
        let status = response.status();
        if status.as_u16() == 429 || status.is_server_error() {
            let conn = store.lock().unwrap();
            store::registrar_pagina(
                &conn,
                fuente.id,
                &canonical,
                depth,
                Some(status.as_u16()),
                Some("se detuvo ante límite o error temporal del servidor"),
            )?;
            bail!(
                "el sitio respondió HTTP {}; el rastreo se detuvo y puede reintentarse más tarde",
                status.as_u16()
            );
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.context("leyendo página rastreada")? {
            if bytes.len() + chunk.len() > MAX_HTML_BYTES {
                break;
            }
            bytes.extend_from_slice(&chunk);
        }
        let html = String::from_utf8_lossy(&bytes).into_owned();
        {
            let conn = store.lock().unwrap();
            store::registrar_pagina(
                &conn,
                fuente.id,
                &canonical,
                depth,
                Some(status.as_u16()),
                None,
            )?;
        }
        paginas += 1;
        if !status.is_success() || !content_type.contains("text/html") {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            continue;
        }
        let parsed = parsear_html(&html, &url);
        for (index, negocio) in parsed.negocios.into_iter().enumerate() {
            if guardar_lead(store, fuente, &canonical, index, negocio).await? {
                leads += 1;
            }
        }
        if depth < fuente.profundidad.clamp(0, 4) {
            for link in parsed.links {
                if link
                    .host_str()
                    .is_some_and(|h| h.eq_ignore_ascii_case(&host))
                {
                    if !vistos.contains(&canonical_url(&link)) {
                        cola.push_back((link, depth + 1));
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
    Ok(
        serde_json::json!({"paginas":paginas,"leads":leads,"max_paginas_dia":max_paginas,"host":host}),
    )
}

struct ParsedPage {
    negocios: Vec<ExtractedBusiness>,
    links: Vec<reqwest::Url>,
}

fn parsear_html(html: &str, base: &reqwest::Url) -> ParsedPage {
    let document = Html::parse_document(html);
    let script_sel = Selector::parse("script[type='application/ld+json']").unwrap();
    let mut negocios = Vec::new();
    for script in document.select(&script_sel) {
        if let Some(raw) = script.text().next() {
            if let Ok(value) = serde_json::from_str::<Value>(raw) {
                extraer_jsonld(&value, &mut negocios);
            }
        }
    }
    let anchor_sel = Selector::parse("a[href]").unwrap();
    let mut links = Vec::new();
    for a in document.select(&anchor_sel) {
        if let Some(href) = a.value().attr("href") {
            if let Ok(mut url) = base.join(href) {
                if ["http", "https"].contains(&url.scheme()) {
                    url.set_fragment(None);
                    if !links.contains(&url) {
                        links.push(url);
                    }
                }
            }
        }
    }
    ParsedPage { negocios, links }
}

fn extraer_jsonld(value: &Value, out: &mut Vec<ExtractedBusiness>) {
    match value {
        Value::Array(items) => {
            for item in items {
                extraer_jsonld(item, out);
            }
        }
        Value::Object(obj) => {
            if let Some(graph) = obj.get("@graph") {
                extraer_jsonld(graph, out);
            }
            let kind = obj
                .get("@type")
                .map(|t| match t {
                    Value::String(s) => vec![s.as_str()],
                    Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
                    _ => Vec::new(),
                })
                .unwrap_or_default();
            if kind.iter().any(|t| schema_sector(t).is_some()) {
                if let Some(name) = obj
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                {
                    let address = obj.get("address");
                    let address_string = |key: &str| {
                        address
                            .and_then(|a| a.get(key))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    };
                    let category = kind.first().map(|s| s.to_string());
                    out.push(ExtractedBusiness {
                        name: name.to_string(),
                        category,
                        region: address_string("addressRegion"),
                        postal: address_string("postalCode"),
                        brand: obj
                            .get("brand")
                            .and_then(|v| v.as_str().or_else(|| v["name"].as_str()))
                            .map(str::to_owned),
                        operator: obj
                            .get("parentOrganization")
                            .or_else(|| obj.get("branchOf"))
                            .and_then(|v| v["name"].as_str())
                            .map(str::to_owned),
                        parent: obj.contains_key("parentOrganization")
                            || obj.contains_key("branchOf"),
                        entities: ["brand", "parentOrganization", "branchOf"]
                            .iter()
                            .filter_map(|k| obj.get(*k))
                            .flat_map(|v| {
                                let mut links = Vec::new();
                                for field in ["sameAs", "@id"] {
                                    if let Some(raw) = v.get(field) {
                                        if let Some(s) = raw.as_str() {
                                            links.push(s.to_string());
                                        }
                                        if let Some(a) = raw.as_array() {
                                            links.extend(
                                                a.iter()
                                                    .filter_map(Value::as_str)
                                                    .map(str::to_owned),
                                            );
                                        }
                                    }
                                }
                                links
                                    .into_iter()
                                    .filter_map(|u| {
                                        u.strip_prefix("https://www.wikidata.org/wiki/")
                                            .map(str::to_owned)
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect(),
                        street: address_string("streetAddress"),
                        city: address_string("addressLocality"),
                        phone: obj
                            .get("telephone")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        email: obj.get("email").and_then(Value::as_str).map(str::to_string),
                        website: obj.get("url").and_then(Value::as_str).map(str::to_string),
                    });
                }
            }
            for (key, child) in obj {
                if key != "@graph" {
                    if child.is_array() {
                        extraer_jsonld(child, out);
                    }
                }
            }
        }
        _ => {}
    }
}

fn schema_sector(kind: &str) -> Option<&'static str> {
    let kind = kind.rsplit('/').next().unwrap_or(kind);
    Some(match kind {
        "Restaurant" | "CafeOrCoffeeShop" | "BarOrPub" | "FoodEstablishment" | "Bakery"
        | "Hotel" | "Motel" | "BedAndBreakfast" | "Hostel" | "LodgingBusiness" => "hosteleria",
        "Store" | "GroceryStore" | "ClothingStore" | "ShoeStore" | "BookStore" | "JewelryStore"
        | "HardwareStore" | "FurnitureStore" | "Florist" | "ElectronicsStore"
        | "ConvenienceStore" | "SportingGoodsStore" | "PetStore" => "comercio",
        "ProfessionalService"
        | "AccountingService"
        | "Attorney"
        | "Notary"
        | "LegalService"
        | "RealEstateAgent" => "profesionales",
        "LocalBusiness"
        | "Dentist"
        | "MedicalClinic"
        | "Pharmacy"
        | "HairSalon"
        | "BeautySalon"
        | "HealthAndBeautyBusiness"
        | "AutoRepair"
        | "AutomotiveBusiness"
        | "DryCleaningOrLaundry"
        | "Electrician"
        | "Plumber"
        | "VeterinaryCare"
        | "Locksmith"
        | "TravelAgency"
        | "HomeAndConstructionBusiness" => "servicios",
        _ => return None,
    })
}

async fn guardar_lead(
    store: &Store,
    fuente: &Fuente,
    url_origen: &str,
    index: usize,
    lead: ExtractedBusiness,
) -> Result<bool> {
    let website = lead
        .website
        .as_deref()
        .and_then(|u| reqwest::Url::parse(u).ok())
        .filter(|u| ["http", "https"].contains(&u.scheme()))
        .map(|u| u.to_string());
    let sector = lead.category.as_deref().and_then(schema_sector);
    let in_navarra = {
        let conn = store.lock().unwrap();
        let known = lead
            .city
            .as_deref()
            .map(eligibility::normalize)
            .map(|city| {
                conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM municipios_navarra WHERE nombre=?1)",
                    [city],
                    |r| r.get::<_, bool>(0),
                )
                .unwrap_or(false)
            })
            .unwrap_or(false);
        known
            || lead.region.as_deref().is_some_and(|r| {
                ["navarra", "nafarroa", "navarre"].contains(&eligibility::normalize(r).as_str())
            })
            || lead.postal.as_deref().is_some_and(|p| {
                p.len() == 5 && p.starts_with("31") && p.chars().all(|c| c.is_ascii_digit())
            })
    };
    let evidence = Evidence {
        commercial: sector.is_some(),
        in_navarra,
        category: lead.category.clone().unwrap_or_default(),
        brand: lead.brand,
        operator: lead.operator,
        entities: lead.entities,
        parent_organization: lead.parent,
        ..Default::default()
    };
    let classifier = eligibility::resolve(store, &[evidence.clone()]).await?;
    let now = store::ahora();
    let fuente_id = format!(
        "crawler-{}-{}-{index}",
        fuente.id,
        hex::encode(&sha2::Sha256::digest(url_origen.as_bytes())[..8])
    );
    let negocio = Negocio {
        id: 0,
        nombre: lead.name,
        categoria: lead.category.clone(),
        sector: sector.map(str::to_string),
        rubro: crate::rubros::desde_categoria(lead.category.as_deref(), sector).map(str::to_owned),
        evidencia_rubro: None,
        direccion: lead.street,
        municipio: lead.city.unwrap_or_else(|| "Navarra".into()),
        lat: None,
        lon: None,
        telefono: lead.phone,
        email: lead.email,
        web: website.clone(),
        estado_web_lista: if website.is_some() {
            "confirmada"
        } else {
            "sin_localizar"
        }
        .into(),
        estado_auditoria_tecnica: "pendiente".into(),
        fuente: "crawler".into(),
        fuente_id,
        primera_vista: now.clone(),
        ultima_verificacion: None,
        score_potencial: None,
        prioridad_comercial: None,
        estado_pipeline: None,
        oportunidades: Vec::new(),
        descartado: false,
    };
    let conn = store.lock().unwrap();
    if !eligibility::record(&conn, &negocio, &evidence, &classifier, url_origen)?.admitted() {
        return Ok(false);
    }
    let (_, fusionado) =
        store::upsert_negocio_fuente(&conn, &negocio, fuente.id, &negocio.fuente_id, url_origen)?;
    Ok(!fusionado)
}

async fn buscar_dominios(
    store: &Store,
    client: &reqwest::Client,
    fuente: &Fuente,
    robots: &str,
    delay_ms: u64,
) -> Result<Value> {
    let mut queries = [
        "negocios Navarra",
        "empresas Navarra",
        "comercios Navarra",
        "restaurantes Navarra",
        "servicios profesionales Navarra",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    if let Ok(extra) = std::env::var("RADAR_SEARCH_QUERIES") {
        queries.extend(
            extra
                .split('|')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
        );
    }
    let mut candidates = 0;
    let mut pages = 0;
    for query in queries
        .into_iter()
        .take(fuente.max_paginas_dia.clamp(1, 100) as usize)
    {
        let encoded = query
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join("+");
        let target = reqwest::Url::parse(&fuente.url.replace("{query}", &encoded))?;
        if !DefaultMatcher::default().one_agent_allowed_by_robots(
            robots,
            "RadarNavarra",
            target.as_str(),
        ) {
            continue;
        }
        let response = client
            .get(target.clone())
            .send()
            .await
            .context("consultando proveedor de búsqueda")?;
        if response.status().as_u16() == 429 || response.status().is_server_error() {
            bail!(
                "proveedor de búsqueda respondió HTTP {}; se detiene el job",
                response.status().as_u16()
            );
        }
        let status = response.status().as_u16();
        let bytes = response
            .bytes()
            .await
            .context("leyendo resultados del buscador")?;
        if bytes.len() > MAX_HTML_BYTES {
            continue;
        }
        let parsed = parsear_html(&String::from_utf8_lossy(&bytes), &target);
        let own_host = target.host_str().unwrap_or_default().to_ascii_lowercase();
        {
            let conn = store.lock().unwrap();
            for link in parsed.links {
                if link
                    .host_str()
                    .is_some_and(|h| !h.eq_ignore_ascii_case(&own_host))
                {
                    store::registrar_fuente_candidata(&conn, link.as_str(), fuente.id)?;
                    candidates += 1;
                }
            }
            store::registrar_pagina(&conn, fuente.id, target.as_str(), 0, Some(status), None)?;
        }
        pages += 1;
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
    Ok(serde_json::json!({"busquedas":pages,"dominios_candidatos":candidates}))
}

async fn validar_host_publico(url: &reqwest::Url) -> Result<()> {
    let host = url.host_str().context("la URL no tiene host")?;
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".localhost")
        || host.ends_with(".local")
    {
        bail!("no se permite rastrear hosts locales");
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if !ip_publica(ip) {
            bail!("no se permite rastrear IP privada o reservada");
        }
        return Ok(());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .context("resolviendo dominio de la fuente")?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|addr| !ip_publica(addr.ip())) {
        bail!("el dominio no resuelve exclusivamente a direcciones públicas");
    }
    Ok(())
}

fn ip_publica(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
                || v.is_multicast())
        }
        IpAddr::V6(v) => {
            !(v.is_loopback()
                || v.is_unspecified()
                || v.is_unique_local()
                || v.is_unicast_link_local()
                || v.is_multicast())
        }
    }
}

fn canonical_url(url: &reqwest::Url) -> String {
    let mut u = url.clone();
    u.set_fragment(None);
    u.to_string()
}
