use crate::api::{err500, ApiError, ApiResult, AppState};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use radar_core::{model::*, store, technical, web};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/negocios/{id}/web", get(info).patch(confirm))
        .route("/api/negocios/{id}/web/descubrir", post(discover))
        .route(
            "/api/negocios/{id}/web/candidatas/{candidate}",
            delete(discard),
        )
        .route(
            "/api/negocios/{id}/auditoria-tecnica",
            get(report).post(launch),
        )
        .route(
            "/api/negocios/{id}/auditoria-tecnica/revisiones",
            get(reviews).put(save_review),
        )
        .route(
            "/api/negocios/{id}/diagnostico-visita",
            get(visit_state).put(save_visit_state),
        )
        .route(
            "/api/auditorias-tecnicas/{id}/archivo/{kind}",
            get(download),
        )
        .route(
            "/api/auditorias-tecnicas/{id}/archivo/{kind}/preview",
            get(preview),
        )
        .route("/api/jobs/discovery", post(bulk))
}

#[utoipa::path(get,path="/api/negocios/{id}/web",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=WebInfo,description="Web confirmada, candidatas y búsqueda disponible")))]
pub async fn info(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<WebInfo>> {
    let c = st.store.lock().unwrap();
    Ok(Json(web::info(&c, id).map_err(err500)?))
}
#[utoipa::path(patch,path="/api/negocios/{id}/web",params(("id"=i64,Path)),request_body=ConfirmarWeb,tag="negocios",responses((status=200,body=WebInfo,description="Web confirmada; auditoría programada"),(status=400,body=ErrorApi,description="URL no válida")))]
pub async fn confirm(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<ConfirmarWeb>,
) -> ApiResult<Json<WebInfo>> {
    let result = {
        let c = st.store.lock().unwrap();
        web::confirmed(
            &c,
            id,
            &req.url,
            "correccion_usuario",
            req.municipio.as_deref(),
            req.direccion.as_deref(),
        )
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
        technical::enqueue(&c, id).map_err(err500)?;
        web::info(&c, id).map_err(err500)?
    };
    tokio::spawn(async move {
        if let Err(e) = crate::technical_jobs::refresh_light(&st, id).await {
            eprintln!("Auditoría rápida {id}: {e:#}");
        }
    });
    Ok(Json(result))
}
#[utoipa::path(post,path="/api/negocios/{id}/web/descubrir",params(("id"=i64,Path)),tag="jobs",responses((status=202,body=Job,description="Búsqueda de web programada")))]
pub async fn discover(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    let job = {
        let c = st.store.lock().unwrap();
        if store::get_negocio(&c, id).map_err(err500)?.is_none() {
            return Err(ApiError(
                StatusCode::NOT_FOUND,
                "Negocio no encontrado".into(),
            ));
        }
        technical::discovery_job(
            &c,
            &BusquedaWebParams {
                negocio_id: Some(id),
                pendientes: false,
            },
        )
        .map_err(err500)?
    };
    let result = job.clone();
    tokio::spawn(crate::technical_jobs::discovery(st, job));
    Ok((StatusCode::ACCEPTED, Json(result)))
}
#[utoipa::path(post,path="/api/jobs/discovery",request_body=BusquedaWebParams,tag="jobs",responses((status=202,body=Job,description="Descubrimiento reanudable")))]
pub async fn bulk(
    State(st): State<AppState>,
    Json(params): Json<BusquedaWebParams>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    let job = {
        let c = st.store.lock().unwrap();
        if store::tipo_job_activo(&c, TipoJob::Discovery).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Ya hay un descubrimiento en curso".into(),
            ));
        }
        technical::discovery_job(&c, &params).map_err(err500)?
    };
    let result = job.clone();
    tokio::spawn(crate::technical_jobs::discovery(st, job));
    Ok((StatusCode::ACCEPTED, Json(result)))
}
#[utoipa::path(delete,path="/api/negocios/{id}/web/candidatas/{candidate}",params(("id"=i64,Path),("candidate"=i64,Path)),tag="negocios",responses((status=204,description="Sugerencia descartada")))]
pub async fn discard(
    State(st): State<AppState>,
    Path((id, candidate)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    let c = st.store.lock().unwrap();
    if !web::discard(&c, id, candidate).map_err(err500)? {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Candidata no encontrada".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/negocios/{id}/auditoria-tecnica",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=EstadoAuditoriaTecnica,description="Informes por motor y dispositivo")))]
pub async fn report(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<EstadoAuditoriaTecnica>> {
    let c = st.store.lock().unwrap();
    Ok(Json(technical::state(&c, id).map_err(err500)?))
}
#[utoipa::path(get,path="/api/negocios/{id}/auditoria-tecnica/revisiones",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=[RevisionHallazgo],description="Revisiones humanas persistidas de los hallazgos")))]
pub async fn reviews(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<RevisionHallazgo>>> {
    let c = st.store.lock().unwrap();
    if store::get_negocio(&c, id).map_err(err500)?.is_none() {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Negocio no encontrado".into(),
        ));
    }
    Ok(Json(technical::finding_reviews(&c, id).map_err(err500)?))
}
#[utoipa::path(put,path="/api/negocios/{id}/auditoria-tecnica/revisiones",params(("id"=i64,Path)),request_body=ActualizarRevisionHallazgo,tag="negocios",responses((status=200,body=RevisionHallazgo,description="Estado de revisión guardado"),(status=400,body=ErrorApi,description="Revisión no válida")))]
pub async fn save_review(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<ActualizarRevisionHallazgo>,
) -> ApiResult<Json<RevisionHallazgo>> {
    let c = st.store.lock().unwrap();
    technical::update_finding_review(&c, id, &req)
        .map(Json)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))
}
#[utoipa::path(get,path="/api/negocios/{id}/diagnostico-visita",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=EstadoDiagnosticoVisita,description="Biblioteca de capturas y selección para una visita"),(status=404,description="Negocio no encontrado")))]
pub async fn visit_state(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    let c = st.store.lock().unwrap();
    if store::get_negocio(&c, id).map_err(err500)?.is_none() {
        return Err(ApiError(StatusCode::NOT_FOUND, "Negocio no encontrado".into()));
    }
    Ok(Json(technical::visit_state(&c, id).map_err(err500)?))
}
#[utoipa::path(put,path="/api/negocios/{id}/diagnostico-visita",params(("id"=i64,Path)),request_body=GuardarDiagnosticoVisita,tag="negocios",responses((status=200,body=EstadoDiagnosticoVisita,description="Selección y notas guardadas"),(status=400,description="Captura o estado no válido")))]
pub async fn save_visit_state(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let c = st.store.lock().unwrap();
    let previous = technical::visit_state(&c, id).map_err(err500)?;
    let result = technical::update_visit_state(&c, id, &body)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
    crate::technical_jobs::sync_pinned_captures(&c, id, &previous, &result).map_err(err500)?;
    Ok(Json(result))
}
#[utoipa::path(post,path="/api/negocios/{id}/auditoria-tecnica",params(("id"=i64,Path)),tag="jobs",responses((status=202,body=Job,description="Auditoría técnica programada"),(status=400,description="Confirma primero una URL propia",body=ErrorApi)))]
pub async fn launch(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    let c = st.store.lock().unwrap();
    Ok((
        StatusCode::ACCEPTED,
        Json(
            technical::enqueue(&c, id)
                .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?,
        ),
    ))
}
#[utoipa::path(get,path="/api/auditorias-tecnicas/{id}/archivo/{kind}",params(("id"=String,Path),("kind"=String,Path)),tag="negocios",responses((status=200,description="Informe original",body=String),(status=404,description="Original caducado o no disponible",body=ErrorApi)))]
pub async fn download(
    State(st): State<AppState>,
    Path((id, kind)): Path<(String, String)>,
) -> ApiResult<Response> {
    use std::io::Read;
    let (path, name) = crate::technical_jobs::artifact(&st.store, &id, &kind).map_err(|_| {
        ApiError(
            StatusCode::NOT_FOUND,
            "Informe original no disponible o caducado; los resultados normalizados se conservan."
                .into(),
        )
    })?;
    let file = std::fs::File::open(path).map_err(|e| err500(e.into()))?;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(file)
        .take(25 * 1024 * 1024)
        .read_to_end(&mut out)
        .map_err(|e| err500(e.into()))?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                if kind == "html" {
                    "text/html; charset=utf-8"
                } else if kind.starts_with("responsive-") {
                    "image/png"
                } else {
                    "application/json"
                }
                .to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
            (header::CACHE_CONTROL, "private, no-store".into()),
        ],
        out,
    )
        .into_response())
}

#[utoipa::path(get,path="/api/auditorias-tecnicas/{id}/archivo/{kind}/preview",params(("id"=String,Path),("kind"=String,Path)),tag="negocios",responses((status=200,description="Vista previa aislada del informe HTML original",body=String),(status=404,body=ErrorApi)))]
pub async fn preview(
    State(st): State<AppState>,
    Path((id, kind)): Path<(String, String)>,
) -> ApiResult<Response> {
    use std::io::Read;
    if kind != "html" {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Solo se previsualizan informes HTML.".into(),
        ));
    }
    let (path, _) = crate::technical_jobs::artifact(&st.store, &id, &kind).map_err(|_| {
        ApiError(
            StatusCode::NOT_FOUND,
            "Informe HTML original no disponible o caducado.".into(),
        )
    })?;
    let file = std::fs::File::open(path).map_err(|e| err500(e.into()))?;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(file)
        .take(25 * 1024 * 1024)
        .read_to_end(&mut out)
        .map_err(|e| err500(e.into()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
            (header::CONTENT_DISPOSITION, "inline".to_string()),
            (header::CACHE_CONTROL, "private, no-store".to_string()),
            (header::CONTENT_SECURITY_POLICY, "sandbox allow-scripts; default-src 'self' data: blob: 'unsafe-inline'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline' data:; script-src 'self' 'unsafe-inline' data:; font-src 'self' data:; connect-src 'none'; form-action 'none'; base-uri 'none'".to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        out,
    )
        .into_response())
}
