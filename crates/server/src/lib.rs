//! The server as a library, so integration tests can run it in-process.

mod conn;
mod room;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use go_box_protocol::{CODE_ALPHABET, CODE_LEN, DEFAULT_PORT, WS_PATH};
use go_box_sim::brain::Brain;
use rand::Rng;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tower_http::services::ServeDir;

pub use room::RoomCmd;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub web: PathBuf,
    pub max_rooms: usize,
    pub max_connections: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
        let num = |k: &str, d: usize| -> Result<usize, String> { var(k, &d.to_string()).parse().map_err(|_| format!("{k} must be a number")) };
        Ok(Self {
            bind: var("GO_BOX_BIND", &format!("127.0.0.1:{DEFAULT_PORT}")),
            web: var("GO_BOX_WEB", "web").into(),
            max_rooms: num("GO_BOX_MAX_ROOMS", 16)?,
            max_connections: num("GO_BOX_MAX_CONNECTIONS", 64)?,
        })
    }
}

/// The brain every bot plays with: the strongest committed champion, built into the server.
const BOT_BRAIN: &str = include_str!("../../../brains/gen_219000.json");

pub fn bot_brain() -> Brain {
    Brain::from_json(BOT_BRAIN).expect("the built-in bot brain fits the network")
}

/// Shared by every connection: the rooms that exist and the bots' brain.
pub struct Hub {
    rooms: Mutex<HashMap<String, mpsc::Sender<RoomCmd>>>,
    brain: Arc<Brain>,
    next_id: AtomicU32,
    connections: AtomicUsize,
    config: Config,
}

impl Hub {
    fn new(config: Config) -> Arc<Self> {
        let brain = bot_brain();
        Arc::new(Self { rooms: Mutex::new(HashMap::new()), brain: Arc::new(brain), next_id: AtomicU32::new(1), connections: AtomicUsize::new(0), config })
    }

    /// Opens a new room with a fresh code, or None if the server is at its room cap.
    fn create_room(self: &Arc<Self>) -> Option<(String, mpsc::Sender<RoomCmd>)> {
        let mut rooms = self.rooms.lock().unwrap();
        if rooms.len() >= self.config.max_rooms {
            return None;
        }
        let mut rng = rand::thread_rng();
        let code = loop {
            let code: String = (0..CODE_LEN).map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char).collect();
            if !rooms.contains_key(&code) {
                break code;
            }
        };
        let (tx, rx) = mpsc::channel(256);
        rooms.insert(code.clone(), tx.clone());
        tokio::spawn(room::run(code.clone(), rx, self.clone()));
        tracing::info!(room = %code, rooms = rooms.len(), "room opened");
        Some((code, tx))
    }

    fn find_room(&self, code: &str) -> Option<mpsc::Sender<RoomCmd>> {
        self.rooms.lock().unwrap().get(&code.trim().to_ascii_uppercase()).cloned()
    }

    fn close_room(&self, code: &str) {
        let mut rooms = self.rooms.lock().unwrap();
        rooms.remove(code);
        tracing::info!(room = %code, rooms = rooms.len(), "room closed");
    }
}

/// Serves until `shutdown` resolves.
pub async fn serve(listener: tokio::net::TcpListener, config: Config, shutdown: impl Future<Output = ()> + Send + 'static) {
    if !config.web.join("index.html").exists() {
        tracing::warn!("no browser client in {} (build it with scripts/build-web.sh); only {WS_PATH} works", config.web.display());
    }
    let web = config.web.clone();
    let hub = Hub::new(config);
    let app = Router::new().route(WS_PATH, get(ws_handler)).route("/healthz", get(healthz)).fallback_service(ServeDir::new(web)).with_state(hub);
    axum::serve(listener, app).with_graceful_shutdown(shutdown).await.unwrap();
}

/// For the host's health checks: 200 and a little JSON while the server is up.
async fn healthz(State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    let rooms = hub.rooms.lock().unwrap().len();
    let connections = hub.connections.load(Ordering::SeqCst);
    ([(axum::http::header::CONTENT_TYPE, "application/json")], format!("{{\"rooms\":{rooms},\"connections\":{connections}}}"))
}

async fn ws_handler(ws: WebSocketUpgrade, State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    // Messages are small; anything big is garbage.
    ws.max_message_size(4096).max_frame_size(4096).on_upgrade(move |socket| conn::run(socket, hub))
}

/// Ensures the connection count goes back down however a connection ends.
struct ConnectionSlot(Arc<Hub>);

impl ConnectionSlot {
    fn take(hub: &Arc<Hub>) -> Option<Self> {
        let n = hub.connections.fetch_add(1, Ordering::SeqCst);
        if n >= hub.config.max_connections {
            hub.connections.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self(hub.clone()))
    }
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::SeqCst);
    }
}
