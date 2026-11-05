//! Scoring 0-100 y prioridad A/B/C (diseño §7).

use serde_json::Value;

use crate::model::{EstadoOportunidad, EstadoWeb, Negocio, OportunidadServicio, ServicioWeb};

pub fn prioridad(score: i64) -> String {
    if score >= 70 {
        "A".into()
    } else if score >= 50 {
        "B".into()
    } else {
        "C".into()
    }
}

fn senal_bool(s: &Value, k: &str) -> bool {
    s.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Calcula (score, prioridad) a partir del estado web, las señales de la auditoría y el negocio.
pub fn puntuar(estado: EstadoWeb, senales: &Value, neg: &Negocio) -> (i64, String) {
    let mut score: i64 = match estado {
        EstadoWeb::SoloRedes | EstadoWeb::WebAparcada => 85,
        EstadoWeb::WebRota => 80,
        EstadoWeb::SinWeb => {
            if senal_bool(senales, "busqueda_completada") {
                60
            } else {
                40
            }
        }
        EstadoWeb::WebObsoleta => {
            45 + if senal_bool(senales, "sin_https") {
                15
            } else {
                0
            } + if senal_bool(senales, "sin_viewport") {
                25
            } else {
                0
            }
        }
        EstadoWeb::WebMejorable => 35,
        EstadoWeb::WebOk | EstadoWeb::NoVerificable => 10,
    };

    // Multiplicadores (diseño §7)
    if neg.sector.as_deref() == Some("hosteleria") {
        score += 10;
    }
    if neg.municipio.eq_ignore_ascii_case("pamplona") {
        score += 5;
    }
    if senal_bool(senales, "posible_cadena") {
        score -= 20;
    }

    let score = score.clamp(0, 100);
    (score, prioridad(score))
}

/// Encaje comercial estimado por servicio (heurístico; no es una probabilidad de cierre).
pub fn puntuar_servicios(
    negocio_id: i64,
    estado: EstadoWeb,
    senales: &Value,
    fecha: &str,
) -> Vec<OportunidadServicio> {
    let existe_web = !matches!(estado, EstadoWeb::SinWeb | EstadoWeb::SoloRedes);
    let sin_auditar = senal_bool(senales, "sin_auditar") || estado == EstadoWeb::NoVerificable;
    let mut crear = match estado {
        EstadoWeb::SinWeb => {
            if senal_bool(senales, "busqueda_completada") {
                60
            } else {
                40
            }
        }
        EstadoWeb::SoloRedes => 88,
        EstadoWeb::WebRota | EstadoWeb::WebAparcada => 84,
        EstadoWeb::WebObsoleta => 72,
        EstadoWeb::WebMejorable => 48,
        EstadoWeb::WebOk => 15,
        EstadoWeb::NoVerificable => 20,
    };
    if sin_auditar && existe_web {
        crear = 20;
    }
    if senal_bool(senales, "posible_cadena") {
        crear -= 20;
    }

    let landing = if sin_auditar && existe_web {
        20
    } else if !existe_web {
        if estado == EstadoWeb::SinWeb {
            if senal_bool(senales, "busqueda_completada") {
                60
            } else {
                40
            }
        } else {
            72
        }
    } else {
        (18 + if !senal_bool(senales, "tiene_cta") {
            28
        } else {
            0
        } + if senal_bool(senales, "sin_meta_description") {
            12
        } else {
            0
        } + if senal_bool(senales, "sin_h1") { 10 } else { 0 }
            + if senal_bool(senales, "lenta") { 8 } else { 0 })
        .min(76)
    };
    let visita_pagada_sin_web = estado == EstadoWeb::SinWeb
        && senal_bool(senales, "busqueda_completada")
        && senal_bool(senales, "visita_con_tarifa_declarada");
    let landing = if visita_pagada_sin_web {
        landing.max(78)
    } else {
        landing
    };
    let seo = if sin_auditar || !existe_web {
        0
    } else {
        (18 + if senal_bool(senales, "sin_title") {
            20
        } else {
            0
        } + if senal_bool(senales, "sin_meta_description") {
            20
        } else {
            0
        } + if senal_bool(senales, "sin_h1") { 15 } else { 0 }
            + if senal_bool(senales, "sin_viewport") {
                15
            } else {
                0
            }
            + if senal_bool(senales, "sin_https") {
                10
            } else {
                0
            })
        .min(98)
    };
    let hosting = if sin_auditar || !existe_web {
        0
    } else if senal_bool(senales, "sin_https") {
        30
    } else {
        12
    };

    let razones = [
        (
            ServicioWeb::CreacionWeb,
            crear,
            if sin_auditar && existe_web {
                "Todavía no hay auditoría técnica para valorar una renovación web."
            } else {
                match estado {
            EstadoWeb::SinWeb => "Web no localizada: encaje provisional; requiere confirmación.",
            EstadoWeb::NoVerificable => "No se pudo verificar la web; un fallo temporal no demuestra abandono.",
            EstadoWeb::SoloRedes => "La presencia detectada depende de redes sociales; una web propia daría un canal controlado.",
            EstadoWeb::WebRota => "La web no respondió correctamente; puede requerir rehacerla.",
            EstadoWeb::WebAparcada => "El dominio parece aparcado; puede necesitar una web nueva.",
            EstadoWeb::WebObsoleta => "La web presenta señales de antigüedad; valorar renovación.",
            EstadoWeb::WebMejorable => "La web funciona, aunque puede mejorarse.",
            EstadoWeb::WebOk => "La web parece funcional; menor encaje para una renovación completa.",
        }
            },
        ),
        (
            ServicioWeb::Landing,
            landing,
            if visita_pagada_sin_web {
                "La fuente indica visitas y tarifa; la búsqueda web se completó sin localizar web. Una landing podría reunir visitas, precio y reserva."
            } else if sin_auditar && existe_web {
                "Todavía no hay auditoría que permita valorar una landing."
            } else if !existe_web {
                "Una landing puede servir como primera presencia enfocada a captar contactos."
            } else if !senal_bool(senales, "tiene_cta") {
                "No se detectó una llamada a la acción clara en la página."
            } else {
                "Potencial complementario; conviene revisar el objetivo de conversión con el negocio."
            },
        ),
        (
            ServicioWeb::SeoOnPage,
            seo,
            if sin_auditar {
                "Todavía no hay auditoría técnica para valorar SEO on page."
            } else if !existe_web {
                "No hay una web propia que auditar para SEO on page."
            } else {
                "Comprobaciones HTML preliminares; se completan con los hallazgos técnicos de SiteOne."
            },
        ),
        (
            ServicioWeb::Hosting,
            hosting,
            if sin_auditar {
                "Todavía no hay auditoría de disponibilidad, HTTPS y rendimiento."
            } else if !existe_web {
                "No se estima hosting sin una web propia detectada."
            } else if senal_bool(senales, "sin_https") {
                "Se detectaron señales técnicas que justifican revisar hosting y configuración."
            } else {
                "Sin señales fuertes de problema de alojamiento en la auditoría actual."
            },
        ),
    ];
    razones
        .into_iter()
        .map(|(servicio, score, motivo)| OportunidadServicio {
            negocio_id,
            servicio,
            score: score.clamp(0, 100),
            estado: EstadoOportunidad::Pendiente,
            motivo: motivo.to_string(),
            fecha: fecha.to_string(),
        })
        .collect()
}

/// Año extraído de un texto tipo copyright (para tests y auditoría).
pub fn anio_copyright(texto: &str) -> Option<i64> {
    let re =
        regex::Regex::new(r"(?:©|&copy;|&#169;|Derechos reservados)[^0-9]{0,20}(20\d{2})").ok()?;
    re.captures(texto)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .filter(|y| (2000..=2100).contains(y))
}

/// ¿Parece un dominio aparcado / en venta?
pub fn es_parking(html: &str, url_final: &str) -> bool {
    let lower = html.to_lowercase();
    let url_lower = url_final.to_lowercase();
    [
        "sedo",
        "domain for sale",
        "this domain is for sale",
        "compra este dominio",
        "dominio en venta",
        "buy this domain",
        "parkingcrew",
        "hugedomains",
    ]
    .iter()
    .any(|s| lower.contains(s) || url_lower.contains(s))
}

/// ¿La web "real" acaba en una red social?
pub fn redirige_a_red(url_final: &str) -> bool {
    let candidate = if url_final.contains("://") {
        url_final.to_string()
    } else {
        format!("https://{url_final}")
    };
    let host = reqwest::Url::parse(&candidate)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_default();
    [
        "facebook.com",
        "instagram.com",
        "tiktok.com",
        "twitter.com",
        "x.com",
        "linkedin.com",
    ]
    .iter()
    .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Dominio de una URL o texto tipo dominio.
pub fn es_red_social(web: &str) -> bool {
    redirige_a_red(web)
}

pub fn email_visible(html: &str) -> Option<String> {
    let re = regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").ok()?;
    re.find(html).map(|m| m.as_str().to_string())
}

pub fn tiene_viewport(html: &str) -> bool {
    html.to_lowercase().contains("name=\"viewport\"")
        || html.to_lowercase().contains("name='viewport'")
}

pub fn generator(html: &str) -> Option<String> {
    let re =
        regex::Regex::new(r#"<meta[^>]+name=["']generator["'][^>]+content=["']([^"']+)"#).ok()?;
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_lowercase())
}

pub fn constructor_abandonado(gen: &Option<String>) -> bool {
    matches!(
        gen.as_deref(),
        Some(g) if ["wix", "jimdo", "1&1", "ionos", "webnode", "godaddy", "squarespace"].iter().any(|c| g.contains(c))
    )
}

#[cfg(test)]
mod rubro_tests {
    use super::*;

    #[test]
    fn tarifa_de_visita_solo_prioriza_landing_con_busqueda_web_completada() {
        let normal = puntuar_servicios(
            1,
            EstadoWeb::SinWeb,
            &serde_json::json!({"busqueda_completada":true}),
            "2026-11-05",
        );
        let priorizado = puntuar_servicios(
            1,
            EstadoWeb::SinWeb,
            &serde_json::json!({"busqueda_completada":true,"visita_con_tarifa_declarada":true}),
            "2026-11-05",
        );
        let landing_normal = normal
            .iter()
            .find(|o| o.servicio == ServicioWeb::Landing)
            .unwrap();
        let landing_target = priorizado
            .iter()
            .find(|o| o.servicio == ServicioWeb::Landing)
            .unwrap();
        assert!(landing_target.score > landing_normal.score);
        assert!(landing_target.motivo.contains("visitas y tarifa"));
        let no_verificada = puntuar_servicios(
            1,
            EstadoWeb::SinWeb,
            &serde_json::json!({"visita_con_tarifa_declarada":true}),
            "2026-11-05",
        );
        assert_eq!(
            no_verificada
                .iter()
                .find(|o| o.servicio == ServicioWeb::Landing)
                .unwrap()
                .score,
            40
        );
    }
}
