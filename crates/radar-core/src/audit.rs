//! Auditoría web ligera: 1 GET por web, timeout 8 s, UA identificable (diseño §6.2).

use serde_json::{json, Value};

use crate::model::{EstadoWeb, Negocio};
use crate::scoring::{self, puntuar};

pub struct ResultadoAuditoria {
    pub estado_web: EstadoWeb,
    pub score: i64,
    pub prioridad: String,
    pub senales: Value,
}

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
        return ResultadoAuditoria {
            estado_web: EstadoWeb::SinWeb,
            score,
            prioridad,
            senales,
        };
    };

    if scoring::es_red_social(&web) {
        let senales = merge(&base, json!({"red_social": web}));
        let (score, prioridad) = puntuar(EstadoWeb::SoloRedes, &senales, neg);
        return ResultadoAuditoria {
            estado_web: EstadoWeb::SoloRedes,
            score,
            prioridad,
            senales,
        };
    }

    let page = match crate::public_http::fetch(&web, 8).await {
        Ok(p) => p,
        Err(e) => {
            let senales = merge(&base, json!({"error_temporal":true,"error":e.to_string()}));
            let (score, prioridad) = puntuar(EstadoWeb::NoVerificable, &senales, neg);
            return ResultadoAuditoria {
                estado_web: EstadoWeb::NoVerificable,
                score,
                prioridad,
                senales,
            };
        }
    };
    let (status, url_final, body, elapsed_ms) =
        (page.status, page.url, page.body, page.elapsed_ms as i64);
    let body_short: String = body.chars().take(200_000).collect();
    let lower = body_short.to_lowercase();
    let https_nativo = url_final.starts_with("https://");
    let viewport = scoring::tiene_viewport(&body_short);
    let gen = scoring::generator(&body_short);
    let anio_copy = scoring::anio_copyright(&body_short);
    let copyright_viejo = false; // Un año no demuestra abandono.
    let parking = scoring::es_parking(&body_short, &url_final);
    let redir_red = scoring::redirige_a_red(&url_final);
    let email_vis = scoring::email_visible(&body_short);
    let titulo = regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>")
        .ok()
        .and_then(|re| {
            re.captures(&body_short)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
        });
    let sin_meta_desc =
        !lower.contains("name=\"description\"") && !lower.contains("name='description'");
    let sin_title = titulo.as_deref().map(|t| t.is_empty()).unwrap_or(true);
    let sin_h1 = !lower.contains("<h1");
    let tiene_cta = [
        "contacto",
        "reservar",
        "reserva",
        "presupuesto",
        "pedir cita",
        "llamar",
        "comprar",
    ]
    .iter()
    .any(|t| lower.contains(t));
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
            "constructor_abandonado": false,
            "copyright_anio": anio_copy,
            "copyright_viejo": copyright_viejo,
            "parking": parking,
            "redirige_a_red": redir_red,
            "email_visible": email_vis,
            "sin_meta_description": sin_meta_desc,
            "sin_title": sin_title,
            "sin_h1": sin_h1,
            "tiene_cta": tiene_cta,
            "html_bytes": html_bytes,
            "titulo": titulo,
            "elapsed_ms": elapsed_ms,
            "lenta": elapsed_ms > 3000,
        }),
    );

    let estado = clasificar(
        status,
        &url_final,
        &body_short,
        https_nativo,
        viewport,
        copyright_viejo,
        &gen,
        html_bytes,
        elapsed_ms,
    );
    let (score, prioridad) = puntuar(estado, &senales, neg);
    ResultadoAuditoria {
        estado_web: estado,
        score,
        prioridad,
        senales,
    }
}

#[allow(clippy::too_many_arguments)]
fn clasificar(
    status: u16,
    url_final: &str,
    html: &str,
    https_nativo: bool,
    viewport: bool,
    _copyright_viejo: bool,
    _gen: &Option<String>,
    _html_bytes: usize,
    elapsed_ms: i64,
) -> EstadoWeb {
    if status == 403 || status == 429 || status >= 500 {
        return EstadoWeb::NoVerificable;
    }
    if status >= 400 {
        return EstadoWeb::WebRota;
    }
    if scoring::redirige_a_red(url_final) {
        return EstadoWeb::SoloRedes;
    }
    if scoring::es_parking(html, url_final) {
        return EstadoWeb::WebAparcada;
    }
    let obsoleta = !https_nativo || !viewport;
    if obsoleta {
        return EstadoWeb::WebObsoleta;
    }
    let mejorable = elapsed_ms > 3000 || !html.to_lowercase().contains("name=\"description\"");
    if mejorable {
        return EstadoWeb::WebMejorable;
    }
    EstadoWeb::WebOk
}

/// Audita una lista de negocios con concurrencia limitada (8).
/// Devuelve (negocio_id, resultado) para que el llamador persista en SQLite.
pub async fn auditar_lote(
    negocios: Vec<Negocio>,
    progreso: impl Fn(i64, i64) + Send + Sync + 'static,
) -> Vec<(i64, ResultadoAuditoria)> {
    let limite = std::env::var("RADAR_AUDIT_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(8)
        .clamp(1, 32);
    auditar_lote_con_limite(negocios, limite, progreso).await
}

pub async fn auditar_lote_con_limite(
    negocios: Vec<Negocio>,
    limite: usize,
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
        while set.len() >= limite.max(1) {
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

#[cfg(test)]
mod reliability_tests {
    use super::*;
    #[test]
    fn constructor_y_copyright_no_demuestran_abandono() {
        assert_eq!(
            clasificar(
                200,
                "https://example.com",
                "<meta name=\"description\">",
                true,
                true,
                true,
                &Some("wix".into()),
                100,
                100
            ),
            EstadoWeb::WebOk
        );
    }
    #[test]
    fn fallos_temporales_no_demuestran_web_rota() {
        for code in [403, 429, 500, 503] {
            assert_eq!(
                clasificar(
                    code,
                    "https://example.com",
                    "",
                    true,
                    true,
                    false,
                    &None,
                    0,
                    100
                ),
                EstadoWeb::NoVerificable
            );
        }
    }
}
