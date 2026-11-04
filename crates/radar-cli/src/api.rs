//! API REST servida por el binario (axum) según el contrato OAS generado con utoipa.
//! El mismo contrato se exporta con `radar api-spec` → api/openapi.yaml (ADR-001)
//! y se sirve en /openapi.json. La UI (Preact) genera su cliente desde ese OAS.

use std::net::SocketAddr;

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    middleware,
    response::{IntoResponse, Json, Response},
    routing::{get, patch, post},
    Router,
};
use radar_core::model::*;
use radar_core::store::{self, Store};
use serde::Deserialize;
use tower_http::services::{ServeDir, ServeFile};
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
    pub estado_web: Option<String>,
    pub prioridad: Option<String>,
    pub q: Option<String>,
    pub ordenar: Option<String>,
    pub dir: Option<String>,
    pub pagina: Option<i64>,
    pub tam: Option<i64>,
}

impl From<FiltrosQuery> for FiltrosNegocios {
    fn from(f: FiltrosQuery) -> Self {
        FiltrosNegocios {
            municipio: f.municipio,
            sector: f.sector,
            estado_web: f.estado_web.as_deref().and_then(EstadoWeb::parse),
            prioridad: f.prioridad,
            q: f.q,
            ordenar: f.ordenar,
            dir: f.dir,
            pagina: f.pagina,
            tam: f.tam,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExportQuery {
    pub formato: Option<String>,
    pub prioridad: Option<String>,
    pub municipio: Option<String>,
    pub sector: Option<String>,
    pub q: Option<String>,
}

pub struct ApiError(pub StatusCode, pub String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(ErrorApi { error: self.1 })).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

fn err500(e: anyhow::Error) -> ApiError {
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
async fn list_negocios(State(st): State<AppState>, Query(q): Query<FiltrosQuery>) -> ApiResult<Json<PaginaNegocios>> {
    let conn = st.store.lock().unwrap();
    let p = store::list_negocios(&conn, &q.into()).map_err(err500)?;
    Ok(Json(PaginaNegocios { items: p.items, total: p.total, pagina: p.pagina, tam: p.tam }))
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
async fn get_negocio(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<NegocioDetalle>> {
    let conn = st.store.lock().unwrap();
    match store::detalle_negocio(&conn, id).map_err(err500)? {
        Some(d) => Ok(Json(d)),
        None => Err(ApiError(StatusCode::NOT_FOUND, format!("negocio {id} no existe"))),
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
        return Err(ApiError(StatusCode::NOT_FOUND, format!("negocio {id} no existe")));
    }
    let p = store::set_pipeline(&conn, id, &req).map_err(err500)?;
    Ok(Json(p))
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
        return Err(ApiError(StatusCode::NOT_FOUND, format!("negocio {id} no existe")));
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
async fn list_eventos(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Vec<Evento>>> {
    let conn = st.store.lock().unwrap();
    Ok(Json(store::list_eventos(&conn, id).map_err(err500)?))
}

// ---------- jobs: ejecutar búsquedas desde la app web (ADR-002) ----------

fn nuevo_job(tipo: TipoJob, params: serde_json::Value) -> Job {
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
    }
}

fn actualizar_job(st: &AppState, job: &mut Job) {
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
async fn lanzar_scan(State(st): State<AppState>, Json(params): Json<ScanParams>) -> ApiResult<(StatusCode, Json<Job>)> {
    let job = nuevo_job(TipoJob::Scan, serde_json::to_value(&params).unwrap_or_default());
    {
        let conn = st.store.lock().unwrap();
        store::insert_job(&conn, &job).map_err(err500)?;
    }
    let respuesta = job.clone();
    let municipio = params.municipio.clone();
    let sector = params.sector.clone();
    let st2 = st.clone();
    tokio::spawn(async move {
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
        match radar_core::fuentes::overpass::scan_municipio(&store2, &municipio, sector.as_deref()).await {
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
    });
    Ok((StatusCode::ACCEPTED, Json(respuesta)))
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
async fn lanzar_audit(State(st): State<AppState>, Json(params): Json<AuditParams>) -> ApiResult<(StatusCode, Json<Job>)> {
    let job = nuevo_job(TipoJob::Audit, serde_json::to_value(&params).unwrap_or_default());
    {
        let conn = st.store.lock().unwrap();
        store::insert_job(&conn, &job).map_err(err500)?;
    }
    let prioridad = params.prioridad.clone();
    let limite = params.limite;
    let respuesta = job.clone();
    let st2 = st.clone();
    tokio::spawn(async move {
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
        let negocios = {
            let conn = store2.lock().unwrap();
            match store::negocios_para_auditar(&conn, prioridad.as_deref(), limite) {
                Ok(v) => v,
                Err(e) => {
                    j.estado = EstadoJob::Error;
                    j.error = Some(format!("{e:#}"));
                    actualizar_job(&st2, &mut j);
                    return;
                }
            }
        };
        let total = negocios.len() as i64;
        j.total = total;
        actualizar_job(&st2, &mut j);
        let db_path = st2.db_path.clone();
        let job_id = j.id.clone();
        let resultados = radar_core::audit::auditar_lote(negocios, move |hechos, total| {
            if let Ok(s) = store::abrir(&db_path) {
                let conn = s.lock().unwrap();
                let mut jj = Job {
                    id: job_id.clone(),
                    tipo: TipoJob::Audit,
                    estado: EstadoJob::EnCurso,
                    params: serde_json::json!({}),
                    hechos,
                    total,
                    creado: store::ahora(),
                    actualizado: store::ahora(),
                    error: None,
                };
                jj.actualizado = store::ahora();
                let _ = store::update_job(&conn, &mut jj);
            }
        })
        .await;
        {
            let conn = store2.lock().unwrap();
            let ahora = store::ahora();
            for (id, r) in resultados {
                let _ = store::insert_auditoria(
                    &conn,
                    &Auditoria {
                        id: 0,
                        negocio_id: id,
                        fecha: ahora.clone(),
                        estado_web: r.estado_web,
                        score: r.score,
                        prioridad: r.prioridad,
                        senales: r.senales,
                    },
                );
                let _ = store::set_ultima_verificacion(&conn, id, &ahora);
            }
        }
        j.estado = EstadoJob::Terminado;
        j.hechos = total;
        actualizar_job(&st2, &mut j);
    });
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
async fn lanzar_watch(State(st): State<AppState>, Json(params): Json<WatchParams>) -> ApiResult<(StatusCode, Json<Job>)> {
    let job = nuevo_job(TipoJob::Watch, serde_json::to_value(&params).unwrap_or_default());
    {
        let conn = st.store.lock().unwrap();
        store::insert_job(&conn, &job).map_err(err500)?;
    }
    let respuesta = job.clone();
    let st2 = st.clone();
    tokio::spawn(async move {
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
        let r = match &params.csv {
            Some(path) => radar_core::fuentes::borme::watch_desde_csv(&store2, path),
            None => Err(anyhow::anyhow!("watch: falta --csv (fuente {} aún no implementada)", params.fuente)),
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
    });
    Ok((StatusCode::ACCEPTED, Json(respuesta)))
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
        None => Err(ApiError(StatusCode::NOT_FOUND, format!("job {id} no existe"))),
    }
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
            (String::from_utf8_lossy(&buf).into_owned(), "text/csv", "csv")
        }
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"radar.{ext}\""))
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
        list_negocios, get_negocio, set_pipeline, add_evento, list_eventos,
        lanzar_scan, lanzar_audit, lanzar_watch, list_jobs, get_job,
        export, stats,
        crate::auth::request_code, crate::auth::verify, crate::auth::me, crate::auth::logout
    ),
    components(schemas(
        Negocio, Auditoria, Pipeline, Evento, Job, Stats, ParEtiqueta,
        PaginaNegocios, NegocioDetalle, ScanParams, AuditParams, WatchParams,
        SetPipelineRequest, NuevoEvento, EstadoWeb, EstadoPipeline, TipoJob, EstadoJob, ErrorApi,
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
        .route("/api/negocios/{id}", get(get_negocio))
        .route("/api/negocios/{id}/pipeline", patch(set_pipeline))
        .route("/api/negocios/{id}/eventos", get(list_eventos).post(add_evento))
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/scan", post(lanzar_scan))
        .route("/api/jobs/audit", post(lanzar_audit))
        .route("/api/jobs/watch", post(lanzar_watch))
        .route("/api/jobs/{id}", get(get_job))
        .route("/api/export", get(export))
        .route("/api/stats", get(stats))
        .route_layer(middleware::from_fn_with_state(st.clone(), crate::auth::require_auth));

    Router::new().merge(public).merge(protected).with_state(st)
}

/// Sirve la API en 127.0.0.1:{puerto} y, si existe radar-ui/dist, el dashboard embebido.
pub async fn serve(db_path: &str, puerto: u16) -> anyhow::Result<()> {
    let store = store::abrir(db_path)?;
    let st = AppState { store, db_path: db_path.to_string() };
    let app = router(st).fallback_service(
        ServeDir::new("radar-ui/dist").not_found_service(ServeFile::new("radar-ui/dist/index.html")),
    );
    let addr = SocketAddr::from(([127, 0, 0, 1], puerto));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Radar sirviendo en http://{addr} (API: /api/*, contrato: /openapi.json)");
    println!("Auth: OTP por email para {:?}", crate::auth::emails_autorizados());
    axum::serve(listener, app).await?;
    Ok(())
}
