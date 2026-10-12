//! A room: its players, the lobby choices, and (while playing) the one real match, stepped at
//! 60 ticks/s. Each room is its own task; connections talk to it through `RoomCmd`s.

use crate::Hub;
use crate::conn::Outbox;
use go_box_protocol::{
    ClientMsg, Difficulty, MAX_PLAYERS, MatchStart, Phase, PlayerId, PlayerInfo, Results, RoomInfo, SNAPSHOT_EVERY, Seat, SeatResult, ServerMsg, Snapshot,
    encode,
};
use go_box_sim::brain::Brain;
use go_box_sim::character::Character;
use go_box_sim::sim::{Action, DT, Event, HEALS_PER_ROUND, Match, N_FIGHTERS};
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
    pub reply: oneshot::Sender<Result<(), String>>,
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
                if self.starved > STARVED_RELEASE { Action::default() } else { Action { throw: false, special: false, ..self.last_action } }
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
    brain: Brain,
}

struct Room {
    code: String,
    players: Vec<Player>,
    leader: PlayerId,
    difficulty: Difficulty,
    last_results: Option<Results>,
    running: Option<Running>,
    rng: SmallRng,
    hub: Arc<Hub>,
}

pub async fn run(code: String, mut rx: mpsc::Receiver<RoomCmd>, hub: Arc<Hub>) {
    let mut room = Room {
        code: code.clone(),
        players: Vec::new(),
        leader: 0,
        difficulty: Difficulty::default(),
        last_results: None,
        running: None,
        rng: SmallRng::from_entropy(),
        hub: hub.clone(),
    };
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
            difficulty: self.difficulty,
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
                if self.players.len() >= MAX_PLAYERS {
                    let _ = j.reply.send(Err("That room is full.".into()));
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
                    let start = MatchStart { seats: seats_of(r), your_seat: None };
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
            ClientMsg::SetDifficulty(d) if is_leader && lobby => {
                self.difficulty = d;
                self.broadcast_room();
            }
            ClientMsg::Start if is_leader && lobby => self.start(),
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
        let mut m = Match::new(&mut self.rng, HEALS_PER_ROUND);
        let mut seats = [None; N_FIGHTERS];
        let mut names: [String; N_FIGHTERS] = Default::default();
        for i in 0..N_FIGHTERS {
            match self.players.get(i) {
                Some(p) => {
                    seats[i] = Some(p.id);
                    names[i] = p.name.clone();
                    m.f[i].character = p.character;
                }
                None => {
                    let c = Character::ALL[self.rng.gen_range(0..Character::ALL.len())];
                    m.f[i].character = c;
                    names[i] = format!("{} (bot)", c.name());
                }
            }
        }
        for p in &mut self.players {
            p.ready = false;
            p.inputs.clear();
            p.last_action = Action::default();
        }
        let brain = self.hub.brains.get(self.difficulty).clone();
        let running = Running { m, seats, names, events: Vec::new(), brain };
        for p in &self.players {
            let your_seat = running.seats.iter().position(|s| *s == Some(p.id));
            p.send(&ServerMsg::MatchStart(MatchStart { seats: seats_of(&running), your_seat }));
        }
        self.running = Some(running);
        tracing::info!(room = %self.code, humans = self.players.len(), difficulty = self.difficulty.name(), "match started");
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
    let seats = (0..N_FIGHTERS)
        .map(|i| {
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
    Results { seats, winner: r.m.winner, duration: r.m.tick as f32 * DT }
}

/// Inputs come off the network: never let a NaN or an out-of-range value into the sim.
fn sanitize(a: Action) -> Action {
    let clamp = |v: f32| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
    Action { mv: clamp(a.mv), aim: clamp(a.aim), ..a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_inputs_are_tamed() {
        let a = sanitize(Action { mv: f32::NAN, aim: 1e9, ..Action::default() });
        assert_eq!(a.mv, 0.0);
        assert_eq!(a.aim, 1.0);
    }
}
