use std::fs;
use std::path::PathBuf;

use thiserror::Error;
use tracing::{info, warn};

const CELESTRAK_ACTIVE_TLE_URL: &str = "https://celestrak.org/NORAD/elements/gp.php?GROUP=active&format=tle";

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Fetches the active satellites TLE from Celestrak and caches it under `data/tle/`.
/// Returns the path to the cached file.
pub async fn fetch_celestrak_active_tle() -> Result<PathBuf, FetchError> {
    let dir = PathBuf::from("data/tle");
    fs::create_dir_all(&dir)?;

    let filename = format!(
        "celestrak-active-{}.tle",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let path = dir.join(filename);

    info!("Fetching TLE from {}", CELESTRAK_ACTIVE_TLE_URL);

    let client = reqwest::Client::builder()
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .build()?;

    let resp = client.get(CELESTRAK_ACTIVE_TLE_URL).send().await?;

    if !resp.status().is_success() {
        warn!(status = ?resp.status(), "Non-success response fetching TLE");
    }

    let body = resp.text().await?;
    fs::write(&path, body)?;
    info!(path = %path.display(), "Cached TLE set");

    Ok(path)
}