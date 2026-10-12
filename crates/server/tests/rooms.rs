//! End-to-end: a real server on a random local port, real WebSocket clients.

use futures_util::{SinkExt, StreamExt};
use go_box_protocol::*;
use go_box_server::{Config, serve};
use go_box_sim::character::Character;
use go_box_sim::sim::Action;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = Config { bind: addr.to_string(), web: root.join("web"), brains: root.join("brains"), max_rooms: 2, max_connections: 16 };
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
}

#[tokio::test]
async fn create_join_play_and_leave() {
    let url = start_server().await;
    let mut a = Client::connect(&url, "Alice").await;
    let mut b = Client::connect(&url, "  Bob\n").await;

    a.send(ClientMsg::CreateRoom).await;
    let room = a.room().await;
    assert_eq!(room.leader, a.id);
    assert_eq!(room.code.len(), CODE_LEN);

    b.send(ClientMsg::JoinRoom { code: room.code.to_lowercase() }).await;
    let room = b.room().await;
    assert_eq!(room.players.len(), 2);
    assert_eq!(room.players[1].name, "Bob");

    b.send(ClientMsg::SetCharacter(Character::Tyler)).await;
    b.send(ClientMsg::SetReady(true)).await;
    // Only the leader can start or change difficulty; Bob's attempts are ignored.
    b.send(ClientMsg::Start).await;
    b.send(ClientMsg::SetDifficulty(Difficulty::Hard)).await;
    a.send(ClientMsg::SetDifficulty(Difficulty::Easy)).await;
    let room = a.until(|m| if let ServerMsg::Room(r) = m { (r.difficulty == Difficulty::Easy && r.players[1].ready).then_some(r) } else { None }).await;
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

    a.send(ClientMsg::CreateRoom).await;
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
    fifth.send(ClientMsg::CreateRoom).await;
    fifth.room().await;
    let mut sixth = Client::connect(&url, "Six").await;
    sixth.send(ClientMsg::CreateRoom).await;
    assert_eq!(sixth.error().await, Refusal::TooManyRooms);

    // Wrong protocol version.
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws.send(Message::Binary(encode(&ClientMsg::Hello { version: PROTOCOL_VERSION + 1, name: "Old".into() }).into())).await.unwrap();
    let Some(Ok(Message::Binary(b))) = ws.next().await else { panic!() };
    assert!(matches!(decode(&b), Some(ServerMsg::Error(Refusal::OutOfDate))));
}

#[tokio::test]
async fn garbage_and_silence_dont_hurt() {
    let url = start_server().await;
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws.send(Message::Binary(vec![0xde, 0xad, 0xbe, 0xef].into())).await.unwrap();
    ws.send(Message::Text("hello?".into())).await.unwrap();
    // Still a working server for everyone else.
    let mut a = Client::connect(&url, "A").await;
    a.send(ClientMsg::CreateRoom).await;
    a.room().await;
}
