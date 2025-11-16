use std::net::SocketAddr;
use std::sync::Arc;

use axum::{extract::{Query, Path}, response::IntoResponse, routing::get, Json, Router};
use axum::http::StatusCode;
use tower_http::cors::{CorsLayer, Any};
use tower_http::services::{ServeDir, ServeFile};
use serde::Deserialize;
// use tracing::info;

use crate::api::types::{PassWindowDto, SatelliteDto, StationDto, CreateStationDto};
use crate::predictors::passes::{predict_passes, PassWindow};

#[derive(Clone)]
pub struct AppState {
    pub elements: Arc<Vec<sgp4::Elements>>, // latest parsed elements
}

#[derive(Debug, Deserialize)]
struct PassQuery {
    norad_id: u64,
    #[serde(default)]
    station_id: Option<i64>,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default = "default_duration")] 
    duration: i64,
    #[serde(default = "default_step")] 
    step: i64,
    #[serde(default = "default_min_el")] 
    min_el: f64,
}

fn default_duration() -> i64 { 120 }
fn default_step() -> i64 { 15 }
fn default_min_el() -> f64 { 10.0 }

#[derive(Debug, Deserialize)]
struct SatPosQuery {
    #[serde(default)]
    limit: Option<usize>,
}

pub async fn run_server(state: AppState, addr: SocketAddr) {
    let app = Router::new()
        .route("/health", get(health))
        .route("/stations", get(list_stations).post(create_station))
        .route("/stations/:id", get(get_station).put(update_station).delete(delete_station))
        .route("/satellites", get(list_satellites))
        .route("/satellites/positions", get(list_sat_positions))
        .route("/passes", get(get_passes))
        .route("/satellites/:norad_id/passes", get(get_passes_for_satellite))
        .nest_service("/ui", ServeDir::new("web"))
        .route_service("/", ServeFile::new("web/index.html"))
        .with_state(state)
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("API server listening on http://{}", addr);
    axum::serve(listener, app)
        .await
        .unwrap();
}

async fn list_satellites() -> impl IntoResponse {
    let conn = match crate::utils::db::open_or_init() {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("db error: {}", e)})));
        }
    };

    let mut stmt = match conn.prepare("SELECT norad_id, name FROM satellites ORDER BY norad_id") {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("db error: {}", e)}))),
    };

    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get::<_, String>(1)?;
            Ok(SatelliteDto {
                norad_id: row.get::<_, i64>(0)? as u64,
                name,
            })
        })
        .and_then(|iter| -> Result<Vec<SatelliteDto>, rusqlite::Error> { Ok(iter.filter_map(Result::ok).collect()) });

    match rows {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!(v))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("db error: {}", e)}))),
    }
}

async fn get_passes(Query(q): Query<PassQuery>, axum::extract::State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    let now = chrono::Utc::now();
    let maybe_el = state.elements.iter().find(|e| e.norad_id == q.norad_id);
    let el = match maybe_el {
        Some(e) => e,
        None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "norad_id not found in loaded TLEs"}))),
    };

    // Resolve ground station coordinates
    let (lat, lon) = if let Some(id) = q.station_id {
        match crate::utils::db::open_or_init().and_then(|c| crate::utils::db::get_station(&c, id)) {
            Ok(st) => (st.lat, st.lon),
            Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "station_id not found"}))),
        }
    } else if let (Some(lat), Some(lon)) = (q.lat, q.lon) {
        (lat, lon)
    } else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "missing lat/lon or station_id"})));
    };

    match predict_passes(el, lat, lon, now, q.duration, q.step, q.min_el) {
        Ok(wins) => {
            let out: Vec<PassWindowDto> = wins
                .into_iter()
                .map(|w: PassWindow| PassWindowDto {
                    start: w.start,
                    end: w.end,
                    max_elevation_deg: w.max_elevation_deg,
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(out)))
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("prediction error: {}", e)}))),
    }
}

async fn get_passes_for_satellite(Path(norad_id): Path<u64>, Query(q): Query<PassQuery>, axum::extract::State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    let now = chrono::Utc::now();
    let maybe_el = state.elements.iter().find(|e| e.norad_id == norad_id);
    let el = match maybe_el {
        Some(e) => e,
        None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "norad_id not found in loaded TLEs"}))),
    };

    let (lat, lon) = if let Some(id) = q.station_id {
        match crate::utils::db::open_or_init().and_then(|c| crate::utils::db::get_station(&c, id)) {
            Ok(st) => (st.lat, st.lon),
            Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "station_id not found"}))),
        }
    } else if let (Some(lat), Some(lon)) = (q.lat, q.lon) {
        (lat, lon)
    } else {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "missing lat/lon or station_id"})));
    };

    match predict_passes(el, lat, lon, now, q.duration, q.step, q.min_el) {
        Ok(wins) => {
            let out: Vec<PassWindowDto> = wins
                .into_iter()
                .map(|w: PassWindow| PassWindowDto {
                    start: w.start,
                    end: w.end,
                    max_elevation_deg: w.max_elevation_deg,
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(out)))
        }
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("prediction error: {}", e)}))),
    }
}

async fn health(axum::extract::State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    let count = state.elements.len();
    let db_ok = crate::utils::db::open_or_init().is_ok();
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok", "elements": count, "db": db_ok })))
}

async fn list_stations() -> impl IntoResponse {
    match crate::utils::db::open_or_init().and_then(|c| crate::utils::db::list_stations(&c)) {
        Ok(stations) => {
            let out: Vec<StationDto> = stations
                .into_iter()
                .map(|s| StationDto { id: s.id, name: s.name, lat: s.lat, lon: s.lon })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(out)))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("db error: {}", e)}))),
    }
}

async fn list_sat_positions(axum::extract::State(state): axum::extract::State<AppState>, Query(q): Query<SatPosQuery>) -> impl IntoResponse {
    use chrono::Utc;
    let now = Utc::now();
    let gmst_rad = gmst(now);
    let limit = q.limit.unwrap_or(500);

    let mut out = Vec::with_capacity(limit);
    for e in state.elements.iter().take(limit) {
        let minutes_since_epoch = minutes_since_elements_epoch(e, now);
        match sgp4::Constants::from_elements(e).and_then(|c| c.propagate(minutes_since_epoch)) {
            Ok(pred) => {
                let (x, y, z) = eci_to_ecef(&pred.position, gmst_rad);
                let (lat, lon) = ecef_to_geodetic(x, y, z);
                let speed_km_s = (pred.velocity[0].powi(2) + pred.velocity[1].powi(2) + pred.velocity[2].powi(2)).sqrt();
                let radius_km = (pred.position[0].powi(2) + pred.position[1].powi(2) + pred.position[2].powi(2)).sqrt();
                let alt_km = radius_km - 6378.137f64; // equatorial radius
                out.push(serde_json::json!({
                    "norad_id": e.norad_id,
                    "name": e.object_name.clone().unwrap_or_else(|| "".to_string()),
                    "lat": lat,
                    "lon": lon,
                    "alt_km": alt_km,
                    "speed_km_s": speed_km_s,
                    "epoch": e.datetime.to_string()
                }));
            }
            Err(_) => {}
        }
    }
    (StatusCode::OK, Json(serde_json::json!(out)))
}

fn minutes_since_elements_epoch(elements: &sgp4::Elements, t: chrono::DateTime<chrono::Utc>) -> f64 {
    let epoch = elements.datetime;
    let t_naive = t.naive_utc();
    let diff = t_naive - epoch;
    diff.num_seconds() as f64 / 60.0
}

fn gmst(t: chrono::DateTime<chrono::Utc>) -> f64 {
    use chrono::NaiveDate;
    let j2000_naive = NaiveDate::from_ymd_opt(2000, 1, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let secs = (t.naive_utc() - j2000_naive).num_seconds() as f64;
    let days = secs / 86400.0;
    let gmst_deg = 280.46061837 + 360.98564736629 * days;
    (gmst_deg.rem_euclid(360.0)) * std::f64::consts::PI / 180.0
}

fn eci_to_ecef(pos_eci_km: &[f64; 3], gmst_rad: f64) -> (f64, f64, f64) {
    let (sin_t, cos_t) = gmst_rad.sin_cos();
    let x_ecef = cos_t * pos_eci_km[0] + sin_t * pos_eci_km[1];
    let y_ecef = -sin_t * pos_eci_km[0] + cos_t * pos_eci_km[1];
    let z_ecef = pos_eci_km[2];
    (x_ecef, y_ecef, z_ecef)
}

fn ecef_to_geodetic(x: f64, y: f64, z: f64) -> (f64, f64) {
    // WGS84
    let a = 6378.137f64; // km
    let f = 1.0 / 298.257_223_563;
    let b = a * (1.0 - f);
    let e2 = f * (2.0 - f);
    let ep2 = (a*a - b*b) / (b*b);
    let p = (x*x + y*y).sqrt();
    let th = (a * z).atan2(b * p);
    let sin_th = th.sin();
    let cos_th = th.cos();
    let lat = (z + ep2 * b * sin_th.powi(3)).atan2(p - e2 * a * cos_th.powi(3));
    let lon = y.atan2(x);
    (lat.to_degrees(), lon.to_degrees())
}

async fn create_station(Json(body): Json<CreateStationDto>) -> impl IntoResponse {
    // Basic validation
    if !(body.lat >= -90.0 && body.lat <= 90.0 && body.lon >= -180.0 && body.lon <= 180.0) {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error": "lat/lon out of range"})));
    }

    match crate::utils::db::open_or_init().and_then(|c| {
        let id = crate::utils::db::insert_station(&c, body.name.as_deref(), body.lat, body.lon)?;
        Ok::<i64, crate::utils::db::DbError>(id)
    }) {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({"id": id}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("db error: {}", e)}))),
    }
}

async fn get_station(Path(id): Path<i64>) -> impl IntoResponse {
    match crate::utils::db::open_or_init().and_then(|c| crate::utils::db::get_station(&c, id)) {
        Ok(s) => (StatusCode::OK, Json(serde_json::json!(StationDto { id: s.id, name: s.name, lat: s.lat, lon: s.lon }))),
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "station not found"}))),
    }
}

async fn update_station(Path(id): Path<i64>, Json(body): Json<CreateStationDto>) -> impl IntoResponse {
    if !(body.lat >= -90.0 && body.lat <= 90.0 && body.lon >= -180.0 && body.lon <= 180.0) {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error": "lat/lon out of range"})));
    }
    match crate::utils::db::open_or_init().and_then(|c| crate::utils::db::update_station(&c, id, body.name.as_deref(), body.lat, body.lon)) {
        Ok(()) => (StatusCode::NO_CONTENT, Json(serde_json::json!({}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("db error: {}", e)}))),
    }
}

async fn delete_station(Path(id): Path<i64>) -> impl IntoResponse {
    match crate::utils::db::open_or_init().and_then(|c| crate::utils::db::delete_station(&c, id)) {
        Ok(()) => (StatusCode::NO_CONTENT, Json(serde_json::json!({}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("db error: {}", e)}))),
    }
}