//! Scoring 0-100 y prioridad A/B/C (diseño §7).

use serde_json::Value;

use crate::model::{EstadoWeb, Negocio};

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
        EstadoWeb::SinWeb => 60 + if neg.telefono.is_some() { 8 } else { 0 } + if neg.email.is_some() { 5 } else { 0 },
        EstadoWeb::WebObsoleta => {
            45 + if senal_bool(senales, "sin_https") { 15 } else { 0 }
                + if senal_bool(senales, "sin_viewport") { 25 } else { 0 }
                + if senal_bool(senales, "copyright_viejo") { 15 } else { 0 }
                + if senal_bool(senales, "constructor_abandonado") { 10 } else { 0 }
        }
        EstadoWeb::WebMejorable => 35,
        EstadoWeb::WebOk => 10,
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

/// Año extraído de un texto tipo copyright (para tests y auditoría).
pub fn anio_copyright(texto: &str) -> Option<i64> {
    let re = regex::Regex::new(r"(?:©|&copy;|&#169;|Derechos reservados)[^0-9]{0,20}(20\d{2})").ok()?;
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
        "sedo", "domain for sale", "this domain is for sale", "compra este dominio",
        "dominio en venta", "buy this domain", "parkingcrew", "hugedomains",
    ]
    .iter()
    .any(|s| lower.contains(s) || url_lower.contains(s))
}

/// ¿La web "real" acaba en una red social?
pub fn redirige_a_red(url_final: &str) -> bool {
    let u = url_final.to_lowercase();
    ["facebook.com", "instagram.com", "tiktok.com", "twitter.com", "x.com", "linkedin.com"]
        .iter()
        .any(|d| u.contains(d))
}

/// Dominio de una URL o texto tipo dominio.
pub fn es_red_social(web: &str) -> bool {
    redirige_a_red(&format!("https://{web}"))
}

pub fn email_visible(html: &str) -> Option<String> {
    let re = regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").ok()?;
    re.find(html).map(|m| m.as_str().to_string())
}

pub fn tiene_viewport(html: &str) -> bool {
    html.to_lowercase().contains("name=\"viewport\"") || html.to_lowercase().contains("name='viewport'")
}

pub fn generator(html: &str) -> Option<String> {
    let re = regex::Regex::new(r#"<meta[^>]+name=["']generator["'][^>]+content=["']([^"']+)"#).ok()?;
    re.captures(html).and_then(|c| c.get(1)).map(|m| m.as_str().to_lowercase())
}

pub fn constructor_abandonado(gen: &Option<String>) -> bool {
    matches!(
        gen.as_deref(),
        Some(g) if ["wix", "jimdo", "1&1", "ionos", "webnode", "godaddy", "squarespace"].iter().any(|c| g.contains(c))
    )
}
