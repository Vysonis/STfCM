use tracing_subscriber::{fmt, EnvFilter};

pub fn init() {
    // Initialize global subscriber once safe to call at program start
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .compact()
        .init();
}