//! go-box server: hosts rooms, runs their matches at 60 ticks/s, and serves the browser client.
//!
//! Configuration (environment variables):
//! - `GO_BOX_BIND`   address to listen on (default `127.0.0.1:7878`: this machine only).
//! - `GO_BOX_WEB`    directory with the built browser client (default `web`).
//! - `GO_BOX_BRAINS` directory with the bot brains (default `brains`).
//! - `GO_BOX_MAX_ROOMS` / `GO_BOX_MAX_CONNECTIONS` caps (defaults 16 / 64).
//! - `RUST_LOG`      log filter (default `info`).

use go_box_server::{Config, serve};
use tracing_subscriber::EnvFilter;

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
    serve(listener, config, async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutting down");
    })
    .await;
}
