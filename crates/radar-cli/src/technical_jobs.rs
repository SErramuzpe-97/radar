use crate::api::{self, AppState};
use anyhow::{Context, Result};
use radar_core::{audit, model::*, store, technical, web};
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::Duration,
};

pub async fn refresh_light(st: &AppState, id: i64) -> Result<()> {
    let n = {
        let c = st.store.lock().unwrap();
        store::get_negocio(&c, id)?.context("Negocio no encontrado")?
    };
    let url = n.web.clone();
    let r = audit::auditar_negocio(&n).await;
    let c = st.store.lock().unwrap();
    if store::get_negocio(&c, id)?.and_then(|n| n.web) != url {
        return Ok(());
    }
    let a = Auditoria {
        id: 0,
        negocio_id: id,
        fecha: store::ahora(),
        estado_web: r.estado_web,
        score: r.score,
        prioridad: r.prioridad,
        senales: r.senales,
    };
    store::insert_auditoria(&c, &a)?;
    store::guardar_oportunidades_auditoria(&c, &a)?;
    Ok(())
}
static SITE_LOCK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
static DISCOVERY_LOCK: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

pub async fn discovery(st: AppState, mut job: Job) {
    // El contraste de una web usa peticiones HTTP ligeras; no debe esperar a
    // que Lighthouse/SiteOne termine su rastreo pesado del sitio anterior.
    let _permit = DISCOVERY_LOCK.acquire().await.unwrap();
    job.estado = EstadoJob::EnCurso;
    job.error = None;
    api::actualizar_job(&st, &mut job);
    let result = discovery_inner(&st, &mut job).await;
    if let Err(e) = result {
        job.estado = EstadoJob::Error;
        job.error = Some(format!("{e:#}"));
    } else {
        job.estado = EstadoJob::Terminado;
    }
    api::actualizar_job(&st, &mut job);
}
async fn discovery_inner(st: &AppState, job: &mut Job) -> Result<()> {
    let ids = {
        let c = st.store.lock().unwrap();
        if store::contar_job_items(&c, &job.id)? == 0 {
            let ids = if let Some(id) = job.params["negocio_id"].as_i64() {
                vec![id]
            } else {
                let mut stmt=c.prepare("SELECT id FROM negocios WHERE admisible=1 AND (web IS NULL OR web LIKE '%facebook.com%' OR web LIKE '%instagram.com%') ORDER BY id")?;
                let ids = stmt
                    .query_map([], |r| r.get::<_, i64>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                ids
            };
            store::preparar_job_items(&c, &job.id, &ids, "discovery")?;
        }
        job.total = store::contar_job_items(&c, &job.id)?;
        let ids = store::job_items_pendientes(&c, &job.id)?;
        job.hechos = job.total - ids.len() as i64;
        ids
    };
    let mut confirmed = 0;
    let mut suggested = 0;
    for id in ids {
        let allow_search = job.params["buscar"].as_bool().unwrap_or(true);
        let result = web::discover(&st.store, id, allow_search).await?;
        if matches!(
            result["estado"].as_str(),
            Some("confirmada" | "verificada_automatica")
        ) {
            confirmed += 1;
            refresh_light(st, id).await?;
            let c = st.store.lock().unwrap();
            technical::enqueue(&c, id)?;
        }
        if result["estado"] == "sugerida" {
            suggested += 1;
        }
        {
            let c = st.store.lock().unwrap();
            store::terminar_job_items(&c, &job.id, &[id])?;
        }
        job.hechos += 1;
        job.resumen =
            Some(json!({"confirmadas":confirmed,"sugeridas":suggested,"ultimo_resultado":result}));
        api::actualizar_job(st, job);
        if job.hechos % 100 == 0 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    Ok(())
}

pub fn start(st: AppState) {
    let source_state = st.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            match radar_core::fuentes::navarra::scan(&source_state.store, None).await {
                Ok(result) => eprintln!(
                    "Sincronización automática de registros navarros: {}",
                    result
                ),
                Err(e) => {
                    eprintln!("Falló la sincronización automática de registros navarros: {e:#}")
                }
            }
            let cache = format!("{}.navarra.osm.pbf", source_state.db_path);
            match radar_core::fuentes::overpass::scan_navarra_resumen(
                &source_state.store,
                &cache,
                None,
            )
            .await
            {
                Ok(result) => eprintln!("Sincronización automática de OpenStreetMap: {result}"),
                Err(e) => eprintln!("Falló la sincronización automática de OpenStreetMap: {e:#}"),
            }
            let seed = {
                let c = source_state.store.lock().unwrap();
                c.query_row("SELECT id FROM fuentes WHERE habilitada=1 AND tipo='semilla' AND revisada_en IS NOT NULL AND condiciones IS NOT NULL AND atribucion IS NOT NULL AND (ultima_ejecucion IS NULL OR datetime(ultima_ejecucion)<datetime('now','-7 days')) ORDER BY COALESCE(ultima_ejecucion,'') LIMIT 1",[],|r|r.get::<_,i64>(0)).optional().ok().flatten()
            };
            if let Some(id) = seed {
                match radar_core::fuentes::crawler::rastrear_fuente(&source_state.store, id).await {
                    Ok(result) => eprintln!("Rastreo automático de fuente {id}: {result}"),
                    Err(e) => eprintln!("Falló el rastreo automático de fuente {id}: {e:#}"),
                }
            }
            tokio::time::sleep(Duration::from_secs(24 * 60 * 60)).await;
        }
    });
    let discovery_state = st.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if !web::configured() {
                continue;
            }
            let job = {
                let c = discovery_state.store.lock().unwrap();
                let last = store::meta_get(&c, "web_discovery_next")
                    .ok()
                    .flatten()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(0);
                if last > chrono::Utc::now().timestamp()
                    || store::tipo_job_activo(&c, TipoJob::Discovery).unwrap_or(true)
                {
                    None
                } else {
                    match web::next_automatic_discovery(&c) {
                        Ok(Some((id, buscar))) => {
                            match technical::automatic_discovery_job(&c, id, buscar) {
                                Ok(job) => {
                                    let _ = store::meta_set(
                                        &c,
                                        "web_discovery_next",
                                        &(chrono::Utc::now().timestamp() + 30).to_string(),
                                    );
                                    eprintln!("Descubrimiento web automático programado para el negocio {id}; búsqueda externa: {buscar}");
                                    Some(job)
                                }
                                Err(e) => {
                                    eprintln!("No se pudo encolar la búsqueda de web: {e:#}");
                                    None
                                }
                            }
                        }
                        Ok(None) => None,
                        Err(e) => {
                            eprintln!(
                                "No se pudo seleccionar el siguiente negocio para descubrir: {e:#}"
                            );
                            None
                        }
                    }
                }
            };
            if let Some(job) = job {
                tokio::spawn(discovery(discovery_state.clone(), job));
            }
        }
    });
    tokio::spawn(async move {
        loop {
            let next = {
                let c = st.store.lock().unwrap();
                match technical::depurar_cola(&c) {
                    Ok(n) if n > 0 => {
                        eprintln!("Cola técnica: {n} trabajos obsoletos retirados de pendientes")
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("No se pudo revisar la cola técnica: {e:#}"),
                }
                let allowed = store::meta_get(&c, "technical_next_allowed")
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0)
                    <= chrono::Utc::now().timestamp();
                if allowed && technical::worker_available() {
                    let scan_due = store::meta_get(&c, "technical_autoschedule_next")
                        .ok()
                        .flatten()
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0)
                        <= chrono::Utc::now().timestamp();
                    if scan_due {
                        let hours = std::env::var("RADAR_TECH_REFRESH_HOURS")
                            .ok()
                            .and_then(|v| v.parse::<i64>().ok())
                            .unwrap_or(24)
                            .max(6);
                        let before =
                            (chrono::Utc::now() - chrono::Duration::hours(hours)).to_rfc3339();
                        match technical::enqueue_due(&c, &before) {
                            Ok(Some(job)) => eprintln!(
                                "Auditoría técnica automática programada para el negocio {}",
                                job.params["negocio_id"]
                            ),
                            Ok(None) => {}
                            Err(e) => eprintln!("No se pudo programar auditoría automática: {e:#}"),
                        }
                        let _ = store::meta_set(
                            &c,
                            "technical_autoschedule_next",
                            &(chrono::Utc::now().timestamp() + 60).to_string(),
                        );
                    }
                    match technical::siguiente(&c) {
                        Ok(job) => job,
                        Err(e) => {
                            eprintln!(
                                "No se pudo seleccionar la siguiente auditoría técnica: {e:#}"
                            );
                            None
                        }
                    }
                } else {
                    None
                }
            };
            if let Some(job) = next {
                execute(st.clone(), job).await;
                let pause = std::env::var("RADAR_TECH_PAUSE_SECONDS")
                    .ok()
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(0)
                    .max(0);
                let c = st.store.lock().unwrap();
                let _ = store::meta_set(
                    &c,
                    "technical_next_allowed",
                    &(chrono::Utc::now().timestamp() + pause).to_string(),
                );
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
async fn phase(
    st: &AppState,
    job: &Job,
    engine: &str,
    device: &str,
    deadline: i64,
) -> Result<Value> {
    let phase_id = format!("{}-{engine}-{device}", job.id);
    let base = technical::directory();
    for folder in ["requests", "results", "artifacts"] {
        std::fs::create_dir_all(base.join(folder))?;
    }
    let request = base.join("requests").join(format!("{phase_id}.json"));
    let working = base.join("requests").join(format!("{phase_id}.working"));
    let result = base.join("results").join(format!("{phase_id}.json"));
    let previous = {
        let c = st.store.lock().unwrap();
        technical::reports(&c, job.params["negocio_id"].as_i64().unwrap())?
            .into_iter()
            .find(|r| r.id == phase_id)
    };
    if let Some(prev) = previous {
        if ["completo", "parcial", "no_disponible"].contains(&prev.estado.as_str()) {
            return Ok(json!({"status":prev.estado,"cached":true}));
        }
        if prev.estado == "error" {
            let _ = std::fs::remove_file(&result);
        }
    }
    {
        let c = st.store.lock().unwrap();
        technical::save_result(
            &c,
            job,
            engine,
            device,
            &json!({"status":"en_curso","version":if engine=="lighthouse"{"13.6.0"}else{"2.5.1"},"normalized":{},"files":{}}),
        )?;
    }
    if !working.exists() && !result.exists() {
        let body = json!({"schema":1,"id":phase_id,"job_id":job.id,"engine":engine,"device":device,"url":job.params["url"],"deadline":deadline,"limits":{"urls":100,"rendered":5}});
        let tmp = request.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec(&body)?)?;
        std::fs::rename(&tmp, &request)?;
    }
    loop {
        if let Ok(content) = std::fs::read(&result) {
            let value: Value =
                serde_json::from_slice(&content).context("respuesta del worker inválida")?;
            anyhow::ensure!(
                value["id"] == phase_id,
                "respuesta del worker no corresponde a la fase"
            );
            let c = st.store.lock().unwrap();
            technical::save_result(&c, job, engine, device, &value)?;
            // SQLite ya contiene el resultado normalizado y las referencias al
            // HTML original; el fichero JSON solo sirve como buzón temporal.
            std::fs::remove_file(&result).ok();
            return Ok(value);
        }
        if chrono::Utc::now().timestamp() > deadline + 5 {
            let value = json!({"status":"error","error":"El worker no terminó dentro del presupuesto del sitio","normalized":{},"files":{}});
            let c = st.store.lock().unwrap();
            technical::save_result(&c, job, engine, device, &value)?;
            return Ok(value);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
pub async fn execute(st: AppState, mut job: Job) {
    let _permit = SITE_LOCK
        .acquire()
        .await
        .expect("semaforo de sitios activo");
    job.estado = EstadoJob::EnCurso;
    job.error = None;
    job.hechos = 0;
    job.total = 4;
    api::actualizar_job(&st, &mut job);
    let r = execute_inner(&st, &mut job).await;
    match r {
        Ok(()) => job.estado = EstadoJob::Terminado,
        Err(e) => {
            job.estado = EstadoJob::Error;
            job.error = Some(format!("{e:#}"));
        }
    }
    api::actualizar_job(&st, &mut job);
}
async fn execute_inner(st: &AppState, job: &mut Job) -> Result<()> {
    let id = job.params["negocio_id"]
        .as_i64()
        .context("trabajo sin negocio")?;
    let url = job.params["url"]
        .as_str()
        .context("trabajo sin URL")?
        .to_string();
    {
        let c = st.store.lock().unwrap();
        technical::verificar_web_actual(&c, job)?;
    }
    if !radar_core::public_http::robots_allowed(&url)
        .await
        .unwrap_or(false)
    {
        let c = st.store.lock().unwrap();
        for device in ["mobile", "desktop"] {
            for engine in ["lighthouse", "siteone"] {
                technical::save_result(
                    &c,
                    job,
                    engine,
                    device,
                    &json!({"status":"no_disponible","normalized":{"coverage":{"html_pages":0,"rendered_pages":0,"blocked":1},"warnings":["Robots no permite la auditoría o no se pudo comprobar su política."]},"files":{}}),
                )?;
            }
        }
        job.hechos = 4;
        job.resumen = Some(json!({"motivo":"robots_no_disponible_o_bloqueado"}));
        return Ok(());
    }
    let mut states = Vec::new();
    let mut errors = Vec::new();
    for device in ["mobile", "desktop"] {
        for engine in ["lighthouse", "siteone"] {
            {
                let c = st.store.lock().unwrap();
                technical::verificar_web_actual(&c, job)?;
            }
            job.resumen =
                Some(json!({"motor_actual":engine,"dispositivo_actual":device,"fases":states}));
            api::actualizar_job(st, job);
            // Cada motor y dispositivo recibe su propio presupuesto. Lighthouse
            // suele terminar en segundos; SiteOne puede rastrear páginas lentas
            // y, en móvil, completar además la matriz responsive.
            let phase_budget = if engine == "siteone" { 240 } else { 120 };
            let deadline = chrono::Utc::now().timestamp() + phase_budget;
            let result = phase(st, job, engine, device, deadline).await?;
            if result["status"] == "error" {
                errors.push(format!(
                    "{engine}/{device}: {}",
                    result["error"].as_str().unwrap_or("fallo de ejecución")
                ));
            }
            states.push(json!({"motor":engine,"dispositivo":device,"estado":result["status"]}));
            job.hechos += 1;
            api::actualizar_job(st, job);
        }
    }
    {
        let c = st.store.lock().unwrap();
        technical::update_opportunities(&c, id, &url)?;
    }
    job.resumen = Some(json!({"fases":states,"incidencias":errors}));
    if !errors.is_empty() {
        anyhow::bail!("{}", errors.join("; "));
    }
    Ok(())
}

pub fn sync_pinned_captures(
    conn: &rusqlite::Connection,
    negocio_id: i64,
    previous: &Value,
    current: &Value,
) -> Result<()> {
    let current_kept: std::collections::HashSet<&str> = current["capturas"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|mark| mark["estado"] == "conservar")
        .filter_map(|mark| mark["capture_id"].as_str())
        .collect();
    let dir = technical::directory().join("artifacts");
    let pinned = dir.join("pinned");
    fs::create_dir_all(&pinned)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pinned, fs::Permissions::from_mode(0o770))?;
    }
    let previous_catalog = previous["catalogo"].as_array().cloned().unwrap_or_default();
    for mark in previous["capturas"].as_array().into_iter().flatten() {
        let Some(capture_id) = mark["capture_id"].as_str() else {
            continue;
        };
        if mark["estado"] != "conservar" || current_kept.contains(capture_id) {
            continue;
        }
        if let Some(capture) = previous_catalog
            .iter()
            .find(|capture| capture["capture_id"] == capture_id)
        {
            let filename = format!(
                "{}.{}.gz.keep",
                capture["report_id"].as_str().unwrap_or(""),
                capture["key"].as_str().unwrap_or("")
            );
            fs::remove_file(pinned.join(filename)).ok();
        }
    }
    let catalog = technical::capture_catalog(conn, negocio_id)?;
    for mark in current["capturas"].as_array().into_iter().flatten() {
        if mark["estado"] != "conservar" {
            continue;
        }
        let Some(capture_id) = mark["capture_id"].as_str() else {
            continue;
        };
        let Some(capture) = catalog
            .iter()
            .find(|capture| capture["capture_id"] == capture_id)
        else {
            continue;
        };
        let report_id = capture["report_id"].as_str().unwrap_or("");
        let key = capture["key"].as_str().unwrap_or("");
        let filename = format!("{report_id}.{key}.gz");
        anyhow::ensure!(
            dir.join(&filename).is_file(),
            "La captura seleccionada ya no está disponible"
        );
        let marker = pinned.join(format!("{filename}.keep"));
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o660)
            .open(marker)?;
        writeln!(file, "business={negocio_id} capture={capture_id}")?;
    }
    Ok(())
}

pub fn artifact(conn: &store::Store, id: &str, kind: &str) -> Result<(std::path::PathBuf, String)> {
    let responsive = kind.strip_prefix("responsive-").is_some_and(|v| {
        let mut p = v.split('-');
        p.next().is_some_and(|x| x.parse::<u8>().is_ok())
            && p.next().is_some_and(|x| x.parse::<u16>().is_ok())
            && p.next().is_none()
    });
    anyhow::ensure!(kind == "html" || responsive, "Tipo de informe no válido");
    let c = conn.lock().unwrap();
    let files = c.query_row(
        "SELECT archivos FROM auditorias_tecnicas WHERE id=?1",
        [id],
        |r| r.get::<_, String>(0),
    )?;
    let files: Value = serde_json::from_str(&files)?;
    let relative = files[kind]
        .as_str()
        .context("Informe original no disponible o caducado")?;
    let base = technical::directory().join("artifacts").canonicalize()?;
    let path = base.join(relative).canonicalize()?;
    anyhow::ensure!(
        path.starts_with(&base) && Path::new(relative).is_relative(),
        "Ruta de informe no válida"
    );
    let extension = if kind == "html" { "html" } else { "png" };
    Ok((path, format!("{id}-{kind}.{extension}")))
}
