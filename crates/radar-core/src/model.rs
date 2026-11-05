//! Modelos de datos compartidos (SQLite + API). Mapean 1:1 con el documento de diseño §8.5.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EstadoWeb {
    SinWeb,
    NoVerificable,
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
            EstadoWeb::WebOk | EstadoWeb::NoVerificable => "seguimiento",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EstadoWeb::SinWeb => "SIN_WEB",
            EstadoWeb::NoVerificable => "NO_VERIFICABLE",
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
            "NO_VERIFICABLE" => EstadoWeb::NoVerificable,
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
pub enum ServicioWeb {
    CreacionWeb,
    Landing,
    SeoOnPage,
    Hosting,
}

impl ServicioWeb {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CreacionWeb => "creacion_web",
            Self::Landing => "landing",
            Self::SeoOnPage => "seo_on_page",
            Self::Hosting => "hosting",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "creacion_web" => Self::CreacionWeb,
            "landing" => Self::Landing,
            "seo_on_page" => Self::SeoOnPage,
            "hosting" => Self::Hosting,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EstadoOportunidad {
    Pendiente,
    Propuesta,
    Contratada,
    Descartada,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OportunidadServicio {
    pub negocio_id: i64,
    pub servicio: ServicioWeb,
    pub score: i64,
    pub estado: EstadoOportunidad,
    pub motivo: String,
    pub fecha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SetOportunidadRequest {
    pub estado: EstadoOportunidad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TipoJob {
    Scan,
    Audit,
    Watch,
    Export,
    Crawl,
    Rebuild,
    Discovery,
    Technical,
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
    /// Clasificación comercial detallada; sector conserva la agrupación general.
    #[serde(default)]
    pub rubro: Option<String>,
    /// Evidencias de actividad, tarifa y presencia web registradas por la fuente.
    #[serde(default)]
    pub evidencia_rubro: Option<serde_json::Value>,
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
    #[serde(default)]
    pub estado_web_lista: String,
    #[serde(default)]
    pub estado_auditoria_tecnica: String,
    pub fuente: String,
    pub fuente_id: String,
    pub primera_vista: String,
    #[serde(default)]
    pub ultima_verificacion: Option<String>,
    #[serde(default)]
    pub score_potencial: Option<i64>,
    #[serde(default)]
    pub prioridad_comercial: Option<String>,
    #[serde(default)]
    pub estado_pipeline: Option<EstadoPipeline>,
    #[serde(default)]
    pub oportunidades: Vec<OportunidadServicio>,
    #[serde(default)]
    pub descartado: bool,
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
    pub siguiente_accion_fecha: Option<String>,
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
    pub rubro: Option<String>,
    pub tipo_entidad: Option<String>,
    pub estado_web: Option<EstadoWeb>,
    pub estado_auditoria_tecnica: Option<String>,
    pub estado_pipeline: Option<String>,
    pub prioridad: Option<String>,
    pub q: Option<String>,
    /// score | nombre | municipio | fecha
    pub ordenar: Option<String>,
    /// asc | desc
    pub dir: Option<String>,
    pub pagina: Option<i64>,
    pub tam: Option<i64>,
    pub mostrar_descartados: bool,
    pub sur: Option<f64>,
    pub oeste: Option<f64>,
    pub norte: Option<f64>,
    pub este: Option<f64>,
}

/// Extensión geográfica de una zona o agrupación (WGS84).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct ZonaMapa {
    pub sur: f64,
    pub oeste: f64,
    pub norte: f64,
    pub este: f64,
}

/// Solo los datos necesarios para pintar un punto o una agrupación.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PuntoMapa {
    pub id: Option<i64>,
    pub nombre: Option<String>,
    pub sector: Option<String>,
    pub lat: f64,
    pub lon: f64,
    pub cantidad: i64,
    pub score_potencial: Option<i64>,
    pub zona: ZonaMapa,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RespuestaMapa {
    pub items: Vec<PuntoMapa>,
    /// Empresas con coordenadas válidas dentro de la zona solicitada, sin truncar.
    pub total: i64,
    pub zona: Option<ZonaMapa>,
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
    #[serde(default)]
    pub borradores: Vec<BorradorContacto>,
    pub web_info: WebInfo,
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
    pub conversion_por_servicio: Vec<ConversionServicio>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConversionServicio {
    pub servicio: ServicioWeb,
    pub resueltas: i64,
    pub contratadas: i64,
    pub tasa: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Fuente {
    pub id: i64,
    pub nombre: String,
    pub tipo: String,
    pub url: String,
    pub condiciones: Option<String>,
    pub atribucion: Option<String>,
    pub revisada_en: Option<String>,
    pub habilitada: bool,
    pub profundidad: i64,
    pub max_paginas_dia: i64,
    pub ultima_ejecucion: Option<String>,
    pub error_ultima_ejecucion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NuevaFuente {
    pub nombre: String,
    /// semilla, busqueda o dataset
    pub tipo: String,
    pub url: String,
    pub condiciones: String,
    pub atribucion: String,
    pub revisada_en: String,
    #[serde(default = "profundidad_default")]
    pub profundidad: i64,
    #[serde(default = "paginas_dia_default")]
    pub max_paginas_dia: i64,
    #[serde(default)]
    pub habilitada: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CambiarFuenteRequest {
    pub habilitada: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CrawlParams {
    pub fuente_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FuenteCandidata {
    pub id: i64,
    pub nombre: String,
    pub url: String,
    pub fuente_padre_id: Option<i64>,
    pub descubrimiento: String,
    pub creada: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EstadoCandidataRequest {
    pub estado: String,
}

fn profundidad_default() -> i64 {
    2
}
fn paginas_dia_default() -> i64 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CandidatoDuplicado {
    pub id: i64,
    pub negocio_a: i64,
    pub nombre_a: String,
    pub negocio_b: i64,
    pub nombre_b: String,
    pub razon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FusionDuplicado {
    pub conservar_id: i64,
    pub fusionar_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TareaSeguimiento {
    pub negocio_id: i64,
    pub nombre: String,
    pub municipio: String,
    pub siguiente_accion: String,
    pub fecha: String,
    pub prioridad: Option<String>,
    pub estado: EstadoPipeline,
    #[serde(default)]
    pub servicios: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CoberturaMunicipio {
    pub municipio: String,
    pub negocios: i64,
    pub auditados: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PlantillaContacto {
    pub id: i64,
    pub nombre: String,
    pub canal: String,
    pub servicio: Option<ServicioWeb>,
    pub asunto: Option<String>,
    pub cuerpo: String,
    pub actualizado: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NuevaPlantillaContacto {
    pub nombre: String,
    pub canal: String,
    pub servicio: Option<ServicioWeb>,
    pub asunto: Option<String>,
    pub cuerpo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NuevoBorrador {
    pub negocio_id: i64,
    pub plantilla_id: Option<i64>,
    pub canal: String,
    pub destino: Option<String>,
    pub asunto: Option<String>,
    pub cuerpo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BorradorContacto {
    pub id: i64,
    pub negocio_id: i64,
    pub canal: String,
    pub destino: Option<String>,
    pub asunto: Option<String>,
    pub cuerpo: String,
    pub creado: String,
    pub actualizado: String,
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
    #[serde(default)]
    pub resumen: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScanParams {
    #[serde(default)]
    pub navarra: bool,
    #[serde(default)]
    pub municipio: Option<String>,
    #[serde(default)]
    pub municipios: Vec<String>,
    #[serde(default)]
    pub sector: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActualizarDescartados {
    pub ids: Vec<i64>,
    pub descartado: bool,
    #[serde(default)]
    pub motivo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResultadoDescartados {
    pub actualizados: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct AuditParams {
    /// Auditar únicamente registros sin auditoría (reconstrucción).
    #[serde(default)]
    pub solo_sin_auditar: bool,
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
    pub siguiente_accion_fecha: Option<String>,
    #[serde(default)]
    pub limpiar_fecha_siguiente_accion: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResumenClasificacion {
    pub examinados: i64,
    pub admitidos: i64,
    pub excluidos: i64,
    pub pendientes: i64,
    pub motivos: Vec<ParEtiqueta>,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebCandidata {
    pub id: i64,
    pub url: String,
    pub titulo: Option<String>,
    pub tipo: String,
    pub proveedor: String,
    pub consulta: Option<String>,
    pub posicion: Option<i64>,
    pub fecha: String,
    pub estado: String,
    pub evidencias: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebInfo {
    pub estado: String,
    #[serde(default)]
    pub url_confirmada: Option<String>,
    pub origen: Option<String>,
    pub ultima_busqueda: Option<String>,
    pub consulta: String,
    pub aviso: Option<String>,
    pub buscador_configurado: bool,
    pub creditos_restantes: i64,
    pub candidatas: Vec<WebCandidata>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfirmarWeb {
    pub url: String,
    pub municipio: Option<String>,
    pub direccion: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditoriaTecnica {
    pub id: String,
    pub job_id: String,
    pub negocio_id: i64,
    pub url: String,
    pub motor: String,
    pub dispositivo: String,
    pub version: String,
    pub estado: String,
    pub fecha: String,
    pub resultado: serde_json::Value,
    pub archivos: Vec<String>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EstadoAuditoriaTecnica {
    pub informes: Vec<AuditoriaTecnica>,
    pub job: Option<Job>,
    pub worker_disponible: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RevisionHallazgo {
    pub report_id: String,
    pub finding_key: String,
    pub estado: String,
    pub motivo: Option<String>,
    pub actualizado: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ActualizarRevisionHallazgo {
    pub report_id: String,
    pub finding_key: String,
    pub estado: String,
    pub motivo: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CapturaVisita {
    pub capture_id: String,
    pub estado: String,
    pub orden: i64,
    pub titulo: String,
    pub que: String,
    pub impacto: String,
    pub propuesta: String,
    pub punto_x: Option<f64>,
    pub punto_y: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CapturaCatalogoVisita {
    pub capture_id: String,
    pub report_id: String,
    pub key: String,
    pub width: u16,
    pub page: String,
    pub fecha: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EstadoDiagnosticoVisita {
    pub capturas: Vec<CapturaVisita>,
    pub actualizado: Option<String>,
    pub catalogo: Vec<CapturaCatalogoVisita>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GuardarDiagnosticoVisita {
    pub capturas: Vec<CapturaVisita>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct BusquedaWebParams {
    pub negocio_id: Option<i64>,
    #[serde(default)]
    pub pendientes: bool,
}
