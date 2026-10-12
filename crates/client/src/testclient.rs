//! The native test client's command line and automation: options, the autopilot, room-code
//! files, screenshots. The browser build uses the defaults.

use crate::app::App;
use go_box_protocol::*;
use go_box_sim::character::Character;
use go_box_sim::sim::{Action, Match, N_FIGHTERS};
use macroquad::prelude::*;
use ::rand::rngs::SmallRng;
use ::rand::Rng;
use std::sync::OnceLock;

/// Command-line options (native test client only; the browser uses the defaults).
#[derive(Default)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub struct Options {
    pub server: String,
    pub name: Option<String>,
    /// The room-creation passphrase, if the server wants one.
    pub passphrase: Option<String>,
    pub join: Option<String>,
    pub create: bool,
    pub character: Option<Character>,
    /// As leader, pick this map (by id).
    pub arena: Option<go_box_sim::sim::ArenaId>,
    /// As leader, start once this many players are in the room.
    pub start_when: Option<usize>,
    /// Play with simple scripted inputs (for automated tests).
    pub autopilot: bool,
    /// Write the room code here once in a room (so another test client can join it).
    pub code_file: Option<String>,
    pub shots: Option<String>,
    pub shot_every: f32,
    /// Only take a shot while the match has this kind of effect in it (bomb, pet, table, chain),
    /// e.g. to catch a special in action.
    pub shot_when: Option<String>,
    pub quit_after: Option<f32>,
    /// Open the settings menu at startup (for screenshots).
    pub settings: bool,
    /// ...on its Controls page.
    pub controls: bool,
    /// Start with sound off.
    pub mute: bool,
}

static OPTIONS: OnceLock<Options> = OnceLock::new();

impl Options {
    pub fn get() -> &'static Options {
        OPTIONS.get_or_init(Options::parse)
    }

    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let val = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
        let has = |k: &str| args.iter().any(|a| a == k);
        Options {
            server: val("--server").unwrap_or_else(|| format!("ws://127.0.0.1:{DEFAULT_PORT}{WS_PATH}")),
            name: val("--name"),
            passphrase: val("--passphrase"),
            join: val("--join"),
            create: has("--create"),
            character: val("--character").and_then(|c| Character::ALL.into_iter().find(|x| x.id().eq_ignore_ascii_case(&c))),
            arena: val("--arena").and_then(|a| go_box_sim::sim::ArenaId::ALL.into_iter().find(|x| x.id() == a)),
            start_when: val("--start-when").and_then(|v| v.parse().ok()),
            autopilot: has("--autopilot"),
            code_file: val("--code-file"),
            shots: val("--shots"),
            shot_every: val("--shot-every").and_then(|v| v.parse().ok()).unwrap_or(3.0),
            shot_when: val("--shot-when"),
            quit_after: val("--quit-after").and_then(|v| v.parse().ok()),
            mute: has("--mute"),
            settings: has("--settings"),
            controls: has("--controls"),
        }
    }
}

/// What the test client's autopilot does: chase the nearest opponent, swing, throw, use the
/// special now and then.
pub fn autopilot(m: &Match, seat: usize, rng: &mut SmallRng) -> Action {
    let me = &m.f[seat];
    let foe = (0..N_FIGHTERS).filter(|&j| j != seat && m.f[j].alive()).min_by(|&a, &b| me.pos.distance(m.f[a].pos).total_cmp(&me.pos.distance(m.f[b].pos)));
    let dx = foe.map_or(0.0, |j| m.f[j].pos.x - me.pos.x);
    Action {
        mv: if dx.abs() < 6.0 { 0.0 } else { dx.signum() },
        run: true,
        jump: rng.gen_bool(0.02),
        attack: dx.abs() < 14.0,
        throw: me.holding.is_some() && rng.gen_bool(0.03),
        aim: 0.3,
        grab: me.holding.is_none(),
        special: rng.gen_bool(0.01),
        target: None,
    }
}

/// What the test client does by itself each frame: write the room code to `--code-file`, start
/// the match as leader once `--start-when` players are in, take `--shots` screenshots, and quit
/// after `--quit-after` seconds.
pub struct Automation {
    code_written: bool,
    next_shot: f32,
    shot_n: u32,
}

impl Automation {
    pub fn new() -> Self {
        Self { code_written: false, next_shot: Options::get().shot_every, shot_n: 0 }
    }

    /// True when it's time to quit.
    pub fn step(&mut self, app: &mut App, shown: Option<&Match>) -> bool {
        let opts = Options::get();
        if let Some(r) = &app.room {
            if let Some(path) = &opts.code_file
                && !self.code_written
            {
                let _ = std::fs::write(path, &r.code);
                self.code_written = true;
            }
            let leader = app.me == Some(r.leader);
            if leader && r.phase == Phase::Lobby && r.last_results.is_none() && opts.start_when.is_some_and(|n| r.players.len() >= n) {
                app.send(ClientMsg::Start);
            }
        }
        let wanted = opts.shot_when.as_deref().is_none_or(|kind| shown.is_some_and(|m| m.effects.iter().any(|e| effect_kind(e) == kind)));
        if let Some(prefix) = &opts.shots
            && get_time() as f32 >= self.next_shot
            && wanted
        {
            self.shot_n += 1;
            get_screen_data().export_png(&format!("{prefix}_{:02}.png", self.shot_n));
            self.next_shot += opts.shot_every;
        }
        opts.quit_after.is_some_and(|q| get_time() as f32 >= q)
    }
}

/// What `--shot-when` calls an effect.
fn effect_kind(e: &go_box_sim::character::Effect) -> &'static str {
    use go_box_sim::character::Effect;
    match e {
        Effect::Bomb(_) => "bomb",
        Effect::Pet(_) => "pet",
        Effect::Table(_) => "table",
        Effect::Chain(_) => "chain",
    }
}
