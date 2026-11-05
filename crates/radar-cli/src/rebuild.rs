//! Operación local de reconstrucción. La limpieza se confirma antes de crear el job;
//! reanudarlo nunca vuelve a borrar datos.
use crate::api::{self, AppState};
use anyhow::{Context, Result};
use fs2::FileExt;
use radar_core::{model::*, store};
use serde_json::{json, Value};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

pub fn lock_db(path: &str) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(format!("{path}.service.lock"))?;
    file.try_lock_exclusive()
        .context("La base está en uso por el servidor u otra operación administrativa")?;
    Ok(file)
}

pub fn preparar(db_path: &str, audit: bool) -> Result<()> {
    let _lock = lock_db(db_path)?;
    let db = Path::new(db_path);
    anyhow::ensure!(db.exists(), "no existe la base a reconstruir");
    let backup_dir = db.parent().unwrap_or(Path::new(".")).join("backups");
    std::fs::create_dir_all(&backup_dir)?;
    let backup = backup_dir.join(format!(
        "radar-before-rebuild-{}.sqlite",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%fZ")
    ));
    store::backup_to_path(db_path, &backup)?;
    let store = store::abrir(db_path)?;
    let mut conn = store.lock().unwrap();
    let mut fases = Vec::new();
    let mut omitidas = Vec::new();
    for f in store::fuentes(&conn)?.into_iter().filter(|f| f.habilitada) {
        if f.tipo == "dataset" && f.url.ends_with("/navarra-latest.osm.pbf") {
            fases.insert(
                0,
                json!({"key":"osm","tipo":"scan","params":{"navarra":true}}),
            );
        } else if f.tipo == "dataset"
            && f.url == "https://www.boe.es/datosabiertos/api/borme/sumario"
        {
            fases.push(json!({"key":"borme","tipo":"watch","params":{"fuente":"borme"}}));
        } else if ["semilla", "busqueda"].contains(&f.tipo.as_str()) {
            fases.push(
                json!({"key":format!("fuente-{}",f.id),"tipo":"crawl","params":{"fuente_id":f.id}}),
            );
        } else {
            omitidas.push(json!({"fuente_id":f.id,"nombre":f.nombre,"motivo":"No existe un importador para este dataset"}));
        }
    }
    anyhow::ensure!(
        !fases.is_empty(),
        "No hay fuentes habilitadas con importador compatible; no se han borrado resultados"
    );
    let job = api::nuevo_job(
        TipoJob::Rebuild,
        json!({"fases":fases,"audit":audit,"backup":backup.to_string_lossy(),"policy":radar_core::eligibility::POLICY_VERSION,"fuentes_omitidas":omitidas}),
    );
    store::reset_resultados(&mut conn, &job)?;
    println!("Copia verificada: {}", backup.display());
    println!("Resultados eliminados; accesos, sesiones y configuración conservados. Reconstrucción pendiente: {}. Inicia el servidor para continuar.",job.id);
    Ok(())
}

fn child(
    st: &AppState,
    parent: &Job,
    key: &str,
    tipo: TipoJob,
    mut params: Value,
    force: bool,
) -> Result<Job> {
    let conn = st.store.lock().unwrap();
    let previous: Option<String> = conn
        .query_row(
            "SELECT resultado FROM job_steps WHERE job_id=?1 AND paso=?2",
            [&parent.id, key],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = previous {
        if let Some(job) = store::get_job(&conn, &id)? {
            if !force || job.estado != EstadoJob::Terminado {
                return Ok(job);
            }
        }
    }
    params["rebuild_parent"] = json!(parent.id);
    let job = api::nuevo_job(tipo, params);
    let tx = conn.unchecked_transaction()?;
    store::insert_job(&tx, &job)?;
    tx.execute("INSERT INTO job_steps(job_id,paso,estado,resultado,actualizado) VALUES(?1,?2,'pendiente',?3,?4) ON CONFLICT(job_id,paso) DO UPDATE SET estado='pendiente',resultado=excluded.resultado,actualizado=excluded.actualizado",[&parent.id,key,&job.id,&store::ahora()])?;
    tx.commit()?;
    Ok(job)
}
async fn run_child(st: &AppState, parent: &Job, key: &str, mut job: Job) -> Result<Job> {
    let id = job.id.clone();
    if job.estado != EstadoJob::Terminado {
        job.error = None;
        job.estado = EstadoJob::Pendiente;
        api::actualizar_job(st, &mut job);
        match job.tipo {
            TipoJob::Scan => {
                api::ejecutar_scan_job(st.clone(), job.clone(), serde_json::from_value(job.params)?)
                    .await
            }
            TipoJob::Watch => {
                api::ejecutar_watch_job(
                    st.clone(),
                    job.clone(),
                    serde_json::from_value(job.params)?,
                )
                .await
            }
            TipoJob::Crawl => {
                api::ejecutar_crawl_job(
                    st.clone(),
                    job.clone(),
                    serde_json::from_value(job.params)?,
                )
                .await
            }
            TipoJob::Audit => {
                api::ejecutar_audit_job(
                    st.clone(),
                    job.clone(),
                    serde_json::from_value(job.params)?,
                )
                .await
            }
            _ => anyhow::bail!("fase de reconstrucción no válida"),
        }
    }
    let conn = st.store.lock().unwrap();
    let result = store::get_job(&conn, &id)?.context("job de fase desaparecido")?;
    conn.execute(
        "UPDATE job_steps SET estado=?3,error=?4,actualizado=?5 WHERE job_id=?1 AND paso=?2",
        [
            Some(parent.id.as_str()),
            Some(key),
            Some(if result.estado == EstadoJob::Terminado {
                "terminado"
            } else {
                "error"
            }),
            result.error.as_deref(),
            Some(store::ahora().as_str()),
        ],
    )?;
    Ok(result)
}

pub async fn ejecutar(st: AppState, mut job: Job) {
    job.estado = EstadoJob::EnCurso;
    job.error = None;
    api::actualizar_job(&st, &mut job);
    if let Err(e) = run(&st, &mut job).await {
        job.estado = EstadoJob::Error;
        job.error = Some(format!("{e:#}"));
    }
    api::actualizar_job(&st, &mut job);
}
async fn run(st: &AppState, job: &mut Job) -> Result<()> {
    let fases = job.params["fases"]
        .as_array()
        .context("reconstrucción sin fases")?
        .clone();
    job.total = fases.len() as i64 + i64::from(job.params["audit"].as_bool().unwrap_or(false));
    job.hechos = 0;
    let mut resumen = serde_json::Map::new();
    let mut errors = Vec::new();
    for fase in fases {
        let key = fase["key"].as_str().context("fase sin nombre")?;
        job.resumen = Some(json!({"fase_actual":key,"fases":resumen}));
        api::actualizar_job(st, job);
        let tipo: TipoJob = serde_json::from_value(fase["tipo"].clone())?;
        let step = child(st, job, key, tipo, fase["params"].clone(), false)?;
        let result = run_child(st, job, key, step).await?;
        resumen.insert(
            key.into(),
            json!({"estado":result.estado,"resultado":result.resumen,"error":result.error}),
        );
        if result.estado == EstadoJob::Terminado {
            job.hechos += 1;
        } else {
            errors.push(format!("{key}: {}", result.error.unwrap_or_default()));
            if key == "osm" {
                job.resumen = Some(json!({"fase_actual":key,"fases":resumen}));
                anyhow::bail!(
                    "falló la fuente regional obligatoria: {}",
                    errors.join("; ")
                );
            }
        }
    }
    if job.params["audit"].as_bool().unwrap_or(false) {
        job.resumen = Some(json!({"fase_actual":"audit","fases":resumen}));
        api::actualizar_job(st, job);
        let missing: i64 = {
            let conn = st.store.lock().unwrap();
            conn.query_row("SELECT COUNT(*) FROM negocios n WHERE NOT EXISTS(SELECT 1 FROM auditorias a WHERE a.negocio_id=n.id)",[],|r|r.get(0))?
        };
        let step = child(
            st,
            job,
            "audit",
            TipoJob::Audit,
            json!({"solo_sin_auditar":true}),
            missing > 0,
        )?;
        let result = run_child(st, job, "audit", step).await?;
        resumen.insert("audit".into(),json!({"estado":result.estado,"hechos":result.hechos,"total":result.total,"error":result.error}));
        if result.estado == EstadoJob::Terminado {
            job.hechos += 1;
        } else {
            errors.push(format!("audit: {}", result.error.unwrap_or_default()));
        }
    }
    job.resumen = Some(
        json!({"fase_actual":"finalizado","fases":resumen,"incidencias":errors,"fuentes_omitidas":job.params["fuentes_omitidas"]}),
    );
    if !errors.is_empty() {
        anyhow::bail!("fases con incidencias: {}", errors.join("; "));
    }
    job.estado = EstadoJob::Terminado;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn reconstruccion_audita_mas_de_cinco_mil_y_no_repite_al_reanudar() {
        let dir = std::env::temp_dir().join(format!("radar-rebuild-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("test.sqlite");
        let path = db.to_str().unwrap().to_string();
        let store = store::abrir(&path).unwrap();
        {
            let conn = store.lock().unwrap();
            let tx = conn.unchecked_transaction().unwrap();
            for index in 0..5001 {
                let n:Negocio=serde_json::from_value(json!({"id":0,"nombre":format!("Local {index}"),"municipio":"Navarra","fuente":"test","fuente_id":index.to_string(),"primera_vista":"2026-11-04"})).unwrap();
                store::upsert_negocio(&tx, &n).unwrap();
            }
            tx.commit().unwrap();
        }
        let st = AppState {
            store,
            db_path: path,
        };
        let job = api::nuevo_job(TipoJob::Rebuild, json!({"fases":[],"audit":true}));
        store::insert_job(&st.store.lock().unwrap(), &job).unwrap();
        std::env::set_var("RADAR_AUDIT_PAUSE_MS", "0");
        ejecutar(st.clone(), job.clone()).await;
        let first = store::get_job(&st.store.lock().unwrap(), &job.id)
            .unwrap()
            .unwrap();
        assert_eq!(first.estado, EstadoJob::Terminado, "{:?}", first.error);
        ejecutar(st.clone(), first).await;
        {
            let c = st.store.lock().unwrap();
            assert_eq!(
                c.query_row("SELECT COUNT(*) FROM auditorias", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                5001
            );
            assert_eq!(
                c.query_row(
                    "SELECT COUNT(*) FROM job_items WHERE estado='terminado'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                5001
            );
        }
        std::env::remove_var("RADAR_AUDIT_PAUSE_MS");
        drop(st);
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod interruption_tests {
    use super::*;
    #[tokio::test]
    async fn audit_recupera_progreso_desde_items_si_cae_antes_de_actualizar_job() {
        let dir = std::env::temp_dir().join(format!("radar-resume-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("db.sqlite").to_string_lossy().to_string();
        let store = store::abrir(&path).unwrap();
        let job = api::nuevo_job(TipoJob::Audit, json!({}));
        {
            let c = store.lock().unwrap();
            let mut ids = Vec::new();
            for i in 0..3 {
                let n:Negocio=serde_json::from_value(json!({"id":0,"nombre":"Local","municipio":"Navarra","fuente":"test","fuente_id":i.to_string(),"primera_vista":"2026-11-04"})).unwrap();
                ids.push(store::upsert_negocio(&c, &n).unwrap());
            }
            store::insert_job(&c, &job).unwrap();
            store::preparar_job_items(&c, &job.id, &ids, "audit").unwrap();
            store::insert_auditoria(
                &c,
                &Auditoria {
                    id: 0,
                    negocio_id: ids[0],
                    fecha: store::ahora(),
                    estado_web: EstadoWeb::SinWeb,
                    score: 80,
                    prioridad: "A".into(),
                    senales: json!({}),
                },
            )
            .unwrap();
            store::terminar_job_items(&c, &job.id, &ids[..1]).unwrap();
        }
        let st = AppState {
            store,
            db_path: path,
        };
        api::ejecutar_audit_job(
            st.clone(),
            job.clone(),
            serde_json::from_value(json!({})).unwrap(),
        )
        .await;
        let c = st.store.lock().unwrap();
        let result = store::get_job(&c, &job.id).unwrap().unwrap();
        assert_eq!((result.hechos, result.total), (3, 3));
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM auditorias", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            3
        );
        drop(c);
        drop(st);
        let _ = std::fs::remove_dir_all(dir);
    }
}
