mod utils;
mod collectors;
mod core;
mod predictors;
mod api;
use tracing::info;

#[tokio::main]
async fn main() {
    utils::logging::init();
    info!("STfCM initialized");

    let path = match collectors::tle_fetcher::fetch_celestrak_active_tle().await {
        Ok(path) => {
            info!(path = %path.display(), "Fetched and cached TLEs");
            path
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch TLEs");
            return;
        }
    };

    match core::tle::parse_tle_file_to_elements(&path) {
        Ok(elements) => {
            info!(count = elements.len(), "Parsed elements from TLE file");
            // Initialize DB
            let conn = match utils::db::open_or_init() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to initialize database");
                    return;
                }
            };
            for (idx, el) in elements.iter().take(3).enumerate() {
                let name = el.object_name.as_deref().unwrap_or("<unnamed>");
                info!(sat_index = idx, norad = el.norad_id, name, "Propagating sample satellite");
                match core::orbit::propagate_minutes(el, 10.0) {
                    Ok(pred) => {
                        info!(
                            "Pos (km) = [{:.3}, {:.3}, {:.3}], Vel (km/s) = [{:.5}, {:.5}, {:.5}]",
                            pred.position[0], pred.position[1], pred.position[2],
                            pred.velocity[0], pred.velocity[1], pred.velocity[2]
                        );
                        // Persist snapshot
                        if let Err(e) = utils::db::upsert_satellite(&conn, el.norad_id, el.object_name.as_deref()) {
                            tracing::warn!(error = %e, norad = el.norad_id, "Failed to upsert satellite");
                        }
                        let ts = chrono::Utc::now().to_rfc3339();
                        if let Err(e) = utils::db::insert_snapshot(&conn, el.norad_id, &ts, &pred) {
                            tracing::warn!(error = %e, norad = el.norad_id, "Failed to insert snapshot");
                        } else {
                            info!(norad = el.norad_id, "Inserted snapshot");
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "Propagation failed"),
                }
            }

            if let Some(el0) = elements.get(0) {
                let start = chrono::Utc::now();
                match predictors::passes::predict_passes(el0, 40.7128, -74.0060, start, 120, 15, 10.0) {
                    Ok(windows) => {
                        for (i, w) in windows.iter().take(3).enumerate() {
                            info!(
                                "Pass {}: start={} end={} max_el={:.1}°",
                                i + 1,
                                w.start.to_rfc3339(),
                                w.end.to_rfc3339(),
                                w.max_elevation_deg
                            );
                        }
                    }
                    Err(e) => tracing::warn!(error = %e, "Pass prediction failed"),
                }
            }

            // Start API server with loaded elements
            let state = api::server::AppState { elements: std::sync::Arc::new(elements) };
            let addr: std::net::SocketAddr = "127.0.0.1:3000".parse().unwrap();
            api::server::run_server(state, addr).await;
        }
        Err(e) => tracing::error!(error = %e, "Failed to parse TLE file"),
    }
}
