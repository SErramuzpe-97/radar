//! API REST servida por el binario (axum) según el contrato OAS generado con utoipa.
//! El mismo contrato se exporta con `radar api-spec` → api/openapi.yaml (ADR-001)
//! y se sirve en /openapi.json. La UI (Preact) genera su cliente desde ese OAS.

use std::net::SocketAddr;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode, Uri},
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, patch, post},
    Router,
};
use radar_core::model::*;
use radar_core::store::{self, Store};
use rust_embed::RustEmbed;
use serde::Deserialize;
use utoipa::{IntoParams, OpenApi};

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub db_path: String,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FiltrosQuery {
    pub municipio: Option<String>,
    pub sector: Option<String>,
    pub rubro: Option<String>,
    pub tipo_entidad: Option<String>,
    pub estado_web: Option<String>,
    pub estado_auditoria_tecnica: Option<String>,
    pub estado_pipeline: Option<String>,
    pub prioridad: Option<String>,
    pub q: Option<String>,
    pub ordenar: Option<String>,
    pub dir: Option<String>,
    pub pagina: Option<i64>,
    pub tam: Option<i64>,
    pub mostrar_descartados: Option<bool>,
    pub sur: Option<f64>,
    pub oeste: Option<f64>,
    pub norte: Option<f64>,
    pub este: Option<f64>,
}

impl From<FiltrosQuery> for FiltrosNegocios {
    fn from(f: FiltrosQuery) -> Self {
        FiltrosNegocios {
            municipio: f.municipio,
            sector: f.sector,
            rubro: f.rubro,
            tipo_entidad: f.tipo_entidad,
            estado_web: f.estado_web.as_deref().and_then(EstadoWeb::parse),
            estado_auditoria_tecnica: f.estado_auditoria_tecnica,
            estado_pipeline: f.estado_pipeline,
            prioridad: f.prioridad,
            q: f.q,
            ordenar: f.ordenar,
            dir: f.dir,
            pagina: f.pagina,
            tam: f.tam,
            mostrar_descartados: f.mostrar_descartados.unwrap_or(false),
            sur: f.sur,
            oeste: f.oeste,
            norte: f.norte,
            este: f.este,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MapaQuery {
    /// Nivel de zoom de Leaflet (0–19). La respuesta contiene como máximo 825 elementos.
    pub zoom: Option<u8>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExportQuery {
    pub formato: Option<String>,
    pub prioridad: Option<String>,
    pub municipio: Option<String>,
    pub sector: Option<String>,
    pub rubro: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in=Query)]
pub struct TareasQuery {
    pub desde: Option<String>,
    pub hasta: Option<String>,
    pub prioridad: Option<String>,
    pub servicio: Option<String>,
    pub estado: Option<String>,
}

pub struct ApiError(pub StatusCode, pub String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(ErrorApi { error: self.1 })).into_response()
    }
}

pub(crate) type ApiResult<T> = Result<T, ApiError>;

pub(crate) fn err500(e: anyhow::Error) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
}

// ---------- handlers ----------

#[utoipa::path(
    get,
    path = "/api/negocios",
    tag = "negocios",
    params(FiltrosQuery),
    responses(
        (status = 200, description = "Página de negocios", body = PaginaNegocios),
        (status = 500, description = "Error interno", body = ErrorApi)
    )
)]
async fn list_negocios(
    State(st): State<AppState>,
    Query(q): Query<FiltrosQuery>,
) -> ApiResult<Json<PaginaNegocios>> {
    let filtros = q.into();
    store::zona_mapa(&filtros).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    let p = tokio::task::spawn_blocking(move || {
        let conn = st.store.lock().unwrap();
        store::list_negocios(&conn, &filtros)
    })
    .await
    .map_err(|e| err500(e.into()))?
    .map_err(err500)?;
    Ok(Json(PaginaNegocios {
        items: p.items,
        total: p.total,
        pagina: p.pagina,
        tam: p.tam,
    }))
}

#[utoipa::path(
    get,
    path = "/api/negocios/mapa",
    tag = "negocios",
    params(FiltrosQuery, MapaQuery),
    responses(
        (status = 200, description = "Zona visible agrupada, hasta 825 elementos sin truncar empresas", body = RespuestaMapa),
        (status = 400, description = "Zona inválida", body = ErrorApi)
    )
)]
async fn list_negocios_mapa(
    State(st): State<AppState>,
    Query(q): Query<FiltrosQuery>,
    Query(m): Query<MapaQuery>,
) -> ApiResult<Json<RespuestaMapa>> {
    let filtros = q.into();
    store::zona_mapa(&filtros).map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    // SQLite y la agrupación no deben ocupar un hilo del runtime asíncrono.
    let negocios = tokio::task::spawn_blocking(move || {
        let conn = st.store.lock().unwrap();
        store::list_negocios_mapa(&conn, &filtros, m.zoom.unwrap_or(8))
    })
    .await
    .map_err(|e| err500(e.into()))?
    .map_err(err500)?;
    Ok(Json(negocios))
}

#[utoipa::path(
    get,
    path = "/api/negocios/{id}",
    tag = "negocios",
    params(("id" = i64, Path, description = "ID del negocio")),
    responses(
        (status = 200, description = "Detalle del negocio", body = NegocioDetalle),
        (status = 404, description = "No existe", body = ErrorApi)
    )
)]
async fn get_negocio(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<NegocioDetalle>> {
    let conn = st.store.lock().unwrap();
    match store::detalle_negocio(&conn, id).map_err(err500)? {
        Some(d) => Ok(Json(d)),
        None => Err(ApiError(
            StatusCode::NOT_FOUND,
            format!("negocio {id} no existe"),
        )),
    }
}

#[utoipa::path(
    patch,
    path = "/api/negocios/{id}/pipeline",
    tag = "negocios",
    params(("id" = i64, Path, description = "ID del negocio")),
    request_body = SetPipelineRequest,
    responses(
        (status = 200, description = "Pipeline actualizado", body = Pipeline),
        (status = 404, description = "No existe", body = ErrorApi)
    )
)]
async fn set_pipeline(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<SetPipelineRequest>,
) -> ApiResult<Json<Pipeline>> {
    let conn = st.store.lock().unwrap();
    if store::get_negocio(&conn, id).map_err(err500)?.is_none() {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            format!("negocio {id} no existe"),
        ));
    }
    let p = store::set_pipeline(&conn, id, &req).map_err(err500)?;
    Ok(Json(p))
}

#[utoipa::path(
    patch,
    path = "/api/negocios/{id}/oportunidades/{servicio}",
    tag = "negocios",
    params(("id" = i64, Path), ("servicio" = String, Path)),
    request_body = SetOportunidadRequest,
    responses((status = 200, description = "Estado de oportunidad actualizado", body = OportunidadServicio), (status = 404, description = "Negocio u oportunidad no existe", body = ErrorApi))
)]
async fn set_oportunidad(
    State(st): State<AppState>,
    Path((id, servicio)): Path<(i64, String)>,
    Json(req): Json<SetOportunidadRequest>,
) -> ApiResult<Json<OportunidadServicio>> {
    let servicio = ServicioWeb::parse(&servicio)
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "servicio desconocido".into()))?;
    let conn = st.store.lock().unwrap();
    let updated =
        store::actualizar_estado_oportunidad(&conn, id, servicio, req.estado).map_err(err500)?;
    updated
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "oportunidad no existe".into()))
}

#[utoipa::path(
    patch,
    path = "/api/negocios/descartados",
    tag = "negocios",
    request_body = ActualizarDescartados,
    responses((status = 200, description = "Negocios archivados o restaurados", body = ResultadoDescartados))
)]
async fn set_descartados(
    State(st): State<AppState>,
    Json(req): Json<ActualizarDescartados>,
) -> ApiResult<Json<ResultadoDescartados>> {
    if req.ids.is_empty() || req.ids.len() > 500 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "selecciona entre 1 y 500 negocios".into(),
        ));
    }
    let conn = st.store.lock().unwrap();
    let n = store::actualizar_descartados(&conn, &req).map_err(err500)?;
    Ok(Json(ResultadoDescartados {
        actualizados: n as i64,
    }))
}

#[utoipa::path(
    post,
    path = "/api/negocios/{id}/eventos",
    tag = "negocios",
    params(("id" = i64, Path, description = "ID del negocio")),
    request_body = NuevoEvento,
    responses(
        (status = 201, description = "Evento registrado", body = Evento),
        (status = 404, description = "No existe", body = ErrorApi)
    )
)]
async fn add_evento(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<NuevoEvento>,
) -> ApiResult<(StatusCode, Json<Evento>)> {
    let conn = st.store.lock().unwrap();
    if store::get_negocio(&conn, id).map_err(err500)?.is_none() {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            format!("negocio {id} no existe"),
        ));
    }
    let ev = store::add_evento(&conn, id, &req).map_err(err500)?;
    Ok((StatusCode::CREATED, Json(ev)))
}

#[utoipa::path(
    get,
    path = "/api/negocios/{id}/eventos",
    tag = "negocios",
    params(("id" = i64, Path, description = "ID del negocio")),
    responses((status = 200, description = "Eventos del negocio", body = Vec<Evento>))
)]
async fn list_eventos(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<Evento>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::list_eventos(&conn, id).map_err(err500)?))
}

// ---------- jobs: ejecutar búsquedas desde la app web (ADR-002) ----------

pub(crate) fn nuevo_job(tipo: TipoJob, params: serde_json::Value) -> Job {
    let ahora = store::ahora();
    Job {
        id: uuid::Uuid::new_v4().to_string(),
        tipo,
        estado: EstadoJob::Pendiente,
        params,
        hechos: 0,
        total: 0,
        creado: ahora.clone(),
        actualizado: ahora,
        error: None,
        resumen: None,
    }
}

pub(crate) fn actualizar_job(st: &AppState, job: &mut Job) {
    job.actualizado = store::ahora();
    let conn = st.store.lock().unwrap();
    let _ = store::update_job(&conn, job);
}

#[utoipa::path(
    post,
    path = "/api/jobs/scan",
    tag = "jobs",
    request_body = ScanParams,
    responses(
        (status = 202, description = "Scan lanzado", body = Job),
        (status = 500, description = "Error", body = ErrorApi)
    )
)]
async fn lanzar_scan(
    State(st): State<AppState>,
    Json(params): Json<ScanParams>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Rebuild).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Hay una reconstrucción en curso; consulta su progreso en trabajos.".into(),
            ));
        }
    }
    let navarra = params.navarra;
    let mut municipios = if !params.municipios.is_empty() {
        params.municipios.clone()
    } else {
        params
            .municipio
            .clone()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .map(str::to_string)
            .collect()
    };
    municipios = municipios
        .into_iter()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect();
    if !navarra && municipios.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "la búsqueda debe abarcar Navarra".into(),
        ));
    }
    let job = nuevo_job(
        TipoJob::Scan,
        serde_json::to_value(&params).unwrap_or_default(),
    );
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Scan).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "ya hay una búsqueda activa".into(),
            ));
        }
        store::insert_job(&conn, &job).map_err(err500)?;
    }
    let respuesta = job.clone();
    tokio::spawn(ejecutar_scan_job(st.clone(), job, params));
    Ok((StatusCode::ACCEPTED, Json(respuesta)))
}

pub(crate) async fn ejecutar_scan_job(st2: AppState, job: Job, params: ScanParams) {
    let mut municipios = if !params.municipios.is_empty() {
        params.municipios.clone()
    } else {
        params
            .municipio
            .clone()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .map(str::to_string)
            .collect()
    };
    municipios = municipios
        .into_iter()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect();
    let navarra = params.navarra;
    let sector = params.sector.clone();
    let mut j = job;
    j.estado = EstadoJob::EnCurso;
    actualizar_job(&st2, &mut j);
    let store2 = match store::abrir(&st2.db_path) {
        Ok(s) => s,
        Err(e) => {
            j.estado = EstadoJob::Error;
            j.error = Some(format!("{e:#}"));
            actualizar_job(&st2, &mut j);
            return;
        }
    };
    let total = if navarra { 1 } else { municipios.len() };
    j.total = total as i64;
    actualizar_job(&st2, &mut j);
    if navarra {
        let cache_path = format!("{}.navarra.osm.pbf", st2.db_path);
        match radar_core::fuentes::overpass::scan_navarra_resumen(
            &store2,
            &cache_path,
            sector.as_deref(),
        )
        .await
        {
            Ok(mut resumen) => {
                match radar_core::fuentes::navarra::scan(&store2, sector.as_deref()).await {
                    Ok(extra) => resumen["captura_fuentes_abiertas"] = extra,
                    Err(error) => {
                        resumen["fallo_fuentes_abiertas"] = serde_json::json!(format!("{error:#}"))
                    }
                }
                j.resumen = Some(resumen);
                j.hechos = 1;
            }
            Err(e) => {
                j.estado = EstadoJob::Error;
                j.error = Some(format!("Navarra: {e:#}"));
                actualizar_job(&st2, &mut j);
                return;
            }
        }
    } else {
        for (index, municipio) in municipios.iter().enumerate() {
            match radar_core::fuentes::overpass::scan_municipio(
                &store2,
                municipio,
                sector.as_deref(),
            )
            .await
            {
                Ok(n) => {
                    j.hechos = index as i64 + 1;
                    j.resumen = Some(serde_json::json!({"municipio": municipio, "procesados": n}));
                }
                Err(e) => {
                    j.estado = EstadoJob::Error;
                    j.error = Some(format!("{municipio}: {e:#}"));
                    actualizar_job(&st2, &mut j);
                    return;
                }
            }
            actualizar_job(&st2, &mut j);
            if index + 1 < municipios.len() {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            }
        }
    }
    if let Err(e) = store::backfill_oportunidades(&store2.lock().unwrap()) {
        j.estado = EstadoJob::Error;
        j.error = Some(format!("actualizando oportunidades: {e:#}"));
        actualizar_job(&st2, &mut j);
        return;
    }
    j.estado = EstadoJob::Terminado;
    actualizar_job(&st2, &mut j);
}

pub(crate) async fn ejecutar_audit_job(st2: AppState, job: Job, params: AuditParams) {
    let prioridad = params.prioridad.clone();
    let limite = params.limite;
    let mut j = job;
    j.estado = EstadoJob::EnCurso;
    actualizar_job(&st2, &mut j);
    let store2 = match store::abrir(&st2.db_path) {
        Ok(s) => s,
        Err(e) => {
            j.estado = EstadoJob::Error;
            j.error = Some(format!("{e:#}"));
            actualizar_job(&st2, &mut j);
            return;
        }
    };
    let (_ids, negocios) = {
        let conn = store2.lock().unwrap();
        let inicializar = match store::contar_job_items(&conn, &j.id) {
            Ok(n) => n == 0,
            Err(e) => {
                j.estado = EstadoJob::Error;
                j.error = Some(format!("{e:#}"));
                actualizar_job(&st2, &mut j);
                return;
            }
        };
        if inicializar {
            let mut cola = match store::negocios_para_auditar(&conn, prioridad.as_deref(), limite) {
                Ok(v) => v,
                Err(e) => {
                    j.estado = EstadoJob::Error;
                    j.error = Some(format!("{e:#}"));
                    actualizar_job(&st2, &mut j);
                    return;
                }
            };
            if params.solo_sin_auditar {
                cola.retain(|n| {
                    conn.query_row(
                        "SELECT NOT EXISTS(SELECT 1 FROM auditorias WHERE negocio_id=?1)",
                        [n.id],
                        |r| r.get::<_, bool>(0),
                    )
                    .unwrap_or(false)
                });
            }
            let cola_ids = cola.iter().map(|n| n.id).collect::<Vec<_>>();
            if let Err(e) = store::preparar_job_items(&conn, &j.id, &cola_ids, "audit") {
                j.estado = EstadoJob::Error;
                j.error = Some(format!("{e:#}"));
                actualizar_job(&st2, &mut j);
                return;
            }
        }
        let pending = match store::job_items_pendientes(&conn, &j.id) {
            Ok(v) => v,
            Err(e) => {
                j.estado = EstadoJob::Error;
                j.error = Some(format!("{e:#}"));
                actualizar_job(&st2, &mut j);
                return;
            }
        };
        let negocios = match store::negocios_por_ids(&conn, &pending) {
            Ok(v) => v,
            Err(e) => {
                j.estado = EstadoJob::Error;
                j.error = Some(format!("{e:#}"));
                actualizar_job(&st2, &mut j);
                return;
            }
        };
        let available = negocios
            .iter()
            .map(|n| n.id)
            .collect::<std::collections::HashSet<_>>();
        for id in pending.iter().filter(|id| !available.contains(id)) {
            if let Err(e)=conn.execute("UPDATE job_items SET estado='omitido',error='Registro no disponible o no elegible',actualizado=?3 WHERE job_id=?1 AND item_key=?2",[j.id.clone(),id.to_string(),store::ahora()]) {j.estado=EstadoJob::Error;j.error=Some(e.to_string());actualizar_job(&st2,&mut j);return;}
        }
        j.total = match store::contar_job_items(&conn, &j.id) {
            Ok(n) => n,
            Err(e) => {
                j.estado = EstadoJob::Error;
                j.error = Some(e.to_string());
                actualizar_job(&st2, &mut j);
                return;
            }
        };
        j.hechos = j.total - negocios.len() as i64;
        (pending, negocios)
    };
    actualizar_job(&st2, &mut j);
    let batch_size = std::env::var("RADAR_AUDIT_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100)
        .clamp(1, 500);
    let concurrency = std::env::var("RADAR_AUDIT_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(1, 32);
    let pause_ms = std::env::var("RADAR_AUDIT_PAUSE_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2000)
        .min(60000);
    for (i, bloque) in negocios.chunks(batch_size).enumerate() {
        let lote = bloque.to_vec();
        let bloque_ids = bloque.iter().map(|n| n.id).collect::<Vec<_>>();
        let resultados =
            radar_core::audit::auditar_lote_con_limite(lote, concurrency, |_, _| {}).await;
        let persistidos = {
            let mut conn = store2.lock().unwrap();
            let tx = match conn.transaction() {
                Ok(tx) => tx,
                Err(e) => {
                    j.estado = EstadoJob::Error;
                    j.error = Some(format!("abriendo transacción del lote: {e:#}"));
                    actualizar_job(&st2, &mut j);
                    return;
                }
            };
            let ahora = store::ahora();
            let mut resultado = Ok(());
            for (id, r) in resultados {
                let auditoria = Auditoria {
                    id: 0,
                    negocio_id: id,
                    fecha: ahora.clone(),
                    estado_web: r.estado_web,
                    score: r.score,
                    prioridad: r.prioridad,
                    senales: r.senales,
                };
                if let Err(e) = store::insert_auditoria(&tx, &auditoria)
                    .and_then(|_| store::guardar_oportunidades_auditoria(&tx, &auditoria))
                    .and_then(|_| store::set_ultima_verificacion(&tx, id, &ahora))
                {
                    resultado = Err(e);
                    break;
                }
            }
            if resultado.is_ok() {
                if let Err(e) = store::terminar_job_items(&tx, &j.id, &bloque_ids) {
                    resultado = Err(e);
                }
            }
            if resultado.is_ok() {
                if let Err(e) = tx.commit() {
                    resultado = Err(e.into());
                }
            }
            resultado
        };
        if let Err(e) = persistidos {
            j.estado = EstadoJob::Error;
            j.error = Some(format!("guardando lote de auditoría: {e:#}"));
            actualizar_job(&st2, &mut j);
            return;
        }
        j.hechos += bloque.len() as i64;
        actualizar_job(&st2, &mut j);
        if i + 1 < negocios.chunks(batch_size).len() {
            tokio::time::sleep(std::time::Duration::from_millis(pause_ms)).await;
        }
    }
    j.estado = EstadoJob::Terminado;
    actualizar_job(&st2, &mut j);
}

#[utoipa::path(
    post,
    path = "/api/jobs/audit",
    tag = "jobs",
    request_body = AuditParams,
    responses(
        (status = 202, description = "Auditoría lanzada", body = Job),
        (status = 500, description = "Error", body = ErrorApi)
    )
)]
async fn lanzar_audit(
    State(st): State<AppState>,
    Json(params): Json<AuditParams>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Rebuild).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Hay una reconstrucción en curso; consulta su progreso en trabajos.".into(),
            ));
        }
    }
    let job = nuevo_job(
        TipoJob::Audit,
        serde_json::to_value(&params).unwrap_or_default(),
    );
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Audit).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "ya hay una auditoría activa".into(),
            ));
        }
        store::insert_job(&conn, &job).map_err(err500)?;
    }
    let respuesta = job.clone();
    tokio::spawn(ejecutar_audit_job(st.clone(), job, params));

    Ok((StatusCode::ACCEPTED, Json(respuesta)))
}

#[utoipa::path(
    post,
    path = "/api/jobs/watch",
    tag = "jobs",
    request_body = WatchParams,
    responses(
        (status = 202, description = "Watch lanzado", body = Job),
        (status = 500, description = "Error", body = ErrorApi)
    )
)]
async fn lanzar_watch(
    State(st): State<AppState>,
    Json(params): Json<WatchParams>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Rebuild).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Hay una reconstrucción en curso; consulta su progreso en trabajos.".into(),
            ));
        }
    }
    let job = nuevo_job(
        TipoJob::Watch,
        serde_json::to_value(&params).unwrap_or_default(),
    );
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Watch).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "ya hay una importación activa".into(),
            ));
        }
        store::insert_job(&conn, &job).map_err(err500)?;
    }
    let respuesta = job.clone();
    tokio::spawn(ejecutar_watch_job(st.clone(), job, params));
    Ok((StatusCode::ACCEPTED, Json(respuesta)))
}

pub(crate) async fn ejecutar_crawl_job(st: AppState, job: Job, params: CrawlParams) {
    let mut j = job;
    j.estado = EstadoJob::EnCurso;
    actualizar_job(&st, &mut j);
    j.total = 1;
    actualizar_job(&st, &mut j);
    let store2 = match store::abrir(&st.db_path) {
        Ok(s) => s,
        Err(e) => {
            marcar_job_error(&st, j, format!("{e:#}"));
            return;
        }
    };
    match radar_core::fuentes::crawler::rastrear_fuente(&store2, params.fuente_id).await {
        Ok(resumen) => {
            j.resumen = Some(resumen);
            if let Err(e) = store::backfill_oportunidades(&store2.lock().unwrap()) {
                marcar_job_error(&st, j, format!("backfill de oportunidades: {e:#}"));
                return;
            }
            j.hechos = 1;
            j.estado = EstadoJob::Terminado;
            actualizar_job(&st, &mut j);
        }
        Err(e) => marcar_job_error(&st, j, format!("{e:#}")),
    }
}

#[utoipa::path(post, path="/api/jobs/crawl", tag="jobs", request_body=CrawlParams, responses((status=202,description="Rastreo lanzado",body=Job)))]
async fn lanzar_crawl(
    State(st): State<AppState>,
    Json(params): Json<CrawlParams>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Rebuild).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Hay una reconstrucción en curso; consulta su progreso en trabajos.".into(),
            ));
        }
    }
    let job = nuevo_job(
        TipoJob::Crawl,
        serde_json::to_value(&params).unwrap_or_default(),
    );
    {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Crawl).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "ya hay un rastreo activo".into(),
            ));
        }
        let fuente = store::fuente(&conn, params.fuente_id).map_err(err500)?;
        if fuente.is_none() {
            return Err(ApiError(
                StatusCode::NOT_FOUND,
                "fuente no encontrada".into(),
            ));
        }
        store::insert_job(&conn, &job).map_err(err500)?;
    }
    let respuesta = job.clone();
    tokio::spawn(ejecutar_crawl_job(st, job, params));
    Ok((StatusCode::ACCEPTED, Json(respuesta)))
}

#[utoipa::path(get,path="/api/fuentes",tag="fuentes",responses((status=200,description="Fuentes configuradas",body=Vec<Fuente>)))]
async fn list_fuentes(State(st): State<AppState>) -> ApiResult<Json<Vec<Fuente>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::fuentes(&conn).map_err(err500)?))
}

#[utoipa::path(post,path="/api/fuentes",tag="fuentes",request_body=NuevaFuente,responses((status=201,description="Fuente creada",body=Fuente)))]
async fn create_fuente(
    State(st): State<AppState>,
    Json(req): Json<NuevaFuente>,
) -> ApiResult<(StatusCode, Json<Fuente>)> {
    if req.nombre.trim().is_empty()
        || req.condiciones.trim().is_empty()
        || req.atribucion.trim().is_empty()
        || req.revisada_en.trim().is_empty()
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "nombre, condiciones, atribución y fecha de revisión son obligatorios".into(),
        ));
    }
    if !["semilla", "busqueda", "dataset"].contains(&req.tipo.as_str()) {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "tipo de fuente inválido".into(),
        ));
    }
    if req.tipo == "busqueda" && !req.url.contains("{query}") {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "el proveedor de búsqueda debe incluir {query} en la URL".into(),
        ));
    }
    let conn = st.store.lock().unwrap();
    let id = store::crear_fuente(&conn, &req).map_err(err500)?;
    let fuente = store::fuente(&conn, id).map_err(err500)?.ok_or_else(|| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "no se pudo leer la fuente creada".into(),
        )
    })?;
    Ok((StatusCode::CREATED, Json(fuente)))
}

#[utoipa::path(patch,path="/api/fuentes/{id}",tag="fuentes",params(("id"=i64,Path)),request_body=CambiarFuenteRequest,responses((status=200,description="Fuente actualizada",body=Fuente),(status=404,description="Fuente no encontrada",body=ErrorApi)))]
async fn update_fuente(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<CambiarFuenteRequest>,
) -> ApiResult<Json<Fuente>> {
    let conn = st.store.lock().unwrap();
    if !store::habilitar_fuente(&conn, id, req.habilitada).map_err(err500)? {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "fuente no encontrada".into(),
        ));
    }
    Ok(Json(store::fuente(&conn, id).map_err(err500)?.unwrap()))
}

#[utoipa::path(get,path="/api/fuentes/candidatas",tag="fuentes",responses((status=200,description="Dominios candidatos descubiertos",body=Vec<FuenteCandidata>)))]
async fn list_fuentes_candidatas(
    State(st): State<AppState>,
) -> ApiResult<Json<Vec<FuenteCandidata>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::fuentes_candidatas(&conn).map_err(err500)?))
}

#[utoipa::path(get,path="/api/tareas",tag="crm",params(TareasQuery),responses((status=200,description="Seguimientos pendientes",body=Vec<TareaSeguimiento>)))]
async fn list_tareas(
    State(st): State<AppState>,
    Query(q): Query<TareasQuery>,
) -> ApiResult<Json<Vec<TareaSeguimiento>>> {
    let hasta = q.hasta.unwrap_or_else(|| {
        chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    });
    if chrono::NaiveDate::parse_from_str(&hasta, "%Y-%m-%d").is_err() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "hasta debe tener formato YYYY-MM-DD".into(),
        ));
    }
    if let Some(ref desde) = q.desde {
        if chrono::NaiveDate::parse_from_str(desde, "%Y-%m-%d").is_err() {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "desde debe tener formato YYYY-MM-DD".into(),
            ));
        }
    }
    let conn = st.store.lock().unwrap();
    Ok(Json(
        store::tareas_filtradas(
            &conn,
            q.desde.as_deref(),
            &hasta,
            q.prioridad.as_deref(),
            q.servicio.as_deref(),
            q.estado.as_deref(),
        )
        .map_err(err500)?,
    ))
}

#[utoipa::path(get,path="/api/clasificacion",tag="fuentes",responses((status=200,description="Resultado del filtro comercial por evidencias",body=ResumenClasificacion)))]
async fn clasificacion(State(st): State<AppState>) -> ApiResult<Json<ResumenClasificacion>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::resumen_clasificacion(&conn).map_err(err500)?))
}

#[utoipa::path(get,path="/api/cobertura",tag="stats",responses((status=200,description="Cobertura de registros conocidos por municipio",body=Vec<CoberturaMunicipio>)))]
async fn cobertura(State(st): State<AppState>) -> ApiResult<Json<Vec<CoberturaMunicipio>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::cobertura_municipios(&conn).map_err(err500)?))
}

#[utoipa::path(get,path="/api/duplicados",tag="negocios",responses((status=200,description="Posibles duplicados pendientes de revisión",body=Vec<CandidatoDuplicado>)))]
async fn list_duplicados(State(st): State<AppState>) -> ApiResult<Json<Vec<CandidatoDuplicado>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::listar_duplicados(&conn).map_err(err500)?))
}

#[utoipa::path(post,path="/api/duplicados/{id}/fusionar",tag="negocios",params(("id"=i64,Path)),request_body=FusionDuplicado,responses((status=200,description="Fichas fusionadas",body=MensajeOk)))]
async fn fusionar_duplicado(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<FusionDuplicado>,
) -> ApiResult<Json<MensajeOk>> {
    let mut conn = st.store.lock().unwrap();
    store::fusionar_duplicados(&mut conn, id, req.conservar_id, req.fusionar_id)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("{e:#}")))?;
    Ok(Json(MensajeOk {
        message: "fichas fusionadas".into(),
    }))
}

#[utoipa::path(delete,path="/api/duplicados/{id}",tag="negocios",params(("id"=i64,Path)),responses((status=204,description="Sugerencia descartada"),(status=404,description="No encontrada",body=ErrorApi)))]
async fn descartar_duplicado(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    let conn = st.store.lock().unwrap();
    if !store::rechazar_duplicado(&conn, id).map_err(err500)? {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "sugerencia no encontrada".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get,path="/api/plantillas",tag="crm",responses((status=200,description="Plantillas de contacto",body=Vec<PlantillaContacto>)))]
async fn list_plantillas(State(st): State<AppState>) -> ApiResult<Json<Vec<PlantillaContacto>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::plantillas_contacto(&conn).map_err(err500)?))
}

#[utoipa::path(post,path="/api/plantillas",tag="crm",request_body=NuevaPlantillaContacto,responses((status=201,description="Plantilla creada",body=PlantillaContacto)))]
async fn create_plantilla(
    State(st): State<AppState>,
    Json(req): Json<NuevaPlantillaContacto>,
) -> ApiResult<(StatusCode, Json<PlantillaContacto>)> {
    if req.nombre.trim().is_empty() || req.cuerpo.trim().is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "nombre y cuerpo son obligatorios".into(),
        ));
    }
    let conn = st.store.lock().unwrap();
    let id = store::crear_plantilla_contacto(&conn, &req).map_err(err500)?;
    let plantilla = store::plantillas_contacto(&conn)
        .map_err(err500)?
        .into_iter()
        .find(|p| p.id == id)
        .unwrap();
    Ok((StatusCode::CREATED, Json(plantilla)))
}

#[utoipa::path(get,path="/api/negocios/{id}/borradores",tag="crm",params(("id"=i64,Path)),responses((status=200,description="Borradores guardados",body=Vec<BorradorContacto>)))]
async fn list_borradores(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<BorradorContacto>>> {
    let conn = st.store.lock().unwrap();
    if store::get_negocio(&conn, id).map_err(err500)?.is_none() {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "negocio no encontrado".into(),
        ));
    }
    Ok(Json(store::borradores_negocio(&conn, id).map_err(err500)?))
}

#[utoipa::path(post,path="/api/negocios/{id}/borradores",tag="crm",params(("id"=i64,Path)),request_body=NuevoBorrador,responses((status=201,description="Borrador guardado",body=BorradorContacto)))]
async fn create_borrador(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(mut req): Json<NuevoBorrador>,
) -> ApiResult<(StatusCode, Json<BorradorContacto>)> {
    req.negocio_id = id;
    if req.cuerpo.trim().is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "el borrador no puede estar vacío".into(),
        ));
    }
    let conn = st.store.lock().unwrap();
    if store::get_negocio(&conn, id).map_err(err500)?.is_none() {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "negocio no encontrado".into(),
        ));
    }
    let draft_id = store::crear_borrador(&conn, &req).map_err(err500)?;
    let draft = store::borradores_negocio(&conn, id)
        .map_err(err500)?
        .into_iter()
        .find(|d| d.id == draft_id)
        .unwrap();
    Ok((StatusCode::CREATED, Json(draft)))
}

#[utoipa::path(patch,path="/api/fuentes/candidatas/{id}",tag="fuentes",params(("id"=i64,Path)),request_body=EstadoCandidataRequest,responses((status=200,description="Candidata actualizada",body=MensajeOk)))]
async fn update_fuente_candidata(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<EstadoCandidataRequest>,
) -> ApiResult<Json<MensajeOk>> {
    let conn = st.store.lock().unwrap();
    if !store::marcar_candidata(&conn, id, &req.estado).map_err(err500)? {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "candidata no encontrada".into(),
        ));
    }
    Ok(Json(MensajeOk {
        message: "candidata actualizada".into(),
    }))
}

pub(crate) async fn ejecutar_watch_job(st2: AppState, job: Job, params: WatchParams) {
    let mut j = job;
    j.estado = EstadoJob::EnCurso;
    actualizar_job(&st2, &mut j);
    let store2 = match store::abrir(&st2.db_path) {
        Ok(s) => s,
        Err(e) => {
            j.estado = EstadoJob::Error;
            j.error = Some(format!("{e:#}"));
            actualizar_job(&st2, &mut j);
            return;
        }
    };
    let r = match (&params.fuente[..], &params.csv) {
        ("borme" | "openmercantil", Some(path)) => {
            radar_core::fuentes::borme::watch_desde_csv(&store2, path)
        }
        ("borme" | "openmercantil", None) => {
            radar_core::fuentes::borme::watch_borme_reciente(&store2).await
        }
        (fuente, _) => Err(anyhow::anyhow!("watch: fuente desconocida {fuente}")),
    };
    match r {
        Ok(n) => {
            j.estado = EstadoJob::Terminado;
            j.hechos = n as i64;
            j.total = n as i64;
        }
        Err(e) => {
            j.estado = EstadoJob::Error;
            j.error = Some(format!("{e:#}"));
        }
    }
    actualizar_job(&st2, &mut j);
}

#[utoipa::path(
    get,
    path = "/api/jobs",
    tag = "jobs",
    responses((status = 200, description = "Últimos jobs", body = Vec<Job>))
)]
async fn list_jobs(State(st): State<AppState>) -> ApiResult<Json<Vec<Job>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::list_jobs(&conn).map_err(err500)?))
}

#[utoipa::path(
    get,
    path = "/api/jobs/{id}",
    tag = "jobs",
    params(("id" = String, Path, description = "ID del job")),
    responses(
        (status = 200, description = "Estado del job", body = Job),
        (status = 404, description = "No existe", body = ErrorApi)
    )
)]
async fn get_job(State(st): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Job>> {
    let conn = st.store.lock().unwrap();
    match store::get_job(&conn, &id).map_err(err500)? {
        Some(j) => Ok(Json(j)),
        None => Err(ApiError(
            StatusCode::NOT_FOUND,
            format!("job {id} no existe"),
        )),
    }
}

#[utoipa::path(
    post,
    path = "/api/jobs/{id}/retry",
    tag = "jobs",
    params(("id" = String, Path, description = "ID del job")),
    responses((status = 202, description = "Job reintentado", body = Job), (status = 404, description = "Job no encontrado", body = ErrorApi), (status = 409, description = "Hay otro job activo", body = ErrorApi))
)]
async fn retry_job(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    let mut job = {
        let conn = st.store.lock().unwrap();
        store::get_job(&conn, &id)
            .map_err(err500)?
            .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "job no encontrado".into()))?
    };
    if job.tipo != TipoJob::Rebuild {
        let conn = st.store.lock().unwrap();
        if store::tipo_job_activo(&conn, TipoJob::Rebuild).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "La reconstrucción administra este trabajo; espera a que finalice.".into(),
            ));
        }
    }
    if job.estado != EstadoJob::Error {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "solo se pueden reintentar jobs con error".into(),
        ));
    }
    {
        let conn = st.store.lock().unwrap();
        if job.tipo != TipoJob::Technical
            && store::tipo_job_activo(&conn, job.tipo).map_err(err500)?
        {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "ya hay un job activo de este tipo".into(),
            ));
        }
        job.estado = EstadoJob::Pendiente;
        job.error = None;
        store::update_job(&conn, &mut job).map_err(err500)?;
    }
    let respuesta = job.clone();
    let params = job.params.clone();
    match job.tipo {
        TipoJob::Discovery => tokio::spawn(crate::technical_jobs::discovery(st, job)),
        TipoJob::Technical => tokio::spawn(async {}),
        TipoJob::Rebuild => tokio::spawn(crate::rebuild::ejecutar(st, job)),
        TipoJob::Scan => tokio::spawn(ejecutar_scan_job(
            st,
            job,
            serde_json::from_value(params)
                .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?,
        )),
        TipoJob::Audit => tokio::spawn(ejecutar_audit_job(
            st,
            job,
            serde_json::from_value(params)
                .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?,
        )),
        TipoJob::Watch => tokio::spawn(ejecutar_watch_job(
            st,
            job,
            serde_json::from_value(params)
                .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?,
        )),
        TipoJob::Crawl => tokio::spawn(ejecutar_crawl_job(
            st,
            job,
            serde_json::from_value(params)
                .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?,
        )),
        TipoJob::Export => {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "los jobs de exportación no se pueden reintentar".into(),
            ))
        }
    };
    Ok((StatusCode::ACCEPTED, Json(respuesta)))
}

// ---------- export y stats ----------

#[utoipa::path(
    get,
    path = "/api/export",
    tag = "export",
    params(ExportQuery),
    responses(
        (status = 200, description = "Export (csv / geojson / gpx) según `formato`", body = String, content_type = "text/csv"),
        (status = 500, description = "Error", body = ErrorApi)
    )
)]
async fn export(State(st): State<AppState>, Query(q): Query<ExportQuery>) -> ApiResult<Response> {
    let formato = q.formato.clone().unwrap_or_else(|| "csv".into());
    let filtros = FiltrosNegocios {
        municipio: q.municipio.clone(),
        sector: q.sector.clone(),
        rubro: q.rubro.clone(),
        prioridad: q.prioridad.clone(),
        q: q.q.clone(),
        ..Default::default()
    };
    let conn = st.store.lock().unwrap();
    let (body, content_type, ext) = match formato.as_str() {
        "geojson" => (
            radar_core::export::export_geojson(&conn, &filtros).map_err(err500)?,
            "application/geo+json",
            "geojson",
        ),
        "gpx" => (
            radar_core::export::export_gpx(&conn, &filtros).map_err(err500)?,
            "application/gpx+xml",
            "gpx",
        ),
        _ => {
            let mut buf: Vec<u8> = Vec::new();
            radar_core::export::export_csv(&conn, &filtros, &mut buf).map_err(err500)?;
            (
                String::from_utf8_lossy(&buf).into_owned(),
                "text/csv",
                "csv",
            )
        }
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"radar.{ext}\""),
        )
        .body(axum::body::Body::from(body))
        .unwrap())
}

#[utoipa::path(
    get,
    path = "/api/stats",
    tag = "stats",
    responses((status = 200, description = "KPIs del panel", body = Stats))
)]
async fn stats(State(st): State<AppState>) -> ApiResult<Json<Stats>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::stats(&conn).map_err(err500)?))
}

// ---------- contrato OAS ----------

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Radar API",
        version = "0.1.0",
        description = "API local de Radar (binario Rust en 127.0.0.1). La app web (Preact+TS) consume esta API a través del cliente generado desde este contrato OAS (api/openapi.yaml). Las búsquedas (scan/audit/watch) se ejecutan desde la web vía /api/jobs/*."
    ),
    tags(
        (name = "negocios", description = "Negocios, pipeline CRM y eventos"),
        (name = "jobs", description = "Ejecución asíncrona de scan / audit / watch desde la web"),
        (name = "export", description = "Exportaciones CSV / GeoJSON / GPX"),
        (name = "stats", description = "KPIs del panel")
    ),
    paths(
        list_negocios, list_negocios_mapa, get_negocio, set_pipeline, set_oportunidad, set_descartados, add_evento, list_eventos,
        lanzar_scan, lanzar_audit, lanzar_watch, list_jobs, get_job, retry_job,
        lanzar_crawl, list_fuentes, create_fuente, update_fuente, list_fuentes_candidatas, update_fuente_candidata,
        list_tareas, cobertura, list_duplicados, fusionar_duplicado, descartar_duplicado, list_plantillas, create_plantilla, list_borradores, create_borrador,
        export, stats, clasificacion,
        crate::web_api::info, crate::web_api::confirm, crate::web_api::discover, crate::web_api::discard, crate::web_api::report, crate::web_api::reviews, crate::web_api::save_review, crate::web_api::visit_state, crate::web_api::save_visit_state, crate::web_api::launch, crate::web_api::download, crate::web_api::preview, crate::web_api::bulk,
        crate::auth::request_code, crate::auth::verify, crate::auth::me, crate::auth::logout
    ),
    components(schemas(
        Negocio, Auditoria, Pipeline, Evento, Job, Stats, ParEtiqueta, OportunidadServicio, ConversionServicio,
        PaginaNegocios, NegocioDetalle, RespuestaMapa, PuntoMapa, ZonaMapa, ScanParams, AuditParams, WatchParams,
        WebInfo, WebCandidata, ConfirmarWeb, AuditoriaTecnica, EstadoAuditoriaTecnica, RevisionHallazgo, ActualizarRevisionHallazgo, CapturaVisita, CapturaCatalogoVisita, EstadoDiagnosticoVisita, GuardarDiagnosticoVisita, BusquedaWebParams, ResumenClasificacion, Fuente, NuevaFuente, CambiarFuenteRequest, CrawlParams, FuenteCandidata, EstadoCandidataRequest,
        TareaSeguimiento, CoberturaMunicipio, CandidatoDuplicado, FusionDuplicado, PlantillaContacto, NuevaPlantillaContacto, NuevoBorrador, BorradorContacto,
        SetPipelineRequest, SetOportunidadRequest, ActualizarDescartados, ResultadoDescartados, NuevoEvento, EstadoWeb, EstadoPipeline, EstadoOportunidad, ServicioWeb, TipoJob, EstadoJob, ErrorApi,
        AuthRequest, AuthVerify, AuthMe, MensajeOk
    ))
)]
pub struct ApiDoc;

pub fn router(st: AppState) -> Router {
    // Rutas públicas: login OTP y contrato OAS. El dashboard estático se sirve en el fallback.
    let public = Router::new()
        .route("/api/auth/request-code", post(crate::auth::request_code))
        .route("/api/auth/verify", post(crate::auth::verify))
        .route("/api/auth/logout", post(crate::auth::logout))
        .route("/openapi.json", get(|| async { Json(ApiDoc::openapi()) }));

    // Rutas protegidas con sesión (ADR-005): todo /api/* salvo /api/auth/*
    let protected = Router::new()
        .route("/api/auth/me", get(crate::auth::me))
        .route("/api/negocios", get(list_negocios))
        .route("/api/negocios/mapa", get(list_negocios_mapa))
        .route("/api/negocios/descartados", patch(set_descartados))
        .route("/api/negocios/{id}", get(get_negocio))
        .route("/api/negocios/{id}/pipeline", patch(set_pipeline))
        .route(
            "/api/negocios/{id}/oportunidades/{servicio}",
            patch(set_oportunidad),
        )
        .route(
            "/api/negocios/{id}/eventos",
            get(list_eventos).post(add_evento),
        )
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/scan", post(lanzar_scan))
        .route("/api/jobs/audit", post(lanzar_audit))
        .route("/api/jobs/watch", post(lanzar_watch))
        .route("/api/jobs/crawl", post(lanzar_crawl))
        .route("/api/fuentes", get(list_fuentes).post(create_fuente))
        .route("/api/fuentes/{id}", patch(update_fuente))
        .route("/api/fuentes/candidatas", get(list_fuentes_candidatas))
        .route(
            "/api/fuentes/candidatas/{id}",
            patch(update_fuente_candidata),
        )
        .route("/api/tareas", get(list_tareas))
        .route("/api/cobertura", get(cobertura))
        .route("/api/clasificacion", get(clasificacion))
        .route("/api/duplicados", get(list_duplicados))
        .route("/api/duplicados/{id}/fusionar", post(fusionar_duplicado))
        .route("/api/duplicados/{id}", delete(descartar_duplicado))
        .route(
            "/api/plantillas",
            get(list_plantillas).post(create_plantilla),
        )
        .route(
            "/api/negocios/{id}/borradores",
            get(list_borradores).post(create_borrador),
        )
        .route("/api/jobs/{id}", get(get_job))
        .route("/api/jobs/{id}/retry", post(retry_job))
        .route("/api/export", get(export))
        .route("/api/stats", get(stats))
        .merge(crate::web_api::routes())
        .route_layer(middleware::from_fn_with_state(
            st.clone(),
            crate::auth::require_auth,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .with_state(st)
        .fallback(get(serve_ui))
        .layer(middleware::from_fn(security_headers))
}

async fn security_headers(req: axum::extract::Request, next: middleware::Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert(
        "referrer-policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    // No se habilita CORS: los navegadores solo podrán leer la API desde el mismo origen.
    response
}

#[derive(RustEmbed)]
#[folder = "../../radar-ui/dist/"]
struct UiAssets;

async fn serve_ui(uri: Uri) -> Response {
    let requested = uri.path().trim_start_matches('/');
    let path = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };
    // Los paths sin archivo sirven la entrada de Preact para permitir navegación SPA.
    let (asset, content_path) = match UiAssets::get(path) {
        Some(asset) => (Some(asset), path),
        None => (UiAssets::get("index.html"), "index.html"),
    };
    match asset {
        Some(asset) => {
            let content_type = mime_guess::from_path(content_path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type.as_ref())
                .body(axum::body::Body::from(asset.data.into_owned()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(axum::body::Body::from("UI no incluida en este binario"))
            .unwrap(),
    }
}

/// Sirve la API en 127.0.0.1:{puerto} y los archivos del dashboard incluidos en el binario.
pub async fn serve(db_path: &str, puerto: u16) -> anyhow::Result<()> {
    let _db_lock = crate::rebuild::lock_db(db_path)?;
    let store = store::abrir(db_path)?;
    store::backfill_oportunidades(&store.lock().unwrap())?;
    let st = AppState {
        store,
        db_path: db_path.to_string(),
    };
    let backup_db = db_path.to_string();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
        loop {
            tick.tick().await;
            let path = backup_db.clone();
            match tokio::task::spawn_blocking(move || store::respaldar_si_nuevo_dia(&path, 14))
                .await
            {
                Ok(Ok(Some(path))) => eprintln!("Copia diaria creada: {}", path.display()),
                Ok(Ok(None)) => {}
                Ok(Err(e)) => eprintln!("Error creando copia diaria de SQLite: {e:#}"),
                Err(e) => eprintln!("Falló tarea de copia SQLite: {e}"),
            }
        }
    });
    reanudar_jobs(&st)?;
    crate::technical_jobs::start(st.clone());
    let app = router(st);
    let addr = SocketAddr::from(([127, 0, 0, 1], puerto));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Radar sirviendo en http://{addr} (API: /api/*, contrato: /openapi.json)");
    println!(
        "Auth: OTP por email para {:?}",
        crate::auth::emails_autorizados()
    );
    axum::serve(listener, app).await?;
    Ok(())
}

fn reanudar_jobs(st: &AppState) -> anyhow::Result<()> {
    let jobs = {
        let conn = st.store.lock().unwrap();
        let jobs = store::jobs_reanudables(&conn)?;
        let mut recuperados = Vec::with_capacity(jobs.len());
        for mut job in jobs {
            job.estado = EstadoJob::Pendiente;
            job.error = None;
            store::update_job(&conn, &mut job)?;
            recuperados.push(job);
        }
        recuperados
    };
    if !jobs.is_empty() {
        eprintln!("Reanudando {} jobs pendientes/interrumpidos", jobs.len());
        let st = st.clone();
        tokio::spawn(async move {
            for job in jobs {
                let params = job.params.clone();
                match job.tipo {
                    TipoJob::Technical => {}
                    TipoJob::Discovery => crate::technical_jobs::discovery(st.clone(), job).await,
                    TipoJob::Rebuild => crate::rebuild::ejecutar(st.clone(), job).await,
                    TipoJob::Scan => match serde_json::from_value(params) {
                        Ok(p) => ejecutar_scan_job(st.clone(), job, p).await,
                        Err(e) => marcar_job_error(
                            &st,
                            job,
                            format!("parámetros de scan no válidos: {e}"),
                        ),
                    },
                    TipoJob::Audit => match serde_json::from_value(params) {
                        Ok(p) => ejecutar_audit_job(st.clone(), job, p).await,
                        Err(e) => marcar_job_error(
                            &st,
                            job,
                            format!("parámetros de auditoría no válidos: {e}"),
                        ),
                    },
                    TipoJob::Watch => match serde_json::from_value(params) {
                        Ok(p) => ejecutar_watch_job(st.clone(), job, p).await,
                        Err(e) => marcar_job_error(
                            &st,
                            job,
                            format!("parámetros de importación no válidos: {e}"),
                        ),
                    },
                    TipoJob::Crawl => match serde_json::from_value(params) {
                        Ok(p) => ejecutar_crawl_job(st.clone(), job, p).await,
                        Err(e) => marcar_job_error(
                            &st,
                            job,
                            format!("parámetros de rastreo no válidos: {e}"),
                        ),
                    },
                    TipoJob::Export => {
                        marcar_job_error(&st, job, "reintento de exportación no disponible".into())
                    }
                }
            }
        });
    }
    Ok(())
}

fn marcar_job_error(st: &AppState, mut job: Job, error: String) {
    job.estado = EstadoJob::Error;
    job.error = Some(error);
    actualizar_job(st, &mut job);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use radar_core::model::{EstadoPipeline, Negocio};
    use sha2::{Digest, Sha256};
    use tower::ServiceExt;

    fn estado_prueba() -> AppState {
        AppState {
            store: store::abrir(":memory:").unwrap(),
            db_path: ":memory:".into(),
        }
    }

    fn peticion(metodo: &str, ruta: &str, cuerpo: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(metodo).uri(ruta);
        if cuerpo.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        builder
            .body(
                cuerpo
                    .map(|s| Body::from(s.to_owned()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap()
    }

    #[tokio::test]
    async fn contrato_servido_coincide_con_el_openapi_commiteado() {
        let response = router(estado_prueba())
            .oneshot(peticion("GET", "/openapi.json", None))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(response.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert!(response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
        let servido: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let yaml: serde_json::Value =
            serde_yaml::from_str(include_str!("../../../api/openapi.yaml")).unwrap();
        assert_eq!(servido, yaml);
    }

    #[tokio::test]
    async fn otp_autentica_y_api_permite_consultar_actualizar_y_registrar_eventos() {
        let st = estado_prueba();
        let negocio_id = {
            let conn = st.store.lock().unwrap();
            store::upsert_negocio(
                &conn,
                &Negocio {
                    id: 0,
                    nombre: "Negocio de prueba".into(),
                    categoria: Some("restaurant".into()),
                    sector: Some("hosteleria".into()),
                    rubro: Some("restaurante".into()),
                    evidencia_rubro: None,
                    direccion: Some("Calle Mayor 1".into()),
                    municipio: "Pamplona".into(),
                    lat: Some(42.8),
                    lon: Some(-1.6),
                    telefono: None,
                    email: None,
                    web: None,
                    estado_web_lista: "sin_localizar".into(),
                    estado_auditoria_tecnica: "pendiente".into(),
                    fuente: "test".into(),
                    fuente_id: "negocio-1".into(),
                    primera_vista: store::ahora(),
                    ultima_verificacion: None,
                    score_potencial: None,
                    prioridad_comercial: None,
                    estado_pipeline: None,
                    oportunidades: Vec::new(),
                    descartado: false,
                },
            )
            .unwrap()
        };
        let app = router(st.clone());

        let sin_sesion = app
            .clone()
            .oneshot(peticion("GET", "/api/negocios", None))
            .await
            .unwrap();
        assert_eq!(sin_sesion.status(), StatusCode::UNAUTHORIZED);

        let desconocido = app
            .clone()
            .oneshot(peticion(
                "POST",
                "/api/auth/request-code",
                Some(r#"{"email":"fuera@example.com"}"#),
            ))
            .await
            .unwrap();
        assert_eq!(desconocido.status(), StatusCode::OK);
        let respuesta = to_bytes(desconocido.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<MensajeOk>(&respuesta)
                .unwrap()
                .message,
            "Si el email está autorizado, recibirás un código de acceso."
        );

        let email = crate::auth::emails_autorizados()
            .into_iter()
            .next()
            .expect("allowlist de test");
        let mut hasher = Sha256::new();
        hasher.update(b"123456");
        let hash = hex::encode(hasher.finalize());
        store::crear_auth_code(&st.store.lock().unwrap(), &email, &hash, 600).unwrap();
        let mut verify = peticion(
            "POST",
            "/api/auth/verify",
            Some(&format!("{{\"email\":\"{email}\",\"code\":\"123456\"}}")),
        );
        verify
            .headers_mut()
            .insert("x-forwarded-proto", "https".parse().unwrap());
        let verificada = app.clone().oneshot(verify).await.unwrap();
        assert_eq!(verificada.status(), StatusCode::OK);
        let set_cookie = verificada
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("Secure"));
        let cookie = set_cookie.split(';').next().unwrap().to_owned();

        let mut me = peticion("GET", "/api/auth/me", None);
        me.headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        assert_eq!(
            app.clone().oneshot(me).await.unwrap().status(),
            StatusCode::OK
        );

        let mut listar = peticion("GET", "/api/negocios", None);
        listar
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        let listado = app.clone().oneshot(listar).await.unwrap();
        assert_eq!(listado.status(), StatusCode::OK);
        let body = to_bytes(listado.into_body(), usize::MAX).await.unwrap();
        let pagina: PaginaNegocios = serde_json::from_slice(&body).unwrap();
        assert_eq!(pagina.total, 1);
        assert_eq!(pagina.items[0].nombre, "Negocio de prueba");

        let mut mapa = peticion(
            "GET",
            "/api/negocios/mapa?sur=42&oeste=-2&norte=43&este=-1&zoom=19",
            None,
        );
        mapa.headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        let respuesta = app.clone().oneshot(mapa).await.unwrap();
        assert_eq!(respuesta.status(), StatusCode::OK);
        let body = to_bytes(respuesta.into_body(), usize::MAX).await.unwrap();
        let mapa: RespuestaMapa = serde_json::from_slice(&body).unwrap();
        assert_eq!(mapa.total, 1);
        assert_eq!(mapa.items[0].id, Some(negocio_id));
        for ruta in [
            "/api/negocios/mapa?sur=42",
            "/api/negocios/mapa?sur=43&oeste=-2&norte=42&este=-1",
            "/api/negocios/mapa?sur=NaN&oeste=-2&norte=43&este=-1",
            "/api/negocios?sur=42",
        ] {
            let mut pet = peticion("GET", ruta, None);
            pet.headers_mut()
                .insert(header::COOKIE, cookie.parse().unwrap());
            assert_eq!(
                app.clone().oneshot(pet).await.unwrap().status(),
                StatusCode::BAD_REQUEST
            );
        }

        let mut pipeline = peticion(
            "PATCH",
            &format!("/api/negocios/{negocio_id}/pipeline"),
            Some(r#"{"estado":"contactado","valor_estimado":500.0,"canal":"email"}"#),
        );
        pipeline
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        assert_eq!(
            app.clone().oneshot(pipeline).await.unwrap().status(),
            StatusCode::OK
        );
        let detalle = store::detalle_negocio(&st.store.lock().unwrap(), negocio_id)
            .unwrap()
            .unwrap();
        assert_eq!(detalle.pipeline.unwrap().estado, EstadoPipeline::Contactado);

        let mut evento = peticion(
            "POST",
            &format!("/api/negocios/{negocio_id}/eventos"),
            Some(r#"{"tipo":"email","resultado":"enviado","notas":"primer contacto"}"#),
        );
        evento
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        assert_eq!(
            app.clone().oneshot(evento).await.unwrap().status(),
            StatusCode::CREATED
        );
        let mut listar_eventos =
            peticion("GET", &format!("/api/negocios/{negocio_id}/eventos"), None);
        listar_eventos
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        let respuesta_eventos = app.clone().oneshot(listar_eventos).await.unwrap();
        assert_eq!(respuesta_eventos.status(), StatusCode::OK);
        let body_eventos = to_bytes(respuesta_eventos.into_body(), usize::MAX)
            .await
            .unwrap();
        let eventos: Vec<Evento> = serde_json::from_slice(&body_eventos).unwrap();
        assert_eq!(eventos.len(), 1);
        assert_eq!(eventos[0].tipo, "email");

        let mut logout = peticion("POST", "/api/auth/logout", None);
        logout
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        let respuesta_logout = app.clone().oneshot(logout).await.unwrap();
        assert_eq!(respuesta_logout.status(), StatusCode::OK);
        assert!(respuesta_logout
            .headers()
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Max-Age=0"));
        let mut despues = peticion("GET", "/api/stats", None);
        despues
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        assert_eq!(
            app.oneshot(despues).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }
}
