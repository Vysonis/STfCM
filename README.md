# STfCM – Satellite Tracker for Conflict Monitoring

<img width="1920" height="1080" alt="stfcm" src="https://github.com/user-attachments/assets/88314c0e-e50a-4a36-aa2c-aa7b9897512c" />

STfCM is a small, satellite tracking app. A Rust backend serves a simple web UI, fetches and stores TLEs, propagates orbits with SGP4, and exposes endpoints used by the frontend to render satellites on a 3D globe and compute passes for user saved ground stations.

## Quick Start

- Requirements: `cargo` with Rust stable, internet access.
- Run the server: `cargo run -q`
- Open the app: `http://127.0.0.1:3000/`

## Features

- 3D globe with thousands of satellites (limit adjustable).
- Live health indicator (TLE elements availability, DB connectivity).
- Satellite name filter and click‑to‑inspect details.
- Ground station management (add/delete; persisted in SQLite).
- Pass prediction for a selected `NORAD` ID and station.
- Monochrome Earth texture for higher contrast against satellite markers.

## Project Layout

- `src/` – Rust backend
  - `api/` – HTTP server, types, route handlers (Axum)
  - `collectors/tle_fetcher.rs` – TLE ingestion from Celestrak (Reqwest)
  - `core/` – orbit/TLE parsing, propagation (SGP4)
  - `predictors/passes.rs` – pass prediction engine
  - `utils/` – logging (Tracing), SQLite helpers (Rusqlite)
- `web/` – static frontend assets
  - `index.html` – app shell
  - `styles.css` – minimal dark theme styling
  - `main.js` – globe rendering, UI logic (Globe.gl + Axios)
- `data/`
  - `tle/` – timestamped TLE snapshots
  - `db/` – `tracker.sqlite` with stations and metadata

## Tech Stack

- Backend: Rust `axum`, `tower-http`, `rusqlite` (bundled), `reqwest`, `sgp4`, `chrono`, `tracing`
- Frontend: vanilla HTML/CSS/JS, `globe.gl`, `axios`

## API Overview

- `GET /health`
  - Returns `{ elements: number, db: boolean }` summarizing TLE cache and DB reachability.

- `GET /satellites/positions?limit=<int>`
  - Returns an array of satellites with fields:
    - `norad_id`, `name`, `lat`, `lon`, `alt_km`, `speed_km_s`, `epoch`
  - The frontend applies a local name filter and renders points on the globe.

- `GET /satellites/{noradId}/passes?station_id=<id>&duration=<min>&step=<sec>&min_el=<deg>`
  - Returns predicted pass windows for the specified satellite and station.
  - Each item includes `start`, `end`, and `max_elevation_deg`.

- `GET /stations`
  - Returns the list of saved ground stations.

- `POST /stations` (JSON body)
  - `{ name: string | null, lat: f64, lon: f64 }`

- `DELETE /stations/{id}`
  - Removes a station by ID.

- Static assets: served under `/ui/*` and backed by files in `web/`.

## Frontend Behavior

- Globe
  - Click a satellite to see details in the header and footer.
  - Use the filter input to quickly narrow satellites by name.
  - Adjust the render limit to balance performance vs. detail.

- Stations & Passes
  - Add a station (name optional; lat/lon required) and it is saved in SQLite.
  - Select a station and provide a `NORAD` ID to compute predicted passes.

## Data & Storage

- TLE snapshots are stored in `data/tle/` and updated by the backend.
- SQLite DB lives at `data/db/tracker.sqlite` (created automatically).

## Configuration & Logging

- Logging respects `RUST_LOG` via Tracing’s env filter.
  - Examples:
    - Windows PowerShell: `$env:RUST_LOG = "info"; cargo run -q`
    - More detail: `$env:RUST_LOG = "debug,axum=info"`

## Development

- Run: `cargo run -q` and open `http://127.0.0.1:3000/`.
- Hot reload is not enabled; refresh your browser after changes.
- If your browser shows stale CSS/JS, use a hard refresh (`Ctrl+F5`) or DevTools → Disable cache.

## Project Notes

- No external database setup required; rusqlite uses a bundled SQLite.
- The app targets single‑node local usage; service hardening and multi‑user auth are out of scope for this minimal build.
