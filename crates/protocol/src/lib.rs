//! What the server and clients say to each other over the WebSocket: one postcard-encoded
//! message per binary frame.
//!
//! The server runs the only real match; clients send inputs and draw the snapshots they get
//! back (predicting their own fighter's movement in between).
//!
//! # Keeping the format honest
//!
//! Snapshots carry the sim's own structs (`MatchView`, `Fighter`, `Item`, ...). Anything only
//! the server needs is `#[serde(skip)]`ped there, so it never goes on the wire. The test
//! `wire_format_is_pinned` hashes one encoded sample of every message: any change to the
//! format, including one made in the sim, fails it until `PROTOCOL_VERSION` is bumped (clients
//! on another version are turned away at `Hello` rather than failing to decode).
//!
//! # Adding a kind of message (e.g. chat or emoji reactions)
//!
//! 1. Add a variant at the end of `ClientMsg` (client to server) and/or `ServerMsg` (server to
//!    client): say `ClientMsg::Chat(String)` / `ServerMsg::Chat { from: PlayerId, text: String }`.
//! 2. Give it a sample in `tests::samples` (it won't compile until you do), bump
//!    `PROTOCOL_VERSION` and update `WIRE`.
//! 3. Server: handle it in `room.rs` `on_msg`. Messages are delivered in order and reliably,
//!    except that `conn.rs` drops an `Input` rather than wait on a busy room; chat should be
//!    reliable but rate-limited per connection (see `docs/TODO.md`).
//! 4. Client: handle the `ServerMsg` in `App::update_net`.

use go_box_sim::character::Character;
use go_box_sim::sim::{Action, ArenaId, Event, MatchConfig, MatchView, N_FIGHTERS};
use serde::{Deserialize, Serialize};

/// Bumped on any incompatible change; the server turns away clients on another version.
pub const PROTOCOL_VERSION: u16 = 10;

/// The port the server listens on by default (and the native test client dials).
pub const DEFAULT_PORT: u16 = 7878;
/// The WebSocket endpoint, on the same host as the page. (`web/net.js` uses it too.)
pub const WS_PATH: &str = "/ws";

/// Players per room (one per seat). Seats without a player hold a bot or nobody.
pub const MAX_PLAYERS: usize = N_FIGHTERS;
pub const MAX_NAME_LEN: usize = 16;
/// Room codes are this many letters from `CODE_ALPHABET`.
pub const CODE_LEN: usize = 4;
/// No I or O, so codes can't be misread as 1 or 0.
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

/// The server sends a snapshot every this many sim ticks (TICK_RATE / 2 = 30 per second).
pub const SNAPSHOT_EVERY: u32 = 2;

pub type PlayerId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];
    /// A stable id; players see the text `difficulty.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }
}

/// What fills a seat no player is in: a bot, or nobody (fewer fighters; down to just you).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SeatFill {
    #[default]
    Bot,
    Empty,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    /// First message on every connection.
    Hello { version: u16, name: String },
    CreateRoom,
    JoinRoom { code: String },
    LeaveRoom,
    SetCharacter(Character),
    /// "Play again" from a non-leader: shown to the leader, who can start regardless.
    SetReady(bool),
    /// Leader only.
    SetDifficulty(Difficulty),
    /// Leader only: the map for the next match.
    SetArena(ArenaId),
    /// Leader only: what fills seat `seat` when no player is in it.
    SetSeatFill { seat: u8, fill: SeatFill },
    /// Leader only: start the next match with everyone in the room.
    Start,
    /// One per client tick while playing, numbered so snapshots can say which was last applied.
    Input { seq: u32, action: Action },
}

/// Why the server refused something. The client shows it in the player's language.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Refusal {
    /// Too many connections right now.
    ServerFull,
    /// The page (client) is a different version from the server: reload.
    OutOfDate,
    TooManyRooms,
    NoSuchRoom,
    RoomFull,
    /// The room closed while you were joining.
    RoomClosed,
}

impl Refusal {
    /// A stable id; players see the text `refusal.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Refusal::ServerFull => "server_full",
            Refusal::OutOfDate => "out_of_date",
            Refusal::TooManyRooms => "too_many_rooms",
            Refusal::NoSuchRoom => "no_such_room",
            Refusal::RoomFull => "room_full",
            Refusal::RoomClosed => "room_closed",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome { you: PlayerId },
    /// Something was refused (bad code, room full, version mismatch...).
    Error(Refusal),
    /// The room changed: who's in it, choices, phase. Sent to everyone in the room.
    Room(RoomInfo),
    /// Left the room (or it was closed): back to the home screen.
    LeftRoom,
    MatchStart(MatchStart),
    Snapshot(Snapshot),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub name: String,
    pub character: Character,
    pub ready: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Phase {
    /// Waiting for the leader to start; shows the last match's results if there was one.
    Lobby,
    Playing,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub leader: PlayerId,
    /// In join order.
    pub players: Vec<PlayerInfo>,
    pub difficulty: Difficulty,
    pub arena: ArenaId,
    /// Per seat: what fills it when no player is in it. Players take seats in join order.
    pub fills: [SeatFill; MAX_PLAYERS],
    pub phase: Phase,
    pub last_results: Option<Results>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Seat {
    /// The player's name; empty for a bot (the client names it after its character).
    pub name: String,
    pub character: Character,
    /// None = a bot.
    pub player: Option<PlayerId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchStart {
    pub seats: [Seat; N_FIGHTERS],
    /// The rules: the map, how long it lasts, how it's won.
    pub config: MatchConfig,
    /// Your seat, or None if you're watching (joined mid-match).
    pub your_seat: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    /// The last of your inputs the server has applied (0 = none yet).
    pub ack: u32,
    pub view: MatchView,
    /// Everything that happened since the previous snapshot (hits, jumps, explosions...).
    pub events: Vec<Event>,
}

/// One line of the end-of-match scoreboard.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeatResult {
    /// Empty for a bot, as in `Seat`.
    pub name: String,
    pub character: Character,
    pub bot: bool,
    pub kills: u32,
    pub headshots: u32,
    pub damage_dealt: f32,
    pub damage_taken: f32,
    /// Seconds into the match they went down, or None if they survived.
    pub died_at: Option<f32>,
    pub hp: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Results {
    /// The seats that were in the match (empty ones left out).
    pub seats: Vec<SeatResult>,
    /// Index into `seats`; None = a draw.
    pub winner: Option<usize>,
    /// Match length in seconds.
    pub duration: f32,
}

pub fn encode<T: Serialize>(msg: &T) -> Vec<u8> {
    postcard::to_allocvec(msg).expect("messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Option<T> {
    postcard::from_bytes(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use go_box_sim::sim::Match;

    #[test]
    fn snapshot_round_trips_and_is_small() {
        let m = Match::new(1, &MatchConfig::default());
        let msg = ServerMsg::Snapshot(Snapshot { ack: 7, view: m.view(), events: Vec::new() });
        let bytes = encode(&msg);
        assert!(bytes.len() < 1500, "snapshot is {} bytes", bytes.len());
        let Some(ServerMsg::Snapshot(s)) = decode::<ServerMsg>(&bytes) else { panic!() };
        assert_eq!(s.ack, 7);
        assert_eq!(s.view.f[2].pos, m.f[2].pos);
    }

    /// A match of bots playing every character, with a snapshot taken while a gravity bomb is
    /// out and another while Tyler's dogs are running, so their layouts are on the wire too.
    fn sample_snapshots() -> Vec<Snapshot> {
        use go_box_sim::brain::Brain;
        let brain = Brain::from_json(include_str!("../../../brains/gen_219000.json")).unwrap();
        let mut m = Match::new(5, &MatchConfig::default());
        for (f, c) in m.f.iter_mut().zip(Character::ALL) {
            f.character = c;
        }
        use go_box_sim::character::Effect;
        let (mut bomb, mut dogs) = (None, None);
        while (bomb.is_none() || dogs.is_none()) && !m.over {
            m.step([&brain; N_FIGHTERS]);
            let snap = || Snapshot { ack: m.tick, view: m.view(), events: m.events.clone() };
            if bomb.is_none() && m.effects.iter().any(|e| matches!(e, Effect::Bomb(_))) {
                bomb = Some(snap());
            }
            if dogs.is_none() && m.effects.iter().any(|e| matches!(e, Effect::Dog(_))) {
                dogs = Some(snap());
            }
        }
        vec![Snapshot { ack: 0, view: Match::new(1, &MatchConfig::default()).view(), events: Vec::new() }, bomb.expect("a bomb was thrown"), dogs.expect("dogs ran")]
    }

    /// One of every message, with fixed contents. The `match`es have no wildcard arm, so a new
    /// message kind doesn't compile until it has a sample here.
    fn samples() -> Vec<Vec<u8>> {
        use go_box_sim::sim::Action;
        let action = Action { mv: -1.0, run: true, jump: true, attack: false, throw: true, aim: 0.3, grab: false, special: true, target: Some(glam_vec(0.6, -0.8)) };
        let client = [
            ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Sam".into() },
            ClientMsg::CreateRoom,
            ClientMsg::JoinRoom { code: "ABCD".into() },
            ClientMsg::LeaveRoom,
            ClientMsg::SetCharacter(Character::Tyler),
            ClientMsg::SetReady(true),
            ClientMsg::SetDifficulty(Difficulty::Hard),
            ClientMsg::SetArena(ArenaId::Bridges),
            ClientMsg::SetSeatFill { seat: 2, fill: SeatFill::Empty },
            ClientMsg::Start,
            ClientMsg::Input { seq: 77, action },
        ];
        for m in &client {
            match m {
                ClientMsg::Hello { .. }
                | ClientMsg::CreateRoom
                | ClientMsg::JoinRoom { .. }
                | ClientMsg::LeaveRoom
                | ClientMsg::SetCharacter(_)
                | ClientMsg::SetReady(_)
                | ClientMsg::SetDifficulty(_)
                | ClientMsg::SetArena(_)
                | ClientMsg::SetSeatFill { .. }
                | ClientMsg::Start
                | ClientMsg::Input { .. } => {}
            }
        }
        let seat = |name: &str, c, player| Seat { name: name.into(), character: c, player };
        let result = SeatResult { name: "Bot".into(), character: Character::Sam, bot: true, kills: 2, headshots: 1, damage_dealt: 88.5, damage_taken: 100.0, died_at: Some(31.5), hp: 0.0 };
        let room = RoomInfo {
            code: "ABCD".into(),
            leader: 1,
            players: vec![PlayerInfo { id: 1, name: "Sam".into(), character: Character::Austin, ready: true }],
            difficulty: Difficulty::Easy,
            arena: ArenaId::Bridges,
            fills: [SeatFill::Bot, SeatFill::Bot, SeatFill::Empty, SeatFill::Bot],
            phase: Phase::Lobby,
            last_results: Some(Results { seats: vec![result], winner: Some(0), duration: 44.5 }),
        };
        let start = MatchStart {
            seats: [seat("Sam", Character::Sam, Some(1)), seat("A", Character::Austin, None), seat("T", Character::Tyler, None), seat("C", Character::Classic, None)],
            your_seat: Some(0),
            config: MatchConfig { arena: ArenaId::Bridges, max_ticks: 1234, ..MatchConfig::default() },
        };
        let mut server = vec![ServerMsg::Welcome { you: 9 }, ServerMsg::Error(Refusal::NoSuchRoom), ServerMsg::Room(room), ServerMsg::LeftRoom, ServerMsg::MatchStart(start)];
        server.extend(sample_snapshots().into_iter().map(ServerMsg::Snapshot));
        for m in &server {
            match m {
                ServerMsg::Welcome { .. } | ServerMsg::Error(_) | ServerMsg::Room(_) | ServerMsg::LeftRoom | ServerMsg::MatchStart(_) | ServerMsg::Snapshot(_) => {}
            }
        }
        client.iter().map(encode).chain(server.iter().map(encode)).collect()
    }

    fn glam_vec(x: f32, y: f32) -> go_box_sim::glam::Vec2 {
        go_box_sim::glam::vec2(x, y)
    }

    /// The wire format, pinned: the bytes of every sample message, hashed. If this fails, the
    /// format changed: bump PROTOCOL_VERSION (so old clients are turned away instead of
    /// silently failing to decode), then update WIRE below to the new version and hash.
    #[test]
    fn wire_format_is_pinned() {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for bytes in samples() {
            for b in bytes.iter().copied().chain((bytes.len() as u32).to_le_bytes()) {
                h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
            }
        }
        assert_eq!((PROTOCOL_VERSION, h), WIRE, "the wire format changed ({h:#018x}): bump PROTOCOL_VERSION, then update WIRE");
    }
    const WIRE: (u16, u64) = (10, 0x096a_0abf_30e3_fe27);

    #[test]
    fn every_sample_round_trips() {
        let samples = samples();
        for bytes in &samples[..11] {
            assert_eq!(encode(&decode::<ClientMsg>(bytes).unwrap()), *bytes);
        }
        for bytes in &samples[11..] {
            assert_eq!(encode(&decode::<ServerMsg>(bytes).unwrap()), *bytes);
        }
    }

    #[test]
    fn garbage_is_rejected_not_a_panic() {
        assert!(decode::<ClientMsg>(&[0xff, 0xff, 0xff]).is_none());
        assert!(decode::<ClientMsg>(&[]).is_none());
    }
}
