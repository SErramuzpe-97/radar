//! Binario `radar`: CLI (scan / audit / export / watch) + `serve` (API + dashboard) + `api-spec`.

mod api;
mod auth;
mod rebuild;
mod targets;
mod technical_jobs;
mod web_api;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use radar_core::model::{Auditoria, FiltrosNegocios};
use radar_core::store::{self, Store};
use utoipa::OpenApi;

#[derive(Parser)]
#[command(
    name = "radar",
    version,
    about = "Radar Navarra — prospección web local (Rust + Preact)"
)]
struct Cli {
    /// Ruta de la base de datos SQLite
    #[arg(long, default_value = "radar.db", global = true)]
    db: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Corrección confirmada por el operador; preserva procedencia y programa auditoría.
    WebConfirm {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        url: String,
        #[arg(long)]
        municipio: Option<String>,
        #[arg(long)]
        direccion: Option<String>,
    },
    /// Programa auditorías técnicas graduales (una web cada vez).
    AuditQueue {
        #[arg(long, required_unless_present = "all")]
        id: Option<i64>,
        #[arg(long)]
        all: bool,
    },
    /// Programa búsqueda de las webs pendientes, sin modificar URLs confirmadas.
    DiscoverWebs {
        #[arg(long)]
        id: Option<i64>,
    },
    /// Copia, elimina resultados y programa una reconstrucción reanudable (servicio detenido).
    Rebuild {
        #[arg(long, required = true)]
        reset_results: bool,
        #[arg(long)]
        audit: bool,
    },
    /// Descubre negocios vía Overpass (OSM) por municipio y sector → SQLite
    Scan {
        #[arg(long, required_unless_present = "navarra")]
        municipio: Option<String>,
        #[arg(long)]
        navarra: bool,
        /// hosteleria | comercio | servicios | profesionales | industria | todos
        #[arg(long)]
        sector: Option<String>,
    },
    /// Audita webs (1 GET por web) y puntúa. Con --prioridad filtra la cola.
    Audit {
        #[arg(long)]
        prioridad: Option<String>,
        #[arg(long)]
        limite: Option<i64>,
    },
    /// Exporta la lista priorizada (CSV para visitas, GPX para el móvil, GeoJSON para el mapa)
    Export {
        #[arg(long, default_value = "csv")]
        formato: String,
        #[arg(long)]
        prioridad: Option<String>,
        #[arg(long)]
        salida: Option<String>,
    },
    /// Vigilancia de nuevas constituciones del BORME (últimos 7 días) o importación CSV
    /// con columnas nombre,municipio,domicilio,objeto,cnae,fecha
    Watch {
        #[arg(long, default_value = "borme")]
        fuente: String,
        #[arg(long)]
        csv: Option<String>,
        /// Fecha concreta (YYYY-MM-DD) para importar un día de BORME; si se omite, revisa 7 días.
        #[arg(long)]
        fecha: Option<String>,
    },
    /// Importa o lista objetivos comerciales curados; los no verificados no entran en la cartera.
    Targets {
        #[arg(long)]
        csv: Option<String>,
    },
    /// Vincula un objetivo pendiente con un negocio ya verificado en Navarra.
    TargetResolve {
        #[arg(long)]
        clave: String,
        #[arg(long)]
        negocio_id: i64,
    },
    /// Sirve la API REST + dashboard en 127.0.0.1 (puerto por defecto 7800)
    Serve {
        #[arg(long, default_value_t = 7800)]
        puerto: u16,
    },
    /// Crea una copia SQLite en línea y conserva catorce copias diarias.
    Backup,
    /// Imprime el contrato OAS (OpenAPI) generado desde el código Rust.
    /// `radar api-spec > api/openapi.yaml` regenera el contrato consumido por la UI.
    ApiSpec {
        #[arg(long, default_value = "yaml")]
        formato: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::WebConfirm {
            id,
            url,
            municipio,
            direccion,
        } => {
            let st = store::abrir(&cli.db)?;
            {
                let c = st.lock().unwrap();
                radar_core::web::confirmed(
                    &c,
                    id,
                    &url,
                    "correccion_usuario",
                    municipio.as_deref(),
                    direccion.as_deref(),
                )?;
            }
            let n = {
                let c = st.lock().unwrap();
                store::get_negocio(&c, id)?.context("Negocio no encontrado")?
            };
            let r = radar_core::audit::auditar_negocio(&n).await;
            let c = st.lock().unwrap();
            let a = Auditoria {
                id: 0,
                negocio_id: id,
                fecha: store::ahora(),
                estado_web: r.estado_web,
                score: r.score,
                prioridad: r.prioridad,
                senales: r.senales,
            };
            store::insert_auditoria(&c, &a)?;
            store::guardar_oportunidades_auditoria(&c, &a)?;
            println!(
                "{}",
                serde_json::to_string(&radar_core::technical::enqueue(&c, id)?)?
            );
        }
        Cmd::AuditQueue { id, all: _ } => {
            let st = store::abrir(&cli.db)?;
            let c = st.lock().unwrap();
            let ids = if let Some(id) = id {
                vec![id]
            } else {
                let mut q = c.prepare("SELECT n.id FROM negocios n WHERE n.admisible=1 AND n.web IS NOT NULL AND NOT EXISTS (SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=n.id) AND NOT EXISTS (SELECT 1 FROM auditorias_tecnicas t JOIN jobs j ON j.id=t.job_id WHERE t.negocio_id=n.id AND t.url=n.web AND j.tipo='technical' AND j.estado='terminado' AND json_extract(j.params,'$.generacion')=COALESCE((SELECT generacion FROM web_estado w WHERE w.negocio_id=n.id),1) AND (SELECT COUNT(DISTINCT p.motor||'/'||p.dispositivo) FROM auditorias_tecnicas p WHERE p.job_id=j.id AND p.estado IN ('completo','parcial','no_disponible'))=4) ORDER BY n.id")?;
                let rows = q.query_map([], |r| r.get::<_, i64>(0))?;
                let values = rows.collect::<std::result::Result<Vec<_>, _>>()?;
                values
            };
            let total = ids.len();
            let mut queued = 0;
            let mut skipped = 0;
            for id in ids {
                if radar_core::technical::enqueue(&c, id).is_ok() {
                    queued += 1;
                } else {
                    skipped += 1;
                }
            }
            println!("{queued} auditorías programadas; {skipped} webs no elegibles o ya en cola (de {total} registros pendientes).");
        }
        Cmd::DiscoverWebs { id } => {
            let st = store::abrir(&cli.db)?;
            let c = st.lock().unwrap();
            if store::tipo_job_activo(&c, radar_core::model::TipoJob::Discovery)? {
                anyhow::bail!("Ya hay una búsqueda activa");
            }
            println!(
                "{}",
                serde_json::to_string(&radar_core::technical::discovery_job(
                    &c,
                    &radar_core::model::BusquedaWebParams {
                        negocio_id: id,
                        pendientes: id.is_none()
                    }
                )?)?
            );
        }
        Cmd::Rebuild {
            reset_results: _,
            audit,
        } => {
            rebuild::preparar(&cli.db, audit)?;
        }
        Cmd::Scan {
            municipio,
            sector,
            navarra,
        } => {
            let store = store::abrir(&cli.db)?;
            if navarra {
                let mut resumen = radar_core::fuentes::overpass::scan_navarra_resumen(
                    &store,
                    &format!("{}.navarra.osm.pbf", cli.db),
                    sector.as_deref(),
                )
                .await?;
                match radar_core::fuentes::navarra::scan(&store, sector.as_deref()).await {
                    Ok(extra) => resumen["captura_fuentes_abiertas"] = extra,
                    Err(error) => {
                        resumen["fallo_fuentes_abiertas"] = serde_json::json!(format!("{error:#}"))
                    }
                }
                store::backfill_oportunidades(&store.lock().unwrap())?;
                println!("{}", serde_json::to_string_pretty(&resumen)?);
                return Ok(());
            }
            let municipio = municipio.context("indica --navarra o --municipio")?;
            let n = radar_core::fuentes::overpass::scan_municipio(
                &store,
                &municipio,
                sector.as_deref(),
            )
            .await?;
            store::backfill_oportunidades(&store.lock().unwrap())?;
            println!(
                "scan OK: {n} negocios procesados en {municipio} (sector: {})",
                sector.as_deref().unwrap_or("todos")
            );
        }
        Cmd::Audit { prioridad, limite } => {
            let store = store::abrir(&cli.db)?;
            let negocios = {
                let conn = store.lock().unwrap();
                store::negocios_para_auditar(&conn, prioridad.as_deref(), limite)?
            };
            let total = negocios.len() as i64;
            println!("auditando {total} negocios...");
            let resultados = radar_core::audit::auditar_lote(negocios, |hechos, total| {
                if hechos % 25 == 0 || hechos == total {
                    println!("  {hechos}/{total}");
                }
            })
            .await;
            let conn = store.lock().unwrap();
            let ahora = store::ahora();
            for (id, r) in resultados {
                let auditoria = Auditoria {
                    id: 0,
                    negocio_id: id,
                    fecha: ahora.clone(),
                    estado_web: r.estado_web,
                    score: r.score,
                    prioridad: r.prioridad,
                    senales: r.senales,
                };
                store::insert_auditoria(&conn, &auditoria)?;
                store::guardar_oportunidades_auditoria(&conn, &auditoria)?;
                let _ = store::set_ultima_verificacion(&conn, id, &ahora);
            }
            println!("audit OK: {total} auditorías guardadas");
        }
        Cmd::Export {
            formato,
            prioridad,
            salida,
        } => {
            let store = store::abrir(&cli.db)?;
            let conn = store.lock().unwrap();
            let filtros = FiltrosNegocios {
                prioridad: prioridad.clone(),
                ..Default::default()
            };
            let contenido = match formato.as_str() {
                "geojson" => radar_core::export::export_geojson(&conn, &filtros)?,
                "gpx" => radar_core::export::export_gpx(&conn, &filtros)?,
                _ => {
                    let mut buf: Vec<u8> = Vec::new();
                    radar_core::export::export_csv(&conn, &filtros, &mut buf)?;
                    String::from_utf8(buf)?
                }
            };
            match salida {
                Some(path) => std::fs::write(&path, &contenido)?,
                None => println!("{contenido}"),
            }
            eprintln!(
                "export OK ({formato}, prioridad: {})",
                prioridad.as_deref().unwrap_or("todas")
            );
        }
        Cmd::Watch { fuente, csv, fecha } => {
            let store = store::abrir(&cli.db)?;
            match fuente.as_str() {
                "borme" | "openmercantil" => {
                    let n = match (csv, fecha) {
                        (Some(path), None) => {
                            radar_core::fuentes::borme::watch_desde_csv(&store, &path)?
                        }
                        (None, Some(fecha)) => {
                            radar_core::fuentes::borme::watch_borme_fecha(&store, &fecha).await?
                        }
                        (None, None) => {
                            radar_core::fuentes::borme::watch_borme_reciente(&store).await?
                        }
                        (Some(_), Some(_)) => anyhow::bail!("watch: usa --csv o --fecha, no ambos"),
                    };
                    println!("watch OK: {n} leads de apertura añadidos ({fuente})");
                }
                f => anyhow::bail!("fuente desconocida: {f}"),
            }
        }
        Cmd::Targets { csv } => {
            let st = store::abrir(&cli.db)?;
            let conn = st.lock().unwrap();
            let result = if let Some(path) = csv {
                targets::importar(&conn, std::path::Path::new(&path))?
            } else {
                targets::listar(&conn)?
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Cmd::TargetResolve { clave, negocio_id } => {
            let st = store::abrir(&cli.db)?;
            let conn = st.lock().unwrap();
            targets::resolver(&conn, &clave, negocio_id)?;
            println!("Objetivo verificado y vinculado al negocio {negocio_id}.");
        }
        Cmd::Serve { puerto } => {
            api::serve(&cli.db, puerto).await?;
        }
        Cmd::Backup => match store::respaldar_si_nuevo_dia(&cli.db, 14)? {
            Some(path) => println!("Copia SQLite creada: {}", path.display()),
            None => println!("La copia de hoy ya existe; no se sobrescribió."),
        },
        Cmd::ApiSpec { formato } => {
            let spec = api::ApiDoc::openapi();
            match formato.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&spec)?),
                _ => println!("{}", serde_yaml::to_string(&spec)?),
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: &Store) {}
