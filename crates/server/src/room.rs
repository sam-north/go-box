//! A room: its players, the lobby choices, and (while playing) the one real match, stepped at
//! 60 ticks/s. Each room is its own task; connections talk to it through `RoomCmd`s.

use crate::Hub;
use crate::conn::Outbox;
use go_box_protocol::{
    ClientMsg, MAX_IN_ROOM, MAX_PLAYERS, MatchStart, Phase, PlayerId, PlayerInfo, Refusal, Results, RoomInfo, SNAPSHOT_EVERY, Seat, SeatFill, SeatResult, ServerMsg,
    Snapshot,
    encode,
};
use go_box_sim::brain::Brain;
use go_box_sim::character::Character;
use go_box_sim::glam::vec2;
use go_box_sim::sim::{Action, ArenaId, DT, Event, H, Match, MatchConfig, N_FIGHTERS, W};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

pub enum RoomCmd {
    Join(Joined),
    Leave { id: PlayerId },
    Msg { id: PlayerId, msg: ClientMsg },
}

pub struct Joined {
    pub id: PlayerId,
    pub name: String,
    pub out: Outbox,
    pub reply: oneshot::Sender<Result<(), Refusal>>,
}

/// Inputs waiting to be applied, per player. A little backlog smooths out network jitter; a
/// big one means added lag, so it's trimmed.
const INPUT_BACKLOG_MAX: usize = 8;
const INPUT_BACKLOG_KEEP: usize = 3;
/// After this many ticks without input, a player's fighter releases all buttons.
const STARVED_RELEASE: u32 = 10;

struct Player {
    id: PlayerId,
    name: String,
    character: Character,
    ready: bool,
    out: Outbox,
    inputs: VecDeque<(u32, Action)>,
    /// Highest seq received / applied.
    last_seq: u32,
    ack: u32,
    last_action: Action,
    /// Ticks in a row with no input (the tab is in the background, the network stalled...).
    starved: u32,
}

impl Player {
    fn send(&self, msg: &ServerMsg) {
        // A client that can't keep up just misses messages (it gets the next snapshot).
        let _ = self.out.try_send(encode(msg));
    }

    /// The input for this tick: the next queued one, or (if none arrived in time) the last
    /// one held, minus one-shot presses so a stall never repeats a throw or special. After a
    /// longer gap (a hidden browser tab sends nothing) the fighter lets go of everything.
    fn next_action(&mut self) -> Action {
        match self.inputs.pop_front() {
            Some((seq, a)) => {
                self.ack = seq;
                self.last_action = a;
                self.starved = 0;
                a
            }
            None => {
                self.starved += 1;
                if self.starved > STARVED_RELEASE { Action::default() } else { Action { throw: false, special: false, dash: false, ..self.last_action } }
            }
        }
    }
}

struct Running {
    m: Match,
    /// Who's in each seat (None = bot). A player who leaves mid-match becomes a bot.
    seats: [Option<PlayerId>; N_FIGHTERS],
    names: [String; N_FIGHTERS],
    /// Events since the last snapshot.
    events: Vec<Event>,
    brain: Arc<Brain>,
}

struct Room {
    code: String,
    players: Vec<Player>,
    leader: PlayerId,
    arena: ArenaId,
    fills: [SeatFill; MAX_PLAYERS],
    last_results: Option<Results>,
    running: Option<Running>,
    rng: SmallRng,
    /// The bots' brain (all a room needs from the server), passed in so rooms can be tested alone.
    brain: Arc<Brain>,
}

impl Room {
    fn new(code: String, brain: Arc<Brain>, seed: u64) -> Self {
        Room {
            code,
            players: Vec::new(),
            leader: 0,
            arena: ArenaId::default(),
            fills: [SeatFill::Bot; MAX_PLAYERS],
            last_results: None,
            running: None,
            rng: SmallRng::seed_from_u64(seed),
            brain,
        }
    }
}

pub async fn run(code: String, mut rx: mpsc::Receiver<RoomCmd>, hub: Arc<Hub>) {
    let mut room = Room::new(code.clone(), hub.brain.clone(), rand::random());
    let tick = Duration::from_secs_f32(DT);
    // Wake often and run however many ticks are due: OS timers (Windows especially) are too
    // coarse to wake exactly every 16.7ms, but the average rate stays exact.
    let mut wake = tokio::time::interval(Duration::from_millis(4));
    wake.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut next_tick = Instant::now();
    loop {
        tokio::select! {
            cmd = rx.recv() => {
                let Some(cmd) = cmd else { break };
                let was_playing = room.running.is_some();
                room.handle(cmd);
                if room.running.is_some() && !was_playing {
                    next_tick = Instant::now();
                }
                if room.players.is_empty() {
                    break;
                }
            }
            _ = wake.tick(), if room.running.is_some() => {
                let now = Instant::now();
                if now.duration_since(next_tick) > Duration::from_millis(250) {
                    // Way behind (the machine stalled): skip ahead rather than fast-forward.
                    next_tick = now;
                }
                while room.running.is_some() && next_tick <= now {
                    room.step();
                    next_tick += tick;
                }
            }
        }
    }
    // Close the inbox before unregistering, so nobody can join a room that's going away.
    rx.close();
    hub.close_room(&code);
}

impl Room {
    fn player(&mut self, id: PlayerId) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    fn phase(&self) -> Phase {
        if self.running.is_some() { Phase::Playing } else { Phase::Lobby }
    }

    fn info(&self) -> RoomInfo {
        RoomInfo {
            code: self.code.clone(),
            leader: self.leader,
            players: self.players.iter().map(|p| PlayerInfo { id: p.id, name: p.name.clone(), character: p.character, ready: p.ready }).collect(),
            arena: self.arena,
            fills: self.fills,
            phase: self.phase(),
            last_results: self.last_results.clone(),
        }
    }

    fn broadcast_room(&self) {
        let msg = ServerMsg::Room(self.info());
        for p in &self.players {
            p.send(&msg);
        }
    }

    fn handle(&mut self, cmd: RoomCmd) {
        match cmd {
            RoomCmd::Join(j) => {
                // Past the seats, newcomers spectate (next in line for a seat), up to a limit.
                if self.players.len() >= MAX_IN_ROOM {
                    let _ = j.reply.send(Err(Refusal::RoomFull));
                    return;
                }
                if self.players.is_empty() {
                    self.leader = j.id;
                }
                let character = Character::ALL[self.rng.gen_range(0..Character::ALL.len())];
                self.players.push(Player {
                    id: j.id,
                    name: j.name,
                    character,
                    ready: false,
                    out: j.out,
                    inputs: VecDeque::new(),
                    last_seq: 0,
                    ack: 0,
                    last_action: Action::default(),
                    starved: 0,
                });
                let _ = j.reply.send(Ok(()));
                tracing::info!(room = %self.code, players = self.players.len(), "player joined");
                self.broadcast_room();
                // Joined mid-match: watch it until the next one.
                if let Some(r) = &self.running {
                    let start = MatchStart { seats: seats_of(r), your_seat: None, config: r.m.config.clone() };
                    self.players.last().unwrap().send(&ServerMsg::MatchStart(start));
                }
            }
            RoomCmd::Leave { id } => {
                let Some(k) = self.players.iter().position(|p| p.id == id) else { return };
                let p = self.players.remove(k);
                p.send(&ServerMsg::LeftRoom);
                if let Some(r) = &mut self.running {
                    // Their fighter plays on as a bot.
                    for s in r.seats.iter_mut().filter(|s| **s == Some(id)) {
                        *s = None;
                    }
                }
                if self.leader == id {
                    // Leadership passes to whoever has been here longest.
                    self.leader = self.players.first().map_or(0, |p| p.id);
                }
                tracing::info!(room = %self.code, players = self.players.len(), "player left");
                self.broadcast_room();
            }
            RoomCmd::Msg { id, msg } => self.on_msg(id, msg),
        }
    }

    fn on_msg(&mut self, id: PlayerId, msg: ClientMsg) {
        let is_leader = id == self.leader;
        let lobby = self.running.is_none();
        match msg {
            ClientMsg::SetCharacter(c) => {
                if let Some(p) = self.player(id) {
                    p.character = c;
                }
                self.broadcast_room();
            }
            ClientMsg::SetReady(ready) => {
                if let Some(p) = self.player(id) {
                    p.ready = ready;
                }
                self.broadcast_room();
            }
            ClientMsg::SetArena(a) if is_leader && lobby => {
                self.arena = a;
                self.broadcast_room();
            }
            ClientMsg::SetSeatFill { seat, fill } if is_leader && lobby => {
                if let Some(f) = self.fills.get_mut(seat as usize) {
                    *f = fill;
                    self.broadcast_room();
                }
            }
            ClientMsg::Start if is_leader && lobby => self.start(),
            ClientMsg::EndMatch if is_leader => {
                if let Some(r) = &mut self.running {
                    r.m.end_now();
                    tracing::info!(room = %self.code, "match ended by the leader");
                }
            }
            ClientMsg::Input { seq, action } => {
                let playing = self.running.is_some();
                let Some(p) = self.player(id) else { return };
                if !playing || seq <= p.last_seq {
                    return;
                }
                p.last_seq = seq;
                p.inputs.push_back((seq, sanitize(action)));
                if p.inputs.len() > INPUT_BACKLOG_MAX {
                    let drop = p.inputs.len() - INPUT_BACKLOG_KEEP;
                    p.inputs.drain(..drop);
                }
            }
            _ => {}
        }
    }

    fn start(&mut self) {
        // Logged with the match: the seed plus the players' inputs replay it exactly.
        let seed: u64 = self.rng.r#gen();
        let mut m = Match::new(seed, &MatchConfig { arena: self.arena, ..MatchConfig::default() });
        let mut seats = [None; N_FIGHTERS];
        let mut names: [String; N_FIGHTERS] = Default::default();
        for i in 0..N_FIGHTERS {
            match self.players.get(i) {
                Some(p) => {
                    seats[i] = Some(p.id);
                    names[i] = p.name.clone();
                    m.f[i].character = p.character;
                }
                None if self.fills[i] == SeatFill::Empty => m.remove(i),
                None => {
                    let c = Character::ALL[self.rng.gen_range(0..Character::ALL.len())];
                    m.f[i].character = c;
                    // Bots go unnamed: clients name them after their character.
                }
            }
        }
        for p in &mut self.players {
            p.ready = false;
            p.inputs.clear();
            p.last_action = Action::default();
        }
        let running = Running { m, seats, names, events: Vec::new(), brain: self.brain.clone() };
        for p in &self.players {
            let your_seat = running.seats.iter().position(|s| *s == Some(p.id));
            p.send(&ServerMsg::MatchStart(MatchStart { seats: seats_of(&running), your_seat, config: running.m.config.clone() }));
        }
        self.running = Some(running);
        tracing::info!(room = %self.code, seed, arena = self.arena.id(), humans = self.players.len(), "match started");
        self.broadcast_room();
        self.send_snapshot();
    }

    fn step(&mut self) {
        let Some(r) = &mut self.running else { return };
        let mut humans = [None; N_FIGHTERS];
        for (s, seat) in r.seats.iter().enumerate() {
            if let Some(p) = seat.and_then(|id| self.players.iter_mut().find(|p| p.id == id)) {
                humans[s] = Some(p.next_action());
            }
        }
        let b = &r.brain;
        r.m.step_humans([b, b, b, b], humans);
        r.events.extend_from_slice(&r.m.events);
        if r.m.over || r.m.tick % SNAPSHOT_EVERY == 0 {
            self.send_snapshot();
        }
        let r = self.running.as_ref().unwrap();
        if r.m.over {
            self.last_results = Some(results(r));
            self.running = None;
            tracing::info!(room = %self.code, "match over");
            self.broadcast_room();
        }
    }

    fn send_snapshot(&mut self) {
        let Some(r) = &mut self.running else { return };
        let view = r.m.view();
        let events = std::mem::take(&mut r.events);
        for p in &self.players {
            p.send(&ServerMsg::Snapshot(Snapshot { ack: p.ack, view: view.clone(), events: events.clone() }));
        }
    }
}

fn seats_of(r: &Running) -> [Seat; N_FIGHTERS] {
    std::array::from_fn(|i| Seat { name: r.names[i].clone(), character: r.m.f[i].character, player: r.seats[i] })
}

fn results(r: &Running) -> Results {
    // Only the seats that were in the match; the winner is re-indexed to match.
    let present: Vec<usize> = (0..N_FIGHTERS).filter(|&i| !r.m.f[i].absent).collect();
    let seats = present
        .iter()
        .map(|&i| {
            let f = &r.m.f[i];
            SeatResult {
                name: r.names[i].clone(),
                character: f.character,
                bot: r.seats[i].is_none(),
                kills: f.kills,
                headshots: f.headshots,
                damage_dealt: f.dmg_dealt,
                damage_taken: f.dmg_taken,
                died_at: f.died_at.map(|t| t as f32 * DT),
                hp: f.hp.max(0.0),
            }
        })
        .collect();
    let winner = r.m.winner.and_then(|w| present.iter().position(|&i| i == w));
    Results { seats, winner, duration: r.m.tick as f32 * DT }
}

/// Inputs come off the network: never let a NaN or an out-of-range value into the sim.
fn sanitize(a: Action) -> Action {
    let clamp = |v: f32| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
    // An aim direction must be a real, non-zero direction; only its direction matters.
    let target = a.target.filter(|t| t.is_finite() && t.length_squared() > 1e-6).map(|t| t.normalize());
    // The cursor: a real point, kept to the arena.
    let cursor = a.cursor.filter(|c| c.is_finite()).map(|c| vec2(c.x.clamp(0.0, W), c.y.clamp(0.0, H)));
    Action { mv: clamp(a.mv), aim: clamp(a.aim), target, cursor, ..a }
}

#[cfg(test)]
mod tests {
    use super::*;
    use go_box_protocol::decode;

    /// A room on its own (no server, sockets or timers), stepped by hand.
    fn room() -> Room {
        Room::new("TEST".into(), Arc::new(crate::bot_brain()), 7)
    }

    /// Player `id` joins; returns what the room sends them.
    fn join(room: &mut Room, id: PlayerId) -> mpsc::Receiver<Vec<u8>> {
        let (out, rx) = mpsc::channel(4096);
        let (reply, _) = oneshot::channel();
        room.handle(RoomCmd::Join(Joined { id, name: format!("P{id}"), out, reply }));
        rx
    }

    /// The last room update in `rx`.
    fn last_room(rx: &mut mpsc::Receiver<Vec<u8>>) -> RoomInfo {
        let mut last = None;
        while let Ok(bytes) = rx.try_recv() {
            if let Some(ServerMsg::Room(r)) = decode(&bytes) {
                last = Some(r);
            }
        }
        last.expect("a room update")
    }

    #[test]
    fn only_the_leader_sets_seats_and_the_match_follows_them() {
        let mut r = room();
        let mut a = join(&mut r, 1);
        let _b = join(&mut r, 2);
        r.on_msg(2, ClientMsg::SetSeatFill { seat: 3, fill: SeatFill::Empty });
        assert_eq!(last_room(&mut a).fills[3], SeatFill::Bot, "not the leader: ignored");
        r.on_msg(1, ClientMsg::SetSeatFill { seat: 3, fill: SeatFill::Empty });
        r.on_msg(1, ClientMsg::SetSeatFill { seat: 9, fill: SeatFill::Empty });
        assert_eq!(last_room(&mut a).fills, [SeatFill::Bot, SeatFill::Bot, SeatFill::Bot, SeatFill::Empty]);
        r.on_msg(1, ClientMsg::Start);
        let run = r.running.as_ref().unwrap();
        assert_eq!(run.seats, [Some(1), Some(2), None, None]);
        assert!(!run.m.f[2].absent && run.m.f[3].absent, "seat 3 is a bot, seat 4 is empty");
        // Settings can't change mid-match.
        r.on_msg(1, ClientMsg::SetSeatFill { seat: 2, fill: SeatFill::Empty });
        assert_eq!(r.fills[2], SeatFill::Bot);
    }

    #[test]
    fn a_leaver_hands_over_the_lead_and_their_fighter_plays_on_as_a_bot() {
        let mut r = room();
        let _a = join(&mut r, 1);
        let mut b = join(&mut r, 2);
        r.on_msg(1, ClientMsg::Start);
        r.handle(RoomCmd::Leave { id: 1 });
        assert_eq!(r.leader, 2);
        assert_eq!(r.running.as_ref().unwrap().seats[0], None, "seat 1 is a bot now");
        assert_eq!(last_room(&mut b).players.len(), 1);
    }

    #[test]
    fn a_match_plays_out_to_results_for_the_seats_in_it() {
        let mut r = room();
        let mut a = join(&mut r, 1);
        r.on_msg(1, ClientMsg::SetSeatFill { seat: 1, fill: SeatFill::Empty });
        r.on_msg(1, ClientMsg::Start);
        let mut ticks = 0;
        while r.running.is_some() {
            r.step();
            ticks += 1;
            assert!(ticks <= go_box_sim::sim::COUNTDOWN + go_box_sim::sim::MAX_TICKS + 1, "the match never ended");
        }
        let res = last_room(&mut a).last_results.expect("results");
        assert_eq!(res.seats.len(), 3, "the empty seat isn't on the scoreboard");
        assert!(res.seats[1].bot && !res.seats[0].bot);
        assert!(res.winner.is_none_or(|w| w < 3));
    }

    #[test]
    fn the_leader_can_end_a_match_early() {
        let mut r = room();
        let mut a = join(&mut r, 1);
        let _b = join(&mut r, 2);
        r.on_msg(1, ClientMsg::Start);
        for _ in 0..10 {
            r.step();
        }
        r.on_msg(2, ClientMsg::EndMatch);
        r.step();
        assert!(r.running.is_some(), "only the leader can end it");
        r.on_msg(1, ClientMsg::EndMatch);
        r.step();
        assert!(r.running.is_none(), "over");
        assert!(last_room(&mut a).last_results.is_some(), "with results");
    }

    #[test]
    fn past_four_players_newcomers_spectate_and_move_up_when_a_seat_frees() {
        let mut r = room();
        let mut rx: Vec<_> = (1..=6).map(|id| join(&mut r, id)).collect();
        assert_eq!(last_room(&mut rx[0]).players.len(), 6, "six in the room");
        r.on_msg(1, ClientMsg::Start);
        let seats = r.running.as_ref().unwrap().seats;
        assert_eq!(seats, [Some(1), Some(2), Some(3), Some(4)], "the first four play");
        let start = |rx: &mut mpsc::Receiver<Vec<u8>>| {
            let mut s = None;
            while let Ok(bytes) = rx.try_recv() {
                if let Some(ServerMsg::MatchStart(m)) = decode(&bytes) {
                    s = Some(m);
                }
            }
            s.expect("a match start")
        };
        assert_eq!(start(&mut rx[4]).your_seat, None, "the fifth spectates");
        r.running.as_mut().unwrap().m.end_now();
        r.step();
        // A seated player leaves: the first spectator plays the next match.
        r.handle(RoomCmd::Leave { id: 2 });
        r.on_msg(1, ClientMsg::Start);
        assert!(r.running.as_ref().unwrap().seats.contains(&Some(5)), "the fifth moved up");
        assert!(!r.running.as_ref().unwrap().seats.contains(&Some(6)));
        // And the room has a limit.
        let _more: Vec<_> = (7..=20).map(|id| join(&mut r, id)).collect();
        assert_eq!(r.players.len(), MAX_IN_ROOM);
    }

    #[test]
    fn inputs_are_ordered_and_a_backlog_is_trimmed() {
        let mut r = room();
        let _a = join(&mut r, 1);
        let input = |seq| ClientMsg::Input { seq, action: Action::default() };
        r.on_msg(1, input(1));
        assert!(r.players[0].inputs.is_empty(), "no inputs before the match");
        r.on_msg(1, ClientMsg::Start);
        for seq in 1..=20 {
            r.on_msg(1, input(seq));
        }
        r.on_msg(1, input(5));
        let p = &r.players[0];
        assert!(p.inputs.len() <= INPUT_BACKLOG_MAX, "backlog of {}", p.inputs.len());
        assert_eq!(p.inputs.back().unwrap().0, 20, "a stale input was dropped, the newest kept");
    }

    #[test]
    fn hostile_inputs_are_tamed() {
        let a = sanitize(Action { mv: f32::NAN, aim: 1e9, ..Action::default() });
        assert_eq!(a.mv, 0.0);
        assert_eq!(a.aim, 1.0);
        let a = sanitize(Action { target: Some(glam::vec2(f32::INFINITY, 0.0)), ..Action::default() });
        assert!(a.target.is_none());
        let a = sanitize(Action { target: Some(glam::vec2(300.0, 400.0)), ..Action::default() });
        assert!((a.target.unwrap().length() - 1.0).abs() < 1e-5);
    }
}
