//! Cola e informes de motores técnicos; los procesos de navegador no acceden a SQLite.
use crate::{model::*, public_http, store};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn directory() -> PathBuf {
    std::env::var_os("RADAR_AUDIT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/radar-audits"))
}
pub fn worker_available() -> bool {
    std::fs::read_to_string(directory().join("heartbeat.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .is_some_and(|v| {
            v["ready"] == true
                && chrono::Utc::now().timestamp() - v["timestamp"].as_i64().unwrap_or(0) < 30
        })
}

// La cola solo puede auditar la web propia que sigue confirmada para el negocio.
// Una importación o corrección puede invalidar trabajos antiguos sin borrar su historial.
const TRABAJO_VIGENTE: &str = "EXISTS(
    SELECT 1 FROM negocios n JOIN web_estado w ON w.negocio_id=n.id
    WHERE n.id=CAST(json_extract(j.params,'$.negocio_id') AS INTEGER) AND n.admisible=1
      AND n.web=json_extract(j.params,'$.url') AND w.url_confirmada=n.web
      AND ((w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario'))
        OR (w.estado='verificada_automatica' AND w.origen='verificacion_automatica'))
)";

pub fn depurar_cola(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        &format!("UPDATE jobs AS j SET estado='error',actualizado=?1,
            error='Trabajo obsoleto: la URL cambió o ya no está confirmada. Confirma la web actual y programa una nueva auditoría.'
            WHERE j.tipo='technical' AND j.estado='pendiente' AND NOT {TRABAJO_VIGENTE}"),
        [store::ahora()],
    )?)
}

pub fn siguiente(conn: &Connection) -> Result<Option<Job>> {
    let id: Option<String> = conn
        .query_row(
            &format!(
                "SELECT j.id FROM jobs j WHERE j.tipo='technical' AND j.estado='pendiente'
            AND {TRABAJO_VIGENTE} ORDER BY j.creado,j.id LIMIT 1"
            ),
            [],
            |r| r.get(0),
        )
        .optional()?;
    id.map(|id| store::get_job(conn, &id))
        .transpose()
        .map(Option::flatten)
}

pub fn verificar_web_actual(conn: &Connection, job: &Job) -> Result<()> {
    let vigente: bool = conn
        .query_row(
            &format!("SELECT {TRABAJO_VIGENTE} FROM jobs j WHERE j.id=?1"),
            [&job.id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(false);
    anyhow::ensure!(
        vigente,
        "La web cambió o ya no está confirmada; genera una auditoría para la web actual"
    );
    Ok(())
}
pub fn new_job(tipo: TipoJob, params: Value) -> Job {
    let now = store::ahora();
    Job {
        id: uuid::Uuid::new_v4().to_string(),
        tipo,
        estado: EstadoJob::Pendiente,
        params,
        hechos: 0,
        total: 0,
        creado: now.clone(),
        actualizado: now,
        error: None,
        resumen: None,
    }
}
pub fn enqueue(conn: &Connection, id: i64) -> Result<Job> {
    store::get_negocio(conn, id)?.context("Negocio no encontrado")?;
    let url = conn.query_row("SELECT url_confirmada FROM web_estado WHERE negocio_id=?1 AND ((estado='confirmada' AND origen IN ('correccion_usuario','confirmacion_usuario')) OR (estado='verificada_automatica' AND origen='verificacion_automatica'))",[id],|r|r.get::<_,Option<String>>(0)).optional()?.flatten().context("Confirma primero una web propia")?;
    anyhow::ensure!(
        !crate::scoring::es_red_social(&url),
        "Es necesario confirmar una web propia"
    );
    let url = public_http::normalize(&url)?.to_string();
    let existing:Option<String>=conn.query_row("SELECT id FROM jobs WHERE tipo='technical' AND estado IN ('pendiente','en_curso') AND json_extract(params,'$.negocio_id')=?1 AND json_extract(params,'$.url')=?2 ORDER BY creado DESC LIMIT 1",params![id,url],|r|r.get(0)).optional()?;
    if let Some(job) = existing {
        return store::get_job(conn, &job)?.context("Trabajo no encontrado");
    }
    let generation = conn
        .query_row(
            "SELECT generacion FROM web_estado WHERE negocio_id=?1",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(1);
    let mut job = new_job(
        TipoJob::Technical,
        json!({"negocio_id":id,"url":url,"generacion":generation}),
    );
    job.total = 4;
    store::insert_job(conn, &job)?;
    Ok(job)
}

/// Programa una auditoría para la web confirmada más antigua que haya vencido.
/// El worker es serial, por lo que cada llamada añade como máximo un sitio.
pub fn enqueue_due(conn: &Connection, before: &str) -> Result<Option<Job>> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT n.id FROM negocios n JOIN web_estado w ON w.negocio_id=n.id
             WHERE n.admisible=1 AND n.web IS NOT NULL
               AND ((w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario'))
                 OR (w.estado='verificada_automatica' AND w.origen='verificacion_automatica'))
               AND w.url_confirmada=n.web
               AND NOT EXISTS (SELECT 1 FROM jobs j WHERE j.tipo='technical'
                 AND j.estado IN ('pendiente','en_curso')
                 AND json_extract(j.params,'$.negocio_id')=n.id
                 AND json_extract(j.params,'$.url')=n.web)
               AND COALESCE((SELECT MAX(strftime('%s',t.fecha)) FROM auditorias_tecnicas t
                 WHERE t.negocio_id=n.id AND t.url=n.web), 0) < strftime('%s',?1)
             ORDER BY COALESCE((SELECT MAX(strftime('%s',t.fecha)) FROM auditorias_tecnicas t
                 WHERE t.negocio_id=n.id AND t.url=n.web), 0), n.id LIMIT 1",
            [before],
            |r| r.get(0),
        )
        .optional()?;
    id.map(|id| enqueue(conn, id)).transpose()
}
pub fn discovery_job(conn: &Connection, params: &BusquedaWebParams) -> Result<Job> {
    let job = new_job(TipoJob::Discovery, serde_json::to_value(params)?);
    store::insert_job(conn, &job)?;
    Ok(job)
}
pub fn automatic_discovery_job(conn: &Connection, id: i64, buscar: bool) -> Result<Job> {
    let job = new_job(TipoJob::Discovery, json!({"negocio_id":id,"pendientes":false,"buscar":buscar,"automatico":true}));
    store::insert_job(conn, &job)?;
    Ok(job)
}
pub fn reports(conn: &Connection, id: i64) -> Result<Vec<AuditoriaTecnica>> {
    let mut stmt=conn.prepare("SELECT id,job_id,negocio_id,url,motor,dispositivo,version,estado,fecha,resultado,archivos,error FROM auditorias_tecnicas WHERE negocio_id=?1 ORDER BY fecha DESC,rowid DESC LIMIT 40")?;
    let rows = stmt.query_map([id], |r| {
        let files: Value = serde_json::from_str(&r.get::<_, String>(10)?).unwrap_or(json!({}));
        Ok(AuditoriaTecnica {
            id: r.get(0)?,
            job_id: r.get(1)?,
            negocio_id: r.get(2)?,
            url: r.get(3)?,
            motor: r.get(4)?,
            dispositivo: r.get(5)?,
            version: r.get(6)?,
            estado: r.get(7)?,
            fecha: r.get(8)?,
            resultado: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or(json!({})),
            archivos: files
                .as_object()
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default(),
            error: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
pub fn state(conn: &Connection, id: i64) -> Result<EstadoAuditoriaTecnica> {
    let jobid:Option<String>=conn.query_row("SELECT id FROM jobs WHERE tipo='technical' AND json_extract(params,'$.negocio_id')=?1 ORDER BY creado DESC LIMIT 1",[id],|r|r.get(0)).optional()?;
    let job = jobid
        .map(|id| store::get_job(conn, &id))
        .transpose()?
        .flatten();
    Ok(EstadoAuditoriaTecnica {
        informes: reports(conn, id)?,
        job,
        worker_disponible: worker_available(),
    })
}
pub fn finding_reviews(conn: &Connection, id: i64) -> Result<Vec<RevisionHallazgo>> {
    let mut stmt = conn.prepare("SELECT report_id,finding_key,estado,motivo,actualizado FROM auditoria_hallazgo_revision WHERE negocio_id=?1 ORDER BY actualizado DESC")?;
    let rows = stmt.query_map([id], |r| {
        Ok(RevisionHallazgo {
            report_id: r.get(0)?,
            finding_key: r.get(1)?,
            estado: r.get(2)?,
            motivo: r.get(3)?,
            actualizado: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn capture_catalog(conn: &Connection, id: i64) -> Result<Vec<Value>> {
    let current_url = store::get_negocio(conn, id)?.and_then(|n| n.web);
    let mut out = Vec::new();
    for report in reports(conn, id)? {
        if current_url.as_deref().is_some_and(|url| url != report.url) {
            continue;
        }
        let shots = report.resultado["responsive"]["screenshots"]
            .as_array()
            .into_iter()
            .flatten();
        for shot in shots {
            let Some(key) = shot["key"].as_str() else { continue };
            if !report.archivos.iter().any(|file| file == key) {
                continue;
            }
            let audit_dir = std::env::var_os("RADAR_AUDIT_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/var/lib/radar-audits"));
            if !audit_dir
                .join("artifacts")
                .join(format!("{}.{}.gz", report.id, key))
                .is_file()
            {
                continue;
            }
            out.push(json!({
                "capture_id":format!("{}::{key}",report.id),
                "report_id":report.id,
                "key":key,
                "width":shot["width"],
                "page":shot["page"],
                "fecha":report.fecha,
            }));
        }
    }
    out.sort_by(|a,b| b["fecha"].as_str().cmp(&a["fecha"].as_str()));
    out.truncate(100);
    Ok(out)
}

pub fn visit_state(conn: &Connection, id: i64) -> Result<Value> {
    let state = conn
        .query_row(
            "SELECT estado,actualizado FROM diagnostico_visitas WHERE negocio_id=?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?;
    if let Some((raw, updated)) = state {
        let mut value: Value = serde_json::from_str(&raw).unwrap_or(json!({"capturas":[]}));
        value["actualizado"] = json!(updated);
        value["catalogo"] = json!(capture_catalog(conn, id)?);
        Ok(value)
    } else {
        Ok(json!({"capturas":[],"actualizado":null,"catalogo":capture_catalog(conn,id)?}))
    }
}

pub fn update_visit_state(conn: &Connection, id: i64, incoming: &Value) -> Result<Value> {
    anyhow::ensure!(incoming["capturas"].is_array(), "La lista de capturas no es válida");
    let captures = capture_catalog(conn, id)?;
    let available: std::collections::HashSet<&str> = captures
        .iter()
        .filter_map(|capture| capture["capture_id"].as_str())
        .collect();
    let requested = incoming["capturas"].as_array().unwrap();
    anyhow::ensure!(requested.len() <= 100, "Demasiadas capturas en la visita");
    let mut seen = std::collections::HashSet::new();
    let mut clean = Vec::new();
    let mut kept = 0;
    for capture in requested {
        let capture_id = capture["capture_id"].as_str().context("Falta identificar una captura")?;
        anyhow::ensure!(available.contains(capture_id), "La captura no pertenece a este negocio o auditoría");
        anyhow::ensure!(seen.insert(capture_id), "La visita contiene capturas repetidas");
        let status = capture["estado"].as_str().unwrap_or("pendiente");
        anyhow::ensure!(["pendiente", "conservar", "descartada"].contains(&status), "Estado de captura no válido");
        if status == "conservar" {
            kept += 1;
            anyhow::ensure!(kept <= 7, "Una visita puede conservar hasta siete capturas");
        }
        let bounded = |field: &str, max: usize| -> Result<Value> {
            let value = capture[field].as_str().unwrap_or("").trim();
            anyhow::ensure!(value.len() <= max, "El texto de la captura supera el límite permitido");
            Ok(json!(value))
        };
        let bounded_percent = |field: &str| -> Result<Value> {
            let value = capture[field].as_f64();
            anyhow::ensure!(value.is_none_or(|v| (0.0..=100.0).contains(&v)), "La anotación de captura no es válida");
            Ok(value.map_or(Value::Null, |v| json!(v)))
        };
        clean.push(json!({
            "capture_id":capture_id,
            "estado":status,
            "orden":capture["orden"].as_i64().unwrap_or(0).clamp(0,1000),
            "titulo":bounded("titulo",160)?,
            "que":bounded("que",800)?,
            "impacto":bounded("impacto",800)?,
            "propuesta":bounded("propuesta",800)?,
            "punto_x":bounded_percent("punto_x")?,
            "punto_y":bounded_percent("punto_y")?,
        }));
    }
    let value = json!({"capturas":clean});
    let updated = store::ahora();
    conn.execute(
        "INSERT INTO diagnostico_visitas(negocio_id,estado,actualizado) VALUES(?1,?2,?3)
         ON CONFLICT(negocio_id) DO UPDATE SET estado=excluded.estado,actualizado=excluded.actualizado",
        rusqlite::params![id,value.to_string(),updated],
    )?;
    let mut result = value;
    result["actualizado"] = json!(updated);
    result["catalogo"] = json!(capture_catalog(conn,id)?);
    Ok(result)
}
pub fn update_finding_review(
    conn: &Connection,
    id: i64,
    review: &ActualizarRevisionHallazgo,
) -> Result<RevisionHallazgo> {
    anyhow::ensure!(
        review.finding_key.len() <= 1000 && !review.finding_key.trim().is_empty(),
        "Identificador de hallazgo no válido"
    );
    anyhow::ensure!(
        ["pendiente", "confirmado", "descartado", "resuelto"].contains(&review.estado.as_str()),
        "Estado de revisión no válido"
    );
    anyhow::ensure!(
        review.motivo.as_ref().is_none_or(|m| m.len() <= 2000),
        "El motivo supera el límite permitido"
    );
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM auditorias_tecnicas WHERE id=?1 AND negocio_id=?2)",
        params![review.report_id, id],
        |r| r.get(0),
    )?;
    anyhow::ensure!(exists, "Informe técnico no encontrado para este negocio");
    let updated = store::ahora();
    conn.execute("INSERT INTO auditoria_hallazgo_revision(negocio_id,report_id,finding_key,estado,motivo,actualizado) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(report_id,finding_key) DO UPDATE SET estado=excluded.estado,motivo=excluded.motivo,actualizado=excluded.actualizado", params![id,review.report_id,review.finding_key,review.estado,review.motivo,updated])?;
    Ok(RevisionHallazgo {
        report_id: review.report_id.clone(),
        finding_key: review.finding_key.clone(),
        estado: review.estado.clone(),
        motivo: review.motivo.clone(),
        actualizado: updated,
    })
}
pub fn save_result(
    conn: &Connection,
    job: &Job,
    engine: &str,
    device: &str,
    result: &Value,
) -> Result<()> {
    let allowed = [
        "pendiente",
        "en_curso",
        "completo",
        "parcial",
        "no_disponible",
        "error",
    ];
    let state = result["status"]
        .as_str()
        .filter(|s| allowed.contains(s))
        .unwrap_or("error");
    let files = result["files"].as_object().cloned().unwrap_or_default();
    conn.execute("INSERT INTO auditorias_tecnicas(id,job_id,negocio_id,url,motor,dispositivo,version,estado,fecha,resultado,archivos,error) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(job_id,motor,dispositivo) DO UPDATE SET version=excluded.version,estado=excluded.estado,fecha=excluded.fecha,resultado=excluded.resultado,archivos=excluded.archivos,error=excluded.error",params![format!("{}-{engine}-{device}",job.id),job.id,job.params["negocio_id"].as_i64(),job.params["url"].as_str(),engine,device,result["version"].as_str().unwrap_or(""),state,store::ahora(),result["normalized"].to_string(),serde_json::to_string(&files)?,result["error"].as_str()])?;
    Ok(())
}
pub fn update_opportunities(conn: &Connection, id: i64, url: &str) -> Result<()> {
    let n = store::get_negocio(conn, id)?.context("Negocio no encontrado")?;
    if !n
        .web
        .as_deref()
        .and_then(|w| public_http::normalize(w).ok())
        .is_some_and(|u| u.as_str() == url)
    {
        return Ok(());
    }
    let reports = reports(conn, id)?;
    let mut score = 10;
    let mut assessed = false;
    let mut hosting = 12;
    let mut notes = Vec::new();
    // Solo el informe más reciente por perfil/URL: no mezclar ejecuciones antiguas.
    let mut seen = std::collections::HashSet::new();
    for r in reports.into_iter().filter(|r| {
        r.url == url && r.motor == "siteone" && ["completo", "parcial"].contains(&r.estado.as_str())
    }) {
        if !seen.insert(r.dispositivo) {
            continue;
        }
        assessed |= r.resultado["coverage"]["html_pages"].as_u64().unwrap_or(0) > 0;
        if let Some(issues) = r.resultado["issues"].as_array() {
            for issue in issues {
                let severity = issue["severity"].as_str().unwrap_or("info");
                if issue["category"] == "seo" {
                    score = score.max(match severity {
                        "critical" => 85,
                        "high" => 70,
                        "medium" => 50,
                        "low" => 25,
                        _ => 10,
                    });
                    if notes.len() < 3 {
                        if let Some(t) = issue["title"].as_str() {
                            notes.push(t.to_owned());
                        }
                    }
                }
                if issue["hosting_evidence"] == true {
                    hosting = hosting.max(70);
                }
            }
        }
    }
    if assessed {
        conn.execute("UPDATE oportunidades_servicio SET score=?2,motivo=?3,fecha=?4 WHERE negocio_id=?1 AND servicio='seo_on_page'",params![id,score,if notes.is_empty(){"Sin problemas SEO destacados en la cobertura analizada.".into()}else{format!("Hallazgos técnicos: {}",notes.join("; "))},store::ahora()])?;
        conn.execute("UPDATE oportunidades_servicio SET score=?2,motivo=?3,fecha=?4 WHERE negocio_id=?1 AND servicio='hosting'",params![id,hosting,if hosting>=70{"Problemas de disponibilidad o TLS observados; revisar configuración y alojamiento."}else{"Sin evidencia suficiente para atribuir los problemas al hosting. El rendimiento de laboratorio no determina su causa."},store::ahora()])?;
    }
    Ok(())
}

#[cfg(test)]
mod cola_tests {
    use super::*;

    #[test]
    fn cola_omite_cientos_de_obsoletos_y_elige_el_vigente_sin_consumir_pausas() {
        let db = store::abrir(":memory:").unwrap();
        let c = db.lock().unwrap();
        c.execute_batch(r#"
            WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<800)
            INSERT INTO negocios(id,nombre,municipio,web,fuente,fuente_id,primera_vista)
            SELECT i,'Prueba','Pamplona',CASE WHEN i>=799 THEN 'https://actual.example/' END,'test',CAST(i AS TEXT),'2026-11-05' FROM n;
            INSERT INTO web_estado(negocio_id,estado,url_confirmada,origen)
            SELECT id,'confirmada','https://actual.example/','confirmacion_usuario' FROM negocios WHERE id>=799;
            INSERT INTO jobs(id,tipo,estado,params,creado,actualizado,total)
            SELECT 'job-'||id,'technical','pendiente',json_object('negocio_id',id,'url',CASE WHEN id>=799 THEN 'https://actual.example/' ELSE 'https://anterior.example/' END),
            CASE WHEN id<799 THEN '2026-11-04' WHEN id=799 THEN '2026-11-05T10:00' ELSE '2026-11-05T11:00' END,'2026-11-04',4 FROM negocios;
            INSERT INTO jobs(id,tipo,estado,params,creado,actualizado) VALUES
            ('terminado','technical','terminado','{}','2026-11-04','2026-11-04'),
            ('otro-tipo','audit','pendiente','{}','2026-11-04','2026-11-04');
        "#).unwrap();
        let siguiente = siguiente(&c).unwrap().unwrap();
        assert_eq!(siguiente.id, "job-799");
        verificar_web_actual(&c, &siguiente).unwrap();
        assert_eq!(depurar_cola(&c).unwrap(), 798);
        assert_eq!(depurar_cola(&c).unwrap(), 0);
        assert_eq!(
            store::get_job(&c, "terminado").unwrap().unwrap().estado,
            EstadoJob::Terminado
        );
        assert_eq!(
            store::get_job(&c, "otro-tipo").unwrap().unwrap().estado,
            EstadoJob::Pendiente
        );
        let obsoleto = store::get_job(&c, "job-1").unwrap().unwrap();
        assert_eq!(obsoleto.estado, EstadoJob::Error);
        assert!(obsoleto.error.unwrap().contains("obsoleto"));
        assert_eq!(obsoleto.total, 4);
        // No se tocan informes, oportunidades ni la pausa entre sitios reales.
        assert!(store::meta_get(&c, "technical_next_allowed")
            .unwrap()
            .is_none());
        c.execute("UPDATE jobs SET estado='terminado' WHERE id='job-799'", [])
            .unwrap();
        assert_eq!(super::siguiente(&c).unwrap().unwrap().id, "job-800");
        c.execute(
            "UPDATE web_estado SET url_confirmada='https://cambiada.example/' WHERE negocio_id=800",
            [],
        )
        .unwrap();
        let job = store::get_job(&c, "job-800").unwrap().unwrap();
        assert!(verificar_web_actual(&c, &job).is_err());
        assert!(super::siguiente(&c).unwrap().is_none());
        assert_eq!(depurar_cola(&c).unwrap(), 1);
    }

    #[test]
    fn cola_exige_empresa_admisible_y_confirmacion_actual_sin_alterar_trabajos_en_curso() {
        let db = store::abrir(":memory:").unwrap();
        let c = db.lock().unwrap();
        c.execute_batch(r#"
            INSERT INTO negocios(id,nombre,municipio,web,fuente,fuente_id,primera_vista)
            VALUES(1,'Prueba','Pamplona','https://actual.example/','test','1','2026-11-05');
            INSERT INTO web_estado(negocio_id,estado,url_confirmada,origen)
            VALUES(1,'confirmada','https://actual.example/','migracion_fuente');
            INSERT INTO jobs(id,tipo,estado,params,creado,actualizado) VALUES
            ('fuente','technical','pendiente','{"negocio_id":1,"url":"https://actual.example/"}','2026-11-05','2026-11-05'),
            ('curso','technical','en_curso','{"negocio_id":1,"url":"https://actual.example/"}','2026-11-05','2026-11-05');
        "#).unwrap();
        assert!(siguiente(&c).unwrap().is_none());
        assert_eq!(depurar_cola(&c).unwrap(), 1);
        assert_eq!(
            store::get_job(&c, "curso").unwrap().unwrap().estado,
            EstadoJob::EnCurso
        );
        c.execute(
            "UPDATE web_estado SET origen='correccion_usuario' WHERE negocio_id=1",
            [],
        )
        .unwrap();
        c.execute("UPDATE jobs SET estado='pendiente' WHERE id='fuente'", [])
            .unwrap();
        assert!(siguiente(&c).unwrap().is_some());
        c.execute("UPDATE negocios SET admisible=0 WHERE id=1", [])
            .unwrap();
        assert!(siguiente(&c).unwrap().is_none());
        assert_eq!(depurar_cola(&c).unwrap(), 1);
    }
}
