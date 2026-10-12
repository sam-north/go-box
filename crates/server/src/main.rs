//! go-box server: hosts rooms, runs their matches at 60 ticks/s, and serves the browser client.
//!
//! Configuration (environment variables; the defaults are for running on your own machine, and
//! `fly.toml` sets the cloud ones):
//! - `GO_BOX_BIND`   address to listen on (default `127.0.0.1:7878`: this machine only).
//! - `GO_BOX_WEB`    directory with the built browser client (default `web`).
//! - `GO_BOX_MAX_ROOMS` / `GO_BOX_MAX_CONNECTIONS` caps (defaults 16 / 64).
//! - `RUST_LOG`      log filter (default `info`).

use go_box_server::{Config, serve};
use std::time::Duration;
use tracing_subscriber::EnvFilter;

/// After a stop signal, how long open connections get before the server exits anyway (players'
/// WebSockets would otherwise keep it running forever).
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))).init();
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("go-box-server: {e}");
            std::process::exit(1);
        }
    };
    let listener = match tokio::net::TcpListener::bind(&config.bind).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("go-box-server: can't listen on {}: {e}", config.bind);
            std::process::exit(1);
        }
    };
    tracing::info!("listening on http://{}", listener.local_addr().unwrap());
    tokio::select! {
        _ = serve(listener, config, stop_signal()) => {}
        _ = async {
            stop_signal().await;
            tokio::time::sleep(SHUTDOWN_GRACE).await;
        } => tracing::info!("closing the remaining connections"),
    }
}

/// Ctrl+C, or SIGTERM (what Docker and cloud hosts send to stop a server).
async fn stop_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutting down");
}
