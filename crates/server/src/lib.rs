//! The server as a library, so integration tests can run it in-process.

mod conn;
mod room;

use axum::Router;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use go_box_protocol::{CODE_ALPHABET, CODE_LEN, DEFAULT_PORT, WS_PATH};
use go_box_sim::brain::Brain;
use rand::Rng;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

pub use room::RoomCmd;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub web: PathBuf,
    pub max_rooms: usize,
    pub max_connections: usize,
    /// At most this many connections from one address.
    pub max_per_ip: usize,
    /// Pages allowed to open a game connection (their `Origin`, e.g. `https://go-box.fly.dev`).
    /// Empty: any (fine on your own machine).
    pub origins: Vec<String>,
    /// Behind a proxy (Fly), take the client's address from the `Fly-Client-IP` /
    /// `X-Forwarded-For` header. Only set this when the proxy is the only way in, else anyone could
    /// claim any address.
    pub trust_proxy: bool,
    /// When set, creating a room needs this passphrase (joining one only needs its code). Keep it
    /// out of the repo: on Fly it's a secret (`fly secrets set GO_BOX_CREATE_PASSPHRASE=...`).
    pub create_passphrase: Option<String>,
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
            max_per_ip: num("GO_BOX_MAX_PER_IP", 8)?,
            origins: var("GO_BOX_ORIGINS", "").split(',').map(|o| o.trim().trim_end_matches('/').to_string()).filter(|o| !o.is_empty()).collect(),
            trust_proxy: var("GO_BOX_TRUST_PROXY", "") == "1",
            create_passphrase: Some(var("GO_BOX_CREATE_PASSPHRASE", "")).filter(|p| !p.is_empty()),
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
    /// Open connections per client address.
    per_ip: Mutex<HashMap<IpAddr, usize>>,
    config: Config,
}

impl Hub {
    fn new(config: Config) -> Arc<Self> {
        let brain = bot_brain();
        Arc::new(Self { rooms: Mutex::new(HashMap::new()), brain: Arc::new(brain), next_id: AtomicU32::new(1), connections: AtomicUsize::new(0), per_ip: Mutex::new(HashMap::new()), config })
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
    // The browser client: served pre-compressed when the build made `.br`/`.gz` copies (the Docker
    // image does; it saves ~2/3 of the 1MB download at no CPU cost), and marked to be re-checked
    // on every load, so players pick up a new version right after a deploy (unchanged files
    // just get a quick "not modified").
    let files = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("no-cache")))
        .service(ServeDir::new(web).precompressed_br().precompressed_gzip());
    let app = Router::new().route(WS_PATH, get(ws_handler)).route("/healthz", get(healthz)).fallback_service(files).with_state(hub);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).with_graceful_shutdown(shutdown).await.unwrap();
}

/// For the host's health checks: 200 and a little JSON while the server is up.
async fn healthz(State(hub): State<Arc<Hub>>) -> impl IntoResponse {
    let rooms = hub.rooms.lock().unwrap().len();
    let connections = hub.connections.load(Ordering::SeqCst);
    ([(axum::http::header::CONTENT_TYPE, "application/json")], format!("{{\"rooms\":{rooms},\"connections\":{connections}}}"))
}

async fn ws_handler(ws: WebSocketUpgrade, State(hub): State<Arc<Hub>>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap) -> Response {
    // Only our own pages may connect (when the allowed pages are set): stops other sites from
    // opening game connections from their visitors' browsers.
    if !hub.config.origins.is_empty() {
        let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()).unwrap_or("");
        if !hub.config.origins.iter().any(|o| o == origin) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let ip = client_ip(&hub.config, peer, &headers);
    let Some(ip_slot) = IpSlot::take(&hub, ip) else {
        return (StatusCode::TOO_MANY_REQUESTS, "too many connections from your address").into_response();
    };
    // Messages are small; anything big is garbage.
    ws.max_message_size(4096).max_frame_size(4096)
        .on_upgrade(move |socket| async move {
            let _ip_slot = ip_slot;
            conn::run(socket, hub).await;
        })
        .into_response()
}

/// Who's connecting: the peer address, or behind a trusted proxy the address it reports.
fn client_ip(config: &Config, peer: SocketAddr, headers: &HeaderMap) -> IpAddr {
    let header_ip = |name: &str| headers.get(name)?.to_str().ok()?.split(',').next()?.trim().parse::<IpAddr>().ok();
    if config.trust_proxy {
        header_ip("fly-client-ip").or_else(|| header_ip("x-forwarded-for")).unwrap_or(peer.ip())
    } else {
        peer.ip()
    }
}

/// One of an address's connections; counts back down however the connection ends.
struct IpSlot(Arc<Hub>, IpAddr);

impl IpSlot {
    fn take(hub: &Arc<Hub>, ip: IpAddr) -> Option<Self> {
        let mut per_ip = hub.per_ip.lock().unwrap();
        let n = per_ip.entry(ip).or_insert(0);
        if *n >= hub.config.max_per_ip {
            return None;
        }
        *n += 1;
        Some(Self(hub.clone(), ip))
    }
}

impl Drop for IpSlot {
    fn drop(&mut self) {
        let mut per_ip = self.0.per_ip.lock().unwrap();
        if let Some(n) = per_ip.get_mut(&self.1) {
            *n -= 1;
            if *n == 0 {
                per_ip.remove(&self.1);
            }
        }
    }
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

impl Hub {
    /// May whoever offers `passphrase` create a room? (Yes for anyone if no passphrase is set.)
    fn may_create(&self, passphrase: &str) -> bool {
        match &self.config.create_passphrase {
            None => true,
            Some(want) => constant_time_eq(want.as_bytes(), passphrase.as_bytes()),
        }
    }
}

/// Compares without stopping at the first difference, so timing doesn't reveal how much of a guess
/// was right.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() != b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}
