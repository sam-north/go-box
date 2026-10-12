//! End-to-end: a real server on a random local port, real WebSocket clients.

use futures_util::{SinkExt, StreamExt};
use go_box_protocol::*;
use go_box_server::{Config, serve};
use go_box_sim::character::Character;
use go_box_sim::sim::{Action, ArenaId};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start_server() -> String {
    start_server_with(|_| {}).await
}

/// A server on a free local port, with test-sized limits adjusted by `tweak`.
async fn start_server_with(tweak: impl FnOnce(&mut Config)) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut config = Config { bind: addr.to_string(), web: root.join("web"), max_rooms: 2, max_connections: 16, max_per_ip: 16, origins: Vec::new(), trust_proxy: false, create_passphrase: None };
    tweak(&mut config);
    tokio::spawn(serve(listener, config, std::future::pending()));
    format!("ws://{addr}/ws")
}

struct Client {
    ws: Ws,
    id: PlayerId,
}

impl Client {
    async fn connect(url: &str, name: &str) -> Self {
        let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        ws.send(Message::Binary(encode(&ClientMsg::Hello { version: PROTOCOL_VERSION, name: name.into() }).into())).await.unwrap();
        let mut c = Self { ws, id: 0 };
        match c.recv().await {
            ServerMsg::Welcome { you } => c.id = you,
            other => panic!("expected welcome, got {other:?}"),
        }
        c
    }

    async fn send(&mut self, msg: ClientMsg) {
        self.ws.send(Message::Binary(encode(&msg).into())).await.unwrap();
    }

    async fn recv(&mut self) -> ServerMsg {
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(5), self.ws.next()).await.expect("timed out waiting for the server").unwrap().unwrap();
            if let Message::Binary(b) = frame {
                return decode(&b).expect("server sent garbage");
            }
        }
    }

    /// Skips messages until one matches.
    async fn until<T>(&mut self, mut f: impl FnMut(ServerMsg) -> Option<T>) -> T {
        loop {
            if let Some(t) = f(self.recv().await) {
                return t;
            }
        }
    }

    async fn room(&mut self) -> RoomInfo {
        self.until(|m| if let ServerMsg::Room(r) = m { Some(r) } else { None }).await
    }

    async fn error(&mut self) -> Refusal {
        self.until(|m| if let ServerMsg::Error(e) = m { Some(e) } else { None }).await
    }

    /// True once the server has closed the connection (reading past anything it sent first).
    async fn closed(&mut self) -> bool {
        loop {
            match tokio::time::timeout(Duration::from_secs(5), self.ws.next()).await.expect("timed out waiting for the server") {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return true,
                Some(Ok(_)) => {}
            }
        }
    }
}

#[tokio::test]
async fn create_join_play_and_leave() {
    let url = start_server().await;
    let mut a = Client::connect(&url, "Alice").await;
    let mut b = Client::connect(&url, "  Bob\n").await;

    a.send(ClientMsg::CreateRoom { passphrase: String::new() }).await;
    let room = a.room().await;
    assert_eq!(room.leader, a.id);
    assert_eq!(room.code.len(), CODE_LEN);

    b.send(ClientMsg::JoinRoom { code: room.code.to_lowercase() }).await;
    let room = b.room().await;
    assert_eq!(room.players.len(), 2);
    assert_eq!(room.players[1].name, "Bob");

    // Only the leader can start or change the map; Bob's attempts are ignored. (His messages
    // arrive in order, so once he shows as ready the earlier ones have been dealt with.)
    b.send(ClientMsg::SetCharacter(Character::Tyler)).await;
    b.send(ClientMsg::Start).await;
    b.send(ClientMsg::SetArena(ArenaId::Bridges)).await;
    b.send(ClientMsg::SetReady(true)).await;
    let room = a.until(|m| if let ServerMsg::Room(r) = m { r.players[1].ready.then_some(r) } else { None }).await;
    assert_eq!(room.arena, ArenaId::Tiers, "not the leader: ignored");
    a.send(ClientMsg::SetArena(ArenaId::Bridges)).await;
    let room = a.until(|m| if let ServerMsg::Room(r) = m { (r.arena == ArenaId::Bridges).then_some(r) } else { None }).await;
    assert_eq!(room.phase, Phase::Lobby);
    assert_eq!(room.players[1].character, Character::Tyler);
    assert!(room.players[1].ready);

    // Seat 3 is a bot; seat 4 is left empty (Bob can't change that, Alice can).
    b.send(ClientMsg::SetSeatFill { seat: 3, fill: SeatFill::Empty }).await;
    a.send(ClientMsg::SetSeatFill { seat: 3, fill: SeatFill::Empty }).await;
    let room = a.until(|m| if let ServerMsg::Room(r) = m { (r.fills[3] == SeatFill::Empty).then_some(r) } else { None }).await;
    assert_eq!(room.fills[2], SeatFill::Bot);

    a.send(ClientMsg::Start).await;
    let start = a.until(|m| if let ServerMsg::MatchStart(s) = m { Some(s) } else { None }).await;
    assert_eq!(start.your_seat, Some(0));
    assert_eq!(start.config.arena, ArenaId::Bridges);
    assert_eq!(start.seats[1].character, Character::Tyler);
    assert!(start.seats[2].player.is_none(), "seat 3 is a bot");
    let b_start = b.until(|m| if let ServerMsg::MatchStart(s) = m { Some(s) } else { None }).await;
    assert_eq!(b_start.your_seat, Some(1));

    // Alice walks right; her inputs get acknowledged and her fighter moves.
    let x0 = a.until(|m| if let ServerMsg::Snapshot(s) = m { Some(s.view.f[0].pos.x) } else { None }).await;
    // One input per tick, like a real client (a burst would be trimmed as lag), toward the middle.
    let dir = if x0 > 160.0 { -1.0 } else { 1.0 };
    for seq in 1..=30 {
        a.send(ClientMsg::Input { seq, action: Action { mv: dir, ..Action::default() } }).await;
        tokio::time::sleep(Duration::from_millis(16)).await;
    }
    let (ack, me, empty) = a.until(|m| if let ServerMsg::Snapshot(s) = m { (s.ack >= 30).then(|| (s.ack, s.view.f[0].clone(), s.view.f[3].absent)) } else { None }).await;
    assert_eq!(ack, 30);
    assert!(empty, "seat 4 isn't in the match");
    // (A bot's punch could knock her about, so check the way she faces as well as where she is.)
    assert_eq!(me.facing, dir);
    assert!((me.pos.x - x0) * dir > 0.0 || me.hitstun > 0, "moved from {x0} to {}", me.pos.x);

    // A third player joining mid-match watches.
    let mut c = Client::connect(&url, "Cat").await;
    c.send(ClientMsg::JoinRoom { code: room.code.clone() }).await;
    let watch = c.until(|m| if let ServerMsg::MatchStart(s) = m { Some(s) } else { None }).await;
    assert_eq!(watch.your_seat, None);
    c.until(|m| if let ServerMsg::Snapshot(_) = m { Some(()) } else { None }).await;

    // The leader leaving hands leadership to the next-longest player.
    a.send(ClientMsg::LeaveRoom).await;
    a.until(|m| if let ServerMsg::LeftRoom = m { Some(()) } else { None }).await;
    let a_id = a.id;
    let room = b.until(|m| if let ServerMsg::Room(r) = m { (!r.players.iter().any(|p| p.id == a_id)).then_some(r) } else { None }).await;
    assert_eq!(room.leader, b.id);
    assert_eq!(room.phase, Phase::Playing, "the match plays on, Alice's fighter now a bot");
}

#[tokio::test]
async fn refusals() {
    let url = start_server().await;
    let mut a = Client::connect(&url, "A").await;
    a.send(ClientMsg::JoinRoom { code: "ZZZZ".into() }).await;
    assert_eq!(a.error().await, Refusal::NoSuchRoom);

    a.send(ClientMsg::CreateRoom { passphrase: String::new() }).await;
    let code = a.room().await.code;
    let mut others = Vec::new();
    for n in 0..3 {
        let mut c = Client::connect(&url, &format!("P{n}")).await;
        c.send(ClientMsg::JoinRoom { code: code.clone() }).await;
        c.room().await;
        others.push(c);
    }
    let mut fifth = Client::connect(&url, "Five").await;
    fifth.send(ClientMsg::JoinRoom { code }).await;
    assert_eq!(fifth.error().await, Refusal::RoomFull);

    // Room cap is 2 in this config.
    fifth.send(ClientMsg::CreateRoom { passphrase: String::new() }).await;
    fifth.room().await;
    let mut sixth = Client::connect(&url, "Six").await;
    sixth.send(ClientMsg::CreateRoom { passphrase: String::new() }).await;
    assert_eq!(sixth.error().await, Refusal::TooManyRooms);

    // Another protocol version, or a first message that doesn't even decode (a much older or
    // newer client): turned away with a close code any version can read.
    for first in [encode(&ClientMsg::Hello { version: PROTOCOL_VERSION + 1, name: "Old".into() }), vec![0xff, 0xfe, 0xfd]] {
        let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        ws.send(Message::Binary(first.into())).await.unwrap();
        let Some(Ok(Message::Close(Some(frame)))) = ws.next().await else { panic!("expected a close") };
        assert_eq!(u16::from(frame.code), CLOSE_OUT_OF_DATE);
    }
}

#[tokio::test]
async fn garbage_and_silence_dont_hurt() {
    let url = start_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws.send(Message::Binary(vec![0xde, 0xad, 0xbe, 0xef].into())).await.unwrap();
    ws.send(Message::Text("hello?".into())).await.unwrap();
    // Still a working server for everyone else.
    let mut a = Client::connect(&url, "A").await;
    a.send(ClientMsg::CreateRoom { passphrase: String::new() }).await;
    a.room().await;
}

/// A plain HTTP request to the server (no client library needed): the raw response.
async fn http_get(ws_url: &str, path: &str, headers: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let addr = ws_url.trim_start_matches("ws://").trim_end_matches(WS_PATH);
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n{headers}Connection: close\r\n\r\n").as_bytes()).await.unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).await.unwrap();
    String::from_utf8_lossy(&out).into_owned()
}

#[tokio::test]
async fn health_check() {
    let url = start_server().await;
    let _a = Client::connect(&url, "A").await;
    let r = http_get(&url, "/healthz", "").await;
    assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    assert!(r.ends_with(r#"{"rooms":0,"connections":1}"#), "{r}");
}

#[tokio::test]
async fn guessing_room_codes_or_flooding_gets_you_cut_off() {
    let url = start_server().await;
    let mut a = Client::connect(&url, "Guesser").await;
    for _ in 0..5 {
        a.send(ClientMsg::JoinRoom { code: "QQQQ".into() }).await;
        assert_eq!(a.error().await, Refusal::NoSuchRoom);
    }
    a.send(ClientMsg::JoinRoom { code: "QQQR".into() }).await;
    assert!(a.closed().await, "the 6th guess ends the connection");

    let mut b = Client::connect(&url, "Spammer").await;
    for _ in 0..40 {
        b.send(ClientMsg::SetReady(true)).await;
    }
    assert!(b.closed().await, "40 messages at once is past the allowance");
}

#[tokio::test]
async fn only_allowed_pages_may_connect() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let url = start_server_with(|c| c.origins = vec!["https://go-box.example".into()]).await;
    let from = |origin: &str| {
        let mut req = url.as_str().into_client_request().unwrap();
        req.headers_mut().insert("origin", origin.parse().unwrap());
        req
    };
    assert!(tokio_tungstenite::connect_async(from("https://evil.example")).await.is_err(), "another site is refused");
    assert!(tokio_tungstenite::connect_async(from("https://go-box.example")).await.is_ok());
}

#[tokio::test]
async fn one_address_cant_take_every_connection() {
    let url = start_server_with(|c| c.max_per_ip = 2).await;
    let _a = Client::connect(&url, "A").await;
    let b = Client::connect(&url, "B").await;
    assert!(tokio_tungstenite::connect_async(&url).await.is_err(), "a third from the same address is refused");
    drop(b);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let _c = Client::connect(&url, "C").await;
}

#[tokio::test]
async fn creating_a_room_can_need_a_passphrase() {
    let url = start_server_with(|c| c.create_passphrase = Some("correct horse".into())).await;
    let mut a = Client::connect(&url, "A").await;
    a.send(ClientMsg::CreateRoom { passphrase: "wrong".into() }).await;
    assert_eq!(a.error().await, Refusal::WrongPassphrase);
    a.send(ClientMsg::CreateRoom { passphrase: "correct horse".into() }).await;
    let room = a.room().await;
    // Joining only needs the code.
    let mut b = Client::connect(&url, "B").await;
    b.send(ClientMsg::JoinRoom { code: room.code.clone() }).await;
    assert_eq!(b.room().await.players.len(), 2);
    // Guessing the passphrase is cut off like guessing codes.
    let mut c = Client::connect(&url, "C").await;
    for _ in 0..5 {
        c.send(ClientMsg::CreateRoom { passphrase: "guess".into() }).await;
        assert_eq!(c.error().await, Refusal::WrongPassphrase);
    }
    c.send(ClientMsg::CreateRoom { passphrase: "guess".into() }).await;
    assert!(c.closed().await);
}

#[tokio::test]
async fn the_page_comes_compressed_when_possible_and_always_fresh() {
    let web = std::env::temp_dir().join(format!("go-box-web-{}", std::process::id()));
    std::fs::create_dir_all(&web).unwrap();
    std::fs::write(web.join("index.html"), "<p>plain</p>").unwrap();
    std::fs::write(web.join("index.html.gz"), "pretend-gzip").unwrap();
    let url = start_server_with(|c| c.web = web.clone()).await;

    let plain = http_get(&url, "/index.html", "").await.to_ascii_lowercase();
    assert!(plain.contains("cache-control: no-cache"), "{plain}");
    assert!(plain.ends_with("<p>plain</p>") && !plain.contains("content-encoding"));

    let gz = http_get(&url, "/index.html", "Accept-Encoding: gzip\r\n").await.to_ascii_lowercase();
    assert!(gz.contains("content-encoding: gzip") && gz.ends_with("pretend-gzip"), "{gz}");
    let _ = std::fs::remove_dir_all(&web);
}
