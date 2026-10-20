//! Bots fighting behind the menus.

use go_box_sim::brain::Brain;
use go_box_sim::character::Character;
use go_box_sim::sim::{ArenaId, DT, Match, MatchConfig};
use macroquad::prelude::*;
use ::rand::rngs::SmallRng;
use ::rand::{Rng, SeedableRng};

/// Bots fighting behind the menus.
pub struct Attract {
    pub m: Match,
    pub brain: Brain,
    pub rng: SmallRng,
    pub acc: f32,
    pub pause: f32,
    /// A new match began since the arena art was last cleared.
    pub restarted: bool,
}

impl Attract {
    pub fn new() -> Self {
        let brain = Brain::from_json(include_str!("../../../brains/gen_219000.json")).expect("embedded brain");
        let mut rng = SmallRng::seed_from_u64((get_time() * 1e6) as u64 ^ 0x9e37_79b9);
        let m = Self::fresh(&mut rng);
        Self { m, brain, rng, acc: 0.0, pause: 0.0, restarted: false }
    }

    pub fn fresh(rng: &mut SmallRng) -> Match {
        // A different map now and then.
        let arena = ArenaId::ALL[rng.gen_range(0..ArenaId::ALL.len())];
        let mut m = Match::new(rng.r#gen(), &MatchConfig::for_arena(arena));
        for f in m.f.iter_mut() {
            f.character = Character::ALL[rng.gen_range(0..Character::ALL.len())];
        }
        m
    }

    /// Steps in real time; returns the events of the ticks run.
    pub fn frame(&mut self, dt: f32) -> Vec<go_box_sim::sim::Event> {
        let mut events = Vec::new();
        if self.m.over {
            self.pause += dt;
            if self.pause > 2.0 {
                self.m = Self::fresh(&mut self.rng);
                self.pause = 0.0;
                self.restarted = true;
            }
            return events;
        }
        self.acc += dt;
        let b = &self.brain;
        while self.acc >= DT && !self.m.over {
            self.m.step([b, b, b, b]);
            events.extend_from_slice(&self.m.events);
            self.acc -= DT;
        }
        events
    }
}
