//! Store SQLite (`radar.db`). Esquema según diseño §8.5 + tabla `jobs` para la API.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};

use crate::model::*;

pub type Store = Arc<Mutex<Connection>>;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS clasificacion_ingesta (
 fuente TEXT NOT NULL, external_id TEXT NOT NULL, nombre TEXT NOT NULL, estado TEXT NOT NULL,
 motivo TEXT NOT NULL, evidencias TEXT NOT NULL, url TEXT NOT NULL, observado_en TEXT NOT NULL,
 version INTEGER NOT NULL, PRIMARY KEY(fuente,external_id)
);
CREATE TABLE IF NOT EXISTS organizacion_cache (clave TEXT PRIMARY KEY,payload TEXT NOT NULL,actualizado TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS municipios_navarra (nombre TEXT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS negocios_objetivo (
 clave TEXT PRIMARY KEY, nombre TEXT NOT NULL, municipio TEXT, rubro TEXT,
 descripcion TEXT NOT NULL DEFAULT '', procedencia TEXT NOT NULL DEFAULT '',
 estado TEXT NOT NULL DEFAULT 'pendiente_revision', negocio_id INTEGER REFERENCES negocios(id),
 coincidencias TEXT NOT NULL DEFAULT '[]', actualizado TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS negocios (
  id INTEGER PRIMARY KEY,
  nombre TEXT NOT NULL,
  admisible INTEGER NOT NULL DEFAULT 1,
  categoria TEXT,
  sector TEXT,
  rubro TEXT,
  evidencia_rubro TEXT NOT NULL DEFAULT '{}',
  direccion TEXT,
  municipio TEXT NOT NULL,
  lat REAL,
  lon REAL,
  telefono TEXT,
  email TEXT,
  web TEXT,
  fuente TEXT NOT NULL,
  fuente_id TEXT NOT NULL,
  primera_vista TEXT NOT NULL,
  ultima_verificacion TEXT,
  UNIQUE(fuente, fuente_id)
);
CREATE TABLE IF NOT EXISTS auditorias (
  id INTEGER PRIMARY KEY,
  negocio_id INTEGER NOT NULL REFERENCES negocios(id),
  fecha TEXT NOT NULL,
  estado_web TEXT NOT NULL,
  score INTEGER NOT NULL,
  prioridad TEXT NOT NULL,
  senales TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_auditorias_negocio ON auditorias(negocio_id, fecha DESC, id DESC);
CREATE TABLE IF NOT EXISTS oportunidades_servicio (
  negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,
  servicio TEXT NOT NULL,
  score INTEGER NOT NULL,
  estado TEXT NOT NULL DEFAULT 'pendiente',
  motivo TEXT NOT NULL,
  fecha TEXT NOT NULL,
  PRIMARY KEY (negocio_id, servicio)
);
CREATE INDEX IF NOT EXISTS idx_oportunidades_score ON oportunidades_servicio(score DESC);
CREATE TABLE IF NOT EXISTS negocios_descartados (
  negocio_id INTEGER PRIMARY KEY REFERENCES negocios(id) ON DELETE CASCADE,
  fecha TEXT NOT NULL,
  motivo TEXT
);
CREATE TABLE IF NOT EXISTS pipeline (
  negocio_id INTEGER PRIMARY KEY REFERENCES negocios(id),
  estado TEXT NOT NULL DEFAULT 'nuevo',
  valor_estimado REAL,
  siguiente_accion TEXT,
  siguiente_accion_fecha TEXT,
  canal TEXT,
  fecha TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS eventos (
  id INTEGER PRIMARY KEY,
  negocio_id INTEGER NOT NULL REFERENCES negocios(id),
  fecha TEXT NOT NULL,
  tipo TEXT NOT NULL,
  resultado TEXT,
  notas TEXT
);
CREATE INDEX IF NOT EXISTS idx_eventos_negocio ON eventos(negocio_id, fecha DESC);
CREATE TABLE IF NOT EXISTS jobs (
  id TEXT PRIMARY KEY,
  tipo TEXT NOT NULL,
  estado TEXT NOT NULL,
  params TEXT NOT NULL,
  hechos INTEGER NOT NULL DEFAULT 0,
  total INTEGER NOT NULL DEFAULT 0,
  creado TEXT NOT NULL,
  actualizado TEXT NOT NULL,
  error TEXT,
  resumen TEXT
);
CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS job_items (
  job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  item_key TEXT NOT NULL,
  kind TEXT NOT NULL,
  estado TEXT NOT NULL DEFAULT 'pendiente',
  intentos INTEGER NOT NULL DEFAULT 0,
  error TEXT,
  actualizado TEXT NOT NULL,
  PRIMARY KEY(job_id,item_key)
);
CREATE INDEX IF NOT EXISTS idx_job_items_estado ON job_items(job_id,estado);
CREATE TABLE IF NOT EXISTS job_steps (
  job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  paso TEXT NOT NULL,
  estado TEXT NOT NULL DEFAULT 'pendiente',
  resultado TEXT,
  error TEXT,
  actualizado TEXT NOT NULL,
  PRIMARY KEY(job_id,paso)
);
CREATE TABLE IF NOT EXISTS fuentes (
  id INTEGER PRIMARY KEY,
  nombre TEXT NOT NULL,
  tipo TEXT NOT NULL,
  url TEXT NOT NULL UNIQUE,
  condiciones TEXT,
  atribucion TEXT,
  revisada_en TEXT,
  habilitada INTEGER NOT NULL DEFAULT 1,
  profundidad INTEGER NOT NULL DEFAULT 2,
  max_paginas_dia INTEGER NOT NULL DEFAULT 100,
  ultima_ejecucion TEXT,
  error_ultima_ejecucion TEXT
);
CREATE TABLE IF NOT EXISTS fuentes_candidatas (
  id INTEGER PRIMARY KEY,
  nombre TEXT NOT NULL,
  url TEXT NOT NULL UNIQUE,
  fuente_padre_id INTEGER REFERENCES fuentes(id) ON DELETE SET NULL,
  estado TEXT NOT NULL DEFAULT 'pendiente',
  descubrimiento TEXT NOT NULL,
  creada TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS atributos_fuente (
  negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,
  fuente_id INTEGER NOT NULL REFERENCES fuentes(id) ON DELETE CASCADE,
  atributo TEXT NOT NULL,
  valor TEXT NOT NULL,
  url_origen TEXT NOT NULL,
  observado_en TEXT NOT NULL,
  PRIMARY KEY(negocio_id,fuente_id,atributo)
);
CREATE TABLE IF NOT EXISTS negocio_fuentes (
  negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,
  fuente_id INTEGER NOT NULL REFERENCES fuentes(id) ON DELETE CASCADE,
  external_id TEXT NOT NULL,
  url_origen TEXT NOT NULL,
  visto_en TEXT NOT NULL,
  PRIMARY KEY(fuente_id,external_id)
);
CREATE TABLE IF NOT EXISTS paginas_rastreadas (
  fuente_id INTEGER NOT NULL REFERENCES fuentes(id) ON DELETE CASCADE,
  url TEXT NOT NULL,
  profundidad INTEGER NOT NULL DEFAULT 0,
  rastreada_en TEXT NOT NULL,
  estado_http INTEGER,
  error TEXT,
  PRIMARY KEY(fuente_id,url)
);
CREATE TABLE IF NOT EXISTS duplicados_pendientes (
  id INTEGER PRIMARY KEY,
  negocio_a INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,
  negocio_b INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,
  razon TEXT NOT NULL,
  estado TEXT NOT NULL DEFAULT 'pendiente',
  creado TEXT NOT NULL,
  UNIQUE(negocio_a,negocio_b)
);
CREATE TABLE IF NOT EXISTS plantillas_contacto (
  id INTEGER PRIMARY KEY,
  nombre TEXT NOT NULL,
  canal TEXT NOT NULL,
  servicio TEXT,
  asunto TEXT,
  cuerpo TEXT NOT NULL,
  actualizado TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS borradores_contacto (
  id INTEGER PRIMARY KEY,
  negocio_id INTEGER NOT NULL REFERENCES negocios(id) ON DELETE CASCADE,
  plantilla_id INTEGER REFERENCES plantillas_contacto(id) ON DELETE SET NULL,
  canal TEXT NOT NULL,
  destino TEXT,
  asunto TEXT,
  cuerpo TEXT NOT NULL,
  creado TEXT NOT NULL,
  actualizado TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS auth_codes (
  id INTEGER PRIMARY KEY,
  email TEXT NOT NULL,
  hash TEXT NOT NULL,
  creado TEXT NOT NULL,
  expira TEXT NOT NULL,
  usado INTEGER NOT NULL DEFAULT 0,
  intentos INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_auth_codes_email ON auth_codes(email, id DESC);
CREATE TABLE IF NOT EXISTS sesiones (
  jti TEXT PRIMARY KEY,
  email TEXT NOT NULL,
  creado TEXT NOT NULL,
  expira TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
"#;

pub fn abrir(db_path: &str) -> Result<Store> {
    // Antes de cualquier DDL/migración, conservar el estado anterior de una DB existente.
    if db_path != ":memory:" {
        respaldar_si_nuevo_dia(db_path, 14)?;
    }
    let conn =
        Connection::open(db_path).with_context(|| format!("abrindo base de datos {db_path}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(SCHEMA)?;
    migrar(&conn)?;
    let has_admisible = {
        let mut stmt = conn.prepare("PRAGMA table_info(negocios)")?;
        let columns = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        columns.iter().any(|c| c == "admisible")
    };
    if !has_admisible {
        conn.execute(
            "ALTER TABLE negocios ADD COLUMN admisible INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(5,?1)",
        [ahora()],
    )?;
    let columns = {
        let mut stmt = conn.prepare("PRAGMA table_info(negocios)")?;
        let columns = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        columns
    };
    if !columns.iter().any(|c| c == "rubro") {
        conn.execute("ALTER TABLE negocios ADD COLUMN rubro TEXT", [])?;
    }
    if !columns.iter().any(|c| c == "evidencia_rubro") {
        conn.execute(
            "ALTER TABLE negocios ADD COLUMN evidencia_rubro TEXT NOT NULL DEFAULT '{}'",
            [],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(6,?1)",
        [ahora()],
    )?;
    let rows = {
        let mut stmt =
            conn.prepare("SELECT id,categoria,sector FROM negocios WHERE rubro IS NULL")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for (id, categoria, sector) in rows {
        if let Some(rubro) = crate::rubros::desde_categoria(categoria.as_deref(), sector.as_deref())
        {
            conn.execute(
                "UPDATE negocios SET rubro=?1 WHERE id=?2",
                params![rubro, id],
            )?;
        }
    }
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_negocios_rubro ON negocios(rubro);
        CREATE INDEX IF NOT EXISTS idx_negocios_mapa ON negocios(admisible,lat,lon);
        CREATE INDEX IF NOT EXISTS idx_negocios_sector ON negocios(sector,admisible);
        CREATE INDEX IF NOT EXISTS idx_negocios_tipo_entidad ON negocios(json_extract(evidencia_rubro,'$.tipo_entidad'),admisible);
        CREATE INDEX IF NOT EXISTS idx_oportunidades_negocio_score ON oportunidades_servicio(negocio_id,score DESC);
        CREATE INDEX IF NOT EXISTS idx_jobs_tecnica_lookup ON jobs(
            CAST(json_extract(params,'$.negocio_id') AS INTEGER),estado,json_extract(params,'$.url')
        ) WHERE tipo='technical';",
    )?;
    crate::web::init(&conn)?;
    asegurar_fuentes_iniciales(&conn)?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(4,?1)",
        [ahora()],
    )?;
    Ok(Arc::new(Mutex::new(conn)))
}

/// Genera una copia SQLite consistente diaria y conserva las últimas `retener` copias.
pub fn respaldar_si_nuevo_dia(db_path: &str, retener: usize) -> Result<Option<std::path::PathBuf>> {
    use rusqlite::backup::Backup;
    use std::{path::Path, time::Duration};
    let db = Path::new(db_path);
    if db_path == ":memory:" || !db.exists() || std::fs::metadata(db)?.len() == 0 {
        return Ok(None);
    }
    let parent = db
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dir = std::env::var_os("RADAR_BACKUP_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| parent.join("backups"));
    std::fs::create_dir_all(&dir)?;
    let stem = db.file_stem().and_then(|s| s.to_str()).unwrap_or("radar");
    let fecha = chrono::Local::now().format("%Y-%m-%d");
    let destino = dir.join(format!("{stem}-{fecha}.sqlite"));
    if destino.exists() {
        return Ok(None);
    }
    let temporal = dir.join(format!(".{stem}-{fecha}.sqlite.part"));
    let origen = Connection::open(db_path)?;
    let mut copia = Connection::open(&temporal)?;
    if let Err(e) = Backup::new(&origen, &mut copia)
        .and_then(|b| b.run_to_completion(128, Duration::from_millis(100), None))
    {
        drop(copia);
        let _ = std::fs::remove_file(&temporal);
        return Err(e.into());
    }
    drop(copia);
    std::fs::rename(&temporal, &destino)?;
    let mut copias = std::fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("{stem}-")) && n.ends_with(".sqlite"))
        })
        .collect::<Vec<_>>();
    copias.sort();
    let exceso = copias.len().saturating_sub(retener.max(1));
    for viejo in copias.into_iter().take(exceso) {
        std::fs::remove_file(viejo)?;
    }
    Ok(Some(destino))
}

#[cfg(test)]
mod maintenance_tests {
    use super::*;

    #[test]
    fn normaliza_nombres_con_mayusculas_y_acentos() {
        assert_eq!(normalizar_identidad("CAFÉ Ñandú, S.L."), "cafenandusl");
    }

    #[test]
    fn backup_sqlite_es_consistente_y_no_sobrescribe_el_del_dia() {
        let root = std::env::temp_dir().join(format!("radar-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("radar.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE prueba(v TEXT); INSERT INTO prueba VALUES('persistido');")
            .unwrap();
        drop(conn);
        std::env::set_var("RADAR_BACKUP_DIR", root.join("copias"));
        let copia = respaldar_si_nuevo_dia(db.to_str().unwrap(), 14)
            .unwrap()
            .unwrap();
        let backup = Connection::open(copia).unwrap();
        let valor: String = backup
            .query_row("SELECT v FROM prueba", [], |r| r.get(0))
            .unwrap();
        assert_eq!(valor, "persistido");
        assert!(respaldar_si_nuevo_dia(db.to_str().unwrap(), 14)
            .unwrap()
            .is_none());
        std::env::remove_var("RADAR_BACKUP_DIR");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migracion_de_rubro_acepta_bases_existentes_y_reclasifica_categoria() {
        let root =
            std::env::temp_dir().join(format!("radar-rubro-migration-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let db_path = root.join("radar.sqlite");
        let old = Connection::open(&db_path).unwrap();
        old.execute_batch("CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,applied_at TEXT NOT NULL); INSERT INTO schema_migrations VALUES(6,'2026-11-04'); CREATE TABLE negocios(id INTEGER PRIMARY KEY,nombre TEXT NOT NULL,categoria TEXT,sector TEXT,direccion TEXT,municipio TEXT NOT NULL,lat REAL,lon REAL,telefono TEXT,email TEXT,web TEXT,fuente TEXT NOT NULL,fuente_id TEXT NOT NULL,primera_vista TEXT NOT NULL,ultima_verificacion TEXT,UNIQUE(fuente,fuente_id)); INSERT INTO negocios(id,nombre,categoria,sector,municipio,fuente,fuente_id,primera_vista) VALUES(1,'Flor de prueba','florist','comercio','Tudela','osm','node-1','2026-11-04');").unwrap();
        drop(old);
        let migrated = abrir(db_path.to_str().unwrap()).unwrap();
        let conn = migrated.lock().unwrap();
        let (rubro, evidencia): (String, String) = conn
            .query_row(
                "SELECT rubro,evidencia_rubro FROM negocios WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(rubro, "floristeria");
        assert_eq!(evidencia, "{}");
        drop(conn);
        let _ = std::fs::remove_dir_all(root);
    }
}

fn asegurar_fuentes_iniciales(conn: &Connection) -> Result<()> {
    let fecha = ahora();
    conn.execute("INSERT OR IGNORE INTO fuentes(nombre,tipo,url,condiciones,atribucion,revisada_en,habilitada,profundidad,max_paginas_dia) VALUES('BOE - BORME Navarra','dataset','https://www.boe.es/datosabiertos/api/borme/sumario','Datos públicos del BORME; conservar referencia al BOE, documento y fecha de publicación','Agencia Estatal Boletín Oficial del Estado',?1,1,0,100)",params![fecha])?;
    conn.execute("INSERT OR IGNORE INTO fuentes(nombre,tipo,url,condiciones,atribucion,revisada_en,habilitada,profundidad,max_paginas_dia) VALUES('Open Data Navarra','semilla','https://datosabiertos.navarra.es/es/dataset/cat-logo-de-datasets','Creative Commons Attribution 4.0; conservar fuente y fecha de actualización','Gobierno de Navarra - Datos Abiertos (CC BY 4.0)',?1,1,2,100)", params![fecha])?;
    conn.execute("INSERT OR IGNORE INTO fuentes(nombre,tipo,url,condiciones,atribucion,revisada_en,habilitada,profundidad,max_paginas_dia) VALUES('OpenStreetMap Navarra (Geofabrik)','dataset','https://download.geofabrik.de/europe/spain/navarra-latest.osm.pbf','ODbL 1.0; incluir atribución visible a OpenStreetMap','© OpenStreetMap contributors (ODbL)',?1,1,0,100)", params![fecha])?;
    Ok(())
}

fn migrar(conn: &Connection) -> Result<()> {
    let version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    if version < 1 {
        let tiene_resumen = {
            let mut stmt = conn.prepare("PRAGMA table_info(jobs)")?;
            let columnas = stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columnas.iter().any(|n| n == "resumen")
        };
        if !tiene_resumen {
            conn.execute("ALTER TABLE jobs ADD COLUMN resumen TEXT", [])?;
        }
        conn.execute(
            "INSERT INTO schema_migrations(version,applied_at) VALUES(1,?1)",
            [ahora()],
        )?;
    }
    let version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    if version < 2 {
        let tiene_fecha = {
            let mut stmt = conn.prepare("PRAGMA table_info(pipeline)")?;
            let columnas = stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columnas.iter().any(|n| n == "siguiente_accion_fecha")
        };
        if !tiene_fecha {
            conn.execute(
                "ALTER TABLE pipeline ADD COLUMN siguiente_accion_fecha TEXT",
                [],
            )?;
        }
        conn.execute(
            "INSERT INTO schema_migrations(version,applied_at) VALUES(2,?1)",
            [ahora()],
        )?;
    }
    let version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    if version < 3 {
        let tiene_fecha = {
            let mut stmt = conn.prepare("PRAGMA table_info(pipeline)")?;
            let columnas = stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columnas.iter().any(|n| n == "siguiente_accion_fecha")
        };
        if !tiene_fecha {
            conn.execute(
                "ALTER TABLE pipeline ADD COLUMN siguiente_accion_fecha TEXT",
                [],
            )?;
        }
        conn.execute(
            "INSERT INTO schema_migrations(version,applied_at) VALUES(3,?1)",
            [ahora()],
        )?;
    }
    Ok(())
}

pub fn ahora() -> String {
    chrono::Local::now().to_rfc3339()
}

fn row_negocio(row: &rusqlite::Row) -> rusqlite::Result<Negocio> {
    Ok(Negocio {
        id: row.get(0)?,
        nombre: row.get(1)?,
        categoria: row.get(2)?,
        sector: row.get(3)?,
        rubro: row.get(17).unwrap_or(None),
        evidencia_rubro: row
            .get::<_, Option<String>>(18)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok()),
        direccion: row.get(4)?,
        municipio: row.get(5)?,
        lat: row.get(6)?,
        lon: row.get(7)?,
        telefono: row.get(8)?,
        email: row.get(9)?,
        web: row.get(10)?,
        estado_web_lista: row.get(15).unwrap_or_else(|_| "sin_localizar".into()),
        estado_auditoria_tecnica: row.get(16).unwrap_or_else(|_| "pendiente".into()),
        fuente: row.get(11)?,
        fuente_id: row.get(12)?,
        primera_vista: row.get(13)?,
        ultima_verificacion: row.get(14)?,
        score_potencial: None,
        prioridad_comercial: None,
        estado_pipeline: None,
        oportunidades: Vec::new(),
        descartado: false,
    })
}

/// Inserta o actualiza por (fuente, fuente_id). Devuelve el id interno.
pub fn upsert_negocio(conn: &Connection, n: &Negocio) -> Result<i64> {
    conn.execute(
        "INSERT INTO negocios (id, nombre, categoria, sector, rubro, evidencia_rubro, direccion, municipio, lat, lon,
             telefono, email, web, fuente, fuente_id, primera_vista, ultima_verificacion)
           VALUES ((SELECT MAX(COALESCE((SELECT MAX(id) FROM negocios),0),COALESCE((SELECT CAST(value AS INTEGER) FROM meta WHERE key='negocios_id_floor'),0))+1),?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
           ON CONFLICT(fuente, fuente_id) DO UPDATE SET
             nombre=excluded.nombre, categoria=excluded.categoria, sector=excluded.sector,
             rubro=COALESCE(excluded.rubro,negocios.rubro), evidencia_rubro=excluded.evidencia_rubro,
             direccion=CASE WHEN EXISTS(SELECT 1 FROM web_estado w WHERE w.negocio_id=negocios.id AND w.origen IN ('correccion_usuario','confirmacion_usuario')) THEN COALESCE(negocios.direccion,excluded.direccion) ELSE COALESCE(excluded.direccion,negocios.direccion) END, municipio=CASE WHEN EXISTS(SELECT 1 FROM web_estado w WHERE w.negocio_id=negocios.id AND w.origen IN ('correccion_usuario','confirmacion_usuario')) THEN negocios.municipio ELSE excluded.municipio END, lat=excluded.lat,
             lon=excluded.lon, telefono=excluded.telefono, email=excluded.email,
             web=CASE WHEN EXISTS(SELECT 1 FROM web_estado w WHERE w.negocio_id=negocios.id AND w.url_confirmada IS NOT NULL) THEN negocios.web ELSE COALESCE(excluded.web,negocios.web) END, ultima_verificacion=excluded.ultima_verificacion",
        params![
            n.nombre, n.categoria, n.sector, n.rubro, n.evidencia_rubro.as_ref().map(serde_json::Value::to_string).unwrap_or_else(|| "{}".into()), n.direccion, n.municipio, n.lat, n.lon,
            n.telefono, n.email, Option::<String>::None, n.fuente, n.fuente_id, n.primera_vista,
            n.ultima_verificacion
        ],
    )?;
    let id = conn.query_row(
        "SELECT id FROM negocios WHERE fuente=?1 AND fuente_id=?2",
        params![n.fuente, n.fuente_id],
        |r| r.get(0),
    )?;
    if let Some(url) = n.web.as_deref() {
        crate::web::add_candidate(
            conn,
            id,
            url,
            None,
            &format!("fuente:{}", n.fuente),
            None,
            None,
            &serde_json::json!({"fuente":n.fuente,"external_id":n.fuente_id}),
        )?;
    }
    Ok(id)
}

pub fn set_ultima_verificacion(conn: &Connection, id: i64, cuando: &str) -> Result<()> {
    conn.execute(
        "UPDATE negocios SET ultima_verificacion=?1 WHERE id=?2",
        params![cuando, id],
    )?;
    Ok(())
}

pub fn insert_auditoria(conn: &Connection, a: &Auditoria) -> Result<i64> {
    conn.execute(
        "INSERT INTO auditorias (negocio_id, fecha, estado_web, score, prioridad, senales)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            a.negocio_id,
            a.fecha,
            a.estado_web.as_str(),
            a.score,
            a.prioridad,
            a.senales.to_string()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn estado_oportunidad(s: &str) -> EstadoOportunidad {
    match s {
        "propuesta" => EstadoOportunidad::Propuesta,
        "contratada" => EstadoOportunidad::Contratada,
        "descartada" => EstadoOportunidad::Descartada,
        _ => EstadoOportunidad::Pendiente,
    }
}

pub fn guardar_oportunidades_auditoria(conn: &Connection, auditoria: &Auditoria) -> Result<()> {
    let fecha = &auditoria.fecha;
    let mut signals = auditoria.senales.clone();
    let completed = conn
        .query_row(
            "SELECT estado='no_localizada' FROM web_estado WHERE negocio_id=?1",
            [auditoria.negocio_id],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(false);
    signals["busqueda_completada"] = serde_json::json!(completed);
    agregar_senales_rubro(conn, auditoria.negocio_id, &mut signals)?;
    for op in crate::scoring::puntuar_servicios(
        auditoria.negocio_id,
        auditoria.estado_web,
        &signals,
        fecha,
    ) {
        conn.execute(
            "INSERT INTO oportunidades_servicio (negocio_id,servicio,score,estado,motivo,fecha)
             VALUES (?1,?2,?3,'pendiente',?4,?5)
             ON CONFLICT(negocio_id,servicio) DO UPDATE SET
               score=excluded.score,motivo=excluded.motivo,fecha=excluded.fecha",
            params![
                op.negocio_id,
                op.servicio.as_str(),
                op.score,
                op.motivo,
                op.fecha
            ],
        )?;
    }
    if let Some(url) = auditoria.senales["web"].as_str() {
        crate::technical::update_opportunities(conn, auditoria.negocio_id, url)?;
    }
    Ok(())
}

fn agregar_senales_rubro(
    conn: &Connection,
    negocio_id: i64,
    signals: &mut serde_json::Value,
) -> Result<()> {
    let raw: String = conn
        .query_row(
            "SELECT evidencia_rubro FROM negocios WHERE id=?1",
            [negocio_id],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or_else(|| "{}".into());
    let evidence: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::json!({}));
    let fee = evidence["tarifa"]["estado"].as_str().unwrap_or("");
    signals["visita_con_tarifa_declarada"] =
        serde_json::json!(fee == "declarada_de_pago" || fee == "precio_declarado");
    Ok(())
}

pub fn oportunidades_negocio(
    conn: &Connection,
    negocio_id: i64,
) -> Result<Vec<OportunidadServicio>> {
    let mut stmt = conn.prepare(
        "SELECT negocio_id,servicio,score,estado,motivo,fecha FROM oportunidades_servicio
         WHERE negocio_id=?1 ORDER BY score DESC, servicio",
    )?;
    let rows = stmt.query_map(params![negocio_id], |r| {
        let s: String = r.get(1)?;
        let e: String = r.get(3)?;
        Ok(OportunidadServicio {
            negocio_id: r.get(0)?,
            servicio: ServicioWeb::parse(&s).unwrap_or(ServicioWeb::CreacionWeb),
            score: r.get(2)?,
            estado: estado_oportunidad(&e),
            motivo: r.get(4)?,
            fecha: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn asegurar_oportunidades(conn: &Connection, negocio_id: i64) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM oportunidades_servicio WHERE negocio_id=?1",
        params![negocio_id],
        |r| r.get(0),
    )?;
    if count > 0 {
        return Ok(());
    }
    let audit: Option<(String, String, String)> = conn.query_row(
        "SELECT estado_web,senales,fecha FROM auditorias WHERE negocio_id=?1 ORDER BY fecha DESC,id DESC LIMIT 1",
        params![negocio_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)),
    ).optional()?;
    let (estado, senales, fecha) = if let Some((estado, senales, fecha)) = audit {
        (
            EstadoWeb::parse(&estado).unwrap_or(EstadoWeb::SinWeb),
            serde_json::from_str(&senales).unwrap_or(serde_json::json!({})),
            fecha,
        )
    } else {
        let web: Option<String> = conn.query_row(
            "SELECT web FROM negocios WHERE id=?1 AND admisible=1",
            params![negocio_id],
            |r| r.get(0),
        )?;
        let estado = match web.as_deref() {
            None => EstadoWeb::SinWeb,
            Some(w) if crate::scoring::es_red_social(w) => EstadoWeb::SoloRedes,
            Some(_) => EstadoWeb::WebMejorable,
        };
        (estado, serde_json::json!({"sin_auditar":true}), ahora())
    };
    let mut senales = senales;
    agregar_senales_rubro(conn, negocio_id, &mut senales)?;
    if estado == EstadoWeb::SinWeb && !senales["busqueda_completada"].is_boolean() {
        let completed = conn
            .query_row(
                "SELECT estado='no_localizada' FROM web_estado WHERE negocio_id=?1",
                [negocio_id],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false);
        senales["busqueda_completada"] = serde_json::json!(completed);
    }
    for op in crate::scoring::puntuar_servicios(negocio_id, estado, &senales, &fecha) {
        conn.execute(
            "INSERT OR IGNORE INTO oportunidades_servicio (negocio_id,servicio,score,estado,motivo,fecha)
             VALUES (?1,?2,?3,'pendiente',?4,?5)",
            params![op.negocio_id,op.servicio.as_str(),op.score,op.motivo,op.fecha],
        )?;
    }
    Ok(())
}

pub fn backfill_oportunidades(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let ids = {
        let mut stmt = tx.prepare("SELECT id FROM negocios WHERE admisible=1")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for id in ids {
        asegurar_oportunidades(&tx, id)?;
    }
    tx.commit()?;
    Ok(())
}

pub fn actualizar_estado_oportunidad(
    conn: &Connection,
    negocio_id: i64,
    servicio: ServicioWeb,
    estado: EstadoOportunidad,
) -> Result<Option<OportunidadServicio>> {
    let valor = match estado {
        EstadoOportunidad::Pendiente => "pendiente",
        EstadoOportunidad::Propuesta => "propuesta",
        EstadoOportunidad::Contratada => "contratada",
        EstadoOportunidad::Descartada => "descartada",
    };
    conn.execute(
        "UPDATE oportunidades_servicio SET estado=?1 WHERE negocio_id=?2 AND servicio=?3",
        params![valor, negocio_id, servicio.as_str()],
    )?;
    Ok(oportunidades_negocio(conn, negocio_id)?
        .into_iter()
        .find(|o| o.servicio == servicio))
}

fn row_auditoria(row: &rusqlite::Row) -> rusqlite::Result<Auditoria> {
    let estado = EstadoWeb::parse(&row.get::<_, String>(3)?).unwrap_or(EstadoWeb::SinWeb);
    Ok(Auditoria {
        id: row.get(0)?,
        negocio_id: row.get(1)?,
        fecha: row.get(2)?,
        estado_web: estado,
        score: row.get(4)?,
        prioridad: row.get(5)?,
        senales: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(serde_json::json!({})),
    })
}

/// Última auditoría por negocio (subselect reutilizado en filtros y ordenación).
const ULTIMA_AUDITORIA: &str =
    "(SELECT a.estado_web FROM auditorias a WHERE a.negocio_id = negocios.id ORDER BY a.fecha DESC, a.id DESC LIMIT 1)";
const ULTIMO_SCORE: &str =
    "(SELECT a.score FROM auditorias a WHERE a.negocio_id = negocios.id ORDER BY a.fecha DESC, a.id DESC LIMIT 1)";
const ULTIMO_POTENCIAL: &str =
    "(SELECT MAX(o.score) FROM oportunidades_servicio o WHERE o.negocio_id=negocios.id)";
const ULTIMA_PRIORIDAD: &str =
    "(SELECT a.prioridad FROM auditorias a WHERE a.negocio_id = negocios.id ORDER BY a.fecha DESC, a.id DESC LIMIT 1)";

const ESTADO_WEB_LISTA: &str = "CASE
    WHEN EXISTS(SELECT 1 FROM web_estado w WHERE w.negocio_id=negocios.id AND w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario') AND w.url_confirmada IS NOT NULL) THEN 'confirmada'
    WHEN EXISTS(SELECT 1 FROM web_estado w WHERE w.negocio_id=negocios.id AND w.estado='verificada_automatica' AND w.origen='verificacion_automatica' AND w.url_confirmada IS NOT NULL) THEN 'verificada'
    WHEN EXISTS(SELECT 1 FROM web_candidatas c WHERE c.negocio_id=negocios.id AND c.estado='sugerida') THEN 'sugerida'
    ELSE 'sin_localizar' END";
const ESTADO_AUDITORIA_TECNICA: &str = "CASE
    WHEN NOT EXISTS(SELECT 1 FROM web_estado w WHERE w.negocio_id=negocios.id
      AND ((w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario'))
        OR (w.estado='verificada_automatica' AND w.origen='verificacion_automatica'))
      AND w.url_confirmada IS NOT NULL) THEN 'pendiente'
    WHEN EXISTS(SELECT 1 FROM jobs j WHERE j.tipo='technical' AND j.estado='en_curso'
      AND CAST(json_extract(j.params,'$.negocio_id') AS INTEGER)=negocios.id
      AND json_extract(j.params,'$.url')=(SELECT w.url_confirmada FROM web_estado w WHERE w.negocio_id=negocios.id AND ((w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario')) OR (w.estado='verificada_automatica' AND w.origen='verificacion_automatica')))) THEN 'en_curso'
    WHEN EXISTS(SELECT 1 FROM jobs j WHERE j.tipo='technical' AND j.estado='pendiente'
      AND CAST(json_extract(j.params,'$.negocio_id') AS INTEGER)=negocios.id
      AND json_extract(j.params,'$.url')=(SELECT w.url_confirmada FROM web_estado w WHERE w.negocio_id=negocios.id AND ((w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario')) OR (w.estado='verificada_automatica' AND w.origen='verificacion_automatica')))) THEN 'pendiente'
    ELSE COALESCE((SELECT CASE WHEN t.estado IN ('completo','parcial') THEN 'realizada' WHEN t.estado IN ('error','no_disponible') THEN 'error' ELSE 'pendiente' END
      FROM auditorias_tecnicas t WHERE t.negocio_id=negocios.id
      AND t.url=(SELECT w.url_confirmada FROM web_estado w WHERE w.negocio_id=negocios.id AND ((w.estado='confirmada' AND w.origen IN ('correccion_usuario','confirmacion_usuario')) OR (w.estado='verificada_automatica' AND w.origen='verificacion_automatica')))
      ORDER BY t.fecha DESC,t.rowid DESC LIMIT 1),'pendiente') END";

fn filtros_negocios_sql(f: &FiltrosNegocios) -> Result<(String, Vec<rusqlite::types::Value>)> {
    use rusqlite::types::Value;
    let mut wh: Vec<String> = vec!["admisible=1".into()];
    let mut args: Vec<Value> = Vec::new();
    if let Some(m) = &f.municipio {
        wh.push("municipio LIKE ?".into());
        args.push(Value::from(format!("%{m}%")));
    }
    if f.mostrar_descartados {
        wh.push(
            "EXISTS (SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=negocios.id)".into(),
        );
    } else {
        wh.push(
            "NOT EXISTS (SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=negocios.id)"
                .into(),
        );
    }
    if let Some(s) = &f.sector {
        wh.push("sector = ?".into());
        args.push(Value::from(s.clone()));
    }
    if let Some(r) = &f.rubro {
        wh.push("rubro = ?".into());
        args.push(Value::from(r.clone()));
    }
    if let Some(t) = &f.tipo_entidad {
        wh.push("json_extract(evidencia_rubro,'$.tipo_entidad') = ?".into());
        args.push(Value::from(t.clone()));
    }
    if let Some(q) = &f.q {
        wh.push("nombre LIKE ?".into());
        args.push(Value::from(format!("%{q}%")));
    }
    if let Some(e) = &f.estado_web {
        wh.push(format!("{ULTIMA_AUDITORIA} = ?"));
        args.push(Value::from(e.as_str().to_string()));
    }
    if let Some(e) = &f.estado_auditoria_tecnica {
        if ["realizada", "en_curso", "pendiente", "error"].contains(&e.as_str()) {
            wh.push(format!("({ESTADO_AUDITORIA_TECNICA}) = ?"));
            args.push(Value::from(e.clone()));
        }
    }
    if let Some(e) = &f.estado_pipeline {
        if [
            "nuevo",
            "contactado",
            "visita_hecha",
            "interesado",
            "no_interesado",
            "propuesta",
            "cerrado",
        ]
        .contains(&e.as_str())
        {
            wh.push("COALESCE((SELECT p.estado FROM pipeline p WHERE p.negocio_id=negocios.id),'nuevo') = ?".into());
            args.push(Value::from(e.clone()));
        }
    }
    if let Some(p) = &f.prioridad {
        wh.push(format!("(CASE WHEN COALESCE({ULTIMO_POTENCIAL},0)>=70 THEN 'A' WHEN COALESCE({ULTIMO_POTENCIAL},0)>=50 THEN 'B' ELSE 'C' END) = ?"));
        args.push(Value::from(p.clone()));
    }
    if [f.sur, f.oeste, f.norte, f.este]
        .iter()
        .any(Option::is_some)
    {
        let z = zona_mapa(f)?;
        wh.push("lat BETWEEN ? AND ? AND lon BETWEEN ? AND ?".into());
        args.extend([z.sur, z.norte, z.oeste, z.este].map(Value::from));
    }
    Ok((format!("WHERE {}", wh.join(" AND ")), args))
}

pub fn list_negocios(conn: &Connection, f: &FiltrosNegocios) -> Result<Pagina<Negocio>> {
    use rusqlite::types::Value;
    let (where_sql, args) = filtros_negocios_sql(f)?;
    let total: i64 = {
        let sql = format!("SELECT COUNT(*) FROM negocios {where_sql}");
        let refs: Vec<&Value> = args.iter().collect();
        conn.query_row(&sql, params_from_iter(refs.into_iter()), |r| r.get(0))?
    };

    let ordenar = match f.ordenar.as_deref() {
        Some("nombre") => "nombre",
        Some("municipio") => "municipio",
        Some("fecha") => "ultima_verificacion",
        Some("score") | _ => ULTIMO_POTENCIAL,
    };
    let dir = if f.dir.as_deref() == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let tam = f.tam.unwrap_or(50).clamp(1, 200);
    let pagina = f.pagina.unwrap_or(1).max(1);
    let offset = (pagina - 1).saturating_mul(tam);

    // Ordenar y paginar IDs antes de calcular estados o leer evidencias/fichas.
    // SQLite puede evaluar las columnas del SELECT antes del LIMIT al ordenar.
    let sql = format!(
        "SELECT id FROM negocios {where_sql}
         ORDER BY {ordenar} {dir} NULLS LAST, id ASC LIMIT ? OFFSET ?"
    );
    let mut all: Vec<Value> = args.clone();
    all.push(Value::from(tam));
    all.push(Value::from(offset));
    let refs: Vec<&Value> = all.iter().collect();
    let mut stmt = conn.prepare(&sql)?;
    let ids: Vec<i64> = stmt
        .query_map(params_from_iter(refs.into_iter()), |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(mut negocio) = get_negocio(conn, id)? {
            enriquecer_negocio(conn, &mut negocio)?;
            items.push(negocio);
        }
    }

    Ok(Pagina {
        items,
        total,
        pagina,
        tam,
    })
}

fn enriquecer_negocio(conn: &Connection, negocio: &mut Negocio) -> Result<()> {
    asegurar_oportunidades(conn, negocio.id)?;
    negocio.oportunidades = oportunidades_negocio(conn, negocio.id)?;
    negocio.score_potencial = negocio.oportunidades.iter().map(|o| o.score).max();
    negocio.prioridad_comercial = negocio.score_potencial.map(crate::scoring::prioridad);
    negocio.estado_pipeline = conn
        .query_row(
            "SELECT estado FROM pipeline WHERE negocio_id=?1",
            params![negocio.id],
            |r| {
                Ok(match r.get::<_, String>(0)?.as_str() {
                    "contactado" => EstadoPipeline::Contactado,
                    "visita_hecha" => EstadoPipeline::VisitaHecha,
                    "interesado" => EstadoPipeline::Interesado,
                    "no_interesado" => EstadoPipeline::NoInteresado,
                    "propuesta" => EstadoPipeline::Propuesta,
                    "cerrado" => EstadoPipeline::Cerrado,
                    _ => EstadoPipeline::Nuevo,
                })
            },
        )
        .optional()?;
    negocio.descartado = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM negocios_descartados WHERE negocio_id=?1)",
        params![negocio.id],
        |r| r.get(0),
    )?;
    Ok(())
}

/// Sin zona explícita, se devuelve una vista general acotada del mundo.
/// No se admiten zonas parciales, invertidas ni coordenadas fuera de Web Mercator.
pub fn zona_mapa(f: &FiltrosNegocios) -> Result<ZonaMapa> {
    let zona = match (f.sur, f.oeste, f.norte, f.este) {
        (None, None, None, None) => ZonaMapa {
            sur: -85.05112878,
            oeste: -180.0,
            norte: 85.05112878,
            este: 180.0,
        },
        (Some(sur), Some(oeste), Some(norte), Some(este)) => ZonaMapa {
            sur,
            oeste,
            norte,
            este,
        },
        _ => anyhow::bail!("la zona necesita sur, oeste, norte y este"),
    };
    anyhow::ensure!(
        [zona.sur, zona.oeste, zona.norte, zona.este]
            .iter()
            .all(|v| v.is_finite())
            && zona.sur >= -85.05112878
            && zona.norte <= 85.05112878
            && zona.oeste >= -180.0
            && zona.este <= 180.0
            && zona.sur <= zona.norte
            && zona.oeste <= zona.este,
        "zona geográfica inválida"
    );
    Ok(zona)
}

fn mercator_y(lat: f64) -> f64 {
    (1.0 - lat.to_radians().tan().asinh() / std::f64::consts::PI) / 2.0
}

pub fn list_negocios_mapa(
    conn: &Connection,
    f: &FiltrosNegocios,
    zoom: u8,
) -> Result<RespuestaMapa> {
    use std::collections::BTreeMap;
    let zona = zona_mapa(f)?;
    let (where_sql, args) = filtros_negocios_sql(f)?;
    // Una sola lectura ligera: sin COUNT repetidos, ordenación, paginación ni
    // enriquecimiento de fichas. Solo las oportunidades ya calculadas en ingesta.
    let sql = format!(
        "SELECT id,nombre,sector,lat,lon,{ULTIMO_POTENCIAL} FROM negocios {where_sql}
         AND lat BETWEEN -85.05112878 AND 85.05112878 AND lon BETWEEN -180 AND 180"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params_from_iter(args.iter()))?;
    // Celdas de al menos 64 px al zoom solicitado. Incluso si se pide todo el
    // mundo al zoom máximo, como mucho hay 33 × 25 = 825 celdas ocupadas.
    let celda = (64.0 / (256.0 * 2.0_f64.powi(zoom.min(19) as i32)))
        .max((zona.este - zona.oeste) / 360.0 / 32.0)
        .max((mercator_y(zona.sur) - mercator_y(zona.norte)) / 24.0);
    let mut bins: BTreeMap<(i64, i64), PuntoMapa> = BTreeMap::new();
    let mut total = 0;
    let mut extension: Option<ZonaMapa> = None;
    while let Some(row) = rows.next()? {
        let lat: f64 = row.get(3)?;
        let lon: f64 = row.get(4)?;
        let score: Option<i64> = row.get(5)?;
        let key = (
            ((lon + 180.0) / 360.0 / celda).floor() as i64,
            (mercator_y(lat) / celda).floor() as i64,
        );
        let punto_zona = ZonaMapa {
            sur: lat,
            oeste: lon,
            norte: lat,
            este: lon,
        };
        let z = extension.get_or_insert(punto_zona);
        z.sur = z.sur.min(lat);
        z.norte = z.norte.max(lat);
        z.oeste = z.oeste.min(lon);
        z.este = z.este.max(lon);
        total += 1;
        if let Some(p) = bins.get_mut(&key) {
            p.cantidad += 1;
            p.lat += (lat - p.lat) / p.cantidad as f64;
            p.lon += (lon - p.lon) / p.cantidad as f64;
            p.score_potencial = p.score_potencial.max(score);
            p.zona.sur = p.zona.sur.min(lat);
            p.zona.norte = p.zona.norte.max(lat);
            p.zona.oeste = p.zona.oeste.min(lon);
            p.zona.este = p.zona.este.max(lon);
            p.id = None;
            p.nombre = None;
            p.sector = None;
        } else {
            bins.insert(
                key,
                PuntoMapa {
                    id: Some(row.get(0)?),
                    nombre: Some(row.get(1)?),
                    sector: row.get(2)?,
                    lat,
                    lon,
                    cantidad: 1,
                    score_potencial: score,
                    zona: punto_zona,
                },
            );
        }
    }
    Ok(RespuestaMapa {
        items: bins.into_values().collect(),
        total,
        zona: extension,
    })
}

pub fn get_negocio(conn: &Connection, id: i64) -> Result<Option<Negocio>> {
    let sql = format!(
        "SELECT id, nombre, categoria, sector, direccion, municipio, lat, lon, telefono, email,
                web, fuente, fuente_id, primera_vista, ultima_verificacion,
                {ESTADO_WEB_LISTA}, {ESTADO_AUDITORIA_TECNICA}, rubro, evidencia_rubro
         FROM negocios WHERE id=?1 AND admisible=1"
    );
    conn.query_row(&sql, params![id], row_negocio)
        .optional()
        .map_err(Into::into)
}

pub fn detalle_negocio(conn: &Connection, id: i64) -> Result<Option<NegocioDetalle>> {
    let Some(mut negocio) = get_negocio(conn, id)? else {
        return Ok(None);
    };
    enriquecer_negocio(conn, &mut negocio)?;
    let mut stmt = conn.prepare(
        "SELECT id, negocio_id, fecha, estado_web, score, prioridad, senales FROM auditorias
         WHERE negocio_id=?1 ORDER BY fecha DESC, id DESC LIMIT 20",
    )?;
    let auditorias = stmt
        .query_map(params![id], row_auditoria)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let pipeline = conn
        .query_row(
            "SELECT negocio_id, estado, valor_estimado, siguiente_accion, siguiente_accion_fecha, canal, fecha FROM pipeline WHERE negocio_id=?1",
            params![id],
            |r| {
                let estado = match r.get::<_, String>(1)?.as_str() {
                    "contactado" => EstadoPipeline::Contactado,
                    "visita_hecha" => EstadoPipeline::VisitaHecha,
                    "interesado" => EstadoPipeline::Interesado,
                    "no_interesado" => EstadoPipeline::NoInteresado,
                    "propuesta" => EstadoPipeline::Propuesta,
                    "cerrado" => EstadoPipeline::Cerrado,
                    _ => EstadoPipeline::Nuevo,
                };
                Ok(Pipeline {
                    negocio_id: r.get(0)?,
                    estado,
                    valor_estimado: r.get(2)?,
                    siguiente_accion: r.get(3)?,
                    siguiente_accion_fecha: r.get(4)?,
                    canal: r.get(5)?,
                    fecha: r.get(6)?,
                })
            },
        )
        .optional()?;
    let mut stmt = conn.prepare(
        "SELECT id, negocio_id, fecha, tipo, resultado, notas FROM eventos
         WHERE negocio_id=?1 ORDER BY fecha DESC, id DESC LIMIT 100",
    )?;
    let eventos = stmt
        .query_map(params![id], |r| {
            Ok(Evento {
                id: r.get(0)?,
                negocio_id: r.get(1)?,
                fecha: r.get(2)?,
                tipo: r.get(3)?,
                resultado: r.get(4)?,
                notas: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let borradores = borradores_negocio(conn, id)?;
    Ok(Some(NegocioDetalle {
        web_info: crate::web::info(conn, id)?,
        negocio,
        auditorias,
        pipeline,
        eventos,
        borradores,
    }))
}

/// Negocios pendientes de auditar (sin auditoría o con auditoría antigua) filtrados por prioridad.
pub fn negocios_para_auditar(
    conn: &Connection,
    prioridad: Option<&str>,
    limite: Option<i64>,
) -> Result<Vec<Negocio>> {
    let (wh, arg) = match prioridad {
        Some(p) => (format!("WHERE admisible=1 AND ({ULTIMA_PRIORIDAD} = ?1 OR {ULTIMA_PRIORIDAD} IS NULL) AND NOT EXISTS (SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=negocios.id)"), Some(p.to_string())),
        None => ("WHERE admisible=1 AND NOT EXISTS (SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=negocios.id)".into(), None),
    };
    let limite_sql = match limite {
        Some(l) => format!("LIMIT {}", l.max(1)),
        None => String::new(),
    };
    let sql = format!(
        "SELECT id, nombre, categoria, sector, direccion, municipio, lat, lon, telefono, email,
                web, fuente, fuente_id, primera_vista, ultima_verificacion,
                {ESTADO_WEB_LISTA}, {ESTADO_AUDITORIA_TECNICA}, rubro, evidencia_rubro
         FROM negocios {wh} ORDER BY {ULTIMO_SCORE} DESC NULLS LAST {limite_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let map = match &arg {
        Some(a) => stmt.query_map(params![a], row_negocio)?,
        None => stmt.query_map([], row_negocio)?,
    };
    Ok(map.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn preparar_job_items(
    conn: &Connection,
    job_id: &str,
    items: &[i64],
    kind: &str,
) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let ahora = ahora();
    for id in items {
        tx.execute("INSERT OR IGNORE INTO job_items(job_id,item_key,kind,estado,actualizado) VALUES(?1,?2,?3,'pendiente',?4)", params![job_id,id.to_string(),kind,ahora])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn job_items_pendientes(conn: &Connection, job_id: &str) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT item_key FROM job_items WHERE job_id=?1 AND estado NOT IN ('terminado','omitido') ORDER BY CAST(item_key AS INTEGER)")?;
    let keys = stmt
        .query_map(params![job_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    keys.into_iter()
        .map(|k| k.parse::<i64>().map_err(Into::into))
        .collect()
}

pub fn contar_job_items(conn: &Connection, job_id: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM job_items WHERE job_id=?1",
        params![job_id],
        |r| r.get(0),
    )?)
}

pub fn negocios_por_ids(conn: &Connection, ids: &[i64]) -> Result<Vec<Negocio>> {
    let mut out = Vec::with_capacity(ids.len());
    let sql = format!("SELECT id,nombre,categoria,sector,direccion,municipio,lat,lon,telefono,email,web,fuente,fuente_id,primera_vista,ultima_verificacion,{ESTADO_WEB_LISTA},{ESTADO_AUDITORIA_TECNICA},rubro,evidencia_rubro FROM negocios WHERE id=?1 AND admisible=1");
    let mut stmt = conn.prepare(&sql)?;
    for id in ids {
        if let Some(n) = stmt.query_row(params![id], row_negocio).optional()? {
            out.push(n);
        }
    }
    Ok(out)
}

pub fn terminar_job_items(conn: &Connection, job_id: &str, ids: &[i64]) -> Result<()> {
    let ahora = ahora();
    for id in ids {
        conn.execute("UPDATE job_items SET estado='terminado',error=NULL,actualizado=?3 WHERE job_id=?1 AND item_key=?2", params![job_id,id.to_string(),ahora])?;
    }
    Ok(())
}

pub fn actualizar_descartados(conn: &Connection, req: &ActualizarDescartados) -> Result<usize> {
    let mut total = 0;
    let fecha = ahora();
    for id in req
        .ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
    {
        if req.descartado {
            total += conn.execute(
                "INSERT OR IGNORE INTO negocios_descartados (negocio_id,fecha,motivo)
                 SELECT id,?2,?3 FROM negocios WHERE id=?1 AND admisible=1",
                params![id, fecha, req.motivo.as_deref()],
            )?;
        } else {
            total += conn.execute(
                "DELETE FROM negocios_descartados WHERE negocio_id=?1",
                params![id],
            )?;
        }
    }
    Ok(total)
}

pub fn set_pipeline(
    conn: &Connection,
    negocio_id: i64,
    req: &SetPipelineRequest,
) -> Result<Pipeline> {
    let fecha = ahora();
    let estado = req.estado.unwrap_or(EstadoPipeline::Nuevo);
    conn.execute(
        "INSERT INTO pipeline (negocio_id, estado, valor_estimado, siguiente_accion, siguiente_accion_fecha, canal, fecha)
         VALUES (?1,?2,?3,?4,?6,?7,?8)
         ON CONFLICT(negocio_id) DO UPDATE SET
           estado=COALESCE(?2, pipeline.estado),
           valor_estimado=COALESCE(?3, pipeline.valor_estimado),
           siguiente_accion=COALESCE(?4, pipeline.siguiente_accion),
           siguiente_accion_fecha=CASE WHEN ?5=1 THEN NULL ELSE COALESCE(?6,pipeline.siguiente_accion_fecha) END,
           canal=COALESCE(?7, pipeline.canal),
           fecha=?8",
        params![
            negocio_id,
            serde_json::to_value(estado)?.as_str().unwrap_or("nuevo"),
            req.valor_estimado,
            req.siguiente_accion,
            req.limpiar_fecha_siguiente_accion,
            req.siguiente_accion_fecha,
            req.canal,
            fecha
        ],
    )?;
    Ok(Pipeline {
        negocio_id,
        estado,
        valor_estimado: req.valor_estimado,
        siguiente_accion: req.siguiente_accion.clone(),
        siguiente_accion_fecha: req.siguiente_accion_fecha.clone(),
        canal: req.canal.clone(),
        fecha,
    })
}

pub fn add_evento(conn: &Connection, negocio_id: i64, ev: &NuevoEvento) -> Result<Evento> {
    let fecha = ahora();
    conn.execute(
        "INSERT INTO eventos (negocio_id, fecha, tipo, resultado, notas) VALUES (?1,?2,?3,?4,?5)",
        params![negocio_id, fecha, ev.tipo, ev.resultado, ev.notas],
    )?;
    Ok(Evento {
        id: conn.last_insert_rowid(),
        negocio_id,
        fecha,
        tipo: ev.tipo.clone(),
        resultado: ev.resultado.clone(),
        notas: ev.notas.clone(),
    })
}

pub fn list_eventos(conn: &Connection, negocio_id: i64) -> Result<Vec<Evento>> {
    let mut stmt = conn.prepare(
        "SELECT id, negocio_id, fecha, tipo, resultado, notas FROM eventos
         WHERE negocio_id=?1 ORDER BY fecha DESC, id DESC LIMIT 100",
    )?;
    let rows = stmt.query_map(params![negocio_id], |r| {
        Ok(Evento {
            id: r.get(0)?,
            negocio_id: r.get(1)?,
            fecha: r.get(2)?,
            tipo: r.get(3)?,
            resultado: r.get(4)?,
            notas: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn stats(conn: &Connection) -> Result<Stats> {
    let total_negocios: i64 =
        conn.query_row("SELECT COUNT(*) FROM negocios WHERE admisible=1", [], |r| {
            r.get(0)
        })?;
    let por_estado_web = contar(
        conn,
        &format!("SELECT {ULTIMA_AUDITORIA}, COUNT(*) FROM negocios WHERE admisible=1 GROUP BY 1"),
    )?;
    let por_prioridad = contar(
        conn,
        &format!("SELECT {ULTIMA_PRIORIDAD}, COUNT(*) FROM negocios WHERE admisible=1 GROUP BY 1"),
    )?;
    let por_pipeline = contar(conn, "SELECT estado, COUNT(*) FROM pipeline GROUP BY 1")?;
    let eventos_ultimos_7_dias: i64 = conn.query_row(
        "SELECT COUNT(*) FROM eventos WHERE fecha >= datetime('now', '-7 days')",
        [],
        |r| r.get(0),
    )?;
    let mut conversion_por_servicio = Vec::new();
    for servicio in [
        ServicioWeb::CreacionWeb,
        ServicioWeb::Landing,
        ServicioWeb::SeoOnPage,
        ServicioWeb::Hosting,
    ] {
        let (resueltas, contratadas): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(CASE WHEN estado='contratada' THEN 1 ELSE 0 END),0) FROM oportunidades_servicio WHERE servicio=?1 AND estado IN ('contratada','descartada')",
            params![servicio.as_str()], |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        conversion_por_servicio.push(ConversionServicio {
            servicio,
            resueltas,
            contratadas,
            tasa: (resueltas >= 30).then(|| contratadas as f64 / resueltas as f64),
        });
    }
    Ok(Stats {
        total_negocios,
        por_estado_web,
        por_prioridad,
        por_pipeline,
        eventos_ultimos_7_dias,
        conversion_por_servicio,
    })
}

pub fn fuentes(conn: &Connection) -> Result<Vec<Fuente>> {
    let mut stmt = conn.prepare("SELECT id,nombre,tipo,url,condiciones,atribucion,revisada_en,habilitada,profundidad,max_paginas_dia,ultima_ejecucion,error_ultima_ejecucion FROM fuentes ORDER BY nombre")?;
    let rows = stmt.query_map([], |r| {
        Ok(Fuente {
            id: r.get(0)?,
            nombre: r.get(1)?,
            tipo: r.get(2)?,
            url: r.get(3)?,
            condiciones: r.get(4)?,
            atribucion: r.get(5)?,
            revisada_en: r.get(6)?,
            habilitada: r.get::<_, i64>(7)? != 0,
            profundidad: r.get(8)?,
            max_paginas_dia: r.get(9)?,
            ultima_ejecucion: r.get(10)?,
            error_ultima_ejecucion: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn tareas_pendientes(conn: &Connection, hasta: &str) -> Result<Vec<TareaSeguimiento>> {
    let mut stmt=conn.prepare(&format!("SELECT p.negocio_id,n.nombre,n.municipio,p.siguiente_accion,p.siguiente_accion_fecha,p.estado,{ULTIMA_PRIORIDAD} FROM pipeline p JOIN negocios n ON n.id=p.negocio_id WHERE p.siguiente_accion_fecha IS NOT NULL AND date(p.siguiente_accion_fecha)<=date(?1) AND p.estado NOT IN ('cerrado','no_interesado') ORDER BY p.siguiente_accion_fecha"))?;
    let rows = stmt.query_map(params![hasta], |r| {
        Ok(TareaSeguimiento {
            negocio_id: r.get(0)?,
            nombre: r.get(1)?,
            municipio: r.get(2)?,
            siguiente_accion: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            fecha: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            estado: match r.get::<_, String>(5)?.as_str() {
                "contactado" => EstadoPipeline::Contactado,
                "visita_hecha" => EstadoPipeline::VisitaHecha,
                "interesado" => EstadoPipeline::Interesado,
                "propuesta" => EstadoPipeline::Propuesta,
                "cerrado" => EstadoPipeline::Cerrado,
                "no_interesado" => EstadoPipeline::NoInteresado,
                _ => EstadoPipeline::Nuevo,
            },
            prioridad: r.get(6)?,
            servicios: Vec::new(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn tareas_filtradas(
    conn: &Connection,
    desde: Option<&str>,
    hasta: &str,
    prioridad: Option<&str>,
    servicio: Option<&str>,
    estado: Option<&str>,
) -> Result<Vec<TareaSeguimiento>> {
    let mut tareas = tareas_pendientes(conn, hasta)?;
    for tarea in &mut tareas {
        let mut stmt = conn.prepare(
            "SELECT servicio FROM oportunidades_servicio WHERE negocio_id=?1 ORDER BY servicio",
        )?;
        tarea.servicios = stmt
            .query_map(params![tarea.negocio_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
    }
    if let Some(desde) = desde {
        tareas.retain(|t| t.fecha.as_str() >= desde);
    }
    if let Some(p) = prioridad {
        tareas.retain(|t| t.prioridad.as_deref() == Some(p));
    }
    if let Some(e) = estado {
        tareas.retain(|t| match t.estado { EstadoPipeline::Nuevo=>"nuevo",EstadoPipeline::Contactado=>"contactado",EstadoPipeline::VisitaHecha=>"visita_hecha",EstadoPipeline::Interesado=>"interesado",EstadoPipeline::NoInteresado=>"no_interesado",EstadoPipeline::Propuesta=>"propuesta",EstadoPipeline::Cerrado=>"cerrado" } == e);
    }
    if let Some(s) = servicio {
        tareas.retain(|t| t.servicios.iter().any(|x| x == s));
    }
    Ok(tareas)
}

pub fn cobertura_municipios(conn: &Connection) -> Result<Vec<CoberturaMunicipio>> {
    let mut stmt=conn.prepare("SELECT municipio,COUNT(*),SUM(CASE WHEN EXISTS(SELECT 1 FROM auditorias a WHERE a.negocio_id=n.id) THEN 1 ELSE 0 END) FROM negocios n WHERE n.admisible=1 AND NOT EXISTS(SELECT 1 FROM negocios_descartados d WHERE d.negocio_id=n.id) GROUP BY municipio ORDER BY COUNT(*) DESC,municipio")?;
    let rows = stmt.query_map([], |r| {
        Ok(CoberturaMunicipio {
            municipio: r.get(0)?,
            negocios: r.get(1)?,
            auditados: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn rechazar_duplicado(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.execute(
        "UPDATE duplicados_pendientes SET estado='no_duplicado' WHERE id=?1 AND estado='pendiente'",
        params![id],
    )? > 0)
}

pub fn plantillas_contacto(conn: &Connection) -> Result<Vec<PlantillaContacto>> {
    let mut stmt=conn.prepare("SELECT id,nombre,canal,servicio,asunto,cuerpo,actualizado FROM plantillas_contacto ORDER BY nombre")?;
    let rows = stmt.query_map([], |r| {
        Ok(PlantillaContacto {
            id: r.get(0)?,
            nombre: r.get(1)?,
            canal: r.get(2)?,
            servicio: r
                .get::<_, Option<String>>(3)?
                .as_deref()
                .and_then(ServicioWeb::parse),
            asunto: r.get(4)?,
            cuerpo: r.get(5)?,
            actualizado: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn crear_plantilla_contacto(conn: &Connection, p: &NuevaPlantillaContacto) -> Result<i64> {
    conn.execute("INSERT INTO plantillas_contacto(nombre,canal,servicio,asunto,cuerpo,actualizado) VALUES(?1,?2,?3,?4,?5,?6)",params![p.nombre,p.canal,p.servicio.map(ServicioWeb::as_str),p.asunto,p.cuerpo,ahora()])?;
    Ok(conn.last_insert_rowid())
}

pub fn crear_borrador(conn: &Connection, b: &NuevoBorrador) -> Result<i64> {
    conn.execute("INSERT INTO borradores_contacto(negocio_id,plantilla_id,canal,destino,asunto,cuerpo,creado,actualizado) VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",params![b.negocio_id,b.plantilla_id,b.canal,b.destino,b.asunto,b.cuerpo,ahora()])?;
    Ok(conn.last_insert_rowid())
}

pub fn borradores_negocio(conn: &Connection, negocio_id: i64) -> Result<Vec<BorradorContacto>> {
    let mut stmt=conn.prepare("SELECT id,negocio_id,canal,destino,asunto,cuerpo,creado,actualizado FROM borradores_contacto WHERE negocio_id=?1 ORDER BY actualizado DESC")?;
    let rows = stmt.query_map(params![negocio_id], |r| {
        Ok(BorradorContacto {
            id: r.get(0)?,
            negocio_id: r.get(1)?,
            canal: r.get(2)?,
            destino: r.get(3)?,
            asunto: r.get(4)?,
            cuerpo: r.get(5)?,
            creado: r.get(6)?,
            actualizado: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn fuente(conn: &Connection, id: i64) -> Result<Option<Fuente>> {
    conn.query_row("SELECT id,nombre,tipo,url,condiciones,atribucion,revisada_en,habilitada,profundidad,max_paginas_dia,ultima_ejecucion,error_ultima_ejecucion FROM fuentes WHERE id=?1", params![id], |r| Ok(Fuente {
        id:r.get(0)?, nombre:r.get(1)?, tipo:r.get(2)?, url:r.get(3)?, condiciones:r.get(4)?, atribucion:r.get(5)?, revisada_en:r.get(6)?,
        habilitada:r.get::<_,i64>(7)? != 0, profundidad:r.get(8)?, max_paginas_dia:r.get(9)?, ultima_ejecucion:r.get(10)?, error_ultima_ejecucion:r.get(11)?,
    })).optional().map_err(Into::into)
}

pub fn crear_fuente(conn: &Connection, nueva: &NuevaFuente) -> Result<i64> {
    conn.execute("INSERT INTO fuentes(nombre,tipo,url,condiciones,atribucion,revisada_en,habilitada,profundidad,max_paginas_dia) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![
        nueva.nombre,nueva.tipo,nueva.url,nueva.condiciones,nueva.atribucion,nueva.revisada_en,nueva.habilitada as i64,nueva.profundidad.clamp(0,4),nueva.max_paginas_dia.clamp(1,1000)
    ])?;
    Ok(conn.last_insert_rowid())
}

pub fn habilitar_fuente(conn: &Connection, id: i64, habilitada: bool) -> Result<bool> {
    Ok(conn.execute(
        "UPDATE fuentes SET habilitada=?2 WHERE id=?1",
        params![id, habilitada as i64],
    )? > 0)
}

pub fn actualizar_ejecucion_fuente(conn: &Connection, id: i64, error: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE fuentes SET ultima_ejecucion=?2,error_ultima_ejecucion=?3 WHERE id=?1",
        params![id, ahora(), error],
    )?;
    Ok(())
}

pub fn pagina_rastreada(conn: &Connection, fuente_id: i64, url: &str) -> Result<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM paginas_rastreadas WHERE fuente_id=?1 AND url=?2 AND rastreada_en >= date('now'))", params![fuente_id,url], |r| r.get(0))?)
}

pub fn registrar_pagina(
    conn: &Connection,
    fuente_id: i64,
    url: &str,
    profundidad: i64,
    status: Option<u16>,
    error: Option<&str>,
) -> Result<()> {
    conn.execute("INSERT INTO paginas_rastreadas(fuente_id,url,profundidad,rastreada_en,estado_http,error) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(fuente_id,url) DO UPDATE SET profundidad=excluded.profundidad,rastreada_en=excluded.rastreada_en,estado_http=excluded.estado_http,error=excluded.error", params![fuente_id,url,profundidad,ahora(),status.map(i64::from),error])?;
    Ok(())
}

pub fn paginas_hoy(conn: &Connection, fuente_id: i64) -> Result<i64> {
    Ok(conn.query_row("SELECT COUNT(*) FROM paginas_rastreadas WHERE fuente_id=?1 AND date(rastreada_en)=date('now')", params![fuente_id], |r| r.get(0))?)
}

pub fn guardar_atributo_fuente(
    conn: &Connection,
    negocio_id: i64,
    fuente_id: i64,
    atributo: &str,
    valor: &str,
    url: &str,
) -> Result<()> {
    conn.execute("INSERT INTO atributos_fuente(negocio_id,fuente_id,atributo,valor,url_origen,observado_en) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(negocio_id,fuente_id,atributo) DO UPDATE SET valor=excluded.valor,url_origen=excluded.url_origen,observado_en=excluded.observado_en", params![negocio_id,fuente_id,atributo,valor,url,ahora()])?;
    Ok(())
}

pub fn upsert_negocio_fuente(
    conn: &Connection,
    n: &Negocio,
    fuente_id: i64,
    external_id: &str,
    url_origen: &str,
) -> Result<(i64, bool)> {
    if let Some(id) = conn
        .query_row(
            "SELECT negocio_id FROM negocio_fuentes WHERE fuente_id=?1 AND external_id=?2",
            params![fuente_id, external_id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
    {
        conn.execute("UPDATE negocios SET telefono=COALESCE(telefono,?2),email=COALESCE(email,?3),direccion=COALESCE(direccion,?4) WHERE id=?1", params![id,n.telefono,n.email,n.direccion])?;
        if let Some(url) = n.web.as_deref() {
            crate::web::add_candidate(
                conn,
                id,
                url,
                None,
                "fuente",
                None,
                None,
                &serde_json::json!({"fuente":n.fuente,"external_id":external_id}),
            )?;
        }
        conn.execute("UPDATE negocio_fuentes SET visto_en=?3,url_origen=?4 WHERE fuente_id=?1 AND external_id=?2", params![fuente_id,external_id,ahora(),url_origen])?;
        for (field, value) in [
            ("nombre", Some(n.nombre.as_str())),
            ("municipio", Some(n.municipio.as_str())),
            ("web", n.web.as_deref()),
            ("telefono", n.telefono.as_deref()),
            ("email", n.email.as_deref()),
            ("direccion", n.direccion.as_deref()),
        ] {
            if let Some(value) = value {
                guardar_atributo_fuente(conn, id, fuente_id, field, value, url_origen)?;
            }
        }
        return Ok((id, false));
    }
    let incoming_domain = dominio_web(n.web.as_deref());
    let incoming_name = normalizar_identidad(&n.nombre);
    let incoming_city = normalizar_identidad(&n.municipio);
    let mut stmt =
        conn.prepare("SELECT id,nombre,municipio,web,direccion,lat,lon FROM negocios")?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<f64>>(5)?,
            r.get::<_, Option<f64>>(6)?,
        ))
    })?;
    let existentes = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let safe = existentes
        .iter()
        .find(|(_, name, city, web, address, lat, lon)| {
            !incoming_domain.is_empty()
                && dominio_web(web.as_deref()) == incoming_domain
                && normalizar_identidad(name) == incoming_name
                && normalizar_identidad(city) == incoming_city
                && (address
                    .as_deref()
                    .zip(n.direccion.as_deref())
                    .is_some_and(|(a, b)| {
                        !a.is_empty() && normalizar_identidad(a) == normalizar_identidad(b)
                    })
                    || lat
                        .zip(*lon)
                        .zip(n.lat.zip(n.lon))
                        .is_some_and(|((a, b), (c, d))| {
                            (a - c).abs() < 0.0002 && (b - d).abs() < 0.0002
                        }))
        })
        .map(|r| r.0);
    let candidate = if safe.is_none() {
        existentes
            .iter()
            .find(|(_, name, city, _, _, _, _)| {
                normalizar_identidad(name) == incoming_name
                    && normalizar_identidad(city) == incoming_city
            })
            .map(|r| r.0)
    } else {
        None
    };
    let (id, merged) = if let Some(id) = safe {
        conn.execute("UPDATE negocios SET telefono=COALESCE(telefono,?2),email=COALESCE(email,?3),direccion=COALESCE(direccion,?4) WHERE id=?1", params![id,n.telefono,n.email,n.direccion])?;
        if let Some(url) = n.web.as_deref() {
            crate::web::add_candidate(
                conn,
                id,
                url,
                None,
                "fuente",
                None,
                None,
                &serde_json::json!({"fuente":n.fuente,"external_id":external_id}),
            )?;
        }
        (id, true)
    } else {
        (upsert_negocio(conn, n)?, false)
    };
    conn.execute("INSERT INTO negocio_fuentes(negocio_id,fuente_id,external_id,url_origen,visto_en) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(fuente_id,external_id) DO UPDATE SET negocio_id=excluded.negocio_id,url_origen=excluded.url_origen,visto_en=excluded.visto_en", params![id,fuente_id,external_id,url_origen,ahora()])?;
    for (field, value) in [
        ("nombre", Some(n.nombre.as_str())),
        ("municipio", Some(n.municipio.as_str())),
        ("web", n.web.as_deref()),
        ("telefono", n.telefono.as_deref()),
        ("email", n.email.as_deref()),
        ("direccion", n.direccion.as_deref()),
    ] {
        if let Some(value) = value {
            guardar_atributo_fuente(conn, id, fuente_id, field, value, url_origen)?;
        }
    }
    if let Some(other) = candidate.filter(|other| *other != id) {
        let (a, b) = if other < id { (other, id) } else { (id, other) };
        conn.execute("INSERT OR IGNORE INTO duplicados_pendientes(negocio_a,negocio_b,razon,creado) VALUES(?1,?2,'mismo nombre y municipio; dominio distinto o no confirmado',?3)", params![a,b,ahora()])?;
    }
    Ok((id, merged))
}

fn normalizar_identidad(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .flat_map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => "a".chars().collect::<Vec<_>>(),
            'é' | 'è' | 'ë' | 'ê' => "e".chars().collect(),
            'í' | 'ì' | 'ï' | 'î' => "i".chars().collect(),
            'ó' | 'ò' | 'ö' | 'ô' => "o".chars().collect(),
            'ú' | 'ù' | 'ü' | 'û' => "u".chars().collect(),
            'ñ' => "n".chars().collect(),
            'ç' => "c".chars().collect(),
            c if c.is_ascii_alphanumeric() => vec![c],
            _ => Vec::new(),
        })
        .collect()
}

fn dominio_web(web: Option<&str>) -> String {
    web.and_then(|w| reqwest::Url::parse(w).ok())
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .map(|host| host.strip_prefix("www.").unwrap_or(&host).to_string())
        .unwrap_or_default()
}

pub fn registrar_fuente_candidata(
    conn: &Connection,
    url: &str,
    fuente_padre_id: i64,
) -> Result<()> {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string());
    conn.execute("INSERT OR IGNORE INTO fuentes_candidatas(nombre,url,fuente_padre_id,descubrimiento,creada) VALUES(?1,?2,?3,'busqueda',?4)", params![host,url,fuente_padre_id,ahora()])?;
    Ok(())
}

pub fn fuentes_candidatas(conn: &Connection) -> Result<Vec<FuenteCandidata>> {
    let mut stmt=conn.prepare("SELECT id,nombre,url,fuente_padre_id,descubrimiento,creada FROM fuentes_candidatas WHERE estado='pendiente' ORDER BY creada DESC LIMIT 500")?;
    let rows = stmt.query_map([], |r| {
        Ok(FuenteCandidata {
            id: r.get(0)?,
            nombre: r.get(1)?,
            url: r.get(2)?,
            fuente_padre_id: r.get(3)?,
            descubrimiento: r.get(4)?,
            creada: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn marcar_candidata(conn: &Connection, id: i64, estado: &str) -> Result<bool> {
    if !["aprobada", "descartada"].contains(&estado) {
        anyhow::bail!("estado de candidata no válido");
    }
    Ok(conn.execute(
        "UPDATE fuentes_candidatas SET estado=?2 WHERE id=?1",
        params![id, estado],
    )? > 0)
}

pub fn listar_duplicados(conn: &Connection) -> Result<Vec<CandidatoDuplicado>> {
    let mut stmt=conn.prepare("SELECT d.id,d.negocio_a,a.nombre,d.negocio_b,b.nombre,d.razon FROM duplicados_pendientes d JOIN negocios a ON a.id=d.negocio_a JOIN negocios b ON b.id=d.negocio_b WHERE d.estado='pendiente' ORDER BY d.creado DESC")?;
    let rows = stmt.query_map([], |r| {
        Ok(CandidatoDuplicado {
            id: r.get(0)?,
            negocio_a: r.get(1)?,
            nombre_a: r.get(2)?,
            negocio_b: r.get(3)?,
            nombre_b: r.get(4)?,
            razon: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn fusionar_duplicados(
    conn: &mut Connection,
    duplicado_id: i64,
    conservar_id: i64,
    fusionar_id: i64,
) -> Result<()> {
    if conservar_id == fusionar_id {
        anyhow::bail!("los IDs deben ser distintos");
    }
    let pair: Option<(i64,i64)> = conn.query_row("SELECT negocio_a,negocio_b FROM duplicados_pendientes WHERE id=?1 AND estado='pendiente'", params![duplicado_id], |r| Ok((r.get(0)?,r.get(1)?))).optional()?;
    let Some((a, b)) = pair else {
        anyhow::bail!("posible duplicado no encontrado")
    };
    if !((a == conservar_id && b == fusionar_id) || (a == fusionar_id && b == conservar_id)) {
        anyhow::bail!("los negocios no corresponden a la sugerencia");
    }
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE auditorias SET negocio_id=?1 WHERE negocio_id=?2",
        params![conservar_id, fusionar_id],
    )?;
    tx.execute(
        "UPDATE eventos SET negocio_id=?1 WHERE negocio_id=?2",
        params![conservar_id, fusionar_id],
    )?;
    tx.execute(
        "UPDATE borradores_contacto SET negocio_id=?1 WHERE negocio_id=?2",
        params![conservar_id, fusionar_id],
    )?;
    tx.execute("INSERT OR IGNORE INTO atributos_fuente(negocio_id,fuente_id,atributo,valor,url_origen,observado_en) SELECT ?1,fuente_id,atributo,valor,url_origen,observado_en FROM atributos_fuente WHERE negocio_id=?2",params![conservar_id,fusionar_id])?;
    tx.execute(
        "DELETE FROM atributos_fuente WHERE negocio_id=?1",
        params![fusionar_id],
    )?;
    tx.execute("INSERT OR IGNORE INTO negocio_fuentes(negocio_id,fuente_id,external_id,url_origen,visto_en) SELECT ?1,fuente_id,external_id,url_origen,visto_en FROM negocio_fuentes WHERE negocio_id=?2",params![conservar_id,fusionar_id])?;
    tx.execute(
        "DELETE FROM negocio_fuentes WHERE negocio_id=?1",
        params![fusionar_id],
    )?;
    let mut stmt = tx.prepare(
        "SELECT servicio,score,estado,motivo,fecha FROM oportunidades_servicio WHERE negocio_id=?1",
    )?;
    let oportunidades = stmt
        .query_map(params![fusionar_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (servicio, score, estado, motivo, fecha) in oportunidades {
        let existe:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM oportunidades_servicio WHERE negocio_id=?1 AND servicio=?2)",params![conservar_id,servicio],|r|r.get(0))?;
        if existe {
            tx.execute("UPDATE oportunidades_servicio SET score=MAX(score,?3),estado=CASE WHEN estado='contratada' THEN estado ELSE ?4 END WHERE negocio_id=?1 AND servicio=?2",params![conservar_id,servicio,score,estado])?;
        } else {
            tx.execute("UPDATE oportunidades_servicio SET negocio_id=?1 WHERE negocio_id=?2 AND servicio=?3",params![conservar_id,fusionar_id,servicio])?;
        }
        let _ = (motivo, fecha);
    }
    tx.execute(
        "DELETE FROM oportunidades_servicio WHERE negocio_id=?1",
        params![fusionar_id],
    )?;
    let pipeline_target: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM pipeline WHERE negocio_id=?1)",
        params![conservar_id],
        |r| r.get(0),
    )?;
    if !pipeline_target {
        tx.execute(
            "UPDATE pipeline SET negocio_id=?1 WHERE negocio_id=?2",
            params![conservar_id, fusionar_id],
        )?;
    } else {
        tx.execute(
            "DELETE FROM pipeline WHERE negocio_id=?1",
            params![fusionar_id],
        )?;
    }
    let archivado: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM negocios_descartados WHERE negocio_id=?1)",
        params![conservar_id],
        |r| r.get(0),
    )?;
    if !archivado {
        tx.execute("INSERT OR IGNORE INTO negocios_descartados(negocio_id,fecha,motivo) SELECT ?1,fecha,motivo FROM negocios_descartados WHERE negocio_id=?2",params![conservar_id,fusionar_id])?;
    }
    tx.execute(
        "DELETE FROM negocios WHERE id=?1 AND admisible=1",
        params![fusionar_id],
    )?;
    tx.execute(
        "UPDATE duplicados_pendientes SET estado='fusionado' WHERE id=?1",
        params![duplicado_id],
    )?;
    tx.commit()?;
    Ok(())
}

fn contar(conn: &Connection, sql: &str) -> Result<Vec<ParEtiqueta>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok(ParEtiqueta {
            etiqueta: r
                .get::<_, Option<String>>(0)?
                .unwrap_or_else(|| "sin_auditar".into()),
            total: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---- jobs ----

pub fn insert_job(conn: &Connection, job: &Job) -> Result<()> {
    conn.execute(
        "INSERT INTO jobs (id, tipo, estado, params, hechos, total, creado, actualizado, error, resumen)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            job.id,
            match job.tipo {
                TipoJob::Scan => "scan",
                TipoJob::Audit => "audit",
                TipoJob::Watch => "watch",
                TipoJob::Export => "export",
                TipoJob::Crawl => "crawl",
                TipoJob::Rebuild => "rebuild",
                TipoJob::Discovery => "discovery",
                TipoJob::Technical => "technical",
            },
            match job.estado {
                EstadoJob::Pendiente => "pendiente",
                EstadoJob::EnCurso => "en_curso",
                EstadoJob::Terminado => "terminado",
                EstadoJob::Error => "error",
            },
            job.params.to_string(),
            job.hechos,
            job.total,
            job.creado,
            job.actualizado,
            job.error,
            job.resumen.as_ref().map(serde_json::Value::to_string)
        ],
    )?;
    Ok(())
}

fn row_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
    Ok(Job {
        id: row.get(0)?,
        tipo: match row.get::<_, String>(1)?.as_str() {
            "scan" => TipoJob::Scan,
            "audit" => TipoJob::Audit,
            "watch" => TipoJob::Watch,
            "crawl" => TipoJob::Crawl,
            "rebuild" => TipoJob::Rebuild,
            "discovery" => TipoJob::Discovery,
            "technical" => TipoJob::Technical,
            _ => TipoJob::Export,
        },
        estado: match row.get::<_, String>(2)?.as_str() {
            "pendiente" => EstadoJob::Pendiente,
            // `encurso` era el formato histórico producido por Debug + lowercase.
            "en_curso" | "encurso" => EstadoJob::EnCurso,
            "terminado" => EstadoJob::Terminado,
            _ => EstadoJob::Error,
        },
        params: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or(serde_json::json!({})),
        hechos: row.get(4)?,
        total: row.get(5)?,
        creado: row.get(6)?,
        actualizado: row.get(7)?,
        error: row.get(8)?,
        resumen: row
            .get::<_, Option<String>>(9)?
            .and_then(|s| serde_json::from_str(&s).ok()),
    })
}

pub fn update_job(conn: &Connection, job: &Job) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET estado=?2, hechos=?3, total=?4, actualizado=?5, error=?6, resumen=?7 WHERE id=?1",
        params![
            job.id,
            match job.estado {
                EstadoJob::Pendiente => "pendiente",
                EstadoJob::EnCurso => "en_curso",
                EstadoJob::Terminado => "terminado",
                EstadoJob::Error => "error",
            },
            job.hechos,
            job.total,
            job.actualizado,
            job.error,
            job.resumen.as_ref().map(serde_json::Value::to_string)
        ],
    )?;
    Ok(())
}

pub fn get_job(conn: &Connection, id: &str) -> Result<Option<Job>> {
    conn.query_row(
        "SELECT id, tipo, estado, params, hechos, total, creado, actualizado, error, resumen FROM jobs WHERE id=?1",
        params![id],
        row_job,
    )
    .optional()
    .map_err(Into::into)
}

pub fn list_jobs(conn: &Connection) -> Result<Vec<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, tipo, estado, params, hechos, total, creado, actualizado, error, resumen FROM jobs
         ORDER BY CASE estado WHEN 'en_curso' THEN 0 WHEN 'pendiente' THEN 1 ELSE 2 END,
                  actualizado DESC LIMIT 100",
    )?;
    let rows = stmt.query_map([], row_job)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn jobs_reanudables(conn: &Connection) -> Result<Vec<Job>> {
    let mut stmt = conn.prepare("SELECT id,tipo,estado,params,hechos,total,creado,actualizado,error,resumen FROM jobs WHERE estado IN ('pendiente','en_curso','encurso') AND json_extract(params,'$.rebuild_parent') IS NULL ORDER BY creado")?;
    let rows = stmt.query_map([], row_job)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn tipo_job_activo(conn: &Connection, tipo: TipoJob) -> Result<bool> {
    let tipo = match tipo {
        TipoJob::Scan => "scan",
        TipoJob::Audit => "audit",
        TipoJob::Watch => "watch",
        TipoJob::Export => "export",
        TipoJob::Crawl => "crawl",
        TipoJob::Rebuild => "rebuild",
        TipoJob::Discovery => "discovery",
        TipoJob::Technical => "technical",
    };
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE tipo=?1 AND estado IN ('pendiente','en_curso','encurso'))", params![tipo], |r| r.get(0))?)
}

// ---- auth OTP ----

pub fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM meta WHERE key=?1", params![key], |r| {
        r.get(0)
    })
    .optional()
    .map_err(Into::into)
}

pub fn meta_set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn crear_auth_code(conn: &Connection, email: &str, hash: &str, ttl_secs: i64) -> Result<()> {
    let ahora = chrono::Local::now();
    let expira = (ahora + chrono::Duration::seconds(ttl_secs)).to_rfc3339();
    conn.execute(
        "INSERT INTO auth_codes (email, hash, creado, expira) VALUES (?1, ?2, ?3, ?4)",
        params![email, hash, ahora.to_rfc3339(), expira],
    )?;
    Ok(())
}

pub fn count_auth_codes_recientes(
    conn: &Connection,
    email: &str,
    ventana_secs: i64,
) -> Result<i64> {
    let desde = (chrono::Local::now() - chrono::Duration::seconds(ventana_secs)).to_rfc3339();
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM auth_codes WHERE email=?1 AND creado >= ?2",
        params![email, desde],
        |r| r.get(0),
    )?)
}

/// Comprueba el código (hash sha256). Devuelve Ok(true) si es válido (y lo marca usado).
pub fn validar_auth_code(conn: &Connection, email: &str, hash: &str) -> Result<bool> {
    let row: Option<(i64, String, String, i64, i64)> = conn
        .query_row(
            "SELECT id, hash, expira, usado, intentos FROM auth_codes WHERE email=?1 ORDER BY id DESC LIMIT 1",
            params![email],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let Some((id, hash_db, expira, usado, intentos)) = row else {
        return Ok(false);
    };
    if usado == 1 {
        return Ok(false);
    }
    let exp = chrono::DateTime::parse_from_rfc3339(&expira)
        .map(|d| d.with_timezone(&chrono::Local))
        .unwrap_or_else(|_| chrono::Local::now());
    if exp < chrono::Local::now() || intentos >= 5 {
        conn.execute("UPDATE auth_codes SET usado=1 WHERE id=?1", params![id])?;
        return Ok(false);
    }
    if hash_db == hash {
        conn.execute("UPDATE auth_codes SET usado=1 WHERE id=?1", params![id])?;
        Ok(true)
    } else {
        conn.execute(
            "UPDATE auth_codes SET intentos=intentos+1 WHERE id=?1",
            params![id],
        )?;
        Ok(false)
    }
}

pub fn crear_sesion(conn: &Connection, jti: &str, email: &str, ttl_secs: i64) -> Result<String> {
    let ahora = chrono::Local::now();
    let expira = (ahora + chrono::Duration::seconds(ttl_secs)).to_rfc3339();
    conn.execute(
        "INSERT INTO sesiones (jti, email, creado, expira) VALUES (?1, ?2, ?3, ?4)",
        params![jti, email, ahora.to_rfc3339(), expira.clone()],
    )?;
    Ok(expira)
}

pub fn get_sesion(conn: &Connection, jti: &str) -> Result<Option<(String, String)>> {
    conn.query_row(
        "SELECT email, expira FROM sesiones WHERE jti=?1",
        params![jti],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(Into::into)
}

pub fn borrar_sesion(conn: &Connection, jti: &str) -> Result<()> {
    conn.execute("DELETE FROM sesiones WHERE jti=?1", params![jti])?;
    Ok(())
}

pub fn limpiar_sesiones_expiradas(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM sesiones WHERE expira < ?1",
        params![chrono::Local::now().to_rfc3339()],
    )?;
    Ok(())
}

/// Copia independiente para operaciones administrativas, incluyendo WAL y comprobación de integridad.
pub fn backup_to_path(db_path: &str, destination: &std::path::Path) -> Result<()> {
    let source = Connection::open(db_path)?;
    let mut destination_db = Connection::open(destination)?;
    rusqlite::backup::Backup::new(&source, &mut destination_db)?.run_to_completion(
        128,
        std::time::Duration::from_millis(100),
        None,
    )?;
    let integrity: String = destination_db.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    anyhow::ensure!(
        integrity == "ok",
        "la copia SQLite no pasó integrity_check: {integrity}"
    );
    Ok(())
}

pub fn reset_resultados(conn: &mut Connection, reconstruccion: &Job) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("INSERT INTO meta(key,value) VALUES('negocios_id_floor',(SELECT CAST(COALESCE(MAX(id),0) AS TEXT) FROM negocios)) ON CONFLICT(key) DO UPDATE SET value=CAST(MAX(CAST(meta.value AS INTEGER),CAST(excluded.value AS INTEGER)) AS TEXT)",[])?;
    for tabla in [
        "auditorias_tecnicas",
        "web_candidatas",
        "web_estado",
        "job_items",
        "job_steps",
        "jobs",
        "borradores_contacto",
        "eventos",
        "pipeline",
        "auditorias",
        "oportunidades_servicio",
        "duplicados_pendientes",
        "atributos_fuente",
        "negocio_fuentes",
        "negocios_descartados",
        "negocios",
        "clasificacion_ingesta",
        "fuentes_candidatas",
        "paginas_rastreadas",
    ] {
        tx.execute(&format!("DELETE FROM {tabla}"), [])?;
    }
    tx.execute(
        "UPDATE fuentes SET ultima_ejecucion=NULL,error_ultima_ejecucion=NULL",
        [],
    )?;
    insert_job(&tx, reconstruccion)?;
    let integrity: i64 =
        tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?;
    anyhow::ensure!(
        integrity == 0,
        "claves foráneas inconsistentes tras reiniciar resultados"
    );
    tx.commit()?;
    Ok(())
}

pub fn resumen_clasificacion(conn: &Connection) -> Result<ResumenClasificacion> {
    let counts:(i64,i64,i64,i64)=conn.query_row("SELECT COUNT(*),COALESCE(SUM(estado='admitido'),0),COALESCE(SUM(estado='excluido'),0),COALESCE(SUM(estado='pendiente'),0) FROM clasificacion_ingesta",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let motivos = contar(
        conn,
        "SELECT motivo,COUNT(*) FROM clasificacion_ingesta GROUP BY motivo ORDER BY COUNT(*) DESC",
    )?;
    Ok(ResumenClasificacion {
        examinados: counts.0,
        admitidos: counts.1,
        excluidos: counts.2,
        pendientes: counts.3,
        motivos,
        version: crate::eligibility::POLICY_VERSION,
    })
}

#[cfg(test)]
mod rebuild_tests {
    use super::*;
    fn negocio(id: &str) -> Negocio {
        serde_json::from_value(serde_json::json!({"id":0,"nombre":"Comercio de prueba","municipio":"Navarra","fuente":"test","fuente_id":id,"primera_vista":"2026-11-04"})).unwrap()
    }
    #[test]
    fn reset_conserva_configuracion_y_no_recicla_ids() {
        let db = abrir(":memory:").unwrap();
        let mut c = db.lock().unwrap();
        meta_set(&c, "jwt_secret", "test-secret").unwrap();
        let id = upsert_negocio(&c, &negocio("1")).unwrap();
        c.execute(
            "INSERT INTO pipeline(negocio_id,estado,fecha) VALUES(?1,'contactado','2026-11-04')",
            [id],
        )
        .unwrap();
        c.execute(
            "INSERT INTO eventos(negocio_id,fecha,tipo) VALUES(?1,'2026-11-04','llamada')",
            [id],
        )
        .unwrap();
        let sources = fuentes(&c).unwrap().len();
        let job = Job {
            id: "reset-test".into(),
            tipo: TipoJob::Rebuild,
            estado: EstadoJob::Pendiente,
            params: serde_json::json!({}),
            hechos: 0,
            total: 0,
            creado: ahora(),
            actualizado: ahora(),
            error: None,
            resumen: None,
        };
        reset_resultados(&mut c, &job).unwrap();
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM negocios", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM eventos", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            meta_get(&c, "jwt_secret").unwrap().as_deref(),
            Some("test-secret")
        );
        assert_eq!(fuentes(&c).unwrap().len(), sources);
        assert!(upsert_negocio(&c, &negocio("2")).unwrap() > id);
        assert_eq!(
            get_job(&c, "reset-test").unwrap().unwrap().tipo,
            TipoJob::Rebuild
        );
    }

    #[test]
    fn rubro_filtra_sin_cambiar_el_sector_general() {
        let db = abrir(":memory:").unwrap();
        let conn = db.lock().unwrap();
        let mut bodega = negocio("bodega");
        bodega.sector = Some("industria".into());
        bodega.rubro = Some("bodega".into());
        bodega.evidencia_rubro = Some(serde_json::json!({"tipo_entidad":"cooperativa"}));
        upsert_negocio(&conn, &bodega).unwrap();
        let mut floristeria = negocio("floristeria");
        floristeria.sector = Some("comercio".into());
        floristeria.rubro = Some("floristeria".into());
        upsert_negocio(&conn, &floristeria).unwrap();
        let page = list_negocios(
            &conn,
            &FiltrosNegocios {
                rubro: Some("bodega".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].rubro.as_deref(), Some("bodega"));
        assert_eq!(page.items[0].sector.as_deref(), Some("industria"));
        let cooperatives = list_negocios(
            &conn,
            &FiltrosNegocios {
                tipo_entidad: Some("cooperativa".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(cooperatives.total, 1);
        assert_eq!(cooperatives.items[0].rubro.as_deref(), Some("bodega"));
    }
    #[test]
    fn seguimiento_guarda_cualquier_estado_sin_exigir_fecha() {
        let db = abrir(":memory:").unwrap();
        let c = db.lock().unwrap();
        let id = upsert_negocio(&c, &negocio("s")).unwrap();
        for estado in [
            EstadoPipeline::Nuevo,
            EstadoPipeline::VisitaHecha,
            EstadoPipeline::NoInteresado,
            EstadoPipeline::Cerrado,
        ] {
            let req: SetPipelineRequest =
                serde_json::from_value(serde_json::json!({"estado":estado})).unwrap();
            set_pipeline(&c, id, &req).unwrap();
            assert_eq!(
                detalle_negocio(&c, id)
                    .unwrap()
                    .unwrap()
                    .pipeline
                    .unwrap()
                    .estado,
                estado
            );
        }
    }
}

#[cfg(test)]
mod mapa_tests {
    use super::*;

    fn zona(sur: f64, oeste: f64, norte: f64, este: f64) -> FiltrosNegocios {
        FiltrosNegocios {
            sur: Some(sur),
            oeste: Some(oeste),
            norte: Some(norte),
            este: Some(este),
            ..Default::default()
        }
    }

    #[test]
    fn mapa_55000_sin_truncar_y_con_respuesta_acotada_incluso_a_zoom_maximo() {
        let db = abrir(":memory:").unwrap();
        let mut conn = db.lock().unwrap();
        let tx = conn.transaction().unwrap();
        {
            let mut stmt = tx.prepare("INSERT INTO negocios(nombre,municipio,sector,lat,lon,fuente,fuente_id,primera_vista) VALUES('Prueba','Navarra','comercio',?1,?2,'test',?3,'2026-11-05')").unwrap();
            for i in 0..55_000 {
                // Una rejilla densa y repartida por toda Navarra.
                let lat = 42.0 + (i / 250) as f64 * 0.005;
                let lon = -2.5 + (i % 250) as f64 * 0.006;
                stmt.execute(params![lat, lon, i.to_string()]).unwrap();
            }
        }
        tx.commit().unwrap();
        let cambios = conn.total_changes();
        for f in [FiltrosNegocios::default(), zona(41.9, -2.6, 43.3, -0.8)] {
            let inicio = std::time::Instant::now();
            let mapa = list_negocios_mapa(&conn, &f, 19).unwrap();
            assert_eq!(mapa.total, 55_000);
            assert_eq!(mapa.items.iter().map(|p| p.cantidad).sum::<i64>(), 55_000);
            assert!(mapa.items.len() <= 825, "{} elementos", mapa.items.len());
            let bytes = serde_json::to_vec(&mapa).unwrap().len();
            assert!(bytes < 400_000, "respuesta de {bytes} bytes");
            eprintln!(
                "Mapa 55k: {:?}, {} elementos, {} bytes",
                inicio.elapsed(),
                mapa.items.len(),
                bytes
            );
            let extension = mapa.zona.unwrap();
            assert_eq!(extension.sur, 42.0);
            assert!(extension.norte > 43.0);
        }
        assert_eq!(
            conn.total_changes(),
            cambios,
            "leer el mapa no genera ni enriquece fichas"
        );
        let f = zona(42.0, -2.5, 42.001, -2.499);
        let detalle = list_negocios_mapa(&conn, &f, 19).unwrap();
        assert_eq!(detalle.total, 1);
        assert_eq!(detalle.items[0].id, Some(1));
        assert_eq!(detalle.items[0].nombre.as_deref(), Some("Prueba"));
        assert_eq!(detalle.items[0].score_potencial, None);
        assert_eq!(list_negocios(&conn, &f).unwrap().total, 1);
    }

    #[test]
    fn mapa_respeta_filtros_descartados_y_coordenadas_validas() {
        let db = abrir(":memory:").unwrap();
        let conn = db.lock().unwrap();
        conn.execute_batch(r#"INSERT INTO negocios(id,nombre,municipio,sector,rubro,evidencia_rubro,lat,lon,fuente,fuente_id,primera_vista) VALUES
            (1,'Bodega','Tudela','industria','bodega','{"tipo_entidad":"cooperativa"}',42.0,-1.6,'test','1','2026-11-05'),
            (2,'Tienda','Tudela','comercio','alimentacion','{}',42.0,-1.6,'test','2','2026-11-05'),
            (3,'Archivado','Tudela','industria','bodega','{}',42.0,-1.6,'test','3','2026-11-05'),
            (4,'Sin coordenadas','Tudela','industria','bodega','{}',NULL,NULL,'test','4','2026-11-05'),
            (5,'Fuera de rango','Tudela','industria','bodega','{}',95.0,-1.6,'test','5','2026-11-05');
            INSERT INTO negocios_descartados(negocio_id,fecha) VALUES(3,'2026-11-05');
            INSERT INTO oportunidades_servicio(negocio_id,servicio,score,motivo,fecha) VALUES(1,'creacion_web',80,'prueba','2026-11-05');
            INSERT INTO pipeline(negocio_id,estado,fecha) VALUES(1,'contactado','2026-11-05');"#).unwrap();
        let mapa = list_negocios_mapa(&conn, &FiltrosNegocios::default(), 19).unwrap();
        assert_eq!(mapa.total, 2);
        assert_eq!(mapa.items.len(), 1);
        assert_eq!(mapa.items[0].cantidad, 2);
        assert_eq!(mapa.items[0].score_potencial, Some(80));
        assert!(mapa.items[0].id.is_none());
        for f in [
            FiltrosNegocios {
                sector: Some("industria".into()),
                ..Default::default()
            },
            FiltrosNegocios {
                rubro: Some("bodega".into()),
                ..Default::default()
            },
            FiltrosNegocios {
                tipo_entidad: Some("cooperativa".into()),
                ..Default::default()
            },
            FiltrosNegocios {
                prioridad: Some("A".into()),
                ..Default::default()
            },
            FiltrosNegocios {
                estado_pipeline: Some("contactado".into()),
                ..Default::default()
            },
            FiltrosNegocios {
                q: Some("Bodega".into()),
                municipio: Some("Tud".into()),
                ..Default::default()
            },
        ] {
            let mapa = list_negocios_mapa(&conn, &f, 19).unwrap();
            assert_eq!(mapa.total, 1);
            assert_eq!(mapa.items[0].id, Some(1));
        }
        let archivados = list_negocios_mapa(
            &conn,
            &FiltrosNegocios {
                mostrar_descartados: true,
                ..Default::default()
            },
            19,
        )
        .unwrap();
        assert_eq!(archivados.total, 1);
        assert_eq!(archivados.items[0].id, Some(3));
        let vacio = list_negocios_mapa(&conn, &zona(40.0, -3.0, 41.0, -2.0), 8).unwrap();
        assert_eq!(vacio.total, 0);
        assert!(vacio.items.is_empty());
        assert!(vacio.zona.is_none());
    }

    #[test]
    fn mapa_rechaza_zonas_parciales_invertidas_o_no_finitas() {
        for f in [
            FiltrosNegocios {
                sur: Some(42.0),
                ..Default::default()
            },
            zona(43.0, -2.0, 42.0, -1.0),
            zona(42.0, -1.0, 43.0, -2.0),
            zona(-90.0, -2.0, 43.0, -1.0),
            zona(42.0, -181.0, 43.0, -1.0),
            zona(f64::NAN, -2.0, 43.0, -1.0),
            zona(42.0, -2.0, f64::INFINITY, -1.0),
        ] {
            assert!(zona_mapa(&f).is_err());
        }
    }

    #[test]
    fn empresas_superpuestas_son_accesibles_en_paginas_de_20() {
        let db = abrir(":memory:").unwrap();
        let conn = db.lock().unwrap();
        conn.execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<55)
            INSERT INTO negocios(nombre,municipio,lat,lon,fuente,fuente_id,primera_vista)
            SELECT 'Empresa '||i,'Pamplona',42.8,-1.6,'test',CAST(i AS TEXT),'2026-11-05' FROM n;",
        )
        .unwrap();
        let mapa = list_negocios_mapa(&conn, &FiltrosNegocios::default(), 19).unwrap();
        assert_eq!(mapa.items.len(), 1);
        assert_eq!(mapa.items[0].cantidad, 55);
        let z = mapa.items[0].zona;
        let mut ids = std::collections::BTreeSet::new();
        for pagina in 1..=3 {
            let f = FiltrosNegocios {
                tam: Some(20),
                pagina: Some(pagina),
                ordenar: Some("nombre".into()),
                ..zona(z.sur, z.oeste, z.norte, z.este)
            };
            let p = list_negocios(&conn, &f).unwrap();
            assert_eq!(p.total, 55);
            assert!(p.items.len() <= 20);
            for n in p.items {
                assert!(ids.insert(n.id));
            }
        }
        assert_eq!(ids.len(), 55);
    }
}

#[cfg(test)]
mod filtros_tests {
    use super::*;

    #[test]
    fn auditoria_conserva_url_actual_precedencia_de_jobs_y_ultimo_resultado() {
        let db = abrir(":memory:").unwrap();
        let c = db.lock().unwrap();
        c.execute_batch(r#"
            WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<7)
            INSERT INTO negocios(id,nombre,municipio,rubro,fuente,fuente_id,primera_vista)
            SELECT i,'Empresa '||i,'Pamplona','alimentacion','test',CAST(i AS TEXT),'2026-11-05' FROM n;
            INSERT INTO web_estado(negocio_id,estado,url_confirmada,origen)
            SELECT id,'confirmada','https://actual.example/','confirmacion_usuario' FROM negocios WHERE id<7;
            INSERT INTO jobs(id,tipo,estado,params,creado,actualizado) VALUES
                ('completado','technical','terminado','{}','2026-11-05','2026-11-05'),
                ('cola','technical','pendiente','{"negocio_id":2,"url":"https://actual.example/"}','2026-11-05','2026-11-05'),
                ('curso','technical','en_curso','{"negocio_id":3,"url":"https://actual.example/"}','2026-11-05','2026-11-05'),
                ('cola-curso','technical','pendiente','{"negocio_id":3,"url":"https://actual.example/"}','2026-11-05','2026-11-05'),
                ('otra-url','technical','en_curso','{"negocio_id":1,"url":"https://anterior.example/"}','2026-11-05','2026-11-05');
            INSERT INTO jobs(id,tipo,estado,params,creado,actualizado)
            SELECT 'done-'||id,'technical','terminado','{}','2026-11-05','2026-11-05' FROM negocios;
            INSERT INTO auditorias_tecnicas(id,job_id,negocio_id,url,motor,dispositivo,estado,fecha) VALUES
                ('r1','done-1',1,'https://actual.example/','lighthouse','mobile','completo','2026-11-05'),
                ('r2','done-2',2,'https://actual.example/','lighthouse','mobile','completo','2026-11-05'),
                ('r3','done-3',3,'https://actual.example/','lighthouse','mobile','completo','2026-11-05'),
                ('r4','done-4',4,'https://actual.example/','lighthouse','mobile','parcial','2026-11-05'),
                ('r4b','done-4',4,'https://actual.example/','siteone','desktop','error','2026-11-05'),
                ('r5','done-5',5,'https://anterior.example/','lighthouse','mobile','completo','2026-11-05'),
                ('r6','done-6',6,'https://actual.example/','lighthouse','mobile','parcial','2026-11-05'),
                ('r7','done-7',7,'https://actual.example/','lighthouse','mobile','completo','2026-11-05');
        "#).unwrap();
        for (estado, esperados) in [
            ("realizada", vec![1, 6]),
            ("pendiente", vec![2, 5, 7]),
            ("en_curso", vec![3]),
            ("error", vec![4]),
        ] {
            let p = list_negocios(
                &c,
                &FiltrosNegocios {
                    estado_auditoria_tecnica: Some(estado.into()),
                    ordenar: Some("nombre".into()),
                    dir: Some("asc".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(p.total, esperados.len() as i64);
            assert_eq!(p.items.iter().map(|n| n.id).collect::<Vec<_>>(), esperados);
            assert!(p.items.iter().all(|n| n.estado_auditoria_tecnica == estado));
        }
        // Los índices deben reflejar los cambios de estado y de URL sin cachés obsoletas.
        c.execute("UPDATE jobs SET estado='terminado' WHERE id='cola'", [])
            .unwrap();
        assert_eq!(
            get_negocio(&c, 2)
                .unwrap()
                .unwrap()
                .estado_auditoria_tecnica,
            "realizada"
        );
        c.execute(
            "UPDATE web_estado SET url_confirmada='https://nueva.example/' WHERE negocio_id=1",
            [],
        )
        .unwrap();
        assert_eq!(
            get_negocio(&c, 1)
                .unwrap()
                .unwrap()
                .estado_auditoria_tecnica,
            "pendiente"
        );
    }

    #[test]
    fn filtros_55000_con_mil_jobs_usan_indices_y_paginas_estables() {
        let db = abrir(":memory:").unwrap();
        let c = db.lock().unwrap();
        c.execute_batch(r#"
            WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<55000)
            INSERT INTO negocios(id,nombre,municipio,sector,rubro,evidencia_rubro,fuente,fuente_id,primera_vista)
            SELECT i,'Empresa','Pamplona','comercio','alimentacion','{"tipo_entidad":"empresa"}','test',CAST(i AS TEXT),'2026-11-05' FROM n;
            INSERT INTO web_estado(negocio_id,estado,url_confirmada,origen)
            SELECT id,'confirmada','https://empresa.example/','confirmacion_usuario' FROM negocios WHERE id<=1000;
            INSERT INTO jobs(id,tipo,estado,params,creado,actualizado)
            SELECT CAST(id AS TEXT),'technical','terminado',json_object('negocio_id',id,'url','https://empresa.example/'),'2026-11-05','2026-11-05'
            FROM negocios WHERE id<=1000;
            INSERT INTO auditorias_tecnicas(id,job_id,negocio_id,url,motor,dispositivo,estado,fecha)
            SELECT CAST(id AS TEXT),CAST(id AS TEXT),id,'https://empresa.example/','lighthouse','mobile','completo','2026-11-05'
            FROM negocios WHERE id<=1000;
            INSERT INTO oportunidades_servicio(negocio_id,servicio,score,motivo,fecha)
            SELECT id,'creacion_web',80,'Prueba','2026-11-05' FROM negocios;
        "#).unwrap();
        let f = FiltrosNegocios {
            estado_auditoria_tecnica: Some("realizada".into()),
            sector: Some("comercio".into()),
            rubro: Some("alimentacion".into()),
            tipo_entidad: Some("empresa".into()),
            prioridad: Some("A".into()),
            ..Default::default()
        };
        let (wh, args) = filtros_negocios_sql(&f).unwrap();
        let mut stmt = c
            .prepare(&format!(
                "EXPLAIN QUERY PLAN SELECT COUNT(*) FROM negocios {wh}"
            ))
            .unwrap();
        let plan = stmt
            .query_map(params_from_iter(args.iter()), |r| r.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
            .join("\n");
        assert!(plan.contains("idx_jobs_tecnica_lookup"), "{plan}");
        assert!(plan.contains("idx_tecnica_negocio_url_fecha"), "{plan}");
        assert!(!plan.contains("SCAN j"), "{plan}");
        let inicio = std::time::Instant::now();
        let p1 = list_negocios(&c, &f).unwrap();
        let p2 = list_negocios(
            &c,
            &FiltrosNegocios {
                pagina: Some(2),
                ..f.clone()
            },
        )
        .unwrap();
        assert_eq!(p1.total, 1000);
        assert_eq!(p2.total, 1000);
        assert_eq!(
            p1.items.iter().map(|n| n.id).collect::<Vec<_>>(),
            (1..=50).collect::<Vec<_>>()
        );
        assert_eq!(
            p2.items.iter().map(|n| n.id).collect::<Vec<_>>(),
            (51..=100).collect::<Vec<_>>()
        );
        eprintln!("Dos páginas filtradas en {:?}", inicio.elapsed());
    }
}
