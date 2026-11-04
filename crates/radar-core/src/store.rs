//! Store SQLite (`radar.db`). Esquema según diseño §8.5 + tabla `jobs` para la API.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};

use crate::model::*;

pub type Store = Arc<Mutex<Connection>>;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS negocios (
  id INTEGER PRIMARY KEY,
  nombre TEXT NOT NULL,
  categoria TEXT,
  sector TEXT,
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
CREATE TABLE IF NOT EXISTS pipeline (
  negocio_id INTEGER PRIMARY KEY REFERENCES negocios(id),
  estado TEXT NOT NULL DEFAULT 'nuevo',
  valor_estimado REAL,
  siguiente_accion TEXT,
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
  error TEXT
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
    let conn = Connection::open(db_path)
        .with_context(|| format!("abrindo base de datos {db_path}"))?;
    conn.execute_batch(SCHEMA)?;
    Ok(Arc::new(Mutex::new(conn)))
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
        direccion: row.get(4)?,
        municipio: row.get(5)?,
        lat: row.get(6)?,
        lon: row.get(7)?,
        telefono: row.get(8)?,
        email: row.get(9)?,
        web: row.get(10)?,
        fuente: row.get(11)?,
        fuente_id: row.get(12)?,
        primera_vista: row.get(13)?,
        ultima_verificacion: row.get(14)?,
    })
}

/// Inserta o actualiza por (fuente, fuente_id). Devuelve el id interno.
pub fn upsert_negocio(conn: &Connection, n: &Negocio) -> Result<i64> {
    conn.execute(
        "INSERT INTO negocios (nombre, categoria, sector, direccion, municipio, lat, lon,
             telefono, email, web, fuente, fuente_id, primera_vista, ultima_verificacion)
           VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
           ON CONFLICT(fuente, fuente_id) DO UPDATE SET
             nombre=excluded.nombre, categoria=excluded.categoria, sector=excluded.sector,
             direccion=excluded.direccion, municipio=excluded.municipio, lat=excluded.lat,
             lon=excluded.lon, telefono=excluded.telefono, email=excluded.email,
             web=excluded.web, ultima_verificacion=excluded.ultima_verificacion",
        params![
            n.nombre, n.categoria, n.sector, n.direccion, n.municipio, n.lat, n.lon,
            n.telefono, n.email, n.web, n.fuente, n.fuente_id, n.primera_vista,
            n.ultima_verificacion
        ],
    )?;
    let id = conn.query_row(
        "SELECT id FROM negocios WHERE fuente=?1 AND fuente_id=?2",
        params![n.fuente, n.fuente_id],
        |r| r.get(0),
    )?;
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
const ULTIMA_PRIORIDAD: &str =
    "(SELECT a.prioridad FROM auditorias a WHERE a.negocio_id = negocios.id ORDER BY a.fecha DESC, a.id DESC LIMIT 1)";

pub fn list_negocios(conn: &Connection, f: &FiltrosNegocios) -> Result<Pagina<Negocio>> {
    use rusqlite::types::Value;
    let mut wh: Vec<String> = Vec::new();
    let mut args: Vec<Value> = Vec::new();
    if let Some(m) = &f.municipio {
        wh.push("municipio LIKE ?".into());
        args.push(Value::from(format!("%{m}%")));
    }
    if let Some(s) = &f.sector {
        wh.push("sector = ?".into());
        args.push(Value::from(s.clone()));
    }
    if let Some(q) = &f.q {
        wh.push("nombre LIKE ?".into());
        args.push(Value::from(format!("%{q}%")));
    }
    if let Some(e) = &f.estado_web {
        wh.push(format!("{ULTIMA_AUDITORIA} = ?"));
        args.push(Value::from(e.as_str().to_string()));
    }
    if let Some(p) = &f.prioridad {
        wh.push(format!("{ULTIMA_PRIORIDAD} = ?"));
        args.push(Value::from(p.clone()));
    }
    let where_sql = if wh.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", wh.join(" AND "))
    };

    let total: i64 = {
        let sql = format!("SELECT COUNT(*) FROM negocios {where_sql}");
        let refs: Vec<&Value> = args.iter().collect();
        conn.query_row(&sql, params_from_iter(refs.into_iter()), |r| r.get(0))?
    };

    let ordenar = match f.ordenar.as_deref() {
        Some("nombre") => "nombre",
        Some("municipio") => "municipio",
        Some("fecha") => "ultima_verificacion",
        Some("score") | _ => ULTIMO_SCORE,
    };
    let dir = if f.dir.as_deref() == Some("asc") { "ASC" } else { "DESC" };
    let tam = f.tam.unwrap_or(50).clamp(1, 200);
    let pagina = f.pagina.unwrap_or(1).max(1);
    let offset = (pagina - 1) * tam;

    let sql = format!(
        "SELECT id, nombre, categoria, sector, direccion, municipio, lat, lon, telefono, email,
                web, fuente, fuente_id, primera_vista, ultima_verificacion
         FROM negocios {where_sql} ORDER BY {ordenar} {dir} NULLS LAST LIMIT ? OFFSET ?"
    );
    let mut all: Vec<Value> = args.clone();
    all.push(Value::from(tam));
    all.push(Value::from(offset));
    let refs: Vec<&Value> = all.iter().collect();
    let mut stmt = conn.prepare(&sql)?;
    let items = stmt
        .query_map(params_from_iter(refs.into_iter()), row_negocio)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(Pagina { items, total, pagina, tam })
}

pub fn get_negocio(conn: &Connection, id: i64) -> Result<Option<Negocio>> {
    conn.query_row(
        "SELECT id, nombre, categoria, sector, direccion, municipio, lat, lon, telefono, email,
                web, fuente, fuente_id, primera_vista, ultima_verificacion
         FROM negocios WHERE id=?1",
        params![id],
        row_negocio,
    )
    .optional()
    .map_err(Into::into)
}

pub fn detalle_negocio(conn: &Connection, id: i64) -> Result<Option<NegocioDetalle>> {
    let Some(negocio) = get_negocio(conn, id)? else { return Ok(None) };
    let mut stmt = conn.prepare(
        "SELECT id, negocio_id, fecha, estado_web, score, prioridad, senales FROM auditorias
         WHERE negocio_id=?1 ORDER BY fecha DESC, id DESC LIMIT 20",
    )?;
    let auditorias = stmt
        .query_map(params![id], row_auditoria)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let pipeline = conn
        .query_row(
            "SELECT negocio_id, estado, valor_estimado, siguiente_accion, canal, fecha FROM pipeline WHERE negocio_id=?1",
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
                    canal: r.get(4)?,
                    fecha: r.get(5)?,
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
    Ok(Some(NegocioDetalle { negocio, auditorias, pipeline, eventos }))
}

/// Negocios pendientes de auditar (sin auditoría o con auditoría antigua) filtrados por prioridad.
pub fn negocios_para_auditar(conn: &Connection, prioridad: Option<&str>, limite: Option<i64>) -> Result<Vec<Negocio>> {
    let (wh, arg) = match prioridad {
        Some(p) => (format!("WHERE {ULTIMA_PRIORIDAD} = ?1 OR {ULTIMA_PRIORIDAD} IS NULL"), Some(p.to_string())),
        None => (String::new(), None),
    };
    let limite_sql = match limite {
        Some(l) => format!("LIMIT {l}"),
        None => "LIMIT 5000".into(),
    };
    let sql = format!(
        "SELECT id, nombre, categoria, sector, direccion, municipio, lat, lon, telefono, email,
                web, fuente, fuente_id, primera_vista, ultima_verificacion
         FROM negocios {wh} ORDER BY {ULTIMO_SCORE} DESC NULLS LAST {limite_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let map = match &arg {
        Some(a) => stmt.query_map(params![a], row_negocio)?,
        None => stmt.query_map([], row_negocio)?,
    };
    Ok(map.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn set_pipeline(conn: &Connection, negocio_id: i64, req: &SetPipelineRequest) -> Result<Pipeline> {
    let fecha = ahora();
    let estado = req.estado.unwrap_or(EstadoPipeline::Nuevo);
    conn.execute(
        "INSERT INTO pipeline (negocio_id, estado, valor_estimado, siguiente_accion, canal, fecha)
         VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(negocio_id) DO UPDATE SET
           estado=COALESCE(?2, pipeline.estado),
           valor_estimado=COALESCE(?3, pipeline.valor_estimado),
           siguiente_accion=COALESCE(?4, pipeline.siguiente_accion),
           canal=COALESCE(?5, pipeline.canal),
           fecha=?6",
        params![
            negocio_id,
            format!("{:?}", estado).to_lowercase(),
            req.valor_estimado,
            req.siguiente_accion,
            req.canal,
            fecha
        ],
    )?;
    Ok(Pipeline {
        negocio_id,
        estado,
        valor_estimado: req.valor_estimado,
        siguiente_accion: req.siguiente_accion.clone(),
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
    let total_negocios: i64 = conn.query_row("SELECT COUNT(*) FROM negocios", [], |r| r.get(0))?;
    let por_estado_web = contar(conn, &format!("SELECT {ULTIMA_AUDITORIA}, COUNT(*) FROM negocios GROUP BY 1"))?;
    let por_prioridad = contar(conn, &format!("SELECT {ULTIMA_PRIORIDAD}, COUNT(*) FROM negocios GROUP BY 1"))?;
    let por_pipeline = contar(conn, "SELECT estado, COUNT(*) FROM pipeline GROUP BY 1")?;
    let eventos_ultimos_7_dias: i64 = conn.query_row(
        "SELECT COUNT(*) FROM eventos WHERE fecha >= datetime('now', '-7 days')",
        [],
        |r| r.get(0),
    )?;
    Ok(Stats { total_negocios, por_estado_web, por_prioridad, por_pipeline, eventos_ultimos_7_dias })
}

fn contar(conn: &Connection, sql: &str) -> Result<Vec<ParEtiqueta>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| {
        Ok(ParEtiqueta {
            etiqueta: r.get::<_, Option<String>>(0)?.unwrap_or_else(|| "sin_auditar".into()),
            total: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---- jobs ----

pub fn insert_job(conn: &Connection, job: &Job) -> Result<()> {
    conn.execute(
        "INSERT INTO jobs (id, tipo, estado, params, hechos, total, creado, actualizado, error)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            job.id,
            format!("{:?}", job.tipo).to_lowercase(),
            format!("{:?}", job.estado).to_lowercase(),
            job.params.to_string(),
            job.hechos,
            job.total,
            job.creado,
            job.actualizado,
            job.error
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
            _ => TipoJob::Export,
        },
        estado: match row.get::<_, String>(2)?.as_str() {
            "pendiente" => EstadoJob::Pendiente,
            "en_curso" => EstadoJob::EnCurso,
            "terminado" => EstadoJob::Terminado,
            _ => EstadoJob::Error,
        },
        params: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or(serde_json::json!({})),
        hechos: row.get(4)?,
        total: row.get(5)?,
        creado: row.get(6)?,
        actualizado: row.get(7)?,
        error: row.get(8)?,
    })
}

pub fn update_job(conn: &Connection, job: &Job) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET estado=?2, hechos=?3, total=?4, actualizado=?5, error=?6 WHERE id=?1",
        params![
            job.id,
            format!("{:?}", job.estado).to_lowercase(),
            job.hechos,
            job.total,
            job.actualizado,
            job.error
        ],
    )?;
    Ok(())
}

pub fn get_job(conn: &Connection, id: &str) -> Result<Option<Job>> {
    conn.query_row(
        "SELECT id, tipo, estado, params, hechos, total, creado, actualizado, error FROM jobs WHERE id=?1",
        params![id],
        row_job,
    )
    .optional()
    .map_err(Into::into)
}

pub fn list_jobs(conn: &Connection) -> Result<Vec<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, tipo, estado, params, hechos, total, creado, actualizado, error FROM jobs
         ORDER BY creado DESC LIMIT 100",
    )?;
    let rows = stmt.query_map([], row_job)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

// ---- auth OTP ----

pub fn meta_get(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM meta WHERE key=?1", params![key], |r| r.get(0))
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

pub fn count_auth_codes_recientes(conn: &Connection, email: &str, ventana_secs: i64) -> Result<i64> {
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
    let Some((id, hash_db, expira, usado, intentos)) = row else { return Ok(false) };
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
        conn.execute("UPDATE auth_codes SET intentos=intentos+1 WHERE id=?1", params![id])?;
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
