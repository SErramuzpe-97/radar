//! Exportaciones para rutas de visita: CSV, GeoJSON y GPX (diseño §8.2, §8.3).

use std::io::Write;

use anyhow::Result;
use rusqlite::Connection;

use crate::model::FiltrosNegocios;
use crate::store;

pub fn export_csv(conn: &Connection, f: &FiltrosNegocios, out: &mut dyn Write) -> Result<()> {
    let pagina = store::list_negocios(conn, f)?;
    let mut wtr = csv::Writer::from_writer(out);
    wtr.write_record([
        "id",
        "nombre",
        "sector",
        "rubro",
        "municipio",
        "direccion",
        "telefono",
        "email",
        "web",
        "lat",
        "lon",
        "prioridad",
        "score",
        "estado_web",
        "fuente",
    ])?;
    for n in &pagina.items {
        let (estado_web, score, prioridad) = ultima_auditoria(conn, n.id);
        wtr.write_record([
            n.id.to_string(),
            n.nombre.clone(),
            n.sector.clone().unwrap_or_default(),
            n.rubro.clone().unwrap_or_default(),
            n.municipio.clone(),
            n.direccion.clone().unwrap_or_default(),
            n.telefono.clone().unwrap_or_default(),
            n.email.clone().unwrap_or_default(),
            n.web.clone().unwrap_or_default(),
            n.lat.map(|v| v.to_string()).unwrap_or_default(),
            n.lon.map(|v| v.to_string()).unwrap_or_default(),
            prioridad,
            score.map(|s| s.to_string()).unwrap_or_default(),
            estado_web.unwrap_or_else(|| "SIN_AUDITAR".into()),
            n.fuente.clone(),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn export_geojson(conn: &Connection, f: &FiltrosNegocios) -> Result<String> {
    let pagina = store::list_negocios(conn, f)?;
    let mut features = Vec::new();
    for n in &pagina.items {
        let Some(lat) = n.lat else { continue };
        let Some(lon) = n.lon else { continue };
        let (estado_web, score, prioridad) = ultima_auditoria(conn, n.id);
        features.push(serde_json::json!({
            "type": "Feature",
            "geometry": {"type": "Point", "coordinates": [lon, lat]},
            "properties": {
                "id": n.id, "nombre": n.nombre, "sector": n.sector, "rubro":n.rubro, "evidencia_rubro":n.evidencia_rubro, "municipio": n.municipio,
                "direccion": n.direccion, "telefono": n.telefono, "email": n.email, "web": n.web,
                "estado_web": estado_web, "score": score, "prioridad": prioridad, "fuente": n.fuente,
            }
        }));
    }
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "type": "FeatureCollection",
        "features": features,
        "attribution": "© OpenStreetMap contributors (ODbL)"
    }))?)
}

pub fn export_gpx(conn: &Connection, f: &FiltrosNegocios) -> Result<String> {
    let pagina = store::list_negocios(conn, f)?;
    let mut wpts = String::new();
    for n in &pagina.items {
        let (Some(lat), Some(lon)) = (n.lat, n.lon) else {
            continue;
        };
        let (_, score, prioridad) = ultima_auditoria(conn, n.id);
        wpts.push_str(&format!(
            "  <wpt lat=\"{lat}\" lon=\"{lon}\"><name>{}</name><desc>prioridad {} · score {} · {}</desc></wpt>\n",
            xml_escape(&n.nombre),
            prioridad,
            score.map(|s| s.to_string()).unwrap_or_else(|| "?".into()),
            xml_escape(&n.direccion.clone().unwrap_or_default()),
        ));
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"radar\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n{wpts}</gpx>\n"
    ))
}

fn ultima_auditoria(conn: &Connection, negocio_id: i64) -> (Option<String>, Option<i64>, String) {
    let r = conn.query_row(
        "SELECT estado_web, score, prioridad FROM auditorias WHERE negocio_id=?1 ORDER BY fecha DESC, id DESC LIMIT 1",
        [negocio_id],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<i64>>(1)?, row.get::<_, Option<String>>(2)?)),
    );
    match r {
        Ok((e, s, p)) => (e, s, p.unwrap_or_else(|| "C".into())),
        Err(_) => (None, None, "C".into()),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
