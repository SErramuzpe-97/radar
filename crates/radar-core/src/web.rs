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
CREATE TABLE IF NOT EXISTS informes_ia(id INTEGER PRIMARY KEY,negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,url TEXT NOT NULL,creado TEXT NOT NULL,modelo TEXT NOT NULL,texto TEXT NOT NULL,evidencias TEXT NOT NULL DEFAULT '{}');
CREATE INDEX IF NOT EXISTS idx_informes_ia_negocio ON informes_ia(negocio_id,creado DESC);
CREATE TABLE IF NOT EXISTS informes_ia_revisiones(id INTEGER PRIMARY KEY,informe_id INTEGER NOT NULL REFERENCES informes_ia(id) ON DELETE CASCADE,creado TEXT NOT NULL,texto TEXT NOT NULL,autor TEXT NOT NULL DEFAULT 'usuario');
CREATE INDEX IF NOT EXISTS idx_informes_ia_revisiones ON informes_ia_revisiones(informe_id,creado DESC,id DESC);
CREATE TABLE IF NOT EXISTS evaluaciones_ia(negocio_id INTEGER PRIMARY KEY REFERENCES negocios(id) ON DELETE CASCADE,fecha TEXT NOT NULL,modelo TEXT NOT NULL,respuesta TEXT NOT NULL,version TEXT NOT NULL DEFAULT '1');
CREATE TABLE IF NOT EXISTS ia_ejecuciones(id INTEGER PRIMARY KEY,negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,etapa TEXT NOT NULL,huella_entrada TEXT NOT NULL,modelo_solicitado TEXT NOT NULL,modelo_efectivo TEXT,prompt_version TEXT NOT NULL,esquema_version TEXT NOT NULL,politica_version TEXT NOT NULL,estado TEXT NOT NULL,creado TEXT NOT NULL,actualizado TEXT NOT NULL,respuesta TEXT NOT NULL DEFAULT '{}',evidencias TEXT NOT NULL DEFAULT '{}',uso TEXT NOT NULL DEFAULT '{}',error TEXT);
CREATE INDEX IF NOT EXISTS idx_ia_ejecuciones_negocio ON ia_ejecuciones(negocio_id,creado DESC);
CREATE TABLE IF NOT EXISTS ia_revisiones(negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,decision TEXT NOT NULL,estado TEXT NOT NULL,valor TEXT,motivo TEXT,actualizado TEXT NOT NULL,PRIMARY KEY(negocio_id,decision));
"#;
pub fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    let revision_columns: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(ia_revisiones)")?;
        let columns = stmt
            .query_map([], |r| r.get(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        columns
    };
    if !revision_columns.iter().any(|column| column == "valor") {
        conn.execute("ALTER TABLE ia_revisiones ADD COLUMN valor TEXT", [])?;
    }
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
    let revised_verifier: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=7)",
        [],
        |r| r.get(0),
    )?;
    if !revised_verifier {
        let tx = conn.unchecked_transaction()?;
        // Recheck rejected candidates carrying first-party source evidence:
        // OSM website tags belong to a specific mapped business, and official
        // registry records bind their URL to an exact registered business.
        // Also retry pages with a name and matching public contact detail.
        tx.execute(
            "UPDATE web_candidatas SET evidencias=json_remove(evidencias,'$.verificacion.revisada_en')
             WHERE estado='sugerida'
               AND json_extract(evidencias,'$.verificacion.revisada_en') IS NOT NULL
               AND (json_extract(evidencias,'$.verificacion.resultado')='sin_pruebas_suficientes'
                 OR json_extract(evidencias,'$.verificacion.resultado')='directorio')
               AND (proveedor='OSM'
                 OR json_extract(evidencias,'$.registro_oficial')=1
                 OR (json_extract(evidencias,'$.verificacion.nombre_coincide')=1
                   AND (json_extract(evidencias,'$.verificacion.telefono_coincide')=1
                     OR json_extract(evidencias,'$.verificacion.direccion_coincide')=1)))",
            [],
        )?;
        tx.execute(
            "INSERT INTO schema_migrations(version,applied_at) VALUES(7,?1)",
            [store::ahora()],
        )?;
        tx.commit()?;
    }
    let directory_cleanup: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version=8)",
        [],
        |r| r.get(0),
    )?;
    if !directory_cleanup {
        let mut stmt = conn.prepare(
            "SELECT negocio_id,url_confirmada FROM web_estado
             WHERE estado='verificada_automatica' AND origen='verificacion_automatica'
               AND url_confirmada IS NOT NULL",
        )?;
        let confirmed = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let tx = conn.unchecked_transaction()?;
        for (id, url) in confirmed {
            if es_directorio(&url) || crate::scoring::es_red_social(&url) {
                tx.execute("UPDATE negocios SET web=NULL WHERE id=?1", [id])?;
                tx.execute(
                    "UPDATE web_estado SET estado='pendiente',url_confirmada=NULL,origen=NULL,
                     confirmado_en=NULL,generacion=generacion+1,aviso='La URL era un directorio o mapa, no una web propia.'
                     WHERE negocio_id=?1 AND origen='verificacion_automatica'",
                    [id],
                )?;
                tx.execute(
                    "UPDATE web_candidatas SET estado='sugerida',tipo='directorio',
                     evidencias=json_set(evidencias,'$.verificacion.resultado','directorio','$.verificacion.directorio',1)
                     WHERE negocio_id=?1 AND url=?2",
                    params![id, url],
                )?;
            }
        }
        tx.execute(
            "INSERT INTO schema_migrations(version,applied_at) VALUES(8,?1)",
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
    crate::openrouter::configured()
}
pub fn remaining(conn: &Connection) -> Result<i64> {
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let used = conn
        .query_row(
            "SELECT creditos FROM web_search_daily_budget WHERE dia=?1",
            [day],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok((45 - used).max(0))
}

pub fn huella_evaluacion(conn: &Connection, id: i64) -> Result<String> {
    let n = store::get_negocio(conn, id)?.context("Negocio no encontrado")?;
    let estado = conn.query_row(
        "SELECT estado,url_confirmada,generacion FROM web_estado WHERE negocio_id=?1",
        [id],
        |r| Ok(json!({"estado":r.get::<_,String>(0)?,"url":r.get::<_,Option<String>>(1)?,"generacion":r.get::<_,i64>(2)?})),
    ).optional()?.unwrap_or(Value::Null);
    let mut stmt = conn.prepare(
        "SELECT url,estado,evidencias FROM web_candidatas WHERE negocio_id=?1 ORDER BY url",
    )?;
    let candidatas = stmt.query_map([id], |r| Ok(json!({"url":r.get::<_,String>(0)?,"estado":r.get::<_,String>(1)?,"evidencias":r.get::<_,String>(2)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let descartado: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM negocios_descartados WHERE negocio_id=?1)",
        [id],
        |r| r.get(0),
    )?;
    let admisible: bool =
        conn.query_row("SELECT admisible=1 FROM negocios WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
    let mut reviews_stmt=conn.prepare("SELECT decision,estado,valor,motivo FROM ia_revisiones WHERE negocio_id=?1 ORDER BY decision")?;
    let revisiones=reviews_stmt.query_map([id],|r|Ok(json!({"decision":r.get::<_,String>(0)?,"estado":r.get::<_,String>(1)?,"valor":r.get::<_,Option<String>>(2)?,"motivo":r.get::<_,Option<String>>(3)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let snapshot = json!({"id":n.id,"nombre":n.nombre,"municipio":n.municipio,"direccion":n.direccion,"telefono":n.telefono,"categoria":n.categoria,"sector":n.sector,"rubro":n.rubro,"admisible":admisible,"web":n.web,"estado_web":estado,"candidatas":candidatas,"descartado":descartado,"revisiones":revisiones});
    use sha2::{Digest, Sha256};
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&snapshot)?)))
}

pub fn huella_valor(value: &Value) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(value)?)))
}

pub fn registrar_ejecucion_ia(
    conn: &Connection,
    id: i64,
    etapa: &str,
    fingerprint: &str,
    modelo_solicitado: &str,
    modelo_efectivo: Option<&str>,
    estado: &str,
    respuesta: &Value,
    evidencias: &Value,
    uso: &Value,
    error: Option<&str>,
) -> Result<()> {
    conn.execute("INSERT INTO ia_ejecuciones(negocio_id,etapa,huella_entrada,modelo_solicitado,modelo_efectivo,prompt_version,esquema_version,politica_version,estado,creado,actualizado,respuesta,evidencias,uso,error) VALUES(?1,?2,?3,?4,?5,'1','1','1',?6,?7,?7,?8,?9,?10,?11)",params![id,etapa,fingerprint,modelo_solicitado,modelo_efectivo,estado,store::ahora(),respuesta.to_string(),evidencias.to_string(),uso.to_string(),error])?;
    Ok(())
}

pub fn reservar_credito_ia(conn: &Connection) -> Result<bool> {
    reserve_search_credit(conn)
}

fn aplicar_descarte_ia(
    conn: &Connection,
    id: i64,
    n: &Negocio,
    ai: &Value,
    refs: &[Value],
) -> Result<()> {
    let dominios: std::collections::HashSet<_> = refs
        .iter()
        .filter_map(|reference| reference["url"].as_str())
        .filter_map(|url| crate::public_http::normalize(url).ok())
        .filter_map(|url| url.host_str().map(str::to_ascii_lowercase))
        .collect();
    if dominios.len() < 2 {
        return Ok(());
    }
    let answers = &ai["answers"];
    let activity = answers["activity"]["choice"].as_str().unwrap_or("");
    let organization = answers["organization"]["choice"].as_str().unwrap_or("");
    let activity_reviewed: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM ia_revisiones WHERE negocio_id=?1 AND decision='actividad' AND estado IN ('corregida','rechazada'))",[id],|r|r.get(0))?;
    let organization_reviewed: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM ia_revisiones WHERE negocio_id=?1 AND decision='organizacion' AND estado IN ('corregida','rechazada'))",[id],|r|r.get(0))?;
    let strength =
        |answer: &Value, key: &str| -> f64 { answer["probabilities"][key].as_f64().unwrap_or(0.0) };
    let reason = if activity == "closed_permanently"
        && strength(&answers["activity"], activity) >= 0.99
        && !activity_reviewed
    {
        Some("cierre_permanente_verificado")
    } else if matches!(organization, "chain" | "franchise")
        && strength(&answers["organization"], organization) >= 0.99
        && !organization_reviewed
    {
        Some(if organization == "chain" {
            "cadena_verificada"
        } else {
            "franquicia_verificada"
        })
    } else {
        None
    };
    let Some(reason) = reason else { return Ok(()) };
    let evidence = json!({"modelo":ai["model"],"respuesta":answers,"fuentes":refs,"negocio":n.nombre,"municipio":n.municipio});
    // No pisa un descarte humano y conserva las fuentes para revisión/restauración.
    conn.execute("INSERT OR IGNORE INTO negocios_descartados(negocio_id,fecha,motivo) SELECT id,?2,?3 FROM negocios WHERE id=?1 AND admisible=1 AND NOT EXISTS(SELECT 1 FROM negocios_descartados WHERE negocio_id=?1) AND NOT EXISTS(SELECT 1 FROM ia_overrides WHERE negocio_id=?1)",params![id,store::ahora(),format!("ia:{reason}:{}",evidence)])?;
    Ok(())
}

pub fn reserve_search_credit(conn: &Connection) -> Result<bool> {
    let month = chrono::Utc::now().format("%Y-%m").to_string();
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO web_search_budget(periodo) VALUES(?1)",
        [&month],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO web_search_daily_budget(dia) VALUES(?1)",
        [&day],
    )?;
    let daily = tx.execute(
        "UPDATE web_search_daily_budget SET creditos=creditos+1 WHERE dia=?1 AND creditos<45",
        [&day],
    )?;
    if daily == 0 {
        tx.rollback()?;
        return Ok(false);
    }
    tx.execute(
        "UPDATE web_search_budget SET creditos=creditos+1 WHERE periodo=?1",
        [&month],
    )?;
    tx.commit()?;
    Ok(true)
}

pub fn next_automatic_discovery(conn: &Connection) -> Result<Option<(i64, bool)>> {
    let candidato_revisable = "(json_extract(c.evidencias,'$.verificacion.revisada_en') IS NULL OR (json_extract(c.evidencias,'$.verificacion.resultado')='no_disponible' AND datetime(json_extract(c.evidencias,'$.verificacion.revisada_en'))<datetime('now','-1 day')) OR (json_extract(c.evidencias,'$.verificacion.resultado') NOT IN ('no_disponible','bloqueada_por_robots') AND datetime(json_extract(c.evidencias,'$.verificacion.revisada_en'))<datetime('now','-90 days')))";
    let busqueda_disponible = "((COALESCE(w.ultima_busqueda,'')='' OR datetime(w.ultima_busqueda)<datetime('now','-90 days'))
               AND COALESCE((SELECT creditos FROM web_search_daily_budget WHERE dia=strftime('%Y-%m-%d','now')),0)<45)";
    let query = format!("SELECT n.id,
           {busqueda_disponible}
         FROM negocios n LEFT JOIN web_estado w ON w.negocio_id=n.id
         WHERE n.admisible=1 AND n.web IS NULL
           AND NOT EXISTS(SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=n.id)
           AND NOT EXISTS(SELECT 1 FROM web_estado a WHERE a.negocio_id=n.id AND a.estado='verificada_automatica' AND a.url_confirmada IS NOT NULL)
           AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.tipo='discovery' AND j.estado IN ('pendiente','en_curso') AND CAST(json_extract(j.params,'$.negocio_id') AS INTEGER)=n.id)
           AND (EXISTS(SELECT 1 FROM web_candidatas c WHERE c.negocio_id=n.id AND c.estado='sugerida' AND c.tipo NOT IN ('red_social','directorio') AND {candidato_revisable})
             OR {busqueda_disponible})
         ORDER BY CASE WHEN EXISTS(SELECT 1 FROM web_candidatas c WHERE c.negocio_id=n.id AND c.estado='sugerida' AND c.tipo NOT IN ('red_social','directorio') AND {candidato_revisable}) THEN 0 ELSE 1 END,
           CASE n.fuente WHEN 'overpass' THEN 0 WHEN 'industria' THEN 1 WHEN 'turismo' THEN 2 WHEN 'cooperativas' THEN 3 ELSE 4 END,
           COALESCE(w.ultima_busqueda,''),n.id LIMIT 1");
    let row = conn
        .query_row(&query, [], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?))
        })
        .optional()?;
    Ok(row)
}
pub fn info(conn: &Connection, id: i64) -> Result<WebInfo> {
    let n = store::get_negocio(conn, id)?.context("Negocio no encontrado")?;
    let state=conn.query_row("SELECT estado,url_confirmada,origen,ultima_busqueda,consulta,aviso FROM web_estado WHERE negocio_id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?))).optional()?;
    let (estado, url_confirmada, origen, ultima_busqueda, query, aviso) =
        state.unwrap_or_else(|| ("pendiente".into(), None, None, None, None, None));
    let mut evaluacion_ia: Option<Value> = conn.query_row("SELECT json_object('fecha',fecha,'modelo',modelo,'respuesta',json(respuesta)) FROM evaluaciones_ia WHERE negocio_id=?1",[id],|r|r.get::<_,String>(0)).optional()?.and_then(|s|serde_json::from_str(&s).ok());
    if let Some(evaluation) = evaluacion_ia.as_mut() {
        let execution = conn.query_row("SELECT json_object('estado',estado,'huella_entrada',huella_entrada,'creado',creado,'error',error) FROM ia_ejecuciones WHERE negocio_id=?1 AND etapa='evaluacion' ORDER BY id DESC LIMIT 1",[id],|r|r.get::<_,String>(0)).optional()?.and_then(|s|serde_json::from_str::<Value>(&s).ok());
        evaluation["ejecucion"] = execution.unwrap_or(Value::Null);
    }
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
    let mut reviews_stmt=conn.prepare("SELECT negocio_id,decision,estado,valor,motivo,actualizado FROM ia_revisiones WHERE negocio_id=?1 ORDER BY decision")?;
    let revisiones_ia = reviews_stmt
        .query_map([id], |r| {
            Ok(RevisionIA {
                negocio_id: r.get(0)?,
                decision: r.get(1)?,
                estado: r.get(2)?,
                valor: r.get(3)?,
                motivo: r.get(4)?,
                actualizado: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
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
        evaluacion_ia,
        revisiones_ia,
    })
}

pub fn actualizar_revision_ia(
    conn: &Connection,
    id: i64,
    req: &ActualizarRevisionIA,
) -> Result<RevisionIA> {
    anyhow::ensure!(
        matches!(
            req.decision.as_str(),
            "categoria" | "actividad" | "organizacion" | "web"
        ),
        "Decisión IA no válida"
    );
    anyhow::ensure!(
        matches!(req.estado.as_str(), "revisada" | "corregida" | "rechazada"),
        "Estado de revisión no válido"
    );
    anyhow::ensure!(
        req.valor.as_ref().is_none_or(|value| value.len() <= 300),
        "Valor de revisión demasiado largo"
    );
    anyhow::ensure!(
        req.motivo
            .as_ref()
            .is_none_or(|reason| reason.len() <= 1000),
        "Motivo de revisión demasiado largo"
    );
    if req.decision == "categoria" && req.estado == "corregida" {
        let category = req
            .valor
            .as_deref()
            .context("Indica la categoría corregida")?;
        anyhow::ensure!(
            crate::rubros::FILTROS
                .iter()
                .any(|(key, _)| *key == category),
            "Categoría comercial no válida"
        );
    }
    let tx = conn.unchecked_transaction()?;
    if req.decision == "categoria" && req.estado == "corregida" {
        let category = req.valor.as_deref().unwrap();
        tx.execute("UPDATE negocios SET rubro=?2,evidencia_rubro=json_set(evidencia_rubro,'$.revision_humana',json(?3)) WHERE id=?1",params![id,category,json!({"valor":category,"motivo":req.motivo,"fecha":store::ahora()}).to_string()])?;
    }
    tx.execute("INSERT INTO ia_revisiones(negocio_id,decision,estado,valor,motivo,actualizado) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(negocio_id,decision) DO UPDATE SET estado=excluded.estado,valor=excluded.valor,motivo=excluded.motivo,actualizado=excluded.actualizado",params![id,req.decision,req.estado,req.valor,req.motivo,store::ahora()])?;
    tx.commit()?;
    conn.query_row("SELECT negocio_id,decision,estado,valor,motivo,actualizado FROM ia_revisiones WHERE negocio_id=?1 AND decision=?2",params![id,req.decision],|r|Ok(RevisionIA{negocio_id:r.get(0)?,decision:r.get(1)?,estado:r.get(2)?,valor:r.get(3)?,motivo:r.get(4)?,actualizado:r.get(5)?})).map_err(Into::into)
}

pub fn ultimo_informe_ia(conn: &Connection, id: i64) -> Result<Option<InformeIA>> {
    conn.query_row("SELECT id,negocio_id,url,creado,modelo,COALESCE((SELECT texto FROM informes_ia_revisiones WHERE informe_id=informes_ia.id ORDER BY id DESC LIMIT 1),texto),evidencias FROM informes_ia WHERE negocio_id=?1 ORDER BY id DESC LIMIT 1",[id],|r|Ok(InformeIA{id:r.get(0)?,negocio_id:r.get(1)?,url:r.get(2)?,creado:r.get(3)?,modelo:r.get(4)?,texto:r.get(5)?,evidencias:serde_json::from_str(&r.get::<_,String>(6)?).unwrap_or(json!({}))})).optional().map_err(Into::into)
}

pub fn guardar_revision_informe_ia(conn: &Connection, id: i64, texto: &str) -> Result<InformeIA> {
    let text = texto.trim();
    anyhow::ensure!(
        !text.is_empty() && text.len() <= 16_000,
        "El texto revisado está vacío o excede el límite"
    );
    let report_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM informes_ia WHERE negocio_id=?1 ORDER BY id DESC LIMIT 1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    let report_id = report_id.context("No hay un informe IA para revisar")?;
    conn.execute(
        "INSERT INTO informes_ia_revisiones(informe_id,creado,texto) VALUES(?1,?2,?3)",
        params![report_id, store::ahora(), text],
    )?;
    ultimo_informe_ia(conn, id)?.context("No se pudo recuperar el informe revisado")
}

pub fn guardar_informe_ia(
    conn: &Connection,
    id: i64,
    url: &str,
    modelo: &str,
    texto: &str,
    evidencias: &Value,
) -> Result<InformeIA> {
    conn.execute("INSERT INTO informes_ia(negocio_id,url,creado,modelo,texto,evidencias) VALUES(?1,?2,?3,?4,?5,?6)",params![id,url,store::ahora(),modelo,texto,evidencias.to_string()])?;
    ultimo_informe_ia(conn, id)?.context("No se pudo recuperar el informe recién guardado")
}

pub fn evidencia_auditoria_para_ia(conn: &Connection, id: i64, url: &str) -> Result<Value> {
    let mut stmt=conn.prepare("SELECT motor,dispositivo,version,fecha,estado,resultado FROM auditorias_tecnicas WHERE negocio_id=?1 AND url=?2 AND estado IN ('terminado','parcial') ORDER BY fecha DESC LIMIT 8")?;
    let rows = stmt.query_map(params![id, url], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
        ))
    })?;
    let mut informes = Vec::new();
    for row in rows {
        let (motor, dispositivo, version, fecha, estado, raw) = row?;
        let data: Value = serde_json::from_str(&raw).unwrap_or(json!({}));
        let scores = data.get("scores").cloned().unwrap_or(json!({}));
        let issues=data.get("issues").and_then(Value::as_array).map(|items|items.iter().take(15).map(|item|json!({"code":item["code"],"title":item["title"],"severity":item["severity"],"category":item["category"],"url":item["url"],"details":item["details"],"recommendation":item["recommendation"]})).collect::<Vec<_>>()).unwrap_or_default();
        let coverage = data.get("coverage").cloned().unwrap_or(json!({}));
        let warnings = data.get("warnings").cloned().unwrap_or(json!([]));
        informes.push(json!({"motor":motor,"dispositivo":dispositivo,"version":version,"fecha":fecha,"estado":estado,"scores":scores,"hallazgos":issues,"cobertura":coverage,"avisos":warnings}));
    }
    Ok(json!({"url":url,"informes":informes,"limite_informes":8}))
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

pub fn verified_automatically(
    conn: &Connection,
    id: i64,
    raw_url: &str,
    evidence: &Value,
) -> Result<()> {
    let url = public_http::normalize(raw_url)?.to_string();
    anyhow::ensure!(
        !crate::scoring::es_red_social(&url),
        "Una red social no se puede verificar como web propia"
    );
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        "UPDATE negocios SET web=?2 WHERE id=?1 AND admisible=1",
        params![id, url],
    )?;
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

fn save_candidate_verification(
    conn: &Connection,
    candidate: &WebCandidata,
    mut verification: Value,
) -> Result<()> {
    let mut evidence = candidate.evidencias.clone();
    if let Some(object) = evidence.as_object_mut() {
        object.insert("verificacion".into(), verification.take());
    } else {
        evidence = json!({"origen":candidate.proveedor,"verificacion":verification});
    }
    conn.execute(
        "UPDATE web_candidatas SET evidencias=?2 WHERE id=?1",
        params![candidate.id, evidence.to_string()],
    )?;
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
    {
        let c = store.lock().unwrap();
        let discarded: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM negocios_descartados WHERE negocio_id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if discarded {
            return Ok(json!({"estado":"descartado"}));
        }
    }
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
    if allow_search && configured() {
        let input_fingerprint = {
            let c = store.lock().unwrap();
            huella_evaluacion(&c, id)?
        };
        let cache_key = format!("openrouter:{query}:{input_fingerprint}");
        let cached = {
            let c = store.lock().unwrap();
            c.query_row("SELECT resultados FROM web_search_cache WHERE consulta=?1 AND datetime(creado)>datetime('now','-30 days')",[&cache_key],|r|r.get::<_,String>(0)).optional()?
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
                notice = Some(
                    "Cuota gratuita diaria de OpenRouter agotada; búsqueda pausada hasta mañana."
                        .into(),
                );
                json!([])
            } else {
                match crate::openrouter::discover(
                    &n.nombre,
                    &n.municipio,
                    address.as_deref(),
                    n.telefono.as_deref(),
                    crate::rubros::FILTROS,
                )
                .await
                {
                    Ok(ai) => {
                        let answers = &ai["answers"];
                        let annotations = crate::openrouter::citations(&ai);
                        let refs = annotations
                            .iter()
                            .map(|(url, title)| json!({"url":url,"title":title}))
                            .collect::<Vec<_>>();
                        let category = answers["category"]["choice"].as_str().unwrap_or("");
                        let c = store.lock().unwrap();
                        let current_fingerprint = huella_evaluacion(&c, id)?;
                        let stale = current_fingerprint != input_fingerprint;
                        let run_state = if stale { "obsoleta" } else { "aplicada" };
                        registrar_ejecucion_ia(
                            &c,
                            id,
                            "evaluacion",
                            &input_fingerprint,
                            crate::openrouter::DECISIONS_MODEL,
                            ai["model"].as_str(),
                            run_state,
                            &ai,
                            &json!({"fuentes":refs}),
                            &ai["usage"],
                            None,
                        )?;
                        if stale {
                            c.execute("INSERT INTO evaluaciones_ia(negocio_id,fecha,modelo,respuesta,version) VALUES(?1,?2,?3,?4,'1') ON CONFLICT(negocio_id) DO UPDATE SET fecha=excluded.fecha,modelo=excluded.modelo,respuesta=excluded.respuesta,version=excluded.version",params![id,store::ahora(),ai["model"].as_str().unwrap_or(crate::openrouter::DECISIONS_MODEL),ai.to_string()])?;
                            notice = Some("La ficha cambió mientras se evaluaba; el resultado se guardó como histórico y no se aplicó. Vuelve a buscar para evaluar los datos actuales.".into());
                            json!({"resultados":[],"analisis":{},"obsoleta":true})
                        } else {
                            searched = true;
                            c.execute("INSERT INTO evaluaciones_ia(negocio_id,fecha,modelo,respuesta,version) VALUES(?1,?2,?3,?4,'1') ON CONFLICT(negocio_id) DO UPDATE SET fecha=excluded.fecha,modelo=excluded.modelo,respuesta=excluded.respuesta,version=excluded.version",params![id,store::ahora(),ai["model"].as_str().unwrap_or(crate::openrouter::DECISIONS_MODEL),ai.to_string()])?;
                            if crate::rubros::FILTROS.iter().any(|(id, _)| *id == category) {
                                let existing = n.rubro.as_deref().unwrap_or("");
                                let reviewed: bool=c.query_row("SELECT EXISTS(SELECT 1 FROM ia_revisiones WHERE negocio_id=?1 AND decision='categoria')",[id],|r|r.get(0))?;
                                if (existing.is_empty() || existing == "otro") && !reviewed {
                                    c.execute("UPDATE negocios SET rubro=?2,evidencia_rubro=json_set(evidencia_rubro,'$.openrouter_categoria',json(?3)) WHERE id=?1",params![id,category,json!({"modelo":ai["model"],"respuesta":answers["category"],"fuentes":refs}).to_string()])?;
                                }
                            }
                            aplicar_descarte_ia(&c, id, &n, &ai, &refs)?;
                            let result = json!({"resultados":refs,"analisis":{"modelo":ai["model"],"provider":ai["provider"],"id":ai["id"],"answers":answers,"usage":ai["usage"]}});
                            c.execute("INSERT INTO web_search_cache(consulta,resultados,creado) VALUES(?1,?2,?3) ON CONFLICT(consulta) DO UPDATE SET resultados=excluded.resultados,creado=excluded.creado",params![cache_key,result.to_string(),store::ahora()])?;
                            result
                        }
                    }
                    Err(err) => {
                        let c = store.lock().unwrap();
                        registrar_ejecucion_ia(
                            &c,
                            id,
                            "evaluacion",
                            &input_fingerprint,
                            crate::openrouter::DECISIONS_MODEL,
                            None,
                            "error",
                            &json!({}),
                            &json!({}),
                            &json!({}),
                            Some(&err.to_string()),
                        )?;
                        notice = Some(format!(
                            "OpenRouter no pudo completar la búsqueda gratuita: {err}"
                        ));
                        json!({"resultados":[],"analisis":{}})
                    }
                }
            }
        };
        if let Some(rows) = results["resultados"].as_array() {
            let c = store.lock().unwrap();
            for (position, r) in rows.iter().enumerate().take(5) {
                if let Some(url) = r["url"]
                    .as_str()
                    .filter(|u| public_http::normalize(u).is_ok())
                {
                    add_candidate(
                        &c,
                        id,
                        url,
                        r["title"].as_str(),
                        "OpenRouter Decisions Fast",
                        Some(&query),
                        Some(position as i64 + 1),
                        &json!({"tipo_resultado":"web_grounded_citation","identidad_confirmada":false,"analisis":results["analisis"],"fuente":{"url":url,"title":r["title"]}}),
                    )?;
                }
            }
        }
    } else if allow_search {
        notice=Some("Búsqueda gratuita con OpenRouter sin configurar. Puedes consultar fuentes públicas o introducir una URL.".into());
    }
    // Contrastar candidatos públicos, sin convertir su posición de búsqueda en identidad.
    let candidates = {
        let c = store.lock().unwrap();
        info(&c, id)?.candidatas
    };
    for candidate in candidates.iter().take(3) {
        {
            let c = store.lock().unwrap();
            save_candidate_verification(
                &c,
                candidate,
                json!({"revisada_en":store::ahora(),"resultado":"no_disponible"}),
            )?;
        }
        if candidate.tipo == "red_social" {
            continue;
        }
        match public_http::robots_allowed(&candidate.url).await {
            Ok(true) => {}
            Ok(false) => {
                let c = store.lock().unwrap();
                save_candidate_verification(
                    &c,
                    candidate,
                    json!({"revisada_en":store::ahora(),"resultado":"bloqueada_por_robots"}),
                )?;
                continue;
            }
            Err(_) => {
                let c = store.lock().unwrap();
                save_candidate_verification(
                    &c,
                    candidate,
                    json!({"revisada_en":store::ahora(),"resultado":"no_disponible"}),
                )?;
                continue;
            }
        }
        if let Ok(p) = public_http::fetch(&candidate.url, 12).await {
            if (200..300).contains(&p.status) {
                let html = scraper::Html::parse_document(&p.body);
                let page_text = html.root_element().text().collect::<String>();
                let name = eligibility::normalize(&page_text);
                let phone = n
                    .telefono
                    .as_deref()
                    .map(|s| s.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
                    .filter(|s| s.len() >= 8);
                let body_digits = page_text
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
                let is_directory = candidate.tipo == "directorio"
                    || crate::scoring::es_red_social(&p.url)
                    || es_directorio(&p.url)
                    || p.body.contains("\"@type\":\"ItemList\"")
                    || p.body.contains("\"@type\": \"ItemList\"");
                let c = store.lock().unwrap();
                // Nombre + teléfono/dirección visibles ya son dos señales
                // independientes. El marcado estructurado aporta una
                // alternativa fiable para webs que no publican contacto.
                let metodo_fuente = fuente_confirma_vinculo(
                    &candidate.proveedor,
                    &candidate.evidencias,
                    candidate.titulo.as_deref(),
                    &n.nombre,
                );
                let strong = !is_directory
                    && (identidad_fuerte(
                        has_name,
                        has_phone,
                        has_address,
                        structured,
                        is_directory,
                    ) || metodo_fuente.is_some());
                let verification = json!({"revisada_en":store::ahora(),"resultado":if strong{"verificada_automatica"}else if is_directory{"directorio"}else{"sin_pruebas_suficientes"},"nombre_coincide":has_name,"telefono_coincide":has_phone,"direccion_coincide":has_address,"datos_estructurados":structured,"directorio":is_directory,"metodo_fuente":metodo_fuente,"url_final":p.url});
                let mut evidence = candidate.evidencias.clone();
                evidence["verificacion"] = verification.clone();
                c.execute("UPDATE web_candidatas SET evidencias=?2,tipo=CASE WHEN ?3 THEN 'directorio' ELSE tipo END WHERE id=?1",params![candidate.id,evidence.to_string(),is_directory])?;
                if strong {
                    verified_automatically(&c, id, &candidate.url, &evidence)?;
                    return Ok(
                        json!({"estado":"verificada_automatica","negocio_id":id,"url":candidate.url,"evidencias":verification}),
                    );
                }
            }
        }
    }
    {
        let c = store.lock().unwrap();
        let discarded: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM negocios_descartados WHERE negocio_id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if discarded {
            return Ok(
                json!({"estado":"descartado_ia","evaluacion":c.query_row("SELECT json(respuesta) FROM evaluaciones_ia WHERE negocio_id=?1",[id],|r|r.get::<_,String>(0)).optional()?.and_then(|s|serde_json::from_str::<Value>(&s).ok())}),
            );
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

fn es_directorio(url: &str) -> bool {
    let Some(domain) = eligibility::domain(url) else {
        return true;
    };
    [
        "booking.com",
        "mapcarta.com",
        "foursquare.com",
        "geonames.org",
        "wikidata.org",
        "wikipedia.org",
        "wikivoyage.org",
        "here.com",
        "komoot.com",
        "mapy.com",
        "mapy.cz",
        "expedia.com",
        "tripadvisor.com",
        "yelp.com",
        "paginasamarillas.es",
        "qdq.com",
        "cylex.es",
        "infobel.com",
        "hotels.com",
    ]
    .iter()
    .any(|known| domain == *known || domain.ends_with(&format!(".{known}")))
}

fn identidad_fuerte(
    nombre: bool,
    telefono: bool,
    direccion: bool,
    estructurada: bool,
    directorio: bool,
) -> bool {
    nombre && !directorio && (telefono || direccion || estructurada)
}

fn fuente_confirma_vinculo(
    proveedor: &str,
    evidencias: &Value,
    titulo: Option<&str>,
    nombre_negocio: &str,
) -> Option<&'static str> {
    let fuente = evidencias.get("fuente")?.as_str()?;
    let fuente_url = reqwest::Url::parse(fuente).ok()?;
    let dominio = fuente_url.host_str()?.to_ascii_lowercase();
    let path = fuente_url.path();
    if proveedor.eq_ignore_ascii_case("OSM")
        && (dominio == "openstreetmap.org" || dominio == "www.openstreetmap.org")
        && ["/node/", "/way/", "/relation/"].iter().any(|prefix| {
            path.strip_prefix(prefix)
                .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        })
    {
        return Some("openstreetmap");
    }
    if evidencias.get("registro_oficial").and_then(Value::as_bool) == Some(true)
        && evidencias
            .get("registro")
            .and_then(Value::as_str)
            .is_some_and(|v| !v.trim().is_empty())
        && titulo.is_some_and(|t| {
            let title = eligibility::normalize(t);
            let name = eligibility::normalize(nombre_negocio);
            name.len() > 3 && title == name
        })
        && (dominio == "navarra.es"
            || dominio.ends_with(".navarra.es")
            || dominio == "datosabiertos.navarra.es")
    {
        return Some("registro_oficial_navarra");
    }
    None
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
    #[test]
    fn verificacion_admite_dos_senales_sin_exigir_json_ld() {
        assert!(identidad_fuerte(true, true, false, false, false));
        assert!(identidad_fuerte(true, false, true, false, false));
        assert!(identidad_fuerte(true, false, false, true, false));
        assert!(!identidad_fuerte(true, false, false, false, false));
        assert!(!identidad_fuerte(true, true, false, true, true));
    }
    #[test]
    fn se_excluyen_plataformas_de_reservas_y_directorios() {
        assert!(es_directorio("https://booking.com/hotel/example"));
        assert!(es_directorio("https://mapcarta.com/N510997125"));
        assert!(es_directorio(
            "https://www.paginasamarillas.es/empresa/example"
        ));
        assert!(!es_directorio("https://hotel-example.es/"));
    }
    #[test]
    fn procedencia_verifica_web_osm_y_registro_oficial_con_nombre_exacto() {
        assert_eq!(
            fuente_confirma_vinculo(
                "OSM",
                &json!({"fuente":"https://www.openstreetmap.org/way/12345"}),
                None,
                "Negocio"
            ),
            Some("openstreetmap")
        );
        let oficial = json!({"fuente":"https://datosabiertos.navarra.es/dataset/registro","registro":"123","registro_oficial":true});
        assert_eq!(
            fuente_confirma_vinculo(
                "Industria",
                &oficial,
                Some("Taller El Roble"),
                "Taller El Roble"
            ),
            Some("registro_oficial_navarra")
        );
        assert_eq!(
            fuente_confirma_vinculo(
                "Industria",
                &oficial,
                Some("Taller El Roble Norte"),
                "Taller El Roble"
            ),
            None
        );
        assert_eq!(
            fuente_confirma_vinculo(
                "OSM",
                &json!({"fuente":"https://openstreetmap.org/user/123"}),
                None,
                "Negocio"
            ),
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
            "OpenRouter Decisions Fast",
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
    #[test]
    fn automatizacion_usa_openrouter_aunque_ya_haya_candidata_osm() {
        let c = db();
        add_candidate(
            &c,
            1,
            "https://web-osm.example/",
            None,
            "OSM",
            None,
            None,
            &json!({"fuente":"https://www.openstreetmap.org/node/123"}),
        )
        .unwrap();
        assert_eq!(next_automatic_discovery(&c).unwrap(), Some((1, true)));

        let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
        c.execute(
            "INSERT INTO web_search_daily_budget(dia,creditos) VALUES(?1,44)",
            [&day],
        )
        .unwrap();
        assert_eq!(next_automatic_discovery(&c).unwrap(), Some((1, true)));
        c.execute(
            "UPDATE web_search_daily_budget SET creditos=45 WHERE dia=?1",
            [&day],
        )
        .unwrap();
        assert_eq!(next_automatic_discovery(&c).unwrap(), Some((1, false)));
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
