//! Binario `radar`: CLI (scan / audit / export / watch) + `serve` (API + dashboard) + `api-spec`.

mod api;
mod auth;

use anyhow::Result;
use clap::{Parser, Subcommand};
use radar_core::model::{Auditoria, FiltrosNegocios};
use radar_core::store::{self, Store};
use utoipa::OpenApi;

#[derive(Parser)]
#[command(name = "radar", version, about = "Radar Navarra — prospección web local (Rust + Preact)")]
struct Cli {
    /// Ruta de la base de datos SQLite
    #[arg(long, default_value = "radar.db", global = true)]
    db: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Descubre negocios vía Overpass (OSM) por municipio y sector → SQLite
    Scan {
        #[arg(long)]
        municipio: String,
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
    /// Vigilancia de altas nuevas (BORME/OpenMercantil). V1: --csv con columnas
    /// nombre,municipio,domicilio,objeto,cnae,fecha
    Watch {
        #[arg(long, default_value = "borme")]
        fuente: String,
        #[arg(long)]
        csv: Option<String>,
    },
    /// Sirve la API REST + dashboard en 127.0.0.1 (puerto por defecto 7800)
    Serve {
        #[arg(long, default_value_t = 7800)]
        puerto: u16,
    },
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
        Cmd::Scan { municipio, sector } => {
            let store = store::abrir(&cli.db)?;
            let n = radar_core::fuentes::overpass::scan_municipio(&store, &municipio, sector.as_deref()).await?;
            println!("scan OK: {n} negocios procesados en {municipio} (sector: {})", sector.as_deref().unwrap_or("todos"));
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
                let _ = store::insert_auditoria(
                    &conn,
                    &Auditoria {
                        id: 0,
                        negocio_id: id,
                        fecha: ahora.clone(),
                        estado_web: r.estado_web,
                        score: r.score,
                        prioridad: r.prioridad,
                        senales: r.senales,
                    },
                );
                let _ = store::set_ultima_verificacion(&conn, id, &ahora);
            }
            println!("audit OK: {total} auditorías guardadas");
        }
        Cmd::Export { formato, prioridad, salida } => {
            let store = store::abrir(&cli.db)?;
            let conn = store.lock().unwrap();
            let filtros = FiltrosNegocios { prioridad: prioridad.clone(), ..Default::default() };
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
            eprintln!("export OK ({formato}, prioridad: {})", prioridad.as_deref().unwrap_or("todas"));
        }
        Cmd::Watch { fuente, csv } => {
            let store = store::abrir(&cli.db)?;
            match fuente.as_str() {
                "borme" | "openmercantil" => {
                    let path = csv.ok_or_else(|| anyhow::anyhow!("watch: falta --csv altas.csv"))?;
                    let n = radar_core::fuentes::borme::watch_desde_csv(&store, &path)?;
                    println!("watch OK: {n} leads de apertura añadidos ({fuente})");
                }
                f => anyhow::bail!("fuente desconocida: {f}"),
            }
        }
        Cmd::Serve { puerto } => {
            api::serve(&cli.db, puerto).await?;
        }
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
