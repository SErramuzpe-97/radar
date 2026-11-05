//! Admisión comercial por evidencias. No contiene nombres de marcas ni excepciones por empresa.
use crate::{
    model::Negocio,
    store::{self, Store},
};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, time::Duration};

pub const POLICY_VERSION: i64 = 1;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Evidence {
    pub commercial: bool,
    #[serde(default)]
    pub nonprofit_activity: bool,
    pub in_navarra: bool,
    pub inactive: bool,
    pub category: String,
    pub brand: Option<String>,
    pub operator: Option<String>,
    pub entities: Vec<String>,
    pub parent_organization: bool,
    pub branch: bool,
    #[serde(default)]
    pub web_urls: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub status: String,
    pub reason: String,
    pub evidence: Value,
}
impl Decision {
    pub fn admitted(&self) -> bool {
        self.status == "admitido"
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub description: String,
    pub websites: Vec<String>,
    pub employees: Option<f64>,
    pub branches: Option<f64>,
    pub parents: Vec<String>,
    pub network: bool,
    pub source: String,
}
#[derive(Default)]
pub struct Classifier {
    pub organizations: HashMap<String, Organization>,
}

pub fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter_map(|c| {
            Some(match c {
                'á' | 'à' | 'ä' | 'â' => 'a',
                'é' | 'è' | 'ë' | 'ê' => 'e',
                'í' | 'ì' | 'ï' | 'î' => 'i',
                'ó' | 'ò' | 'ö' | 'ô' => 'o',
                'ú' | 'ù' | 'ü' | 'û' => 'u',
                'ñ' => 'n',
                c if c.is_alphanumeric() => c,
                _ => return None,
            })
        })
        .collect()
}
fn words(value: &str) -> String {
    value
        .split(|c: char| !c.is_alphanumeric())
        .map(normalize)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn domain(s: &str) -> Option<String> {
    let u = reqwest::Url::parse(s).ok()?;
    let host = u.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    psl::domain_str(&host).map(str::to_owned)
}
fn decision(status: &str, reason: &str, e: &Evidence, extra: Value) -> Decision {
    Decision {
        status: status.into(),
        reason: reason.into(),
        evidence: json!({"entrada":e,"organizacion":extra,"politica":POLICY_VERSION}),
    }
}
impl Classifier {
    pub fn evaluate(&self, n: &Negocio, e: &Evidence) -> Decision {
        if !e.commercial && !e.nonprofit_activity {
            return decision(
                "excluido",
                "sin_actividad_comercial_demostrada",
                e,
                json!(null),
            );
        }
        if !e.in_navarra {
            return decision(
                "pendiente",
                "ubicacion_navarra_no_demostrada",
                e,
                json!(null),
            );
        }
        if e.inactive {
            return decision("excluido", "actividad_cerrada_o_abandonada", e, json!(null));
        }
        let mut linked = Vec::new();
        for key in e
            .entities
            .iter()
            .cloned()
            .chain(e.brand.iter().map(|s| format!("name:{}", normalize(s))))
            .chain(e.operator.iter().map(|s| format!("name:{}", normalize(s))))
        {
            if let Some(org) = self.organizations.get(&key) {
                let verified = !key.starts_with("name:")
                    || n.web.as_deref().and_then(domain).is_some_and(|d| {
                        org.websites.iter().any(|w| domain(w).as_ref() == Some(&d))
                    });
                if verified {
                    linked.push(org);
                }
            }
        }
        // Un dominio coincidente vincula organizaciones descubiertas en el mismo barrido;
        // nunca agrupar por dominios de directorios ni por parecido del nombre de la tienda.
        if let Some(d) = n.web.as_deref().and_then(domain) {
            for org in self.organizations.values() {
                if org.websites.iter().any(|w| {
                    reqwest::Url::parse(w).is_ok_and(|u| u.path() == "/")
                        && domain(w).as_ref() == Some(&d)
                }) && !linked.iter().any(|o| o.id == org.id)
                {
                    linked.push(org);
                }
            }
        }
        for _ in 0..2 {
            let parents = linked
                .iter()
                .flat_map(|o| o.parents.iter())
                .filter_map(|id| self.organizations.get(id))
                .filter(|org| !linked.iter().any(|o| o.id == org.id))
                .collect::<Vec<_>>();
            linked.extend(parents);
        }
        for org in &linked {
            let scale =
                org.employees.is_some_and(|v| v >= 250.) || org.branches.is_some_and(|v| v >= 20.);
            let network_link = org.network
                || e.brand.is_some()
                || e.branch
                || e.parent_organization
                || n.web
                    .as_deref()
                    .and_then(domain)
                    .is_some_and(|d| org.websites.iter().any(|w| domain(w).as_ref() == Some(&d)));
            if network_link && scale && !org.websites.is_empty() {
                return decision(
                    "excluido",
                    "red_comercial_grande_con_presencia_corporativa_central",
                    e,
                    json!(org),
                );
            }
        }
        // Una coincidencia con la identidad de una organización grande descubierta
        // dinámicamente es una señal para contrastar, nunca prueba suficiente para excluir.
        let name = format!(" {} ", words(&n.nombre));
        for org in self.organizations.values() {
            let large =
                org.employees.is_some_and(|v| v >= 250.) || org.branches.is_some_and(|v| v >= 20.);
            if !large || org.websites.is_empty() || linked.iter().any(|o| o.id == org.id) {
                continue;
            }
            if std::iter::once(&org.label)
                .chain(org.aliases.iter())
                .any(|alias| {
                    let alias = words(alias);
                    !alias.is_empty()
                        && (name == format!(" {alias} ")
                            || (normalize(&alias).len() >= 4
                                && name.contains(&format!(" {alias} "))))
                })
            {
                return decision(
                    "pendiente",
                    "posible_vinculo_corporativo_sin_confirmar",
                    e,
                    json!({"identidad_posible":org,"requiere":"confirmación de pertenencia u homonimia"}),
                );
            }
        }
        let grouped =
            e.parent_organization || e.branch || !e.entities.is_empty() || e.brand.is_some();
        if grouped || linked.iter().any(|o| o.network) {
            // La ausencia de tamaño no demuestra independencia ni que una red sea pequeña.
            let small = linked.iter().any(|o| {
                o.branches.is_some_and(|v| v > 0. && v < 20.)
                    && o.employees.is_some_and(|v| v > 0. && v < 250.)
            });
            if !small {
                return decision(
                    "pendiente",
                    "autonomia_comercial_no_demostrada",
                    e,
                    json!(linked),
                );
            }
        }
        decision(
            "admitido",
            if grouped {
                "grupo_pequeno_con_dimension_documentada"
            } else {
                "actividad_local_sin_vinculo_corporativo_detectado"
            },
            e,
            json!(linked),
        )
    }
}

pub fn record(
    conn: &Connection,
    n: &Negocio,
    e: &Evidence,
    classifier: &Classifier,
    url: &str,
) -> Result<Decision> {
    let d = classifier.evaluate(n, e);
    conn.execute("INSERT INTO clasificacion_ingesta(fuente,external_id,nombre,estado,motivo,evidencias,url,observado_en,version) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(fuente,external_id) DO UPDATE SET nombre=excluded.nombre,estado=excluded.estado,motivo=excluded.motivo,evidencias=excluded.evidencias,url=excluded.url,observado_en=excluded.observado_en,version=excluded.version",params![n.fuente,n.fuente_id,n.nombre,d.status,d.reason,d.evidence.to_string(),url,store::ahora(),POLICY_VERSION])?;
    conn.execute(
        "UPDATE negocios SET admisible=?3 WHERE fuente=?1 AND fuente_id=?2",
        params![n.fuente, n.fuente_id, d.admitted()],
    )?;
    Ok(d)
}

fn values(entity: &Value, p: &str) -> Vec<Value> {
    entity["claims"][p]
        .as_array()
        .map(|claims| {
            claims
                .iter()
                .filter(|c| c["rank"] != "deprecated")
                .filter_map(|c| c.pointer("/mainsnak/datavalue/value").cloned())
                .collect()
        })
        .unwrap_or_default()
}
fn quantity(entity: &Value, p: &str) -> Option<f64> {
    // Preferir la observación con fecha más reciente, no el máximo histórico.
    let claims = entity["claims"][p].as_array()?;
    claims
        .iter()
        .filter(|c| c["rank"] != "deprecated")
        .filter_map(|c| {
            let amount = c
                .pointer("/mainsnak/datavalue/value/amount")?
                .as_str()?
                .parse::<f64>()
                .ok()?;
            let date = c
                .pointer("/qualifiers/P585/0/datavalue/value/time")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            Some((date, amount))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|v| v.1)
}
fn ids(entity: &Value, p: &str) -> Vec<String> {
    values(entity, p)
        .iter()
        .filter_map(|v| v["id"].as_str().map(str::to_owned))
        .collect()
}
fn organization(entity: &Value, id: &str, types: &[Value]) -> Organization {
    let desc = entity
        .pointer("/descriptions/en/value")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    let mut description = desc.clone();
    for ty in types {
        if let Some(label) = ty.pointer("/labels/en/value").and_then(Value::as_str) {
            description.push(' ');
            description.push_str(&label.to_lowercase());
        }
    }
    let branches = quantity(entity, "P8368");
    Organization {
        id: id.into(),
        aliases: entity["aliases"]
            .as_object()
            .into_iter()
            .flat_map(|m| m.values())
            .filter_map(Value::as_array)
            .flatten()
            .filter_map(|a| a["value"].as_str().map(str::to_owned))
            .collect(),
        label: entity
            .pointer("/labels/en/value")
            .or_else(|| entity.pointer("/labels/es/value"))
            .and_then(Value::as_str)
            .unwrap_or(id)
            .into(),
        description: desc,
        websites: values(entity, "P856")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        employees: quantity(entity, "P1128"),
        branches,
        parents: ids(entity, "P749")
            .into_iter()
            .chain(ids(entity, "P127"))
            .collect(),
        network: branches.is_some_and(|v| v > 1.)
            || ["chain", "multinational", "franchise", "retail network"]
                .iter()
                .any(|s| description.contains(s)),
        source: format!("https://www.wikidata.org/wiki/{id}"),
    }
}
async fn entity(client: &reqwest::Client, id: &str) -> Result<Value> {
    if !id.starts_with('Q') || !id[1..].chars().all(|c| c.is_ascii_digit()) {
        anyhow::bail!("identificador Wikidata inválido");
    }
    let result: Value = client
        .get(format!(
            "https://www.wikidata.org/wiki/Special:EntityData/{id}.json"
        ))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    result["entities"][id]
        .as_object()
        .context("respuesta Wikidata sin entidad")?;
    Ok(result["entities"][id].clone())
}

/// Conocimiento público ya contrastado, compartido también por importadores sin HTTP.
pub fn from_cache(conn: &Connection) -> Result<Classifier> {
    let mut classifier = Classifier::default();
    let mut stmt=conn.prepare("SELECT clave,payload FROM organizacion_cache WHERE payload!='null' AND datetime(actualizado)>datetime('now','-30 days')")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (key, payload) in rows {
        if let Ok(org) = serde_json::from_str::<Organization>(&payload) {
            classifier.organizations.insert(key, org);
        }
    }
    Ok(classifier)
}

/// Resolución agrupada por organización, con caché local y máximo de una petición por segundo.
pub async fn resolve(store: &Store, evidence: &[Evidence]) -> Result<Classifier> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(
            "RadarNavarra/0.1 (https://radar-navarra.duckdns.org; commercial evidence classifier)",
        )
        .build()?;
    let mut classifier = {
        let conn = store.lock().unwrap();
        from_cache(&conn)?
    };
    let mut requested = std::collections::BTreeSet::new();
    let mut labels = HashMap::<String, String>::new();
    for e in evidence.iter().filter(|e| e.commercial && !e.inactive) {
        for id in &e.entities {
            requested.insert(id.clone());
        }
        if e.entities.is_empty() {
            for name in e.brand.iter().chain(
                e.operator
                    .iter()
                    .filter(|_| e.parent_organization || e.branch || e.brand.is_some()),
            ) {
                labels.insert(format!("name:{}", normalize(name)), name.clone());
            }
        }
    }
    eprintln!(
        "Clasificación: {} candidatos, {} identidades de organización y {} marcas por resolver",
        evidence.len(),
        requested.len(),
        labels.len()
    );
    for (key, label) in labels {
        let cached: Option<String> = {
            let conn = store.lock().unwrap();
            conn.query_row("SELECT payload FROM organizacion_cache WHERE clave=?1 AND ((payload='null' AND datetime(actualizado)>datetime('now','-1 day')) OR (payload!='null' AND datetime(actualizado)>datetime('now','-30 days')))",params![key],|r|r.get(0)).optional()?
        };
        if let Some(cached) = cached {
            if let Ok(org) = serde_json::from_str::<Organization>(&cached) {
                requested.insert(org.id.clone());
                classifier.organizations.insert(key, org);
            }
            continue;
        }
        // Resolver únicamente identidades explícitas de marca/operador; nunca búsquedas por cada nombre de local.
        tokio::time::sleep(Duration::from_secs(1)).await;
        let response = client
            .get("https://www.wikidata.org/w/api.php")
            .query(&[
                ("action", "wbsearchentities"),
                ("search", label.as_str()),
                ("language", "en"),
                ("uselang", "en"),
                ("format", "json"),
                ("limit", "5"),
                ("maxlag", "5"),
            ])
            .send()
            .await;
        let mut found = None;
        if let Ok(response) = response {
            if response.status().as_u16() == 429 || response.status().is_server_error() {
                eprintln!("Wikidata limita el acceso; se mantienen pendientes las organizaciones sin verificar");
                break;
            }
            if let Ok(data) = response.json::<Value>().await {
                if let Some(rows) = data["search"].as_array() {
                    for row in rows {
                        if normalize(row["label"].as_str().unwrap_or("")) == normalize(&label) {
                            if let Some(id) = row["id"].as_str() {
                                found = Some(id.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
        if let Some(id) = found {
            requested.insert(id.clone());
            classifier.organizations.insert(
                key,
                Organization {
                    id,
                    ..Default::default()
                },
            );
        } else {
            let conn = store.lock().unwrap();
            conn.execute("INSERT INTO organizacion_cache(clave,payload,actualizado) VALUES(?1,'null',?2) ON CONFLICT(clave) DO UPDATE SET payload='null',actualizado=excluded.actualizado",params![key,store::ahora()])?;
        }
        // Las resoluciones fallidas no se convierten en una prueba de independencia.
    }
    let mut entities = HashMap::<String, Value>::new();
    let mut pending = requested.into_iter().collect::<Vec<_>>();
    let mut depth = HashMap::<String, usize>::new();
    while let Some(id) = pending.pop() {
        if classifier.organizations.contains_key(&id) {
            continue;
        }
        let cached: Option<String> = {
            let conn = store.lock().unwrap();
            conn.query_row("SELECT payload FROM organizacion_cache WHERE clave=?1 AND ((payload='null' AND datetime(actualizado)>datetime('now','-1 day')) OR (payload!='null' AND datetime(actualizado)>datetime('now','-30 days')))",params![id],|r|r.get(0)).optional()?
        };
        if let Some(cached) = cached {
            if let Ok(org) = serde_json::from_str::<Organization>(&cached) {
                let level = *depth.get(&id).unwrap_or(&0);
                if level < 2 {
                    for parent in &org.parents {
                        if parent != &id {
                            depth.insert(parent.clone(), level + 1);
                            pending.push(parent.clone());
                        }
                    }
                }
                classifier.organizations.insert(id.clone(), org);
            }
            continue;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        let ent = match entity(&client, &id).await {
            Ok(e) => e,
            Err(err) => {
                eprintln!("Clasificación: organización {id} sin verificar: {err:#}");
                if err.to_string().contains("429") {
                    break;
                }
                continue;
            }
        };
        let mut types = Vec::new();
        for ty in ids(&ent, "P31").into_iter().take(4) {
            if !entities.contains_key(&ty) {
                tokio::time::sleep(Duration::from_secs(1)).await;
                if let Ok(e) = entity(&client, &ty).await {
                    entities.insert(ty.clone(), e);
                }
            }
            if let Some(e) = entities.get(&ty) {
                types.push(e.clone());
            }
        }
        let org = organization(&ent, &id, &types);
        let level = *depth.get(&id).unwrap_or(&0);
        if level < 2 {
            for parent in &org.parents {
                if parent != &id {
                    depth.insert(parent.clone(), level + 1);
                    pending.push(parent.clone());
                }
            }
        }
        {
            let conn = store.lock().unwrap();
            conn.execute("INSERT INTO organizacion_cache(clave,payload,actualizado) VALUES(?1,?2,?3) ON CONFLICT(clave) DO UPDATE SET payload=excluded.payload,actualizado=excluded.actualizado",params![id,serde_json::to_string(&org)?,store::ahora()])?;
        }
        classifier.organizations.insert(id, org);
        if classifier.organizations.len() % 10 == 0 {
            eprintln!(
                "Clasificación: {} organizaciones resueltas; {} pendientes",
                classifier.organizations.len(),
                pending.len()
            );
        }
    }
    let aliases = classifier
        .organizations
        .iter()
        .filter(|(k, _)| k.starts_with("name:"))
        .map(|(k, o)| (k.clone(), o.id.clone()))
        .collect::<Vec<_>>();
    for (key, id) in aliases {
        if let Some(org) = classifier.organizations.get(&id).cloned() {
            let conn = store.lock().unwrap();
            conn.execute("INSERT INTO organizacion_cache(clave,payload,actualizado) VALUES(?1,?2,?3) ON CONFLICT(clave) DO UPDATE SET payload=excluded.payload,actualizado=excluded.actualizado",params![key,serde_json::to_string(&org)?,store::ahora()])?;
            classifier.organizations.insert(key, org);
        }
    }
    Ok(classifier)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Negocio {
        serde_json::from_value(json!({"id":0,"nombre":"Taller de prueba","municipio":"Navarra","fuente":"test","fuente_id":"1","primera_vista":"2026-01-01"})).unwrap()
    }
    #[test]
    fn structural_decisions_do_not_depend_on_brand_names() {
        let n = sample();
        let mut e = Evidence {
            commercial: true,
            in_navarra: true,
            ..Default::default()
        };
        assert!(Classifier::default().evaluate(&n, &e).admitted());
        e.entities.push("Q999999".into());
        assert_eq!(Classifier::default().evaluate(&n, &e).status, "pendiente");
        let mut c = Classifier::default();
        c.organizations.insert(
            "Q999999".into(),
            Organization {
                id: "Q999999".into(),
                network: true,
                employees: Some(1000.),
                websites: vec!["https://corporacion.test".into()],
                ..Default::default()
            },
        );
        assert_eq!(c.evaluate(&n, &e).status, "excluido");
        c.organizations.get_mut("Q999999").unwrap().employees = Some(15.);
        c.organizations.get_mut("Q999999").unwrap().branches = Some(3.);
        assert!(c.evaluate(&n, &e).admitted());
        e.commercial = false;
        assert_eq!(c.evaluate(&n, &e).status, "excluido");
    }
    #[test]
    fn no_local_inference_from_incomplete_corporate_data() {
        let n = sample();
        let e = Evidence {
            commercial: true,
            in_navarra: true,
            parent_organization: true,
            ..Default::default()
        };
        assert_eq!(Classifier::default().evaluate(&n, &e).status, "pendiente");
        assert_eq!(
            domain("https://tienda.example.co.uk/abc").as_deref(),
            Some("example.co.uk")
        );
    }
    #[test]
    fn actividad_no_lucrativa_local_es_elegible_sin_fingir_actividad_comercial() {
        let n = sample();
        let e = Evidence {
            commercial: false,
            nonprofit_activity: true,
            in_navarra: true,
            ..Default::default()
        };
        assert!(Classifier::default().evaluate(&n, &e).admitted());
        let sin_actividad = Evidence {
            nonprofit_activity: false,
            in_navarra: true,
            ..Default::default()
        };
        assert_eq!(
            Classifier::default().evaluate(&n, &sin_actividad).status,
            "excluido"
        );
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    #[test]
    fn identidad_sin_corroborar_es_pendiente_y_dominio_corporativo_es_evidencia() {
        let mut n:Negocio=serde_json::from_value(json!({"id":0,"nombre":"Viajes Grupo Ejemplo","municipio":"Navarra","fuente":"test","fuente_id":"1","primera_vista":"2026-11-04"})).unwrap();
        let e = Evidence {
            commercial: true,
            in_navarra: true,
            ..Default::default()
        };
        let mut classifier = Classifier::default();
        classifier.organizations.insert(
            "Q123".into(),
            Organization {
                id: "Q123".into(),
                label: "Grupo Ejemplo".into(),
                employees: Some(2000.),
                websites: vec!["https://corporacion.example".into()],
                ..Default::default()
            },
        );
        let d = classifier.evaluate(&n, &e);
        assert_eq!(d.status, "pendiente");
        assert_eq!(d.reason, "posible_vinculo_corporativo_sin_confirmar");
        n.nombre = "Sucursal sin marca".into();
        n.web = Some("https://tiendas.corporacion.example/local/1".into());
        assert_eq!(classifier.evaluate(&n, &e).status, "excluido");
        n.web = Some("https://local.example".into());
        assert!(classifier.evaluate(&n, &e).admitted());
    }
    #[test]
    fn dimension_se_toma_de_la_observacion_mas_reciente() {
        let entity = json!({"claims":{"P1128":[
            {"rank":"normal","mainsnak":{"datavalue":{"value":{"amount":"+1000"}}},"qualifiers":{"P585":[{"datavalue":{"value":{"time":"+2000-01-01T00:00:00Z"}}}]}},
            {"rank":"normal","mainsnak":{"datavalue":{"value":{"amount":"+20"}}},"qualifiers":{"P585":[{"datavalue":{"value":{"time":"+2025-01-01T00:00:00Z"}}}]}}
        ]}});
        assert_eq!(quantity(&entity, "P1128"), Some(20.));
    }
}
