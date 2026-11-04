//! Vigilancia de constituciones BORME. El sumario diario y los documentos XML se consultan
//! en la API oficial BOE; las filas CSV siguen disponibles para importaciones manuales.

use std::time::Duration as StdDuration;

use anyhow::{bail, Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use reqwest::{Client, StatusCode};
use roxmltree::{Document, Node};
use rusqlite::{params, Connection};

use crate::model::{Auditoria, EstadoWeb, Negocio};
use crate::scoring;
use crate::store::{self, Store};

const BOE_SUMARIO: &str = "https://www.boe.es/datosabiertos/api/borme/sumario";
const UA: &str = "RadarNavarra/0.1 (+https://radar-navarra.duckdns.org; contact: hello@carlotacarrillo.eu)";
const DIAS_RETROACTIVOS: i64 = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
struct LeadBorme {
    numero: String,
    nombre: String,
    domicilio: Option<String>,
    municipio: Option<String>,
}

/// Descarga los sumarios de los últimos siete días, omite fines de semana y procesa Navarra.
/// El identificador de artículo del BORME hace que las ejecuciones diarias sean idempotentes.
pub async fn watch_borme_reciente(store: &Store) -> Result<usize> {
    let client = cliente_borme()?;
    let hoy = Local::now().date_naive();
    let mut insertados = 0;

    for offset in (0..DIAS_RETROACTIVOS).rev() {
        let fecha = hoy - Duration::days(offset);
        if matches!(fecha.weekday(), Weekday::Sat | Weekday::Sun) {
            continue;
        }
        insertados += watch_fecha(&client, store, fecha).await?;
    }

    Ok(insertados)
}

/// Descarga e importa un sumario de fecha concreta (formato YYYY-MM-DD), útil para backfills.
pub async fn watch_borme_fecha(store: &Store, fecha: &str) -> Result<usize> {
    let fecha = NaiveDate::parse_from_str(fecha, "%Y-%m-%d")
        .context("la fecha de BORME debe tener formato YYYY-MM-DD")?;
    watch_fecha(&cliente_borme()?, store, fecha).await
}

fn cliente_borme() -> Result<Client> {
    Client::builder()
        .timeout(StdDuration::from_secs(30))
        .user_agent(UA)
        .build()
        .context("creando cliente HTTP para BOE")
}

async fn watch_fecha(client: &Client, store: &Store, fecha: NaiveDate) -> Result<usize> {
    let fecha_api = fecha.format("%Y%m%d").to_string();
    let url_sumario = format!("{BOE_SUMARIO}/{fecha_api}");
    let response = client.get(&url_sumario).header("Accept", "application/xml").send().await
        .with_context(|| format!("consultando sumario BORME del {fecha_api}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        // Días laborables festivos pueden no tener publicación.
        return Ok(0);
    }
    let xml_sumario = texto_respuesta(response, &url_sumario).await?;
    let Some((id_documento, url_xml)) = documento_navarra(&xml_sumario)
        .with_context(|| format!("leyendo sumario BORME del {fecha_api}"))?
    else {
        return Ok(0);
    };

    let response = client.get(&url_xml).header("Accept", "application/xml").send().await
        .with_context(|| format!("descargando documento BORME de Navarra {id_documento}"))?;
    let xml_documento = texto_respuesta(response, &url_xml).await?;
    importar_documento(store, &id_documento, &url_xml, &fecha_api, &xml_documento)
}

async fn texto_respuesta(response: reqwest::Response, url: &str) -> Result<String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("BOE respondió HTTP {status} en {url}: {}", body.chars().take(400).collect::<String>());
    }
    response.text().await.with_context(|| format!("leyendo respuesta de {url}"))
}

fn documento_navarra(xml: &str) -> Result<Option<(String, String)>> {
    let doc = Document::parse(xml).context("XML del sumario BOE no válido")?;
    let seccion_a = doc.descendants().find(|n| {
        n.has_tag_name("seccion") && n.attribute("codigo") == Some("A")
    });
    let Some(seccion_a) = seccion_a else { return Ok(None) };

    for item in seccion_a.descendants().filter(|n| n.has_tag_name("item")) {
        let provincia = texto_hijo(item, "titulo").unwrap_or_default();
        if normalizar(provincia) != "NAVARRA" {
            continue;
        }
        let id = texto_hijo(item, "identificador").unwrap_or_default();
        let url = texto_hijo(item, "url_xml").unwrap_or_default();
        if !id.is_empty() && url.starts_with("https://www.boe.es/") {
            return Ok(Some((id.to_string(), url.to_string())));
        }
    }
    Ok(None)
}

fn texto_hijo<'a, 'input>(node: Node<'a, 'input>, nombre: &str) -> Option<&'a str> {
    node.children().find(|hijo| hijo.has_tag_name(nombre)).and_then(|hijo| hijo.text())
}

fn normalizar(s: &str) -> String {
    s.trim().to_uppercase()
}

fn importar_documento(
    store: &Store,
    id_documento: &str,
    url_xml: &str,
    fecha: &str,
    xml: &str,
) -> Result<usize> {
    let leads = leads_del_documento(xml)?;
    let conn = store.lock().unwrap();
    let mut insertados = 0;
    for lead in leads {
        let ahora = store::ahora();
        let negocio = Negocio {
            id: 0,
            nombre: lead.nombre,
            categoria: Some("constitucion".into()),
            sector: None,
            direccion: lead.domicilio,
            municipio: lead.municipio.unwrap_or_else(|| "Navarra".into()),
            lat: None,
            lon: None,
            telefono: None,
            email: None,
            web: None,
            fuente: "borme".into(),
            fuente_id: format!("{id_documento}-{}", lead.numero),
            primera_vista: ahora.clone(),
            ultima_verificacion: Some(ahora.clone()),
        };
        if guardar_lead(&conn, &negocio, &ahora, fecha, Some(url_xml))? {
            insertados += 1;
        }
    }
    Ok(insertados)
}

fn leads_del_documento(xml: &str) -> Result<Vec<LeadBorme>> {
    let doc = Document::parse(xml).context("XML del documento BORME no válido")?;
    let mut leads = Vec::new();
    let mut articulo: Option<(String, String, String)> = None;

    for nodo in doc.descendants().filter(|n| n.is_element() && n.has_tag_name("p")) {
        match nodo.attribute("class") {
            Some("articulo") => {
                if let Some(datos) = articulo.take() {
                    if let Some(lead) = extraer_lead(datos) { leads.push(lead); }
                }
                let titulo = nodo.text().unwrap_or_default().trim();
                let (numero, nombre) = titulo.split_once(" - ").unwrap_or(("", titulo));
                articulo = Some((numero.trim().to_string(), nombre.trim().to_string(), String::new()));
            }
            Some("parrafo") => {
                if let (Some((_, _, cuerpo)), Some(texto)) = (articulo.as_mut(), nodo.text()) {
                    if !cuerpo.is_empty() { cuerpo.push(' '); }
                    cuerpo.push_str(texto.trim());
                }
            }
            _ => {}
        }
    }
    if let Some(datos) = articulo {
        if let Some(lead) = extraer_lead(datos) { leads.push(lead); }
    }
    Ok(leads)
}

fn extraer_lead((numero, nombre, cuerpo): (String, String, String)) -> Option<LeadBorme> {
    if numero.is_empty() || nombre.is_empty() || !cuerpo.to_lowercase().contains("constitución") {
        return None;
    }
    let nombre = nombre.trim().trim_end_matches('.').trim().to_string();
    let domicilio = cuerpo.split_once("Domicilio:").map(|(_, resto)| {
        [". Capital:", ". Nombramientos:", ". Datos registrales:", ". Declaración" ]
            .iter()
            .filter_map(|separador| resto.find(separador))
            .min()
            .map(|fin| resto[..fin].trim().trim_end_matches('.').to_string())
            .unwrap_or_else(|| resto.trim().trim_end_matches('.').to_string())
    }).filter(|d| !d.is_empty());
    let municipio = domicilio.as_deref().and_then(municipio_al_final);
    Some(LeadBorme { numero, nombre, domicilio, municipio })
}

fn municipio_al_final(domicilio: &str) -> Option<String> {
    if !domicilio.ends_with(')') { return None; }
    let mut profundidad = 0;
    for (indice, caracter) in domicilio.char_indices().rev() {
        match caracter {
            ')' => profundidad += 1,
            '(' => {
                profundidad -= 1;
                if profundidad == 0 {
                    let municipio = domicilio[indice + 1..domicilio.len() - 1].trim();
                    return (!municipio.is_empty()).then(|| municipio.to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// Importa un CSV con columnas nombre, municipio, domicilio, objeto, cnae y fecha.
/// Devuelve leads nuevos; volver a importar el mismo fichero no duplica auditorías.
pub fn watch_desde_csv(store: &Store, path: &str) -> Result<usize> {
    let mut rdr = csv::Reader::from_path(path).context("leyendo CSV de altas")?;
    let conn = store.lock().unwrap();
    let mut insertados = 0;
    for rec in rdr.records() {
        let rec = rec?;
        if rec.len() < 2 { continue; }
        let nombre = rec.get(0).unwrap_or("").trim().to_string();
        if nombre.is_empty() { continue; }
        let municipio = rec.get(1).unwrap_or("").trim().to_string();
        let domicilio = rec.get(2).unwrap_or("").trim().to_string();
        let fecha_const = rec.get(5).unwrap_or("").trim().to_string();
        let stable_name = nombre.to_lowercase().replace(' ', "-");
        let fuente_id = format!("csv-{stable_name}-{}", fecha_const.replace(['/', '-'], ""));
        let ahora = store::ahora();
        let negocio = Negocio {
            id: 0,
            nombre,
            categoria: Some("constitucion".into()),
            sector: None,
            direccion: if domicilio.is_empty() { None } else { Some(domicilio) },
            municipio: if municipio.is_empty() { "Navarra".into() } else { municipio },
            lat: None,
            lon: None,
            telefono: None,
            email: None,
            web: None,
            fuente: "borme".into(),
            fuente_id,
            primera_vista: ahora.clone(),
            ultima_verificacion: Some(ahora.clone()),
        };
        if guardar_lead(&conn, &negocio, &ahora, &fecha_const, None)? {
            insertados += 1;
        }
    }
    Ok(insertados)
}

fn guardar_lead(
    conn: &Connection,
    negocio: &Negocio,
    fecha: &str,
    fecha_publicacion: &str,
    url_fuente: Option<&str>,
) -> Result<bool> {
    let ya_existe: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM negocios WHERE fuente=?1 AND fuente_id=?2)",
        params![negocio.fuente, negocio.fuente_id],
        |row| row.get(0),
    )?;
    let id = store::upsert_negocio(conn, negocio)?;
    if ya_existe { return Ok(false); }

    let mut senales = serde_json::json!({"lead_apertura": true, "fuente": "BOE/BORME"});
    if !fecha_publicacion.is_empty() {
        senales["fecha_publicacion"] = serde_json::Value::String(fecha_publicacion.to_string());
    }
    if let Some(url) = url_fuente {
        senales["fuente_url"] = serde_json::Value::String(url.to_string());
    }
    store::insert_auditoria(conn, &Auditoria {
        id: 0,
        negocio_id: id,
        fecha: fecha.to_string(),
        estado_web: EstadoWeb::SinWeb,
        score: 90,
        prioridad: scoring::prioridad(90),
        senales,
    })?;
    Ok(true)
}

/// Lista los negocios dados de alta como lead de apertura.
pub fn leads_apertura(conn: &Connection) -> Result<Vec<Negocio>> {
    let mut stmt = conn.prepare(
        "SELECT id, nombre, categoria, sector, direccion, municipio, lat, lon, telefono, email,
                web, fuente, fuente_id, primera_vista, ultima_verificacion
         FROM negocios WHERE fuente='borme' ORDER BY primera_vista DESC LIMIT 100",
    )?;
    let rows = stmt.query_map([], |r| Ok(Negocio {
        id: r.get(0)?, nombre: r.get(1)?, categoria: r.get(2)?, sector: r.get(3)?,
        direccion: r.get(4)?, municipio: r.get(5)?, lat: r.get(6)?, lon: r.get(7)?,
        telefono: r.get(8)?, email: r.get(9)?, web: r.get(10)?, fuente: r.get(11)?,
        fuente_id: r.get(12)?, primera_vista: r.get(13)?, ultima_verificacion: r.get(14)?,
    }))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::Write};

    fn guardar_csv_temporal() -> String {
        let path = std::env::temp_dir().join(format!(
            "radar-borme-{}-{}.csv",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
        ));
        let mut file = File::create(&path).unwrap();
        writeln!(file, "nombre,municipio,domicilio,objeto,cnae,fecha").unwrap();
        writeln!(file, r#"Apertura Uno SL,Pamplona,"Calle Uno 1",servicios,6201,2026-11-01"#).unwrap();
        writeln!(file, r#"Apertura Dos SL,Tudela,"Calle Dos 2",comercio,4711,2026-11-01"#).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn csv_crea_dos_leads_score_90_prioridad_a_y_es_idempotente() {
        let store = store::abrir(":memory:").unwrap();
        let path = guardar_csv_temporal();
        assert_eq!(watch_desde_csv(&store, &path).unwrap(), 2);
        assert_eq!(watch_desde_csv(&store, &path).unwrap(), 0);
        let conn = store.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM negocios WHERE fuente='borme'", [], |r| r.get(0)).unwrap();
        let auditorias: i64 = conn.query_row(
            "SELECT COUNT(*) FROM auditorias WHERE score=90 AND prioridad='A' AND estado_web='SIN_WEB'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 2);
        assert_eq!(auditorias, 2);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn parsea_constituciones_y_omite_otros_actos() {
        let xml = r#"<documento><texto>
          <p class="articulo">451230 - RESYBLAKE SL.</p>
          <p class="parrafo">Constitución. Comienzo de operaciones: 27.08.26. Objeto social: servicios. Domicilio: C/ DONANTES DE SANGRE, 20 2º B (TUDELA). Capital: 3.000,00 Euros.</p>
          <p class="articulo">451231 - OTRA SL.</p>
          <p class="parrafo">Nombramientos. Adm. Unico: Persona. Datos registrales.</p>
        </texto></documento>"#;
        let leads = leads_del_documento(xml).unwrap();
        assert_eq!(leads, vec![LeadBorme {
            numero: "451230".into(), nombre: "RESYBLAKE SL".into(),
            domicilio: Some("C/ DONANTES DE SANGRE, 20 2º B (TUDELA)".into()),
            municipio: Some("TUDELA".into()),
        }]);
        assert_eq!(municipio_al_final("NOAIN (NOAIN (VALLE DE ELORZ))"), Some("NOAIN (VALLE DE ELORZ)".into()));
    }
}
