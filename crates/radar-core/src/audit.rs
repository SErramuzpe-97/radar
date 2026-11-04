//! Auditoría web ligera: 1 GET por web, timeout 8 s, UA identificable (diseño §6.2).

use std::time::{Duration, Instant};

use chrono::Datelike;
use serde_json::{json, Value};

use crate::model::{EstadoWeb, Negocio};
use crate::scoring::{self, puntuar};

pub struct ResultadoAuditoria {
    pub estado_web: EstadoWeb,
    pub score: i64,
    pub prioridad: String,
    pub senales: Value,
}

const UA: &str = "RadarNavarra/0.1 (+auditoria web B2B local; 1 GET por web)";

fn merge(base: &Value, extra: Value) -> Value {
    let mut m = base.clone();
    if let (Some(mobj), Some(eobj)) = (m.as_object_mut(), extra.as_object()) {
        for (k, v) in eobj {
            mobj.insert(k.clone(), v.clone());
        }
    }
    m
}

pub async fn auditar_negocio(neg: &Negocio) -> ResultadoAuditoria {
    let base = json!({
        "web": neg.web,
        "telefono": neg.telefono,
        "email": neg.email,
        "sector": neg.sector,
        "municipio": neg.municipio,
    });

    let Some(web) = neg.web.clone() else {
        // SIN_WEB es hipótesis a verificar in situ (diseño §5, regla de oro)
        let senales = merge(&base, json!({"hipotesis_sin_web": true}));
        let (score, prioridad) = puntuar(EstadoWeb::SinWeb, &senales, neg);
        return ResultadoAuditoria { estado_web: EstadoWeb::SinWeb, score, prioridad, senales };
    };

    if scoring::es_red_social(&web) {
        let senales = merge(&base, json!({"red_social": web}));
        let (score, prioridad) = puntuar(EstadoWeb::SoloRedes, &senales, neg);
        return ResultadoAuditoria { estado_web: EstadoWeb::SoloRedes, score, prioridad, senales };
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent(UA)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let senales = merge(&base, json!({"error": e.to_string()}));
            let (score, prioridad) = puntuar(EstadoWeb::WebRota, &senales, neg);
            return ResultadoAuditoria { estado_web: EstadoWeb::WebRota, score, prioridad, senales };
        }
    };

    let inicio = Instant::now();
    let resp = client.get(&web).send().await;
    let elapsed_ms = inicio.elapsed().as_millis() as i64;

    let (status, url_final, body) = match resp {
        Err(e) => {
            let senales = merge(&base, json!({"error": e.to_string(), "elapsed_ms": elapsed_ms}));
            let (score, prioridad) = puntuar(EstadoWeb::WebRota, &senales, neg);
            return ResultadoAuditoria { estado_web: EstadoWeb::WebRota, score, prioridad, senales };
        }
        Ok(r) => {
            let status = r.status().as_u16();
            let url_final = r.url().to_string();
            let body = r.text().await.unwrap_or_default();
            (status, url_final, body)
        }
    };

    let body_short: String = body.chars().take(200_000).collect();
    let lower = body_short.to_lowercase();
    let https_nativo = url_final.starts_with("https://");
    let viewport = scoring::tiene_viewport(&body_short);
    let gen = scoring::generator(&body_short);
    let anio_copy = scoring::anio_copyright(&body_short);
    let ahora_anio = chrono::Local::now().year() as i64;
    let copyright_viejo = anio_copy.map(|a| a <= ahora_anio - 3).unwrap_or(false);
    let parking = scoring::es_parking(&body_short, &url_final);
    let redir_red = scoring::redirige_a_red(&url_final);
    let email_vis = scoring::email_visible(&body_short);
    let titulo = regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>")
        .ok()
        .and_then(|re| re.captures(&body_short).and_then(|c| c.get(1)).map(|m| m.as_str().trim().to_string()));
    let sin_meta_desc = !lower.contains("name=\"description\"") && !lower.contains("name='description'");
    let html_bytes = body.len();

    let senales = merge(
        &base,
        json!({
            "status": status,
            "url_final": url_final,
            "https_nativo": https_nativo,
            "sin_https": !https_nativo,
            "sin_viewport": !viewport,
            "generator": gen,
            "constructor_abandonado": scoring::constructor_abandonado(&gen),
            "copyright_anio": anio_copy,
            "copyright_viejo": copyright_viejo,
            "parking": parking,
            "redirige_a_red": redir_red,
            "email_visible": email_vis,
            "sin_meta_description": sin_meta_desc,
            "html_bytes": html_bytes,
            "titulo": titulo,
            "elapsed_ms": elapsed_ms,
            "lenta": elapsed_ms > 3000,
        }),
    );

    let estado = clasificar(status, &url_final, &body_short, https_nativo, viewport, copyright_viejo, &gen, html_bytes, elapsed_ms);
    let (score, prioridad) = puntuar(estado, &senales, neg);
    ResultadoAuditoria { estado_web: estado, score, prioridad, senales }
}

#[allow(clippy::too_many_arguments)]
fn clasificar(
    status: u16,
    url_final: &str,
    html: &str,
    https_nativo: bool,
    viewport: bool,
    copyright_viejo: bool,
    gen: &Option<String>,
    html_bytes: usize,
    elapsed_ms: i64,
) -> EstadoWeb {
    if status >= 400 {
        return EstadoWeb::WebRota;
    }
    if scoring::redirige_a_red(url_final) {
        return EstadoWeb::SoloRedes;
    }
    if scoring::es_parking(html, url_final) {
        return EstadoWeb::WebAparcada;
    }
    let obsoleta =
        !https_nativo || !viewport || copyright_viejo || scoring::constructor_abandonado(gen) || html_bytes < 2000;
    if obsoleta {
        return EstadoWeb::WebObsoleta;
    }
    let mejorable = elapsed_ms > 3000 || !html.to_lowercase().contains("name=\"description\"");
    if mejorable {
        return EstadoWeb::WebMejorable;
    }
    EstadoWeb::WebOk
}

/// Audita una lista de negocios con concurrencia limitada (16).
/// Devuelve (negocio_id, resultado) para que el llamador persista en SQLite.
pub async fn auditar_lote(
    negocios: Vec<Negocio>,
    progreso: impl Fn(i64, i64) + Send + Sync + 'static,
) -> Vec<(i64, ResultadoAuditoria)> {
    let total = negocios.len() as i64;
    let progreso = std::sync::Arc::new(progreso);
    let mut set = tokio::task::JoinSet::new();
    let mut hechos = 0i64;
    let mut out = Vec::new();
    for neg in negocios {
        let p = progreso.clone();
        let id = neg.id;
        set.spawn(async move {
            let r = auditar_negocio(&neg).await;
            (id, r, p)
        });
        while set.len() >= 16 {
            if let Some(res) = set.join_next().await {
                if let Ok((id, r, p)) = res {
                    hechos += 1;
                    p(hechos, total);
                    out.push((id, r));
                }
            }
        }
    }
    while let Some(res) = set.join_next().await {
        if let Ok((id, r, p)) = res {
            hechos += 1;
            p(hechos, total);
            out.push((id, r));
        }
    }
    out
}
