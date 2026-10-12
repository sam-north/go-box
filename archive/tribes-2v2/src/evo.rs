//! Co-evolution of four populations — spear humans, rock throwers, biters, spitters — in
//! 2v2 tribal matches with a genetic algorithm. Each individual is scored by the matches it
//! appears in, alongside teammates and against enemies drawn from the other populations and
//! from a hall of fame of past champions (so old counter-strategies aren't forgotten).

use crate::brain::{gauss, Brain};
use crate::sim::{random_kinds, Kind, Match, Tribe, HEALS_PER_ROUND, N_FIGHTERS};
use std::ops::RangeInclusive;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

pub const POP: usize = 64;
pub const KINDS: usize = 4;
/// Every individual is evaluated in at least this many matches per generation.
const MIN_EVALS: usize = 3;
const HOF_SLOT_CHANCE: f64 = 0.2;
const ELITE: usize = 4;
const HOF_CAP: usize = 40;
const TOURNAMENT: usize = 4;
const CROSSOVER: f32 = 0.6;
const MUT_RATE: f32 = 0.08;
const MUT_STD: f32 = 0.25;
const RESET_RATE: f32 = 0.005;
/// The "get close to an enemy" hint fades to zero over this many generations.
const SHAPING_GENS: f32 = 300.0;
const AUTOSAVE_EVERY: u32 = 25;

/// Hardware budget: use at most half the logical CPUs, minus one left for the viewer window.
pub fn training_threads() -> usize {
    let logical = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    (logical / 2).saturating_sub(1).max(1)
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GenStats {
    pub generation: u32,
    pub best: [f32; KINDS],
    pub avg: [f32; KINDS],
    pub h_wins: u32,
    pub c_wins: u32,
    pub draws: u32,
    /// Total damage dealt to enemies / to allies across the generation's matches.
    #[serde(default)]
    pub enemy_dmg: f32,
    #[serde(default)]
    pub ally_dmg: f32,
    /// Health pickups consumed / spawned across the generation's matches.
    #[serde(default)]
    pub heals: u32,
    #[serde(default)]
    pub heals_spawned: u32,
    /// Wins decided by health at the timeout (the rest were wipe-outs).
    #[serde(default)]
    pub wins_on_health: u32,
}

/// "Lesson" metrics, each `None` for generations recorded before it was tracked.
impl GenStats {
    /// Damage to allies as a fraction of damage to enemies.
    pub fn friendly_fire(&self) -> Option<f32> {
        (self.enemy_dmg > 0.0).then(|| self.ally_dmg / self.enemy_dmg)
    }
    /// Fraction of spawned health pickups that somebody grabbed.
    pub fn heal_pickup_rate(&self) -> Option<f32> {
        (self.heals_spawned > 0).then(|| self.heals as f32 / self.heals_spawned as f32)
    }
    /// Fraction of wins that came from wiping out the other tribe rather than health at the timeout.
    pub fn wipeout_rate(&self) -> Option<f32> {
        let wins = self.h_wins + self.c_wins;
        (self.heals_spawned > 0 && wins > 0).then(|| (wins - self.wins_on_health.min(wins)) as f32 / wins as f32)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Trainer {
    pub generation: u32,
    /// Indexed by `Kind::index()`.
    pub pops: Vec<Vec<Brain>>,
    pub hofs: Vec<Vec<Brain>>,
    pub history: Vec<GenStats>,
    seed: u64,
    /// Generation at which health pickups were introduced; the heal curriculum counts from here.
    #[serde(default)]
    heal_curriculum_start: Option<u32>,
}

/// Heal curriculum: pickups start plentiful so brains discover them, then fade over this many
/// generations to the final rules (`sim::HEALS_PER_ROUND`).
const HEAL_CURRICULUM_GENS: f32 = 300.0;
const EARLY_HEALS: (usize, usize) = (5, 6);

/// Heals per round at a given curriculum progress (0 = start, 1 = final rules).
pub fn heals_at(progress: f32) -> RangeInclusive<usize> {
    let lerp = |a: usize, b: usize| (a as f32 + (b as f32 - a as f32) * progress).round() as usize;
    lerp(EARLY_HEALS.0, *HEALS_PER_ROUND.start())..=lerp(EARLY_HEALS.1, *HEALS_PER_ROUND.end())
}

/// What the trainer keeps from each match.
struct MatchResult {
    fitness: [f32; N_FIGHTERS],
    winner: Option<Tribe>,
    on_health: bool,
    enemy_dmg: f32,
    ally_dmg: f32,
    heals_taken: u32,
    heals_spawned: u32,
}

/// The original 1v1 save format (spear humans vs biters).
#[derive(Deserialize)]
struct TrainerV1 {
    generation: u32,
    humans: Vec<Brain>,
    creatures: Vec<Brain>,
    hof_h: Vec<Brain>,
    hof_c: Vec<Brain>,
}

#[derive(Clone, Copy)]
enum Slot {
    Pop(usize),
    Hof(usize),
}

/// `save/state.json` in the project, unless overridden with the BOX_ARENA_SAVE env var.
pub fn save_path() -> PathBuf {
    std::env::var_os("BOX_ARENA_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("save").join("state.json"))
}

impl Trainer {
    pub fn new() -> Self {
        let mut rng = SmallRng::from_entropy();
        Self {
            generation: 0,
            pops: (0..KINDS).map(|_| (0..POP).map(|_| Brain::random(&mut rng)).collect()).collect(),
            hofs: vec![Vec::new(); KINDS],
            history: Vec::new(),
            seed: rng.r#gen(),
            heal_curriculum_start: Some(0),
        }
    }

    /// Carries 1v1 progress forward: spear humans and biters keep their brains (upgraded to the
    /// new senses); throwers and spitters start as mutated copies of their tribe's population.
    fn from_v1(v1: TrainerV1) -> Self {
        let mut rng = SmallRng::from_entropy();
        let up = |bs: &[Brain], rng: &mut SmallRng| bs.iter().map(|b| b.upgrade(rng)).collect::<Vec<_>>();
        let spear = up(&v1.humans, &mut rng);
        let biter = up(&v1.creatures, &mut rng);
        let mut thrower = up(&v1.humans, &mut rng);
        let mut spitter = up(&v1.creatures, &mut rng);
        for b in thrower.iter_mut().chain(spitter.iter_mut()) {
            mutate(b, &mut rng);
        }
        Self {
            generation: v1.generation,
            pops: vec![spear, thrower, biter, spitter],
            hofs: vec![up(&v1.hof_h, &mut rng), Vec::new(), up(&v1.hof_c, &mut rng), Vec::new()],
            history: Vec::new(),
            seed: rng.r#gen(),
            heal_curriculum_start: Some(v1.generation),
        }
    }

    /// Copies the save aside before an upgrade rewrites it in the new format.
    fn backup_before_upgrade(generation: u32) {
        let path = save_path();
        let backup = path.with_file_name(format!("state_pre_upgrade_gen{generation}.json"));
        if std::fs::copy(&path, &backup).is_ok() {
            println!("backed up old save to {}", backup.display());
        }
    }

    pub fn load_or_new(fresh: bool) -> Self {
        if fresh {
            return Self::new();
        }
        let Ok(text) = std::fs::read_to_string(save_path()) else {
            return Self::new();
        };
        if let Ok(mut t) = serde_json::from_str::<Trainer>(&text) {
            // Saves from before health pickups start the heal curriculum now.
            t.heal_curriculum_start.get_or_insert(t.generation);
            let brains = || t.pops.iter().chain(t.hofs.iter()).flatten();
            if t.pops.len() == KINDS && brains().all(|b| b.is_current()) {
                println!("resumed from {} at generation {}", save_path().display(), t.generation);
                return t;
            }
            if t.pops.len() == KINDS && brains().all(|b| b.is_current() || b.is_legacy()) {
                Self::backup_before_upgrade(t.generation);
                let mut rng = SmallRng::from_entropy();
                for b in t.pops.iter_mut().chain(t.hofs.iter_mut()).flatten() {
                    *b = b.upgrade(&mut rng);
                }
                println!("upgraded save at generation {} to the current brain layout", t.generation);
                return t;
            }
        }
        if let Ok(v1) = serde_json::from_str::<TrainerV1>(&text) {
            if v1.humans.first().is_some_and(|b| b.is_legacy()) {
                Self::backup_before_upgrade(v1.generation);
                println!("migrated 1v1 save at generation {} to 2v2 tribes", v1.generation);
                return Self::from_v1(v1);
            }
        }
        println!("save file incompatible, starting fresh");
        Self::new()
    }

    pub fn save(&self) {
        let path = save_path();
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let tmp = path.with_extension("json.tmp");
        if let Ok(text) = serde_json::to_string(self) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    /// How far through the heal curriculum training is (0 = plentiful heals, 1 = final rules).
    pub fn curriculum_progress(&self) -> f32 {
        let start = self.heal_curriculum_start.unwrap_or(self.generation);
        (self.generation.saturating_sub(start) as f32 / HEAL_CURRICULUM_GENS).min(1.0)
    }

    /// Latest champion of each kind (falls back to the population's first member).
    pub fn champions(&self) -> Vec<Brain> {
        (0..KINDS).map(|k| self.hofs[k].last().unwrap_or(&self.pops[k][0]).clone()).collect()
    }

    /// Runs one generation: evaluate everyone, record champions, breed the next populations.
    pub fn step(&mut self) -> GenStats {
        let mut rng = SmallRng::seed_from_u64(self.seed);
        self.seed = rng.r#gen();

        // Build 2v2 matches until every individual of every kind has played MIN_EVALS times.
        // Individuals are drawn from shuffled queues so coverage stays even.
        let mut queues: Vec<Vec<usize>> = vec![Vec::new(); KINDS];
        let mut drawn = [0usize; KINDS];
        let mut jobs: Vec<([Kind; N_FIGHTERS], [Slot; N_FIGHTERS], u64)> = Vec::new();
        while drawn.iter().any(|&d| d < MIN_EVALS * POP) {
            let kinds = random_kinds(&mut rng);
            let slots = kinds.map(|kind| {
                let k = kind.index();
                if !self.hofs[k].is_empty() && rng.gen_bool(HOF_SLOT_CHANCE) {
                    return Slot::Hof(rng.gen_range(0..self.hofs[k].len()));
                }
                if queues[k].is_empty() {
                    queues[k] = (0..POP).collect();
                    queues[k].shuffle(&mut rng);
                }
                drawn[k] += 1;
                Slot::Pop(queues[k].pop().unwrap())
            });
            jobs.push((kinds, slots, rng.r#gen()));
        }

        let shaping = (1.0 - self.generation as f32 / SHAPING_GENS).max(0.0);
        let heals = heals_at(self.curriculum_progress());
        let results: Vec<MatchResult> = jobs
            .par_iter()
            .map(|(kinds, slots, seed)| {
                let brains: [&Brain; N_FIGHTERS] = std::array::from_fn(|s| {
                    let k = kinds[s].index();
                    match slots[s] {
                        Slot::Pop(i) => &self.pops[k][i],
                        Slot::Hof(i) => &self.hofs[k][i],
                    }
                });
                let mut r = SmallRng::seed_from_u64(*seed);
                let mut m = Match::new(&mut r, *kinds, heals.clone());
                while !m.over {
                    m.step(brains);
                }
                MatchResult {
                    fitness: std::array::from_fn(|s| m.fitness(s, shaping)),
                    winner: m.winner,
                    on_health: m.winner.is_some() && m.timed_out(),
                    enemy_dmg: m.f.iter().map(|f| f.dmg_enemy).sum(),
                    ally_dmg: m.f.iter().map(|f| f.dmg_ally).sum(),
                    heals_taken: m.f.iter().map(|f| f.heals_taken).sum(),
                    heals_spawned: m.heals_spawned,
                }
            })
            .collect();

        let mut sum = vec![vec![0.0f32; POP]; KINDS];
        let mut cnt = vec![vec![0u32; POP]; KINDS];
        let mut stats = GenStats { generation: self.generation, ..Default::default() };
        for ((kinds, slots, _), r) in jobs.iter().zip(results.iter()) {
            stats.enemy_dmg += r.enemy_dmg;
            stats.ally_dmg += r.ally_dmg;
            stats.heals += r.heals_taken;
            stats.heals_spawned += r.heals_spawned;
            stats.wins_on_health += r.on_health as u32;
            for s in 0..N_FIGHTERS {
                if let Slot::Pop(i) = slots[s] {
                    sum[kinds[s].index()][i] += r.fitness[s];
                    cnt[kinds[s].index()][i] += 1;
                }
            }
            match r.winner {
                Some(Tribe::Humans) => stats.h_wins += 1,
                Some(Tribe::Creatures) => stats.c_wins += 1,
                None => stats.draws += 1,
            }
        }

        for k in 0..KINDS {
            let fit: Vec<f32> = (0..POP).map(|i| sum[k][i] / cnt[k][i].max(1) as f32).collect();
            let best = (0..POP).max_by(|&a, &b| fit[a].total_cmp(&fit[b])).unwrap();
            stats.best[k] = fit[best];
            stats.avg[k] = fit.iter().sum::<f32>() / POP as f32;
            self.hofs[k].push(self.pops[k][best].clone());
            if self.hofs[k].len() > HOF_CAP {
                self.hofs[k].remove(0);
            }
            self.pops[k] = breed(&self.pops[k], &fit, &mut rng);
        }
        self.generation += 1;
        self.history.push(stats.clone());
        stats
    }
}

fn mutate(b: &mut Brain, rng: &mut SmallRng) {
    for w in b.w.iter_mut() {
        let r: f32 = rng.r#gen();
        if r < RESET_RATE {
            *w = gauss(rng);
        } else if r < MUT_RATE {
            *w += gauss(rng) * MUT_STD;
        }
    }
}

fn breed(pop: &[Brain], fit: &[f32], rng: &mut SmallRng) -> Vec<Brain> {
    let mut order: Vec<usize> = (0..pop.len()).collect();
    order.sort_by(|&a, &b| fit[b].total_cmp(&fit[a]));
    let mut out: Vec<Brain> = order[..ELITE].iter().map(|&i| pop[i].clone()).collect();
    let tourney = |rng: &mut SmallRng| {
        (0..TOURNAMENT).map(|_| rng.gen_range(0..pop.len())).max_by(|&a, &b| fit[a].total_cmp(&fit[b])).unwrap()
    };
    while out.len() < pop.len() {
        let a = tourney(rng);
        let mut child = pop[a].clone();
        if rng.r#gen::<f32>() < CROSSOVER {
            let b = &pop[tourney(rng)];
            for (w, &wb) in child.w.iter_mut().zip(b.w.iter()) {
                if rng.gen_bool(0.5) {
                    *w = wb;
                }
            }
        }
        mutate(&mut child, rng);
        out.push(child);
    }
    out
}

/// What the viewer sees of the training thread.
pub struct Shared {
    pub generation: u32,
    pub champs: Vec<Brain>,
    pub history: Vec<GenStats>,
    pub gens_per_sec: f32,
    pub stop: bool,
    pub save_now: bool,
    pub last_save_gen: u32,
    /// Heal curriculum progress (0..1); the viewer plays by the same rules as training.
    pub curriculum: f32,
}

fn build_pool() -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(training_threads()).build().expect("thread pool")
}

pub fn spawn(fresh: bool) -> (Arc<Mutex<Shared>>, JoinHandle<()>) {
    let mut t = Trainer::load_or_new(fresh);
    let shared = Arc::new(Mutex::new(Shared {
        generation: t.generation,
        champs: t.champions(),
        history: t.history.clone(),
        gens_per_sec: 0.0,
        stop: false,
        save_now: false,
        last_save_gen: t.generation,
        curriculum: t.curriculum_progress(),
    }));
    let s2 = shared.clone();
    let handle = std::thread::spawn(move || {
        let pool = build_pool();
        let mut rate = 0.0f32;
        loop {
            let (stop, save_now) = {
                let mut s = s2.lock().unwrap();
                let r = (s.stop, s.save_now);
                s.save_now = false;
                r
            };
            if stop {
                t.save();
                break;
            }
            let start = Instant::now();
            let stats = pool.install(|| t.step());
            let gps = 1.0 / start.elapsed().as_secs_f32().max(1e-4);
            rate = if rate == 0.0 { gps } else { rate * 0.9 + gps * 0.1 };
            let saved = save_now || t.generation % AUTOSAVE_EVERY == 0;
            if saved {
                t.save();
            }
            let mut s = s2.lock().unwrap();
            s.generation = t.generation;
            s.champs = t.champions();
            s.curriculum = t.curriculum_progress();
            s.history.push(stats);
            s.gens_per_sec = rate;
            if saved {
                s.last_save_gen = t.generation;
            }
        }
    });
    (shared, handle)
}

/// Train without a window, printing progress. Same hardware budget as the viewer.
pub fn headless(gens: u32, fresh: bool) {
    let mut t = Trainer::load_or_new(fresh);
    let pool = build_pool();
    println!("training {gens} generations on {} threads", training_threads());
    for _ in 0..gens {
        let start = Instant::now();
        let s = pool.install(|| t.step());
        let avg: Vec<String> = Kind::ALL.iter().map(|k| format!("{} {:6.1}", k.name(), s.avg[k.index()])).collect();
        println!(
            "gen {:5} | avg {} | humans {:3} creatures {:3} draws {:3} | wipeouts {:3.0}% | friendly fire {:3.0}% | heals {:4}/{:4} | {:.2}s",
            s.generation, avg.join("  "), s.h_wins, s.c_wins, s.draws,
            s.wipeout_rate().unwrap_or(0.0) * 100.0, s.friendly_fire().unwrap_or(0.0) * 100.0,
            s.heals, s.heals_spawned, start.elapsed().as_secs_f32()
        );
        if t.generation % AUTOSAVE_EVERY == 0 {
            t.save();
        }
    }
    t.save();
}
