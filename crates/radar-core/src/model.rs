//! Modelos de datos compartidos (SQLite + API). Mapean 1:1 con el documento de diseño §8.5.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EstadoWeb {
    SinWeb,
    SoloRedes,
    WebRota,
    WebAparcada,
    WebObsoleta,
    WebMejorable,
    WebOk,
}

impl EstadoWeb {
    /// Oferta comercial asociada (diseño §6.1).
    pub fn oferta(&self) -> &'static str {
        match self {
            EstadoWeb::SinWeb | EstadoWeb::SoloRedes => "crear",
            EstadoWeb::WebRota | EstadoWeb::WebAparcada | EstadoWeb::WebObsoleta => "rehacer",
            EstadoWeb::WebMejorable => "actualizar",
            EstadoWeb::WebOk => "seguimiento",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EstadoWeb::SinWeb => "SIN_WEB",
            EstadoWeb::SoloRedes => "SOLO_REDES",
            EstadoWeb::WebRota => "WEB_ROTA",
            EstadoWeb::WebAparcada => "WEB_APARCADA",
            EstadoWeb::WebObsoleta => "WEB_OBSOLETA",
            EstadoWeb::WebMejorable => "WEB_MEJORABLE",
            EstadoWeb::WebOk => "WEB_OK",
        }
    }

    pub fn parse(s: &str) -> Option<EstadoWeb> {
        Some(match s {
            "SIN_WEB" => EstadoWeb::SinWeb,
            "SOLO_REDES" => EstadoWeb::SoloRedes,
            "WEB_ROTA" => EstadoWeb::WebRota,
            "WEB_APARCADA" => EstadoWeb::WebAparcada,
            "WEB_OBSOLETA" => EstadoWeb::WebObsoleta,
            "WEB_MEJORABLE" => EstadoWeb::WebMejorable,
            "WEB_OK" => EstadoWeb::WebOk,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EstadoPipeline {
    Nuevo,
    Contactado,
    VisitaHecha,
    Interesado,
    NoInteresado,
    Propuesta,
    Cerrado,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TipoJob {
    Scan,
    Audit,
    Watch,
    Export,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EstadoJob {
    Pendiente,
    EnCurso,
    Terminado,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Negocio {
    pub id: i64,
    pub nombre: String,
    #[serde(default)]
    pub categoria: Option<String>,
    #[serde(default)]
    pub sector: Option<String>,
    #[serde(default)]
    pub direccion: Option<String>,
    pub municipio: String,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
    #[serde(default)]
    pub telefono: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub web: Option<String>,
    pub fuente: String,
    pub fuente_id: String,
    pub primera_vista: String,
    #[serde(default)]
    pub ultima_verificacion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Auditoria {
    pub id: i64,
    pub negocio_id: i64,
    pub fecha: String,
    pub estado_web: EstadoWeb,
    pub score: i64,
    pub prioridad: String,
    pub senales: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Pipeline {
    pub negocio_id: i64,
    pub estado: EstadoPipeline,
    #[serde(default)]
    pub valor_estimado: Option<f64>,
    #[serde(default)]
    pub siguiente_accion: Option<String>,
    #[serde(default)]
    pub canal: Option<String>,
    pub fecha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Evento {
    pub id: i64,
    pub negocio_id: i64,
    pub fecha: String,
    pub tipo: String,
    #[serde(default)]
    pub resultado: Option<String>,
    #[serde(default)]
    pub notas: Option<String>,
}

/// Filtros de búsqueda de negocios (API GET /api/negocios y exports).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FiltrosNegocios {
    pub municipio: Option<String>,
    pub sector: Option<String>,
    pub estado_web: Option<EstadoWeb>,
    pub prioridad: Option<String>,
    pub q: Option<String>,
    /// score | nombre | municipio | fecha
    pub ordenar: Option<String>,
    /// asc | desc
    pub dir: Option<String>,
    pub pagina: Option<i64>,
    pub tam: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagina<T> {
    pub items: Vec<T>,
    pub total: i64,
    pub pagina: i64,
    pub tam: i64,
}

/// Respuesta plana de GET /api/negocios/{id} (negocio + últimas auditorías + pipeline + eventos).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NegocioDetalle {
    pub negocio: Negocio,
    pub auditorias: Vec<Auditoria>,
    pub pipeline: Option<Pipeline>,
    pub eventos: Vec<Evento>,
}

/// GET /api/negocios → página de negocios (schema propio, sin genéricos, para el OAS).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PaginaNegocios {
    pub items: Vec<Negocio>,
    pub total: i64,
    pub pagina: i64,
    pub tam: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Stats {
    pub total_negocios: i64,
    pub por_estado_web: Vec<ParEtiqueta>,
    pub por_prioridad: Vec<ParEtiqueta>,
    pub por_pipeline: Vec<ParEtiqueta>,
    pub eventos_ultimos_7_dias: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ParEtiqueta {
    pub etiqueta: String,
    pub total: i64,
}

// ---- Jobs (ejecución de búsquedas desde la app web, ADR-002) ----

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Job {
    pub id: String,
    pub tipo: TipoJob,
    pub estado: EstadoJob,
    pub params: serde_json::Value,
    pub hechos: i64,
    pub total: i64,
    pub creado: String,
    pub actualizado: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScanParams {
    pub municipio: String,
    #[serde(default)]
    pub sector: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct AuditParams {
    #[serde(default)]
    pub prioridad: Option<String>,
    #[serde(default)]
    pub limite: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WatchParams {
    /// Fuente a vigilar. `borme` consulta el sumario diario oficial del BOE; `openmercantil` es alias compatible.
    pub fuente: String,
    /// CSV manual opcional. Si se omite, descarga las publicaciones recientes del BORME.
    #[serde(default)]
    pub csv: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct SetPipelineRequest {
    #[serde(default)]
    pub estado: Option<EstadoPipeline>,
    #[serde(default)]
    pub valor_estimado: Option<f64>,
    #[serde(default)]
    pub siguiente_accion: Option<String>,
    #[serde(default)]
    pub canal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NuevoEvento {
    pub tipo: String,
    #[serde(default)]
    pub resultado: Option<String>,
    #[serde(default)]
    pub notas: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ErrorApi {
    pub error: String,
}

// ---- Auth OTP por email (ADR-005: exposición duckdns con login por código) ----

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthVerify {
    pub email: String,
    /// Código de 6 dígitos enviado por email
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthMe {
    pub email: String,
    pub expira: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MensajeOk {
    pub message: String,
}
