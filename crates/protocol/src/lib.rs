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
pub const PROTOCOL_VERSION: u16 = 30;

/// The port the server listens on by default (and the native test client dials).
pub const DEFAULT_PORT: u16 = 7878;
/// The WebSocket endpoint, on the same host as the page. (`web/net.js` uses it too.)
pub const WS_PATH: &str = "/ws";

/// WebSocket close codes the server uses to turn a connection away. Unlike messages, these read
/// the same on every protocol version, so a client on another version still learns why.
pub const CLOSE_OUT_OF_DATE: u16 = 4001;
pub const CLOSE_SERVER_FULL: u16 = 4002;

/// Players per room (one per seat). Seats without a player hold a bot or nobody.
pub const MAX_PLAYERS: usize = N_FIGHTERS;
/// Everyone a room holds: the players in its seats (the first `MAX_PLAYERS` to join), and the rest
/// spectating, next in line for a seat.
pub const MAX_IN_ROOM: usize = 12;
pub const MAX_NAME_LEN: usize = 16;
/// Room codes are this many letters from `CODE_ALPHABET`.
pub const CODE_LEN: usize = 4;
/// No I or O, so codes can't be misread as 1 or 0.
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

/// The server sends a snapshot every this many sim ticks (TICK_RATE / 2 = 30 per second).
pub const SNAPSHOT_EVERY: u32 = 2;

/// The first byte of every encoded `ServerMsg::Snapshot` (its variant number). `web/net.js` reads
/// it to drop stale snapshots piling up in a hidden tab, so it's pinned by a test.
pub const SNAPSHOT_TAG: u8 = 5;

pub type PlayerId = u32;

/// What fills a seat no player is in: a bot, or nobody (fewer fighters; down to just you).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SeatFill {
    #[default]
    Bot,
    Empty,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    /// First message on every connection. Never change it: it's how versions recognise each other.
    Hello { version: u16, name: String },
    /// Opens a new room. On a server with a creation passphrase (the cloud one), only with it.
    CreateRoom { passphrase: String },
    JoinRoom { code: String },
    LeaveRoom,
    SetCharacter(Character),
    /// "Play again" from a non-leader: shown to the leader, who can start regardless.
    SetReady(bool),
    /// Leader only: the map for the next match.
    SetArena(ArenaId),
    /// Leader only: what fills seat `seat` when no player is in it.
    SetSeatFill { seat: u8, fill: SeatFill },
    /// Leader only: who the bot in seat `seat` plays (None: someone at random each match).
    SetSeatBot { seat: u8, character: Option<Character> },
    /// Leader only: start the next match with everyone in the room.
    Start,
    /// One per client tick while playing, numbered so snapshots can say which was last applied.
    Input { seq: u32, action: Action },
    /// Leader only: end the match now (scored as if time ran out).
    EndMatch,
}

go_box_sim::enum_all! {
    /// Why the server refused something. The client shows it in the player's language.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
    pub enum Refusal {
        /// Too many connections right now (sent as `CLOSE_SERVER_FULL`, not a message).
        ServerFull,
        /// The page (client) is a different version from the server: reload. (Sent as
        /// `CLOSE_OUT_OF_DATE`, not a message.)
        OutOfDate,
        TooManyRooms,
        NoSuchRoom,
        RoomFull,
        /// The room closed while you were joining.
        RoomClosed,
        /// Creating a room needs the server's passphrase, and this wasn't it.
        WrongPassphrase,
    }
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
            Refusal::WrongPassphrase => "wrong_passphrase",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    /// `layout`: the server's `arena::layout_fingerprint` (a client with other maps reloads).
    Welcome { you: PlayerId, layout: u64 },
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
    /// In join order: the first `MAX_PLAYERS` play, the rest spectate.
    pub players: Vec<PlayerInfo>,
    pub arena: ArenaId,
    /// Per seat: what fills it when no player is in it. Players take seats in join order.
    pub fills: [SeatFill; MAX_PLAYERS],
    /// Per seat: who a bot there plays (None: someone at random each match).
    pub bots: [Option<Character>; MAX_PLAYERS],
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
    /// Health gained over the match (health pickups, so far).
    pub health_gained: f32,
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

/// One snapshot's view and events, encoded once for everyone in the room. Only `ack` differs
/// between players, and it comes first, so each player's message is the tag, their ack, then
/// these same bytes (rather than the whole view encoded again per player, 30 times a second).
pub struct SnapshotBody(Vec<u8>);

impl SnapshotBody {
    pub fn new(view: &MatchView, events: &[Event]) -> Self {
        // (postcard writes a struct as its fields one after another, so this is the tail of a
        // `Snapshot`; a test holds `with_ack` to `encode` byte for byte.)
        Self(encode(&(view, events)))
    }

    /// The bytes of `ServerMsg::Snapshot(Snapshot { ack, view, events })`.
    pub fn with_ack(&self, ack: u32) -> Vec<u8> {
        let mut buf = [0u8; 5];
        let ack = postcard::to_slice(&ack, &mut buf).expect("a u32 fits in 5 bytes");
        let mut out = Vec::with_capacity(1 + ack.len() + self.0.len());
        out.push(SNAPSHOT_TAG);
        out.extend_from_slice(ack);
        out.extend_from_slice(&self.0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use go_box_sim::sim::Match;

    #[test]
    fn a_shared_snapshot_body_encodes_the_same_as_a_whole_snapshot() {
        for s in sample_snapshots() {
            let body = SnapshotBody::new(&s.view, &s.events);
            for ack in [0, 1, 127, 128, 70_000, u32::MAX] {
                let whole = encode(&ServerMsg::Snapshot(Snapshot { ack, view: s.view.clone(), events: s.events.clone() }));
                assert_eq!(body.with_ack(ack), whole, "ack {ack}");
            }
        }
    }

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

    /// Matches of bots playing every character, with a snapshot taken the first time each
    /// special's leftovers are about (stuck glue, dogs, minis, cats, a table and its sushi,
    /// a chain, Jake in his phone, lightning, a pin, someone thrown, a quake, bugs, a tornado, an arrow in flight, Vid reading, Kyle's inbox), so their layouts are
    /// on the wire too.
    fn sample_snapshots() -> Vec<Snapshot> {
        use go_box_sim::brain::Brain;
        use go_box_sim::character::{Breed, Effect};
        use go_box_sim::sim::ItemKind;
        let brain = Brain::from_json(include_str!("../../../brains/gen_219000.json")).unwrap();
        type Seen = fn(&Match) -> bool;
        let wanted: [(&str, Seen); 17] = [
            ("glue", |m| m.effects.iter().any(|e| matches!(e, Effect::Glue(g) if g.stuck))),
            ("dogs", |m| m.effects.iter().any(|e| matches!(e, Effect::Pet(p) if p.breed == Breed::Hound))),
            ("minis", |m| m.f[N_FIGHTERS..].iter().any(|f| !f.absent)),
            ("cats", |m| m.effects.iter().any(|e| matches!(e, Effect::Pet(p) if matches!(p.breed, Breed::Cat(_))))),
            ("table", |m| m.effects.iter().any(|e| matches!(e, Effect::Table(_)))),
            ("sushi", |m| m.items.iter().any(|it| it.kind == ItemKind::Sushi)),
            ("phone", |m| m.items.iter().any(|it| it.carries.is_some())),
            ("lightning", |m| m.events.iter().any(|e| matches!(e, go_box_sim::sim::Event::Zap { .. }))),
            ("pinned", |m| m.f.iter().any(|f| f.pinned > 0)),
            ("ridden", |m| m.f.iter().any(|f| f.ridden > 0)),
            ("thrown", |m| m.effects.iter().any(|e| matches!(e, Effect::Judo(_)))),
            ("quake", |m| m.events.iter().any(|e| matches!(e, go_box_sim::sim::Event::Quake { .. })) && m.f.iter().any(|f| f.bulked > 0)),
            ("bugs", |m| m.effects.iter().any(|e| matches!(e, Effect::Pet(p) if matches!(p.breed, Breed::Fly(_))))),
            ("tornado", |m| m.effects.iter().any(|e| matches!(e, Effect::Tornado(_)))),
            ("arrow", |m| m.effects.iter().any(|e| matches!(e, Effect::Arrow(a) if a.state != go_box_sim::character::ArrowState::Hover))),
            ("books", |m| m.effects.iter().any(|e| matches!(e, Effect::Book(_))) && m.f.iter().any(|f| f.reading > 0)),
            ("inbox", |m| m.f.iter().any(|f| f.backlog > 0 && f.inbox > 0)),
        ];
        let mut found: Vec<Option<Snapshot>> = vec![None; wanted.len()];
        let casts = [
            [Character::Seth, Character::Sam, Character::Austin, Character::Tyler],
            [Character::Corwin, Character::Justin, Character::Ryan, Character::Jake],
            [Character::Robert, Character::Michelle, Character::Zack, Character::Kyle],
            [Character::Vid, Character::Jake, Character::Shawn, Character::Jake],
            [Character::Doug, Character::Travis, Character::Carrie, Character::Matt],
            [Character::Doug, Character::Classic, Character::Doug, Character::Classic],
            [Character::Kaleb, Character::Classic, Character::Kaleb, Character::Classic],
        ];
        for (seed, cast) in casts.into_iter().enumerate() {
            let mut m = Match::new(5 + seed as u64, &MatchConfig::default());
            for (f, c) in m.f.iter_mut().zip(cast) {
                f.character = c;
            }
            while !m.over {
                m.step([&brain; N_FIGHTERS]);
                for (k, (_, seen)) in wanted.iter().enumerate() {
                    if found[k].is_none() && seen(&m) {
                        found[k] = Some(Snapshot { ack: m.tick, view: m.view(), events: m.events.clone() });
                    }
                }
            }
        }
        let first = Snapshot { ack: 0, view: Match::new(1, &MatchConfig::default()).view(), events: Vec::new() };
        // Rocket launchers are rare, so one is set up by hand: one lying around, a rocket in flight.
        let mut m = Match::new(2, &MatchConfig::default());
        m.items.push(go_box_sim::sim::Item::new(ItemKind::RocketLauncher, glam_vec(200.0, 250.0)));
        m.add_effect(Effect::Rocket(go_box_sim::character::Rocket::new(glam_vec(100.0, 200.0), glam_vec(1.0, 0.0), 0)));
        // And the rarer items and what they leave: a sword, a Molotov and a radio lying about, fire,
        // a jet and one of its bombs.
        for (k, kind) in [ItemKind::Sword, ItemKind::Molotov, ItemKind::AirStrike].into_iter().enumerate() {
            m.items.push(go_box_sim::sim::Item::new(kind, glam_vec(260.0 + 20.0 * k as f32, 250.0)));
        }
        m.add_effect(Effect::Fire(go_box_sim::character::Fire::at(m.arena(), &[], glam_vec(300.0, 240.0), 1)));
        let mut rng = <go_box_sim::sim::SimRng as rand::SeedableRng>::seed_from_u64(4);
        m.add_effect(Effect::Plane(go_box_sim::character::Plane::new(&mut rng, 2)));
        m.add_effect(Effect::AirBomb(go_box_sim::character::AirBomb { pos: glam_vec(150.0, 60.0), vel: glam_vec(40.0, 90.0), owner: 2, ticks: 300 }));
        // (Nobody has the chain now: Ryan only sometimes picks it up. So one is set up by hand.)
        m.add_effect(Effect::Chain(go_box_sim::character::Chain { tip: glam_vec(120.0, 230.0), dir: glam_vec(1.0, 0.0), owner: 0, out: 40.0, hooked: None, back: false, ticks: 60 }));
        m.f[1].character = go_box_sim::character::Character::Ryan;
        m.f[1].hobby = Some(6);
        let rocket = Snapshot { ack: 3, view: m.view(), events: vec![go_box_sim::sim::Event::RocketLaunch { pos: glam_vec(100.0, 200.0) }] };
        std::iter::once(first).chain(std::iter::once(rocket)).chain(found.into_iter().zip(wanted).map(|(s, (name, _))| s.unwrap_or_else(|| panic!("no {name} in the sample matches")))).collect()
    }

    /// One of every message, with fixed contents. The `match`es have no wildcard arm, so a new
    /// message kind doesn't compile until it has a sample here.
    fn samples() -> Vec<Vec<u8>> {
        use go_box_sim::sim::Action;
        let action = Action { mv: -1.0, run: true, jump: true, attack: false, throw: true, aim: 0.3, grab: false, special: true, target: Some(glam_vec(0.6, -0.8)), dash: Some(-1.0), down: true, cursor: Some(glam_vec(240.0, 120.0)) };
        let client = [
            ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Sam".into() },
            ClientMsg::CreateRoom { passphrase: "open sesame".into() },
            ClientMsg::JoinRoom { code: "ABCD".into() },
            ClientMsg::LeaveRoom,
            ClientMsg::SetCharacter(Character::Tyler),
            ClientMsg::SetReady(true),
            ClientMsg::SetArena(ArenaId::Bridges),
            ClientMsg::SetSeatFill { seat: 2, fill: SeatFill::Empty },
            ClientMsg::SetSeatBot { seat: 1, character: Some(Character::Vid) },
            ClientMsg::Start,
            ClientMsg::Input { seq: 77, action },
            ClientMsg::EndMatch,
        ];
        for m in &client {
            match m {
                ClientMsg::Hello { .. }
                | ClientMsg::CreateRoom { .. }
                | ClientMsg::JoinRoom { .. }
                | ClientMsg::LeaveRoom
                | ClientMsg::SetCharacter(_)
                | ClientMsg::SetReady(_)
                | ClientMsg::SetArena(_)
                | ClientMsg::SetSeatFill { .. }
                | ClientMsg::SetSeatBot { .. }
                | ClientMsg::Start
                | ClientMsg::Input { .. }
                | ClientMsg::EndMatch => {}
            }
        }
        let seat = |name: &str, c, player| Seat { name: name.into(), character: c, player };
        let result = SeatResult { name: "Bot".into(), character: Character::Sam, bot: true, kills: 2, headshots: 1, damage_dealt: 88.5, damage_taken: 100.0, died_at: Some(31.5), hp: 0.0, health_gained: 50.0 };
        let room = RoomInfo {
            code: "ABCD".into(),
            leader: 1,
            players: vec![PlayerInfo { id: 1, name: "Sam".into(), character: Character::Austin, ready: true }],
            arena: ArenaId::Bridges,
            fills: [SeatFill::Bot, SeatFill::Bot, SeatFill::Empty, SeatFill::Bot],
            bots: [None, Some(Character::Doug), None, None],
            phase: Phase::Lobby,
            last_results: Some(Results { seats: vec![result], winner: Some(0), duration: 44.5 }),
        };
        let start = MatchStart {
            seats: [seat("Sam", Character::Sam, Some(1)), seat("A", Character::Austin, None), seat("T", Character::Tyler, None), seat("C", Character::Classic, None)],
            your_seat: Some(0),
            config: MatchConfig { arena: ArenaId::Bridges, max_ticks: 1234, ..MatchConfig::default() },
        };
        let mut server = vec![ServerMsg::Welcome { you: 9, layout: 0x1234_5678_9abc_def0 }, ServerMsg::Error(Refusal::NoSuchRoom), ServerMsg::Room(room), ServerMsg::LeftRoom, ServerMsg::MatchStart(start)];
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

    /// The wire format, pinned: the bytes of every sample message, hashed. The samples include a
    /// simulated match, so a change to the game's physics trips it too, which is right: clients
    /// predict their own movement with that physics. If this fails, the format or the game changed: bump PROTOCOL_VERSION (so old clients are turned away instead of
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
    const WIRE: (u16, u64) = (30, 0x12bd_c5db_69d1_dbf2);

    #[test]
    fn every_sample_round_trips() {
        let samples = samples();
        for bytes in &samples[..12] {
            assert_eq!(encode(&decode::<ClientMsg>(bytes).unwrap()), *bytes);
        }
        for bytes in &samples[12..] {
            assert_eq!(encode(&decode::<ServerMsg>(bytes).unwrap()), *bytes);
        }
    }

    #[test]
    fn a_snapshot_starts_with_its_tag() {
        let m = Match::new(1, &MatchConfig::default());
        let bytes = encode(&ServerMsg::Snapshot(Snapshot { ack: 0, view: m.view(), events: Vec::new() }));
        assert_eq!(bytes[0], SNAPSHOT_TAG, "web/net.js relies on this");
        let room = encode(&ServerMsg::LeftRoom);
        assert_ne!(room[0], SNAPSHOT_TAG);
    }

    #[test]
    fn garbage_is_rejected_not_a_panic() {
        assert!(decode::<ClientMsg>(&[0xff, 0xff, 0xff]).is_none());
        assert!(decode::<ClientMsg>(&[]).is_none());
    }
}
