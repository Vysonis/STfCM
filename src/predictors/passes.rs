use chrono::{DateTime, Duration, NaiveDate, Utc};
use sgp4::Elements;

#[derive(Debug, Clone)]
pub struct PassWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub max_elevation_deg: f64,
}

/// Predict simple visibility passes over a ground location using elevation threshold.
/// - `ground_lat_deg`, `ground_lon_deg`: ground station geodetic coordinates (WGS84), altitude assumed 0.
/// - `start`: UTC start time for prediction window.
/// - `duration_minutes`: total minutes to scan.
/// - `step_seconds`: sampling step in seconds (e.g., 10).
/// - `min_elevation_deg`: minimum elevation angle to count as visible (e.g., 10°).
pub fn predict_passes(
    elements: &Elements,
    ground_lat_deg: f64,
    ground_lon_deg: f64,
    start: DateTime<Utc>,
    duration_minutes: i64,
    step_seconds: i64,
    min_elevation_deg: f64,
) -> sgp4::Result<Vec<PassWindow>> {
    let mut windows: Vec<PassWindow> = Vec::new();

    let end = start + Duration::minutes(duration_minutes);
    let mut t = start;

    let mut in_pass = false;
    let mut current_start: Option<DateTime<Utc>> = None;
    let mut max_el = f64::NEG_INFINITY;

    while t <= end {
        let minutes_since_epoch = minutes_since_elements_epoch(elements, t);
        let pred = sgp4::Constants::from_elements(elements)?.propagate(minutes_since_epoch)?;

        let gmst_rad = gmst(t);
        let (el_deg, _az_deg) = elevation_azimuth_deg(
            &pred.position,
            gmst_rad,
            ground_lat_deg,
            ground_lon_deg,
        );

        if el_deg >= min_elevation_deg {
            if !in_pass {
                in_pass = true;
                current_start = Some(t);
                max_el = el_deg;
            } else if el_deg > max_el {
                max_el = el_deg;
            }
        } else if in_pass {
            // pass ended
            in_pass = false;
            windows.push(PassWindow {
                start: current_start.unwrap(),
                end: t,
                max_elevation_deg: max_el,
            });
            current_start = None;
            max_el = f64::NEG_INFINITY;
        }

        t = t + Duration::seconds(step_seconds);
    }

    // If still in pass at the end, close it
    if in_pass {
        windows.push(PassWindow {
            start: current_start.unwrap(),
            end,
            max_elevation_deg: max_el,
        });
    }

    Ok(windows)
}

fn minutes_since_elements_epoch(elements: &Elements, t: DateTime<Utc>) -> f64 {
    let epoch = elements.datetime;
    let t_naive = t.naive_utc();
    let diff = t_naive - epoch;
    diff.num_seconds() as f64 / 60.0
}

/// Compute GMST (radians) from UTC time using a simplified expression.
fn gmst(t: DateTime<Utc>) -> f64 {
    // Seconds since J2000 (2000-01-01 12:00:00 UTC)
    let j2000_naive = NaiveDate::from_ymd_opt(2000, 1, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let secs = (t.naive_utc() - j2000_naive).num_seconds() as f64;
    let days = secs / 86400.0;
    let gmst_deg = 280.46061837 + 360.98564736629 * days;
    let gmst_rad = (gmst_deg.rem_euclid(360.0)) * std::f64::consts::PI / 180.0;
    gmst_rad
}

/// Convert satellite TEME/ECI position to elevation and azimuth from ground station.
fn elevation_azimuth_deg(
    pos_eci_km: &[f64; 3],
    gmst_rad: f64,
    ground_lat_deg: f64,
    ground_lon_deg: f64,
) -> (f64, f64) {
    let (sin_t, cos_t) = gmst_rad.sin_cos();
    // Rotate ECI -> ECEF about Z by GMST
    let x_ecef = cos_t * pos_eci_km[0] + sin_t * pos_eci_km[1];
    let y_ecef = -sin_t * pos_eci_km[0] + cos_t * pos_eci_km[1];
    let z_ecef = pos_eci_km[2];

    let lat = ground_lat_deg.to_radians();
    let lon = ground_lon_deg.to_radians();

    // WGS84 constants
    let a = 6378.137; // km
    let f = 1.0 / 298.257_223_563;
    let e2 = f * (2.0 - f);
    let sin_lat = lat.sin();
    let cos_lat = lat.cos();
    let sin_lon = lon.sin();
    let cos_lon = lon.cos();

    let n = a / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let x_gs = n * cos_lat * cos_lon;
    let y_gs = n * cos_lat * sin_lon;
    let z_gs = n * (1.0 - e2) * sin_lat;

    // Relative vector satellite - ground station in ECEF
    let rx = x_ecef - x_gs;
    let ry = y_ecef - y_gs;
    let rz = z_ecef - z_gs;

    // Transform to local ENU
    let east = -sin_lon * rx + cos_lon * ry;
    let north = -sin_lat * cos_lon * rx - sin_lat * sin_lon * ry + cos_lat * rz;
    let up = cos_lat * cos_lon * rx + cos_lat * sin_lon * ry + sin_lat * rz;

    let range = (east * east + north * north + up * up).sqrt();
    let el = (up / range).asin();
    let az = east.atan2(north);

    (el.to_degrees(), az.to_degrees())
}