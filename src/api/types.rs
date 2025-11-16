use serde::Serialize;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize)]
pub struct SatelliteDto {
    pub norad_id: u64,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct PassWindowDto {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub max_elevation_deg: f64,
}

#[derive(Debug, Serialize)]
pub struct StationDto {
    pub id: i64,
    pub name: Option<String>,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateStationDto {
    pub name: Option<String>,
    pub lat: f64,
    pub lon: f64,
}