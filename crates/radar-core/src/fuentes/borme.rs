//! Vigilancia de negocios nuevos (diseño §5.1): constituciones BORME vía OpenMercantil.
//! V1: importación desde CSV exportado (nombre, municipio, domicilio, objeto, cnae, fecha).
//! Cada alta nueva entra como lead de apertura: SIN_WEB, score fijo 90, prioridad A.

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::model::{Auditoria, EstadoWeb, Negocio};
use crate::scoring;
use crate::store::{self, Store};

pub fn watch_desde_csv(store: &Store, path: &str) -> Result<usize> {
    let mut rdr = csv::Reader::from_path(path).context("leyendo CSV de altas")?;
    let mut count = 0;
    let conn = store.lock().unwrap();
    for rec in rdr.records() {
        let rec = rec?;
        if rec.len() < 2 {
            continue;
        }
        let nombre = rec.get(0).unwrap_or("").trim().to_string();
        if nombre.is_empty() {
            continue;
        }
        let municipio = rec.get(1).unwrap_or("").trim().to_string();
        let domicilio = rec.get(2).unwrap_or("").trim().to_string();
        let fecha_const = rec.get(5).unwrap_or("").trim().to_string();
        let fuente_id = format!(
            "borme-{}",
            nombre.to_lowercase().replace(' ', "-") + &fecha_const.replace(['/', '-'], "")
        );
        let ahora = store::ahora();
        let neg = Negocio {
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
        let id = store::upsert_negocio(&conn, &neg)?;
        let senales = serde_json::json!({"lead_apertura": true, "fuente": "borme"});
        let (score, _) = scoring::puntuar(EstadoWeb::SinWeb, &senales, &neg);
        let score = score.max(90); // lead caliente: score fijo alto (diseño §5.1)
        let prioridad = scoring::prioridad(score);
        store::insert_auditoria(
            &conn,
            &Auditoria {
                id: 0,
                negocio_id: id,
                fecha: ahora,
                estado_web: EstadoWeb::SinWeb,
                score,
                prioridad,
                senales,
            },
        )?;
        count += 1;
    }
    Ok(count)
}

/// Lista negocios dados de alta como lead de apertura (para debug/CLI).
pub fn leads_apertura(conn: &Connection) -> Result<Vec<Negocio>> {
    let mut stmt = conn.prepare(
        "SELECT id, nombre, categoria, sector, direccion, municipio, lat, lon, telefono, email,
                web, fuente, fuente_id, primera_vista, ultima_verificacion
         FROM negocios WHERE fuente='borme' ORDER BY primera_vista DESC LIMIT 100",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Negocio {
            id: r.get(0)?,
            nombre: r.get(1)?,
            categoria: r.get(2)?,
            sector: r.get(3)?,
            direccion: r.get(4)?,
            municipio: r.get(5)?,
            lat: r.get(6)?,
            lon: r.get(7)?,
            telefono: r.get(8)?,
            email: r.get(9)?,
            web: r.get(10)?,
            fuente: r.get(11)?,
            fuente_id: r.get(12)?,
            primera_vista: r.get(13)?,
            ultima_verificacion: r.get(14)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
