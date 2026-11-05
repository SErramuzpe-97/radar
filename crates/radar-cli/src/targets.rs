use anyhow::{bail, Context, Result};
use radar_core::{eligibility, rubros, store};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct ObjetivoCsv {
    nombre: String,
    #[serde(default)]
    municipio: String,
    rubro: String,
    #[serde(default)]
    descripcion: String,
    #[serde(default)]
    procedencia: String,
}

fn clave(nombre: &str, municipio: &str) -> String {
    format!(
        "{}:{}",
        eligibility::normalize(nombre),
        eligibility::normalize(municipio)
    )
}

pub fn importar(conn: &Connection, archivo: &Path) -> Result<serde_json::Value> {
    let mut reader = csv::Reader::from_path(archivo)
        .with_context(|| format!("abriendo lista curada {}", archivo.display()))?;
    let negocios = {
        let mut stmt =
            conn.prepare("SELECT id,nombre,municipio FROM negocios WHERE admisible=1")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut estados = Vec::new();
    for raw in reader.deserialize::<ObjetivoCsv>() {
        let row = raw.context("leyendo fila de objetivos CSV")?;
        let nombre = row.nombre.trim();
        let municipio = row.municipio.trim();
        anyhow::ensure!(
            !nombre.is_empty(),
            "el nombre de un objetivo no puede estar vacío"
        );
        anyhow::ensure!(
            rubros::FILTROS.iter().any(|(key, _)| *key == row.rubro),
            "rubro desconocido '{}' para {nombre}",
            row.rubro
        );
        let key = clave(nombre, municipio);
        let nombre_n = eligibility::normalize(nombre);
        let municipio_n = eligibility::normalize(municipio);
        let candidates = negocios
            .iter()
            .filter(|(_, n, m)| {
                eligibility::normalize(n) == nombre_n
                    && (municipio_n.is_empty() || eligibility::normalize(m) == municipio_n)
            })
            .collect::<Vec<_>>();
        let (estado, negocio_id) = match candidates.as_slice() {
            [only] => ("coincidencia_verificada", Some(only.0)),
            [] => ("pendiente_revision", None),
            _ => ("ambigua", None),
        };
        let candidate_details = candidates
            .iter()
            .map(|(id, name, city)| json!({"id":id,"nombre":name,"municipio":city}))
            .collect::<Vec<_>>();
        conn.execute(
            "INSERT INTO negocios_objetivo(clave,nombre,municipio,rubro,descripcion,procedencia,estado,negocio_id,coincidencias,actualizado)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(clave) DO UPDATE SET nombre=excluded.nombre,municipio=excluded.municipio,rubro=excluded.rubro,
             descripcion=excluded.descripcion,procedencia=excluded.procedencia,estado=excluded.estado,
             negocio_id=excluded.negocio_id,coincidencias=excluded.coincidencias,actualizado=excluded.actualizado",
            params![key,nombre,if municipio.is_empty(){None}else{Some(municipio)},row.rubro,row.descripcion,row.procedencia,estado,negocio_id,json!(candidate_details).to_string(),store::ahora()],
        )?;
        if let Some(id) = negocio_id {
            conn.execute(
                "UPDATE negocios SET rubro=COALESCE(rubro,?1) WHERE id=?2",
                params![row.rubro, id],
            )?;
        }
        estados.push(json!({"nombre":nombre,"municipio":municipio,"estado":estado,"negocio_id":negocio_id,"coincidencias":candidate_details}));
    }
    let verificados = estados
        .iter()
        .filter(|r| r["estado"] == "coincidencia_verificada")
        .count();
    let ambiguos = estados.iter().filter(|r| r["estado"] == "ambigua").count();
    Ok(
        json!({"filas":estados.len(),"coincidencias_verificadas":verificados,"pendientes_revision":estados.len()-verificados-ambiguos,"ambiguas":ambiguos,"objetivos":estados,"nota":"Los objetivos sin coincidencia exacta permanecen fuera de la cartera hasta verificar su identidad y ubicación."}),
    )
}

pub fn listar(conn: &Connection) -> Result<serde_json::Value> {
    let mut stmt = conn.prepare("SELECT clave,nombre,municipio,rubro,descripcion,procedencia,estado,negocio_id,coincidencias,actualizado FROM negocios_objetivo ORDER BY estado,nombre")?;
    let rows = stmt.query_map([], |r| {
        let coincidencias: String = r.get(8)?;
        Ok(json!({"clave":r.get::<_,String>(0)?,"nombre":r.get::<_,String>(1)?,"municipio":r.get::<_,Option<String>>(2)?,"rubro":r.get::<_,Option<String>>(3)?,"descripcion":r.get::<_,String>(4)?,"procedencia":r.get::<_,String>(5)?,"estado":r.get::<_,String>(6)?,"negocio_id":r.get::<_,Option<i64>>(7)?,"coincidencias":serde_json::from_str::<serde_json::Value>(&coincidencias).unwrap_or(json!([])),"actualizado":r.get::<_,String>(9)?}))
    })?;
    Ok(json!(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

pub fn resolver(conn: &Connection, key: &str, negocio_id: i64) -> Result<()> {
    let target: Option<(String, Option<String>, String)> = conn
        .query_row(
            "SELECT nombre,municipio,rubro FROM negocios_objetivo WHERE clave=?1",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((nombre_objetivo, municipio, rubro)) = target else {
        bail!("objetivo no encontrado: {key}")
    };
    let negocio: Option<(String, String)> = conn
        .query_row(
            "SELECT nombre,municipio FROM negocios WHERE id=?1 AND admisible=1",
            [negocio_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((nombre_negocio, municipio_negocio)) = negocio else {
        bail!("el negocio no existe o no es admisible en Navarra")
    };
    anyhow::ensure!(
        eligibility::normalize(&nombre_objetivo) == eligibility::normalize(&nombre_negocio),
        "el nombre del negocio no coincide con el objetivo; revisa el objetivo y vuelve a importar la lista"
    );
    if let Some(m) = municipio {
        anyhow::ensure!(
            eligibility::normalize(&m) == eligibility::normalize(&municipio_negocio),
            "el municipio del negocio no coincide con el municipio declarado para el objetivo"
        );
    }
    let detalle = json!([{"id":negocio_id,"nombre":nombre_negocio,"municipio":municipio_negocio}]);
    let changed = conn.execute("UPDATE negocios_objetivo SET estado='coincidencia_verificada',negocio_id=?1,coincidencias=?2,actualizado=?3 WHERE clave=?4", params![negocio_id,detalle.to_string(),store::ahora(),key])?;
    anyhow::ensure!(
        changed == 1,
        "no se pudo guardar la coincidencia del objetivo"
    );
    conn.execute(
        "UPDATE negocios SET rubro=COALESCE(rubro,?1) WHERE id=?2",
        params![rubro, negocio_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objetivos_sin_match_quedan_pendientes_y_no_entran_como_negocios() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"CREATE TABLE negocios_objetivo(clave TEXT PRIMARY KEY,nombre TEXT,municipio TEXT,rubro TEXT,descripcion TEXT,procedencia TEXT,estado TEXT,negocio_id INTEGER,coincidencias TEXT,actualizado TEXT); CREATE TABLE negocios(id INTEGER,nombre TEXT,municipio TEXT,admisible INTEGER,rubro TEXT);"#).unwrap();
        let path = std::env::temp_dir().join(format!("radar-targets-{}.csv", std::process::id()));
        std::fs::write(&path, "nombre,municipio,rubro,descripcion,procedencia\nNegocio Pendiente,,bodega,verificar,fuente\n").unwrap();
        let result = importar(&conn, &path).unwrap();
        assert_eq!(result["pendientes_revision"], 1);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM negocios", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn importacion_identifica_por_nombre_municipio_y_es_idempotente() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(r#"CREATE TABLE negocios_objetivo(clave TEXT PRIMARY KEY,nombre TEXT,municipio TEXT,rubro TEXT,descripcion TEXT,procedencia TEXT,estado TEXT,negocio_id INTEGER,coincidencias TEXT,actualizado TEXT); CREATE TABLE negocios(id INTEGER,nombre TEXT,municipio TEXT,admisible INTEGER,rubro TEXT); INSERT INTO negocios VALUES(7,'Bodega Ejemplo','Fitero',1,NULL);"#).unwrap();
        let path =
            std::env::temp_dir().join(format!("radar-targets-match-{}.csv", std::process::id()));
        std::fs::write(&path, "nombre,municipio,rubro,descripcion,procedencia\nBodega Ejemplo,Fitero,bodega,verificar,fuente\n").unwrap();
        assert_eq!(
            importar(&conn, &path).unwrap()["coincidencias_verificadas"],
            1
        );
        assert_eq!(
            importar(&conn, &path).unwrap()["coincidencias_verificadas"],
            1
        );
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM negocios_objetivo", [], |r| r.get(0))
            .unwrap();
        let rubro: String = conn
            .query_row("SELECT rubro FROM negocios WHERE id=7", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(rubro, "bodega");
        std::fs::remove_file(path).unwrap();
    }
}
