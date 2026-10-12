//! What the server and clients say to each other over the WebSocket: one postcard-encoded
//! message per binary frame.
//!
//! The server runs the only real match; clients send inputs and draw the snapshots they get
//! back (predicting their own fighter's movement in between).

use go_box_sim::character::Character;
use go_box_sim::sim::{Action, Event, MatchView, N_FIGHTERS};
use serde::{Deserialize, Serialize};

/// Bumped on any incompatible change; the server turns away clients on another version.
pub const PROTOCOL_VERSION: u16 = 3;

/// Players per room (one per seat). Empty seats are bots.
pub const MAX_PLAYERS: usize = N_FIGHTERS;
pub const MAX_NAME_LEN: usize = 16;
/// Room codes are this many letters from `CODE_ALPHABET`.
pub const CODE_LEN: usize = 4;
/// No I or O, so codes can't be misread as 1 or 0.
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

/// The server sends a snapshot every this many ticks (60 / 2 = 30 per second).
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
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }
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
    /// Leader only: start the next match with everyone in the room.
    Start,
    /// One per client tick while playing, numbered so snapshots can say which was last applied.
    Input { seq: u32, action: Action },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome { you: PlayerId },
    /// Something was refused (bad code, room full, version mismatch...). Human-readable.
    Error(String),
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
    pub phase: Phase,
    pub last_results: Option<Results>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Seat {
    pub name: String,
    pub character: Character,
    /// None = a bot.
    pub player: Option<PlayerId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchStart {
    pub seats: [Seat; N_FIGHTERS],
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
    use go_box_sim::sim::{HEALS_PER_ROUND, Match};
    use rand::SeedableRng;

    #[test]
    fn snapshot_round_trips_and_is_small() {
        let m = Match::new(&mut rand::rngs::SmallRng::seed_from_u64(1), HEALS_PER_ROUND);
        let msg = ServerMsg::Snapshot(Snapshot { ack: 7, view: m.view(), events: Vec::new() });
        let bytes = encode(&msg);
        assert!(bytes.len() < 1500, "snapshot is {} bytes", bytes.len());
        let Some(ServerMsg::Snapshot(s)) = decode::<ServerMsg>(&bytes) else { panic!() };
        assert_eq!(s.ack, 7);
        assert_eq!(s.view.f[2].pos, m.f[2].pos);
    }

    #[test]
    fn garbage_is_rejected_not_a_panic() {
        assert!(decode::<ClientMsg>(&[0xff, 0xff, 0xff]).is_none());
        assert!(decode::<ClientMsg>(&[]).is_none());
    }
}
