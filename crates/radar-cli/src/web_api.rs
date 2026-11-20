use crate::api::{err500, ApiError, ApiResult, AppState};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use radar_core::{model::*, store, technical, web};
use serde_json::json;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/negocios/{id}/web", get(info).patch(confirm))
        .route("/api/negocios/{id}/seo-competidores", get(seo_list))
        .route(
            "/api/negocios/{id}/seo-competidores/sugerencias",
            post(seo_suggest),
        )
        .route(
            "/api/negocios/{id}/seo-competidores/{analysis_id}",
            get(seo_get),
        )
        .route(
            "/api/negocios/{id}/seo-competidores/{analysis_id}/analizar",
            post(seo_start),
        )
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
            "/api/negocios/{id}/ia-informe",
            get(ai_report)
                .post(generate_ai_report)
                .put(save_ai_report_revision),
        )
        .route(
            "/api/negocios/{id}/ia-revisiones",
            axum::routing::put(save_ai_review),
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

#[utoipa::path(get,path="/api/negocios/{id}/seo-competidores",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=[SeoComparacion])))]
pub async fn seo_list(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<SeoComparacion>>> {
    let c = st.store.lock().unwrap();
    Ok(Json(
        radar_core::seo_competitors::list(&c, id).map_err(err500)?,
    ))
}

#[utoipa::path(get,path="/api/negocios/{id}/seo-competidores/{analysis_id}",params(("id"=i64,Path),("analysis_id"=String,Path)),tag="negocios",responses((status=200,body=SeoComparacion),(status=404,body=ErrorApi)))]
pub async fn seo_get(
    State(st): State<AppState>,
    Path((id, analysis_id)): Path<(i64, String)>,
) -> ApiResult<Json<SeoComparacion>> {
    let c = st.store.lock().unwrap();
    radar_core::seo_competitors::get(&c, id, &analysis_id)
        .map_err(err500)?
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Análisis SEO no encontrado".into()))
}

#[utoipa::path(post,path="/api/negocios/{id}/seo-competidores/sugerencias",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=SeoComparacion),(status=400,body=ErrorApi),(status=503,body=ErrorApi)))]
pub async fn seo_suggest(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<SeoComparacion>> {
    if !radar_core::seo_competitors::enabled() {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "La comparación SEO está desactivada".into(),
        ));
    }
    radar_core::seo_competitors::sugerir(&st.store, id)
        .await
        .map(Json)
        .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))
}

#[utoipa::path(post,path="/api/negocios/{id}/seo-competidores/{analysis_id}/analizar",params(("id"=i64,Path),("analysis_id"=String,Path)),request_body=SeleccionarSeoCompetidores,tag="negocios",responses((status=202,body=Job),(status=400,body=ErrorApi),(status=404,body=ErrorApi),(status=409,body=ErrorApi)))]
pub async fn seo_start(
    State(st): State<AppState>,
    Path((id, analysis_id)): Path<(i64, String)>,
    Json(req): Json<SeleccionarSeoCompetidores>,
) -> ApiResult<(StatusCode, Json<Job>)> {
    if !radar_core::seo_competitors::enabled() {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "La comparación SEO está desactivada".into(),
        ));
    }
    let job = {
        let c = st.store.lock().unwrap();
        if store::tipo_job_activo(&c, TipoJob::SeoCompare).map_err(err500)? {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "Ya hay una comparación SEO activa".into(),
            ));
        }
        let prior = radar_core::seo_competitors::get(&c, id, &analysis_id)
            .map_err(err500)?
            .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Análisis SEO no encontrado".into()))?;
        let current = web::huella_evaluacion(&c, id).map_err(err500)?;
        if prior.resultado["huella_entrada"].as_str() != Some(current.as_str()) {
            return Err(ApiError(
                StatusCode::CONFLICT,
                "La ficha cambió desde que se propusieron competidores; vuelve a buscarlos".into(),
            ));
        }
        radar_core::seo_competitors::guardar_seleccion(&c, id, &analysis_id, &req.competidores)
            .map_err(|e| ApiError(StatusCode::BAD_REQUEST, e.to_string()))?;
        let now = store::ahora();
        let job = Job {
            id: uuid::Uuid::new_v4().to_string(),
            tipo: TipoJob::SeoCompare,
            estado: EstadoJob::Pendiente,
            params: json!({"analysis_id":analysis_id,"negocio_id":id}),
            hechos: 0,
            total: 0,
            creado: now.clone(),
            actualizado: now,
            error: None,
            resumen: None,
        };
        store::insert_job(&c, &job).map_err(err500)?;
        radar_core::seo_competitors::enlazar_job(&c, &analysis_id, &job.id).map_err(err500)?;
        job
    };
    let response = job.clone();
    tokio::spawn(crate::technical_jobs::seo_compare(st, job));
    Ok((StatusCode::ACCEPTED, Json(response)))
}

#[utoipa::path(get,path="/api/negocios/{id}/ia-informe",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=InformeIA,description="Último borrador generado con modelo gratuito"),(status=404,description="Todavía no hay informe")))]
pub async fn ai_report(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<InformeIA>> {
    let c = st.store.lock().unwrap();
    web::ultimo_informe_ia(&c, id)
        .map_err(err500)?
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Todavía no hay informe IA".into()))
}

#[utoipa::path(post,path="/api/negocios/{id}/ia-informe",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=InformeIA,description="Borrador basado en auditorías técnicas vigentes"),(status=400,body=ErrorApi,description="No hay web confirmada o auditorías utilizables")))]
pub async fn generate_ai_report(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<InformeIA>> {
    let (name, url, evidence) = {
        let c = st.store.lock().unwrap();
        let negocio = store::get_negocio(&c, id)
            .map_err(err500)?
            .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Negocio no encontrado".into()))?;
        let url = negocio.web.clone().ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                "Confirma una web antes de generar el informe".into(),
            )
        })?;
        let current:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM web_estado WHERE negocio_id=?1 AND url_confirmada=?2 AND ((estado='confirmada' AND origen IN ('correccion_usuario','confirmacion_usuario')) OR (estado='verificada_automatica' AND origen='verificacion_automatica'))) AND NOT EXISTS(SELECT 1 FROM negocios_descartados WHERE negocio_id=?1)",rusqlite::params![id,url],|r|r.get(0)).map_err(|e|err500(e.into()))?;
        if !current {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "La web no está confirmada o el negocio está descartado".into(),
            ));
        }
        let evidence = web::evidencia_auditoria_para_ia(&c, id, &url).map_err(err500)?;
        if evidence["informes"].as_array().is_none_or(Vec::is_empty) {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "No hay auditorías técnicas completas o parciales para esta web".into(),
            ));
        }
        if !web::reservar_credito_ia(&c).map_err(err500)? {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "Se agotaron las solicitudes gratuitas de IA de hoy; se reanudarán mañana".into(),
            ));
        }
        (negocio.nombre, url, evidence)
    };
    let fingerprint = web::huella_valor(&evidence).map_err(err500)?;
    let (texto, modelo) = match radar_core::openrouter::report(&name, &url, &evidence).await {
        Ok(value) => value,
        Err(e) => {
            let c = st.store.lock().unwrap();
            web::registrar_ejecucion_ia(
                &c,
                id,
                "informe",
                &fingerprint,
                radar_core::openrouter::CHAT_MODEL,
                None,
                "error",
                &json!({}),
                &evidence,
                &json!({}),
                Some(&e.to_string()),
            )
            .map_err(err500)?;
            return Err(ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                format!("No se generó el informe gratuito: {e}"),
            ));
        }
    };
    let c = st.store.lock().unwrap();
    let still_current:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM negocios n JOIN web_estado w ON w.negocio_id=n.id WHERE n.id=?1 AND n.web=?2 AND w.url_confirmada=?2 AND n.admisible=1 AND NOT EXISTS(SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=n.id))",rusqlite::params![id,url],|r|r.get(0)).map_err(|e|err500(e.into()))?;
    if !still_current {
        web::registrar_ejecucion_ia(
            &c,
            id,
            "informe",
            &fingerprint,
            radar_core::openrouter::CHAT_MODEL,
            Some(&modelo),
            "obsoleto",
            &json!({"texto":texto}),
            &evidence,
            &json!({}),
            None,
        )
        .map_err(err500)?;
        return Err(ApiError(
            StatusCode::CONFLICT,
            "La web cambió o el negocio se descartó durante la generación; vuelve a intentarlo"
                .into(),
        ));
    }
    let current_evidence = web::evidencia_auditoria_para_ia(&c, id, &url).map_err(err500)?;
    if web::huella_valor(&current_evidence).map_err(err500)? != fingerprint {
        web::registrar_ejecucion_ia(
            &c,
            id,
            "informe",
            &fingerprint,
            radar_core::openrouter::CHAT_MODEL,
            Some(&modelo),
            "obsoleto",
            &json!({"texto":texto}),
            &evidence,
            &json!({}),
            None,
        )
        .map_err(err500)?;
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Las auditorías cambiaron durante la generación; el borrador no se aplicó".into(),
        ));
    }
    let report =
        web::guardar_informe_ia(&c, id, &url, &modelo, &texto, &evidence).map_err(err500)?;
    web::registrar_ejecucion_ia(
        &c,
        id,
        "informe",
        &fingerprint,
        radar_core::openrouter::CHAT_MODEL,
        Some(&modelo),
        "aplicada",
        &json!({"texto":texto}),
        &evidence,
        &json!({}),
        None,
    )
    .map_err(err500)?;
    Ok(Json(report))
}

#[utoipa::path(put,path="/api/negocios/{id}/ia-informe",params(("id"=i64,Path)),request_body=GuardarRevisionInformeIA,tag="negocios",responses((status=200,body=InformeIA,description="Versión revisada del borrador IA guardada sin borrar el original"),(status=400,body=ErrorApi,description="Informe o texto no válido")))]
pub async fn save_ai_report_revision(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<GuardarRevisionInformeIA>,
) -> ApiResult<Json<InformeIA>> {
    let c = st.store.lock().unwrap();
    let current:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM negocios n JOIN web_estado w ON w.negocio_id=n.id JOIN informes_ia r ON r.negocio_id=n.id WHERE n.id=?1 AND n.web=r.url AND w.url_confirmada=r.url AND n.admisible=1 AND NOT EXISTS(SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=n.id))",[id],|r|r.get(0)).map_err(|e|err500(e.into()))?;
    if !current {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "El informe ya no corresponde a la web vigente del negocio".into(),
        ));
    }
    web::guardar_revision_informe_ia(&c, id, &req.texto)
        .map(Json)
        .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))
}

#[utoipa::path(get,path="/api/negocios/{id}/web",params(("id"=i64,Path)),tag="negocios",responses((status=200,body=WebInfo,description="Web confirmada, candidatas y búsqueda disponible")))]
pub async fn info(State(st): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<WebInfo>> {
    let c = st.store.lock().unwrap();
    Ok(Json(web::info(&c, id).map_err(err500)?))
}
#[utoipa::path(put,path="/api/negocios/{id}/ia-revisiones",params(("id"=i64,Path)),request_body=ActualizarRevisionIA,tag="negocios",responses((status=200,body=RevisionIA,description="Corrección o revisión humana persistida"),(status=400,body=ErrorApi,description="Valor de revisión no válido")))]
pub async fn save_ai_review(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<ActualizarRevisionIA>,
) -> ApiResult<Json<RevisionIA>> {
    let c = st.store.lock().unwrap();
    web::actualizar_revision_ia(&c, id, &req)
        .map(Json)
        .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))
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
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            "Negocio no encontrado".into(),
        ));
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
