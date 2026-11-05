//! Auth OTP por email (ADR-005): allowlist de emails, código de 6 dígitos hasheado en SQLite,
//! envío por SMTP (variables RADAR_SMTP_*) o log en modo dev, sesión JWT (HS256) en cookie
//! HttpOnly + tabla `sesiones` para revocación. Protege /api/* salvo /api/auth/*.

use axum::{
    extract::{Extension, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use lettre::AsyncTransport;
use radar_core::model::*;
use radar_core::store::{self};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::{ApiError, AppState};

const SESSION_COOKIE: &str = "radar_session";
const CODE_TTL_SECS: i64 = 600; // 10 min
const SESSION_TTL_SECS: i64 = 7 * 24 * 3600; // 7 días
const MAX_CODES_POR_HORA: i64 = 5;

/// Emails autorizados (ADR-005). Por defecto los 4 del proyecto; se puede ampliar con
/// RADAR_AUTH_EMAILS="a@x,b@y".
pub fn emails_autorizados() -> Vec<String> {
    let default = "hello@carlotacarrillo.eu,sergio.erramuzpe@gmail.com,pezonetis@gmail.com,duarmeler@gmail.com";
    let lista = std::env::var("RADAR_AUTH_EMAILS").unwrap_or_else(|_| default.to_string());
    lista
        .split(',')
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty())
        .collect()
}

pub fn email_autorizado(email: &str) -> bool {
    emails_autorizados().contains(&email.trim().to_lowercase())
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    jti: String,
    iat: usize,
    exp: usize,
}

#[derive(Clone)]
pub struct AuthUser {
    pub email: String,
}

fn jwt_secret(st: &AppState) -> String {
    let conn = st.store.lock().unwrap();
    if let Ok(Some(s)) = store::meta_get(&conn, "jwt_secret") {
        return s;
    }
    let secret: String = (0..32)
        .map(|_| format!("{:02x}", rand::thread_rng().gen::<u8>()))
        .collect();
    let _ = store::meta_set(&conn, "jwt_secret", &secret);
    secret
}

fn emitir_sesion(st: &AppState, email: &str) -> Result<(String, String), ApiError> {
    let secret = jwt_secret(st); // lockea y suelta antes de volver a lockear abajo
    let (jti, expira) = {
        let conn = st.store.lock().unwrap();
        let jti = uuid::Uuid::new_v4().to_string();
        let expira = store::crear_sesion(&conn, &jti, email, SESSION_TTL_SECS)
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
        (jti, expira)
    };
    let ahora = chrono::Local::now().timestamp() as usize;
    let claims = Claims {
        sub: email.to_string(),
        jti,
        iat: ahora,
        exp: ahora + SESSION_TTL_SECS as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    Ok((token, expira))
}

fn extraer_token(req: &Request) -> Option<String> {
    // 1) Authorization: Bearer <jwt>
    if let Some(h) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(t) = h.strip_prefix("Bearer ") {
            return Some(t.trim().to_string());
        }
    }
    // 2) cookie radar_session
    if let Some(c) = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
    {
        for par in c.split(';') {
            let par = par.trim();
            if let Some(v) = par.strip_prefix(&format!("{SESSION_COOKIE}=")) {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn validar_token(st: &AppState, token: &str) -> Option<AuthUser> {
    let secret = jwt_secret(st);
    let datos = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()?;
    let claims = datos.claims;
    let conn = st.store.lock().unwrap();
    let (email, expira) = store::get_sesion(&conn, &claims.jti).ok()??;
    let exp = chrono::DateTime::parse_from_rfc3339(&expira)
        .ok()?
        .with_timezone(&chrono::Local);
    if exp < chrono::Local::now() {
        return None;
    }
    Some(AuthUser { email })
}

/// Middleware: exige sesión válida en /api/* (salvo /api/auth/*).
pub async fn require_auth(
    State(st): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = extraer_token(&req);
    match token.as_deref().and_then(|t| validar_token(&st, t)) {
        Some(user) => {
            req.extensions_mut().insert(user);
            Ok(next.run(req).await)
        }
        None => Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "sesión no válida o expirada".into(),
        )),
    }
}

async fn enviar_codigo(st: &AppState, email: &str, code: &str) -> Result<(), String> {
    let host = std::env::var("RADAR_SMTP_HOST").unwrap_or_default();
    if host.is_empty() {
        // Modo dev local: el código sale por log
        println!("[DEV] código de acceso para {email}: {code}");
        return Ok(());
    }
    let port: u16 = std::env::var("RADAR_SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(587);
    let user = std::env::var("RADAR_SMTP_USER").unwrap_or_default();
    let pass = std::env::var("RADAR_SMTP_PASS").unwrap_or_default();
    let from_addr = std::env::var("RADAR_SMTP_FROM").unwrap_or_else(|_| user.clone());
    let nombre = std::env::var("RADAR_SMTP_FROM_NAME").unwrap_or_else(|_| "Radar Navarra".into());

    let transport = if port == 465 {
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::relay(&host)
            .map_err(|e| e.to_string())?
            .port(465)
            .credentials(lettre::transport::smtp::authentication::Credentials::new(
                user, pass,
            ))
            .build()
    } else {
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::starttls_relay(&host)
            .map_err(|e| e.to_string())?
            .port(port)
            .credentials(lettre::transport::smtp::authentication::Credentials::new(
                user, pass,
            ))
            .build()
    };

    let msg = lettre::Message::builder()
        .from(format!("{nombre} <{from_addr}>").parse().map_err(|e| format!("from inválido: {e}"))?)
        .to(email.parse().map_err(|e| format!("to inválido: {e}"))?)
        .subject("Tu código de acceso a Radar")
        .body(format!(
            "Hola,\n\nTu código de acceso a Radar es: {code}\n\nCaduca en 10 minutos. Si no lo has pedido tú, ignora este email.\n\n— Radar Navarra"
        ))
        .map_err(|e| e.to_string())?;
    let _ = st;
    transport
        .send(msg)
        .await
        .map_err(|e| format!("SMTP: {e}"))?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/auth/request-code",
    tag = "auth",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Si el email está autorizado se envía un código (no se revela la allowlist)", body = MensajeOk),
        (status = 429, description = "Demasiados intentos", body = ErrorApi)
    )
)]
pub async fn request_code(
    State(st): State<AppState>,
    Json(req): Json<AuthRequest>,
) -> Result<Json<MensajeOk>, ApiError> {
    let email = req.email.trim().to_lowercase();
    let generic = Json(MensajeOk {
        message: "Si el email está autorizado, recibirás un código de acceso.".into(),
    });
    if !email_autorizado(&email) {
        return Ok(generic);
    }
    let code = {
        let conn = st.store.lock().unwrap();
        let recientes = store::count_auth_codes_recientes(&conn, &email, 3600)
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
        if recientes >= MAX_CODES_POR_HORA {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "Demasiados códigos solicitados; espera un rato.".into(),
            ));
        }
        let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000u32));
        let hash = sha256_hex(&code);
        store::crear_auth_code(&conn, &email, &hash, CODE_TTL_SECS)
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
        code
        // el guard del Mutex se suelta aquí, antes del .await
    };
    if let Err(e) = enviar_codigo(&st, &email, &code).await {
        eprintln!("error enviando código a {email}: {e}");
        return Err(ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "No se pudo enviar el email de acceso.".into(),
        ));
    }
    Ok(generic)
}

#[utoipa::path(
    post,
    path = "/api/auth/verify",
    tag = "auth",
    request_body = AuthVerify,
    responses(
        (status = 200, description = "Sesión creada (cookie radar_session HttpOnly)", body = AuthMe),
        (status = 401, description = "Código incorrecto o caducado", body = ErrorApi)
    )
)]
pub async fn verify(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<AuthVerify>,
) -> Result<Response, ApiError> {
    let email = req.email.trim().to_lowercase();
    let ok = {
        let conn = st.store.lock().unwrap();
        store::validar_auth_code(&conn, &email, &sha256_hex(&req.code))
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?
    };
    if !ok {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "Código incorrecto o caducado.".into(),
        ));
    }
    let (token, expira) = emitir_sesion(&st, &email)?;
    let https = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        == Some("https");
    let mut cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECS}"
    );
    if https {
        cookie.push_str("; Secure");
    }
    let body = Json(AuthMe { email, expira });
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::SET_COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&body.0).unwrap(),
        ))
        .unwrap())
}

#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "auth",
    responses(
        (status = 200, description = "Sesión actual", body = AuthMe),
        (status = 401, description = "No autenticado", body = ErrorApi)
    )
)]
pub async fn me(Extension(user): Extension<AuthUser>) -> Json<AuthMe> {
    let conn_expira = String::new();
    Json(AuthMe {
        email: user.email,
        expira: conn_expira,
    })
}

#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "auth",
    responses((status = 200, description = "Sesión cerrada", body = MensajeOk))
)]
pub async fn logout(State(st): State<AppState>, req: Request) -> Result<Response, ApiError> {
    if let Some(token) = extraer_token(&req) {
        if let Some(user) = validar_token(&st, &token) {
            // Leer el secreto antes de tomar el lock de SQLite: jwt_secret también consulta
            // meta y volver a llamarlo dentro del lock produciría un deadlock.
            let secret = jwt_secret(&st);
            let conn = st.store.lock().unwrap();
            if let Ok(datos) = jsonwebtoken::decode::<Claims>(
                &token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &Validation::default(),
            ) {
                let _ = store::borrar_sesion(&conn, &datos.claims.jti);
            }
            let _ = user;
        }
    }
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::SET_COOKIE,
            format!("{SESSION_COOKIE}=; Path=/; HttpOnly; Max-Age=0"),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&MensajeOk {
                message: "Sesión cerrada.".into(),
            })
            .unwrap(),
        ))
        .unwrap())
}
