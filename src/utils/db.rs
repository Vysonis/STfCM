use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn open_or_init() -> Result<Connection, DbError> {
    let dir = PathBuf::from("data/db");
    fs::create_dir_all(&dir)?;
    let path = dir.join("tracker.sqlite");
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS satellites (
            norad_id INTEGER PRIMARY KEY,
            name TEXT
        );
        CREATE TABLE IF NOT EXISTS snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            norad_id INTEGER NOT NULL,
            timestamp TEXT NOT NULL,
            pos_x REAL NOT NULL,
            pos_y REAL NOT NULL,
            pos_z REAL NOT NULL,
            vel_x REAL NOT NULL,
            vel_y REAL NOT NULL,
            vel_z REAL NOT NULL,
            FOREIGN KEY(norad_id) REFERENCES satellites(norad_id)
        );
        CREATE TABLE IF NOT EXISTS stations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT,
            lat REAL NOT NULL,
            lon REAL NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS stations_name_unique ON stations(name) WHERE name IS NOT NULL;
        "#,
    )?;
    Ok(conn)
}

pub fn upsert_satellite(conn: &Connection, norad_id: u64, name: Option<&str>) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO satellites (norad_id, name) VALUES (?1, ?2)
         ON CONFLICT(norad_id) DO UPDATE SET name=excluded.name",
        params![norad_id as i64, name.unwrap_or("")],
    )?;
    Ok(())
}

pub fn insert_snapshot(
    conn: &Connection,
    norad_id: u64,
    timestamp: &str,
    prediction: &sgp4::Prediction,
) -> Result<(), DbError> {
    conn.execute(
        "INSERT INTO snapshots (norad_id, timestamp, pos_x, pos_y, pos_z, vel_x, vel_y, vel_z)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            norad_id as i64,
            timestamp,
            prediction.position[0],
            prediction.position[1],
            prediction.position[2],
            prediction.velocity[0],
            prediction.velocity[1],
            prediction.velocity[2],
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Station {
    pub id: i64,
    pub name: Option<String>,
    pub lat: f64,
    pub lon: f64,
}

pub fn insert_station(conn: &Connection, name: Option<&str>, lat: f64, lon: f64) -> Result<i64, DbError> {
    conn.execute(
        "INSERT INTO stations (name, lat, lon) VALUES (?1, ?2, ?3)",
        params![name, lat, lon],
    )?;
    let id = conn.last_insert_rowid();
    Ok(id)
}

pub fn list_stations(conn: &Connection) -> Result<Vec<Station>, DbError> {
    let mut stmt = conn.prepare("SELECT id, name, lat, lon FROM stations ORDER BY id")?;
    let iter = stmt.query_map([], |row| {
        Ok(Station {
            id: row.get::<_, i64>(0)?,
            name: row.get::<_, String>(1).ok(),
            lat: row.get::<_, f64>(2)?,
            lon: row.get::<_, f64>(3)?,
        })
    })?;
    Ok(iter.filter_map(Result::ok).collect())
}

pub fn get_station(conn: &Connection, id: i64) -> Result<Station, DbError> {
    let mut stmt = conn.prepare("SELECT id, name, lat, lon FROM stations WHERE id = ?1")?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Station {
            id: row.get::<_, i64>(0)?,
            name: row.get::<_, String>(1).ok(),
            lat: row.get::<_, f64>(2)?,
            lon: row.get::<_, f64>(3)?,
        })
    } else {
        Err(rusqlite::Error::QueryReturnedNoRows.into())
    }
}

pub fn update_station(conn: &Connection, id: i64, name: Option<&str>, lat: f64, lon: f64) -> Result<(), DbError> {
    conn.execute(
        "UPDATE stations SET name = ?1, lat = ?2, lon = ?3 WHERE id = ?4",
        params![name, lat, lon, id],
    )?;
    Ok(())
}

pub fn delete_station(conn: &Connection, id: i64) -> Result<(), DbError> {
    conn.execute("DELETE FROM stations WHERE id = ?1", params![id])?;
    Ok(())
}