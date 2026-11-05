//! Importadores del portal de datos abiertos de Navarra (CKAN DataStore).
use crate::{
    eligibility,
    model::Negocio,
    store::{self, Store},
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::HashMap;

const API: &str = "https://datosabiertos.navarra.es/api/3/action/datastore_search";
const DENA: &str = "0c4b4747-026f-479a-9966-6568a88957f5";
const COOPS: &str = "5ee5a208-0299-4d3c-b0e8-5f7bb1f59876";
const TOURISM: &[(&str, &str, &str)] = &[
    (
        "0a197290-2e3b-4b0c-b383-4f1ec46d9468",
        "Registro de restaurantes turísticos",
        "restaurantes-inscritos-en-el-registro-de-turismo-de-navarra",
    ),
    (
        "5527debf-7e72-4f9e-8e12-b562f3027fc2",
        "Registro de alojamientos turísticos",
        "alojamientos-inscritos-en-el-registro-de-turismo-de-navarra",
    ),
    (
        "dfbc24bb-2285-49db-af21-361338bc9ad7",
        "Registro de empresas de actividades turísticas",
        "empresas-de-actividades-inscritas-en-el-registro-de-turismo-de-navarra",
    ),
];
const INDUSTRY: &[(&str, &str, &str)] = &[
    (
        "d83e2397-13db-495c-8e35-6fc296f534e6",
        "Registro industrial Navarra · A",
        "registro-industrial-de-navarra-empresas-divisi-n-a",
    ),
    (
        "286e73d0-13f8-4f67-9d34-9bb169175a78",
        "Registro industrial Navarra · B",
        "registro-industrial-de-navarra-empresas-divisi-n-b",
    ),
    (
        "5548b6ec-c66e-4671-8d1c-fe9161b5f72d",
        "Registro industrial Navarra · C",
        "registro-industrial-de-navarra-empresas-divisi-n-c",
    ),
];
const PORTAL: &str = "https://datosabiertos.navarra.es/dataset/";
fn text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
    .filter(|s| !s.is_empty() && !eligibility::normalize(s).contains("noseconsiente") && s != "-")
}
fn joined(row: &Value, keys: &[&str], sep: &str) -> Option<String> {
    let x = keys
        .iter()
        .filter_map(|k| text(&row[*k]))
        .collect::<Vec<_>>();
    (!x.is_empty()).then(|| x.join(sep))
}
fn sector(code: &str, desc: &str) -> (&'static str, Option<&'static str>) {
    let c = code.replace('.', "");
    let d = eligibility::normalize(desc);
    if c.starts_with("11")
        || d.contains("bodega")
        || d.contains("vino")
        || d.contains("vinificacion")
    {
        ("industria", Some("bodega"))
    } else if c.starts_with("56")
        || d.contains("restaurante")
        || d.contains("bar")
        || d.contains("comidas")
    {
        ("hosteleria", Some("restaurante"))
    } else if c == "7500" || d.contains("veterinari") {
        ("servicios", Some("veterinaria"))
    } else if c == "4776" || d.contains("floristeria") {
        ("comercio", Some("floristeria"))
    } else if c.starts_with("45") || d.contains("reparacion de vehiculos") {
        ("servicios", Some("automocion"))
    } else if c == "4321" || d.contains("instalaciones electricas") {
        ("servicios", Some("electricidad"))
    } else if c == "4322" || d.contains("fontaneria") {
        ("servicios", Some("fontaneria"))
    } else if c.starts_with("88") && (d.contains("infantil") || d.contains("guarderia")) {
        ("servicios", Some("guarderia"))
    } else if c.starts_with("81") && d.contains("plagas") {
        ("servicios", Some("control_plagas"))
    } else if c.starts_with("96") && d.contains("funerari") {
        ("servicios", Some("tanatorio"))
    } else if c.starts_with("55") {
        ("hosteleria", Some("otro"))
    } else if c.starts_with("47") {
        ("comercio", Some("otro"))
    } else if c.starts_with("41") || c.starts_with("42") || c.starts_with("43") {
        ("industria", Some("otro"))
    } else if (10..=33).any(|d| c.starts_with(&format!("{d}"))) {
        ("industria", Some("otro"))
    } else if c.starts_with("01") || c.starts_with("02") || c.starts_with("03") {
        ("industria", Some("otro"))
    } else if c.starts_with("68")
        || c.starts_with("69")
        || c.starts_with("70")
        || c.starts_with("71")
        || c.starts_with("72")
        || c.starts_with("73")
        || c.starts_with("74")
    {
        ("profesionales", Some("otro"))
    } else if c.starts_with("84") {
        ("servicios", Some("otro"))
    } else {
        ("servicios", Some("otro"))
    }
}
async fn page(
    client: &reqwest::Client,
    rid: &str,
    offset: usize,
    filters: Option<&str>,
) -> Result<Value> {
    let mut q = vec![
        ("resource_id", rid.to_string()),
        ("limit", "5000".into()),
        ("offset", offset.to_string()),
        ("sort", "_id asc".into()),
    ];
    if let Some(f) = filters {
        q.push(("filters", f.to_string()));
    }
    let v = client
        .get(API)
        .query(&q)
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    anyhow::ensure!(
        v["success"] == true,
        "CKAN devolvió success=false: {}",
        v["error"]
    );
    Ok(v["result"].clone())
}
async fn all(
    client: &reqwest::Client,
    rid: &str,
    filters: Option<&str>,
) -> Result<(Vec<Value>, i64)> {
    let mut out = Vec::new();
    let mut offset = 0;
    let total = loop {
        let p = page(client, rid, offset, filters).await?;
        let total = p["total"].as_i64().context("CKAN no informó total")?;
        let rows = p["records"]
            .as_array()
            .context("CKAN no devolvió records")?;
        if rows.is_empty() {
            break total;
        }
        let count = rows.len();
        out.extend(rows.iter().cloned());
        offset += count;
        if offset as i64 >= total {
            break total;
        }
    };
    anyhow::ensure!(
        out.len() as i64 == total,
        "Paginación incompleta en {rid}: {} de {total}",
        out.len()
    );
    Ok((out, total))
}
async fn latest_dena_year(client: &reqwest::Client) -> Result<i64> {
    let v = client
        .get(API)
        .query(&[("resource_id", DENA), ("limit", "1"), ("sort", "anio desc")])
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await?;
    anyhow::ensure!(
        v["success"] == true,
        "CKAN no pudo determinar la edición DENA"
    );
    v["result"]["records"]
        .as_array()
        .and_then(|r| r.first())
        .and_then(|r| r["anio"].as_i64())
        .context("DENA no publicó una edición con año")
}
fn source_id(c: &rusqlite::Connection, name: &str, url: &str, attrib: &str) -> Result<i64> {
    c.execute("INSERT OR IGNORE INTO fuentes(nombre,tipo,url,condiciones,atribucion,revisada_en,habilitada,profundidad,max_paginas_dia) VALUES(?1,'dataset',?2,'CC BY 4.0',?3,?4,1,0,0)",rusqlite::params![name,url,attrib,chrono::Utc::now().format("%Y-%m-%d").to_string()])?;
    Ok(c.query_row("SELECT id FROM fuentes WHERE url=?1", [url], |r| r.get(0))?)
}
fn link(c: &rusqlite::Connection, id: i64, source: i64, external: &str, url: &str) -> Result<()> {
    c.execute("INSERT INTO negocio_fuentes(negocio_id,fuente_id,external_id,url_origen,visto_en) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(fuente_id,external_id) DO UPDATE SET negocio_id=excluded.negocio_id,url_origen=excluded.url_origen,visto_en=excluded.visto_en",rusqlite::params![id,source,external,url,store::ahora()])?;
    Ok(())
}
fn attribute(
    c: &rusqlite::Connection,
    id: i64,
    source: i64,
    key: &str,
    value: Option<&str>,
    url: &str,
) -> Result<()> {
    if let Some(value) = value.filter(|s| !s.trim().is_empty()) {
        store::guardar_atributo_fuente(c, id, source, key, value, url)?;
    }
    Ok(())
}
fn identities(c: &rusqlite::Connection) -> Result<HashMap<(String, String), i64>> {
    let mut s = c.prepare("SELECT id,nombre,municipio FROM negocios WHERE admisible=1")?;
    let rows = s.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, n, m) = row?;
        out.entry((eligibility::normalize(&n), eligibility::normalize(&m)))
            .or_insert(id);
    }
    Ok(out)
}
pub async fn scan(store: &Store, sector_filter: Option<&str>) -> Result<Value> {
    let sector_filter = sector_filter.filter(|s| *s != "todos");
    if sector_filter.is_none() {
        let c = store.lock().unwrap();
        if let Some(stamp) =
            store::meta_get(&c, "navarra_open_data_last_scan")?.and_then(|s| s.parse::<i64>().ok())
        {
            if chrono::Utc::now().timestamp() - stamp < 86400 {
                return Ok(
                    json!({"cache":"datos oficiales sincronizados en las últimas 24 horas","ultima_sincronizacion":chrono::DateTime::from_timestamp(stamp,0).map(|d|d.to_rfc3339()),"nuevas_consultas":0}),
                );
            }
        }
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("Radar Navarra/1.0 (open data ingestion)")
        .build()?;
    let year = latest_dena_year(&client).await?;
    let filters = json!({"anio":year}).to_string();
    let (dena, total) = all(&client, DENA, Some(&filters)).await?;
    let (coops, coop_total) = all(&client, COOPS, None).await?;
    let mut supplements: Vec<(String, String, String, Vec<Value>, i64)> = Vec::new();
    let mut source_errors = Vec::new();
    for (rid, title, slug) in TOURISM.iter().chain(INDUSTRY.iter()) {
        match all(&client, rid, None).await {
            Ok((rows, count)) => {
                supplements.push(((*rid).into(), (*title).into(), (*slug).into(), rows, count))
            }
            Err(error) => {
                source_errors.push(json!({"resource_id":rid,"error":format!("{error:#}")}))
            }
        }
    }
    let c = store.lock().unwrap();
    let c = c.unchecked_transaction()?;
    let dsrc = source_id(
        &c,
        "DENA · Establecimientos de actividad económica",
        &format!("{PORTAL}directorio-de-establecimientos-con-actividad-economica-en-navarra"),
        "© Gobierno de Navarra · CC BY 4.0",
    )?;
    let csrc=source_id(&c,"Registro de cooperativas vigentes de Navarra",&format!("{PORTAL}datos-de-cooperativas-vigentes-inscritas-en-el-registro-de-cooperativas-de-navarra"),"© Gobierno de Navarra · CC BY 4.0")?;
    let mut existing = identities(&c)?;
    let mut stats = json!({"dena":{"total":total,"nuevos":0,"actualizados":0,"vinculados":0,"sin_nombre":0,"sin_municipio":0,"fuera_navarra":0,"errores":0},"cooperativas":{"total":coop_total,"nuevos":0,"actualizados":0,"vinculados":0,"sin_nombre":0,"sin_municipio":0,"fuera_navarra":0,"errores":0}});
    for (rows, source, kind) in [(&dena, dsrc, "dena"), (&coops, csrc, "cooperativas")] {
        for row in rows {
            let is_coop = kind == "cooperativas";
            let name = text(
                &row[if is_coop {
                    "DenominacionCooperativa"
                } else {
                    "nombre"
                }],
            )
            .unwrap_or_default();
            let city =
                text(&row[if is_coop { "Localidad" } else { "nombre_cmun" }]).unwrap_or_default();
            if name.is_empty() {
                stats[kind]["sin_nombre"] =
                    json!(stats[kind]["sin_nombre"].as_i64().unwrap_or(0) + 1);
                continue;
            }
            if city.is_empty() {
                stats[kind]["sin_municipio"] =
                    json!(stats[kind]["sin_municipio"].as_i64().unwrap_or(0) + 1);
                continue;
            }
            let code = text(&row[if is_coop { "CNAE" } else { "cnae09_ppal" }]).unwrap_or_default();
            let desc = text(
                &row[if is_coop {
                    "ClaseCooperativa"
                } else {
                    "cnae09_descrip_ppal"
                }],
            )
            .unwrap_or_default();
            let (general, detail) = sector(&code, &desc);
            if sector_filter.is_some_and(|f| f != general) {
                continue;
            }
            let idkey = if is_coop {
                text(&row["NumInscripcion"])
                    .unwrap_or_else(|| text(&row["_id"]).unwrap_or_default())
            } else {
                text(&row["dnici"])
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| {
                        format!("{}-{}", year, text(&row["_id"]).unwrap_or_default())
                    })
            };
            let ext = format!("{}-{idkey}", if is_coop { "coop" } else { "dena" });
            let addr = if is_coop {
                text(&row["DomicilioSocial"])
            } else {
                joined(row, &["msiglat", "callet", "portalt"], " ")
            };
            let n = Negocio {
                id: 0,
                nombre: name.clone(),
                categoria: Some(if code.is_empty() {
                    desc.clone()
                } else {
                    format!("CNAE {code} · {desc}")
                }),
                sector: Some(general.into()),
                rubro: detail.map(str::to_owned),
                evidencia_rubro: Some(
                    json!({"fuente_registro":if is_coop{"Registro de cooperativas vigentes"}else{"DENA"},"cnae_original":code,"descripcion_original":desc,"edicion":year,"tipo_entidad":if is_coop{"cooperativa"}else{"establecimiento"}}),
                ),
                direccion: addr.clone(),
                municipio: city.clone(),
                lat: None,
                lon: None,
                telefono: None,
                email: None,
                web: None,
                estado_web_lista: String::new(),
                estado_auditoria_tecnica: String::new(),
                fuente: if is_coop {
                    "cooperativas".into()
                } else {
                    "dena".into()
                },
                fuente_id: ext.clone(),
                primera_vista: store::ahora(),
                ultima_verificacion: None,
                score_potencial: None,
                prioridad_comercial: None,
                estado_pipeline: None,
                oportunidades: Vec::new(),
                descartado: false,
            };
            let key = (eligibility::normalize(&name), eligibility::normalize(&city));
            let known = existing.get(&key).copied();
            let already_linked: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM negocio_fuentes WHERE fuente_id=?1 AND external_id=?2)",rusqlite::params![source,ext],|r|r.get(0))?;
            let id = if let Some(id) = known {
                id
            } else {
                store::upsert_negocio(&c, &n)?
            };
            existing.insert(key, id);
            let url = if is_coop {
                format!("{PORTAL}datos-de-cooperativas-vigentes-inscritas-en-el-registro-de-cooperativas-de-navarra")
            } else {
                format!("{PORTAL}directorio-de-establecimientos-con-actividad-economica-en-navarra")
            };
            link(&c, id, source, &ext, &url)?;
            attribute(&c, id, source, "nombre", Some(&name), &url)?;
            attribute(&c, id, source, "municipio", Some(&city), &url)?;
            attribute(&c, id, source, "direccion", addr.as_deref(), &url)?;
            attribute(&c, id, source, "actividad", Some(&desc), &url)?;
            attribute(&c, id, source, "cnae", Some(&code), &url)?;
            attribute(
                &c,
                id,
                source,
                "tipo_entidad",
                Some(if is_coop {
                    "cooperativa"
                } else {
                    "establecimiento"
                }),
                &url,
            )?;
            let keystat = if is_coop { "cooperativas" } else { "dena" };
            let field = if already_linked {
                "actualizados"
            } else if known.is_some() {
                "vinculados"
            } else {
                "nuevos"
            };
            stats[keystat][field] = json!(stats[keystat][field].as_i64().unwrap_or(0) + 1);
        }
    }
    let mut other = json!({});
    for (rid, title, slug, rows, count) in supplements {
        let is_tourism = TOURISM.iter().any(|(id, _, _)| *id == rid);
        let source = source_id(
            &c,
            &title,
            &format!("{PORTAL}{slug}"),
            "© Gobierno de Navarra · CC BY 4.0",
        )?;
        let mut stat = json!({"total":count,"nuevos":0,"actualizados":0,"vinculados":0,"sin_nombre":0,"sin_municipio":0,"fuera_navarra":0,"errores":0});
        for row in rows {
            let name = text(
                &row[if is_tourism {
                    "NOMBRE"
                } else {
                    "Denominacion/Rotulo del establecimiento"
                }],
            )
            .or_else(|| text(&row["Titular"]))
            .unwrap_or_default();
            let city = text(&row["LOCALIDAD"])
                .or_else(|| text(&row["Localidad"]))
                .or_else(|| text(&row["MUNICIPIO"]))
                .or_else(|| text(&row["Municipio"]))
                .unwrap_or_default();
            if name.is_empty() {
                stat["sin_nombre"] = json!(stat["sin_nombre"].as_i64().unwrap_or(0) + 1);
                continue;
            }
            if city.is_empty() {
                stat["sin_municipio"] = json!(stat["sin_municipio"].as_i64().unwrap_or(0) + 1);
                continue;
            }
            if !is_tourism
                && text(&row["Provincia"])
                    .is_some_and(|p| !eligibility::normalize(&p).contains("navarra"))
            {
                stat["fuera_navarra"] = json!(stat["fuera_navarra"].as_i64().unwrap_or(0) + 1);
                continue;
            }
            let modality = text(&row["MODALIDAD"])
                .or_else(|| text(&row["TIPO"]))
                .unwrap_or_default();
            let code = text(&row["Actividad Principal"]).unwrap_or_default();
            let desc = text(&row["Descripcion Actividad Principal"])
                .or_else(|| text(&row["Seccion"]))
                .unwrap_or_default();
            let (general, mut detail) = if is_tourism {
                let m = eligibility::normalize(&modality);
                if m.contains("restaurante") {
                    ("hosteleria", Some("restaurante"))
                } else if m.contains("alojamiento")
                    || m.contains("hotel")
                    || m.contains("hostal")
                    || m.contains("apartamento")
                {
                    ("hosteleria", Some("alojamiento"))
                } else {
                    ("servicios", Some("turismo_cultural"))
                }
            } else {
                sector(&code, &desc)
            };
            if sector_filter.is_some_and(|s| s != general) {
                continue;
            }
            if !is_tourism && detail == Some("otro") {
                detail = Some("otro");
            }
            let reg_id = text(&row["COD_INSCRIPCION"])
                .or_else(|| text(&row["No Identificacion RII"]))
                .or_else(|| text(&row["_id"]))
                .unwrap_or_default();
            let ext = format!("{rid}-{reg_id}");
            let mut web = text(&row[if is_tourism { "WEB" } else { "URL" }]);
            if let Some(u) = web.as_mut() {
                if !u.starts_with("http://") && !u.starts_with("https://") {
                    *u = format!("https://{u}");
                }
            }
            let evidence = json!({
                "fuente_registro":title,"registro":reg_id,"actividad_registrada":modality,
                "visitas":if is_tourism && detail==Some("turismo_cultural"){json!({"estado":"actividad_turistica_registrada","confianza":"media","fuente":title})}else{Value::Null},
                "tarifa":if is_tourism{json!({"estado":"no_informada","confianza":"sin_evidencia"})}else{Value::Null},
                "presencia_web":if web.is_some(){json!({"estado":"registrada_en_fuente","confianza":"media"})}else{json!({"estado":"sin_url_en_fuente","confianza":"baja"})},
                "cnae_original":code,"descripcion_original":desc,"tipo_entidad":if is_tourism{"empresa_turistica"}else{"empresa_industrial"}
            });
            let n = Negocio {
                id: 0,
                nombre: name.clone(),
                categoria: Some(if modality.is_empty() {
                    format!("CNAE {code} · {desc}")
                } else {
                    format!("{modality} · {desc}")
                }),
                sector: Some(general.into()),
                rubro: detail.map(str::to_owned),
                evidencia_rubro: Some(evidence),
                direccion: text(&row["DIRECCION"]).or_else(|| text(&row["Direccion"])),
                municipio: city.clone(),
                lat: None,
                lon: None,
                telefono: text(&row["TELEFONO_RESERVAS_1"]),
                email: text(&row["EMAIL"]),
                web,
                estado_web_lista: String::new(),
                estado_auditoria_tecnica: String::new(),
                fuente: if is_tourism {
                    "turismo".into()
                } else {
                    "industria".into()
                },
                fuente_id: ext.clone(),
                primera_vista: store::ahora(),
                ultima_verificacion: None,
                score_potencial: None,
                prioridad_comercial: None,
                estado_pipeline: None,
                oportunidades: Vec::new(),
                descartado: false,
            };
            let key = (eligibility::normalize(&name), eligibility::normalize(&city));
            let known = existing.get(&key).copied();
            let already:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM negocio_fuentes WHERE fuente_id=?1 AND external_id=?2)",rusqlite::params![source,ext],|r|r.get(0))?;
            let id = if let Some(id) = known {
                id
            } else {
                store::upsert_negocio(&c, &n)?
            };
            existing.insert(key, id);
            link(&c, id, source, &ext, &format!("{PORTAL}{slug}"))?;
            let source_url = format!("{PORTAL}{slug}");
            attribute(&c, id, source, "nombre", Some(&name), &source_url)?;
            attribute(&c, id, source, "municipio", Some(&city), &source_url)?;
            attribute(
                &c,
                id,
                source,
                "direccion",
                n.direccion.as_deref(),
                &source_url,
            )?;
            attribute(&c, id, source, "actividad", Some(&modality), &source_url)?;
            attribute(&c, id, source, "cnae", Some(&code), &source_url)?;
            attribute(
                &c,
                id,
                source,
                "tipo_entidad",
                Some(if is_tourism {
                    "empresa_turistica"
                } else {
                    "empresa_industrial"
                }),
                &source_url,
            )?;
            if let Some(url) = n.web.as_deref() {
                crate::web::add_candidate(
                    &c,
                    id,
                    url,
                    Some(&name),
                    &title,
                    None,
                    None,
                    &json!({"fuente":source_url,"registro":reg_id,"registro_oficial":true}),
                )?;
                attribute(&c, id, source, "web", Some(url), &source_url)?;
            }
            let field = if already {
                "actualizados"
            } else if known.is_some() {
                "vinculados"
            } else {
                "nuevos"
            };
            stat[field] = json!(stat[field].as_i64().unwrap_or(0) + 1);
        }
        other[&rid] = stat;
    }
    if source_errors.is_empty() && sector_filter.is_none() {
        store::meta_set(
            &c,
            "navarra_open_data_last_scan",
            &chrono::Utc::now().timestamp().to_string(),
        )?;
    }
    let result = json!({"fuentes_abiertas":{"dena":stats["dena"],"cooperativas":stats["cooperativas"],"turismo_e_industria":other,"errores_de_fuente":source_errors},"nota":"Conteos por fuente; vinculados y registros no equivalen a altas únicas. Los registros turísticos indican inscripción, no confirman por sí solos disponibilidad de visitas ni precio."});
    c.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cnae_map_sources_to_general_sector_and_optional_rubro() {
        assert_eq!(
            sector("11.02", "Elaboración de vinos"),
            ("industria", Some("bodega"))
        );
        assert_eq!(
            sector("56.10", "Restaurantes"),
            ("hosteleria", Some("restaurante"))
        );
        assert_eq!(
            sector("75.00", "Actividades veterinarias"),
            ("servicios", Some("veterinaria"))
        );
        assert_eq!(
            sector("43.21", "Instalaciones eléctricas"),
            ("servicios", Some("electricidad"))
        );
    }
}
