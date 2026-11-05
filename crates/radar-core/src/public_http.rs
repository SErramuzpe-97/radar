//! HTTP de páginas públicas con DNS fijado y validación en cada redirección.
use anyhow::{bail, Context, Result};
use reqwest::Url;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

pub fn normalize(input: &str) -> Result<Url> {
    let s = input.trim();
    let full = if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{s}")
    };
    let mut url = Url::parse(&full).context("URL inválida")?;
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || ![80, 443].contains(&url.port_or_known_default().unwrap_or(0))
    {
        bail!("Solo se permiten webs públicas HTTP(S) sin credenciales, en puertos 80/443");
    }
    let host = url.host_str().unwrap();
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".local")
        || host.ends_with(".localhost")
    {
        bail!("No se permiten direcciones locales");
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        if !public_ip(ip) {
            bail!("No se permiten redes privadas o reservadas");
        }
    }
    url.set_fragment(None);
    Ok(url)
}
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let a = v.octets();
            !v.is_private()
                && !v.is_loopback()
                && !v.is_link_local()
                && !v.is_multicast()
                && !v.is_unspecified()
                && a[0] != 0
                && a[0] < 224
                && a[0] != 127
                && !(a[0] == 100 && (64..128).contains(&a[1]))
                && !(a[0] == 192 && a[1] == 0 && a[2] == 0)
                && !v.is_documentation()
                && !(a[0] == 198 && (a[1] == 18 || a[1] == 19))
        }
        IpAddr::V6(v) => {
            if let Some(a) = v.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(a));
            }
            let s = v.segments();
            !v.is_loopback()
                && !v.is_unspecified()
                && !v.is_multicast()
                && (s[0] & 0xfe00) != 0xfc00
                && (s[0] & 0xffc0) != 0xfe80
                && !(s[0] == 0x2001 && s[1] == 0xdb8)
                && (s[0] & 0xe000) == 0x2000
        }
    }
}
#[derive(Debug)]
pub struct Page {
    pub url: String,
    pub status: u16,
    pub body: String,
    pub headers: reqwest::header::HeaderMap,
    pub elapsed_ms: i64,
    pub truncated: bool,
}
pub async fn fetch(input: &str, timeout: u64) -> Result<Page> {
    let start = std::time::Instant::now();
    let mut url = normalize(input)?;
    for _ in 0..6 {
        let host = url
            .host_str()
            .context("falta dominio")?
            .trim_matches(['[', ']'])
            .to_string();
        let port = url.port_or_known_default().unwrap();
        let addresses = tokio::net::lookup_host((host.as_str(), port))
            .await?
            .collect::<Vec<SocketAddr>>();
        if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
            bail!("El dominio no resuelve exclusivamente a IP públicas");
        }
        let client = reqwest::Client::builder()
            .user_agent(
                "RadarNavarra/0.2 (public website audit; https://radar-navarra.duckdns.org)",
            )
            .timeout(Duration::from_secs(timeout))
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(&host, &addresses)
            .build()?;
        let mut response = client.get(url.clone()).send().await?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .context("Redirección sin destino")?
                .to_str()?;
            url = normalize(url.join(location)?.as_str())?;
            continue;
        }
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let mut bytes = Vec::new();
        let mut truncated = false;
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                truncated = true;
                break;
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(Page {
            url: url.to_string(),
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
            headers,
            elapsed_ms: start.elapsed().as_millis() as i64,
            truncated,
        });
    }
    bail!("Demasiadas redirecciones")
}
pub async fn robots_allowed(url: &str) -> Result<bool> {
    let u = normalize(url)?;
    let robots = u.join("/robots.txt")?;
    let p = fetch(robots.as_str(), 12).await?;
    if p.status == 404 {
        return Ok(true);
    }
    if !(200..300).contains(&p.status) {
        bail!("robots.txt respondió HTTP {}", p.status);
    }
    Ok(
        robotstxt::DefaultMatcher::default().one_agent_allowed_by_robots(
            &p.body,
            "RadarNavarra",
            u.as_str(),
        ),
    )
}
