//! The committed brains play each other: run it before and after a gameplay change to check the
//! bots still play as before (the trainer that made them is retired, and they can't adapt).
//!
//!     cargo run --release -p go-box-sim --example tournament [matches] [map id]
//!
//! Every brain gets every seat equally often. A brain's share of the wins is its strength:
//! 25% is average in a four-way match.

use go_box_sim::brain::Brain;
use go_box_sim::sim::{ArenaId, Match, MatchConfig, N_FIGHTERS};
use std::path::Path;

fn main() {
    let matches: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(400);
    let arena = std::env::args().nth(2).and_then(|id| ArenaId::ALL.into_iter().find(|a| a.id() == id)).unwrap_or_default();
    let config = MatchConfig::for_arena(arena);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brains");
    let (names, brains): (Vec<String>, Vec<Brain>) = Brain::load_dir(&dir).unwrap().into_iter().unzip();
    let n = brains.len();
    let (mut wins, mut played, mut kills, mut draws, mut ticks) = (vec![0.0f32; n], vec![0u32; n], vec![0u32; n], 0, 0u64);
    for k in 0..matches {
        let mut m = Match::new(k as u64, &config);
        // Rotate who sits where so every brain gets every seat and every opponent.
        let lineup: [usize; N_FIGHTERS] = std::array::from_fn(|s| (k + s * (1 + k / n)) % n);
        while !m.over {
            m.step(lineup.map(|b| &brains[b]));
        }
        ticks += m.tick as u64;
        for (s, &b) in lineup.iter().enumerate() {
            played[b] += 1;
            kills[b] += m.f[s].kills;
        }
        match m.winner {
            Some(w) => wins[lineup[w]] += 1.0,
            None => draws += 1,
        }
    }
    println!("{matches} matches on {}, average {:.1}s, {draws} draws\n", arena.id(), ticks as f32 / matches as f32 / 60.0);
    println!("{:<20} {:>8} {:>10} {:>10}", "brain", "seats", "win share", "kills/match");
    for b in 0..n {
        // Wins per seat played, scaled so that a fair share is 25%.
        let share = wins[b] / played[b] as f32;
        println!("{:<20} {:>8} {:>9.1}% {:>10.2}", names[b], played[b], share * 100.0, kills[b] as f32 / played[b] as f32);
    }
}
