//! Evolution of one population of humans in four-way free-for-all matches, with a genetic
//! algorithm. Each individual is scored by the matches it appears in, against other members of
//! the population and a hall of fame of past champions (so old counter-strategies aren't forgotten).

use crate::brain::{gauss, Brain};
use crate::sim::{Match, Source, HEALS_PER_ROUND, N_FIGHTERS};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Write as _;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const POP: usize = 96;
/// Each generation, every individual plays the same SCENARIOS: a fixed spawn seed and seat, against
/// the same three opponents (last generation's best, a recent champion, and a champion from the
/// whole archive). Shared scenarios make scores comparable (skill, not luck of the draw), and
/// archive opponents stop the population drifting in circles and forgetting old counters.
const SCENARIOS: usize = 6;

/// Benchmark: every BENCH_EVERY generations the current top 4 play BENCH_MATCHES fixed-seed
/// matches against three of the PANEL_GENS champions. 25% = as good as the panel. Panel
/// champions are held out of training so the benchmark can't be memorised.
pub const BENCH_EVERY: u32 = 250;
const BENCH_MATCHES: u64 = 600;
const PANEL_GENS: [u32; 5] = [31_000, 50_000, 100_000, 150_000, 184_000];

/// Population size and matches per individual, overridable with GO_BOX_POP and
/// GO_BOX_EVALS (used to compare training settings).
fn ga_settings() -> (usize, usize) {
    let var = |name: &str, default: usize| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
    (var("GO_BOX_POP", POP).max(8), var("GO_BOX_EVALS", SCENARIOS).max(1))
}
const ELITE: usize = 6;
const HOF_CAP: usize = 50;
const TOURNAMENT: usize = 4;
const CROSSOVER: f32 = 0.6;
const MUT_RATE: f32 = 0.08;
const MUT_STD: f32 = 0.25;
const RESET_RATE: f32 = 0.005;
/// The "get close to someone" hint fades to zero over this many generations.
const SHAPING_GENS: f32 = 300.0;
/// Heal curriculum: pickups start plentiful so brains discover them, then fade over this many
/// generations to the final rules (`sim::HEALS_PER_ROUND`).
const HEAL_CURRICULUM_GENS: f32 = 300.0;
const EARLY_HEALS: (usize, usize) = (5, 6);
/// Save the full state at least this often, plus on exit.
const AUTOSAVE_EVERY: Duration = Duration::from_secs(180);
/// Keep a copy of the champion every this many generations, so old generations can be replayed.
pub const ARCHIVE_EVERY: u32 = 1000;

/// Hardware budget: use at most half the logical CPUs, minus one left for the viewer window.
pub fn training_threads() -> usize {
    let logical = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    (logical / 2).saturating_sub(1).max(1)
}

/// Heals per round at a given curriculum progress (0 = start, 1 = final rules).
pub fn heals_at(progress: f32) -> RangeInclusive<usize> {
    let lerp = |a: usize, b: usize| (a as f32 + (b as f32 - a as f32) * progress).round() as usize;
    lerp(EARLY_HEALS.0, *HEALS_PER_ROUND.start())..=lerp(EARLY_HEALS.1, *HEALS_PER_ROUND.end())
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct GenStats {
    pub generation: u32,
    pub best: f32,
    pub avg: f32,
    pub matches: u32,
    pub draws: u32,
    /// Wins decided by health at the timeout (the rest were last-one-standing).
    pub wins_on_health: u32,
    /// Damage dealt to others, split by `Source`.
    pub dmg_by_source: [f32; 5],
    pub self_dmg: f32,
    pub hits: u32,
    pub headshots: u32,
    pub heals: u32,
    pub heals_spawned: u32,
}

/// "Lesson" metrics, each `None` when there's nothing to measure yet.
impl GenStats {
    pub fn total_dmg(&self) -> f32 {
        self.dmg_by_source.iter().sum()
    }
    /// Share of all damage dealt that came from `source`.
    pub fn share(&self, source: Source) -> Option<f32> {
        let total = self.total_dmg();
        (total > 0.0).then(|| self.dmg_by_source[source.index()] / total)
    }
    pub fn headshot_rate(&self) -> Option<f32> {
        (self.hits > 0).then(|| self.headshots as f32 / self.hits as f32)
    }
    pub fn heal_pickup_rate(&self) -> Option<f32> {
        (self.heals_spawned > 0).then(|| self.heals as f32 / self.heals_spawned as f32)
    }
    /// Fraction of decided matches won by being the last one standing.
    pub fn knockout_rate(&self) -> Option<f32> {
        let wins = self.matches - self.draws;
        (wins > 0).then(|| (wins - self.wins_on_health.min(wins)) as f32 / wins as f32)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Trainer {
    pub generation: u32,
    pub pop: Vec<Brain>,
    pub hof: Vec<Brain>,
    pub history: Vec<GenStats>,
    /// The best few of the last generation, for the viewer.
    pub top: Vec<Brain>,
    seed: u64,
    /// Benchmark results: (generation, win rate against the panel).
    #[serde(default)]
    pub bench: Vec<(u32, f32)>,
    /// Archived champions used as training opponents (panel excluded), and the benchmark panel.
    /// Both are read from `archive_dir()`, not stored in the save.
    #[serde(skip)]
    archive: Vec<Brain>,
    #[serde(skip)]
    panel: Vec<Brain>,
    /// Evolution-strategies state, when training with GO_BOX_ALGO=es.
    #[serde(default)]
    es: Option<EsState>,
}

/// Evolution strategies (OpenAI-ES): one central brain. Each generation, mirrored pairs of random
/// perturbations of it (centre + sigma*e, centre - sigma*e) play the shared scenarios, and the
/// centre moves along the rank-weighted average of the perturbations (with Adam). Unlike the GA,
/// every result counts, including the bad ones, so it keeps making small, steady improvements.
#[derive(Serialize, Deserialize, Clone)]
pub struct EsState {
    center: Brain,
    m: Vec<f32>,
    v: Vec<f32>,
    t: u32,
}
const ES_SIGMA: f32 = 0.04;
const ES_LR: f32 = 0.015;
const ES_DECAY: f32 = 0.0005;

/// Training method: the genetic algorithm (default) or evolution strategies (GO_BOX_ALGO=es).
pub fn use_es() -> bool {
    std::env::var("GO_BOX_ALGO").is_ok_and(|v| v.eq_ignore_ascii_case("es"))
}

#[derive(Clone, Copy)]
enum Slot {
    Pop(usize),
    Hof(usize),
    Archive(usize),
    Top(usize),
}

/// What the trainer keeps from each match.
struct MatchResult {
    fitness: [f32; N_FIGHTERS],
    winner: Option<usize>,
    on_health: bool,
    dmg_by_source: [f32; 5],
    self_dmg: f32,
    hits: u32,
    headshots: u32,
    heals_taken: u32,
    heals_spawned: u32,
}

/// `save/ffa.json` in the project, unless overridden with the GO_BOX_SAVE env var.
pub fn save_path() -> PathBuf {
    std::env::var_os("GO_BOX_SAVE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../save").join("ffa.json"))
}

/// Per-generation stats log next to the save (CSV, appended every generation).
pub fn log_path() -> PathBuf {
    save_path().with_extension("csv")
}

impl Trainer {
    pub fn new() -> Self {
        let mut rng = SmallRng::from_entropy();
        let pop: Vec<Brain> = (0..ga_settings().0).map(|_| Brain::random(&mut rng)).collect();
        Self { generation: 0, top: pop[..N_FIGHTERS].to_vec(), pop, hof: Vec::new(), history: Vec::new(), seed: rng.r#gen(), bench: Vec::new(), archive: Vec::new(), panel: Vec::new(), es: None }
    }

    pub fn load_or_new(fresh: bool) -> Self {
        if !fresh {
            if let Ok(text) = std::fs::read_to_string(save_path()) {
                match serde_json::from_str::<Trainer>(&text) {
                    Ok(t) if t.pop.iter().chain(&t.hof).all(Brain::is_current) => {
                        println!("resumed from {} at generation {}", save_path().display(), t.generation);
                        let mut t = t;
                        t.resize_population(ga_settings().0);
                        if archived_champions().is_empty() && t.generation > 0 {
                            archive_champion(t.generation, &t.top[0]);
                        }
                        t.load_opponents();
                        return t;
                    }
                    _ => println!("save file incompatible, starting fresh"),
                }
            }
        }
        let mut t = Self::new();
        t.load_opponents();
        t
    }

    /// Reads the champion archive: the benchmark panel, and everything else as training opponents.
    fn load_opponents(&mut self) {
        self.archive.clear();
        self.panel.clear();
        for (g, b) in archived_champions() {
            if PANEL_GENS.contains(&g) {
                self.panel.push(b);
            } else {
                self.archive.push(b);
            }
        }
        println!("{} archived opponents, benchmark panel of {}", self.archive.len(), self.panel.len());
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

    /// Grows (with mutated copies of the best) or shrinks the population to `n`.
    fn resize_population(&mut self, n: usize) {
        let mut rng = SmallRng::seed_from_u64(self.seed);
        let k = self.pop.len();
        while self.pop.len() < n {
            let mut child = self.pop[rng.gen_range(0..k.min(ELITE * 4))].clone();
            mutate(&mut child, &mut rng);
            self.pop.push(child);
        }
        self.pop.truncate(n);
    }

    /// How far through the heal curriculum training is (0 = plentiful heals, 1 = final rules).
    pub fn curriculum_progress(&self) -> f32 {
        (self.generation as f32 / HEAL_CURRICULUM_GENS).min(1.0)
    }

    /// Plays the current top 4 against the fixed panel (same seeds every time, so results are
    /// comparable across generations) and records the win rate. 25% = as good as the panel.
    fn run_benchmark(&mut self) {
        if self.panel.len() < N_FIGHTERS - 1 {
            return;
        }
        let wins: u32 = (0..BENCH_MATCHES)
            .into_par_iter()
            .map(|seed| {
                let mut rng = SmallRng::seed_from_u64(seed ^ 0xbe7c);
                let mut panel: Vec<usize> = (0..self.panel.len()).collect();
                panel.shuffle(&mut rng);
                let me = rng.gen_range(0..N_FIGHTERS);
                let brains: [&Brain; N_FIGHTERS] = std::array::from_fn(|s| {
                    if s == me { &self.top[seed as usize % self.top.len()] } else { &self.panel[panel[s]] }
                });
                let mut m = Match::new(&mut rng, HEALS_PER_ROUND);
                while !m.over {
                    m.step(brains);
                }
                (m.winner == Some(me)) as u32
            })
            .sum();
        let rate = wins as f32 / BENCH_MATCHES as f32;
        self.bench.push((self.generation, rate));
        let path = save_path().with_file_name(format!("{}_bench.csv", save_path().file_stem().unwrap().to_string_lossy()));
        let new = !path.exists();
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            if new {
                let _ = writeln!(f, "generation,win_rate_vs_panel");
            }
            let _ = writeln!(f, "{},{:.4}", self.generation, rate);
        }
        println!("benchmark at gen {}: {:.1}% wins vs panel (25% = equal)", self.generation, rate * 100.0);
    }

    /// ES: fills the population with mirrored perturbations of the centre brain (starting the
    /// centre from the current best if ES is new) and returns the noise vectors.
    fn es_perturb(&mut self, rng: &mut SmallRng) -> Vec<Vec<f32>> {
        let es = self.es.get_or_insert_with(|| {
            let center = self.top[0].clone();
            let dim = center.w.len();
            EsState { center, m: vec![0.0; dim], v: vec![0.0; dim], t: 0 }
        });
        let pairs = (self.pop.len() / 2).max(1);
        let noise: Vec<Vec<f32>> = (0..pairs).map(|_| (0..es.center.w.len()).map(|_| gauss(rng)).collect()).collect();
        self.pop = noise
            .iter()
            .flat_map(|e| {
                [1.0f32, -1.0].map(|sign| Brain { w: es.center.w.iter().zip(e).map(|(c, n)| c + sign * ES_SIGMA * n).collect() })
            })
            .collect();
        noise
    }

    /// ES: moves the centre along the rank-weighted noise (centred ranks, mirrored pairs), with
    /// Adam and a little weight decay.
    fn es_update(&mut self, noise: &[Vec<f32>], fit: &[f32]) {
        let n = fit.len();
        let mut idx: Vec<usize> = (0..n).collect();
        idx.sort_by(|&a, &b| fit[a].total_cmp(&fit[b]));
        let mut rank = vec![0.0f32; n];
        for (r, &i) in idx.iter().enumerate() {
            rank[i] = r as f32 / (n - 1).max(1) as f32 - 0.5;
        }
        let es = self.es.as_mut().unwrap();
        let mut g = vec![0.0f32; es.center.w.len()];
        for (k, e) in noise.iter().enumerate() {
            let a = rank[2 * k] - rank[2 * k + 1];
            for (gi, ei) in g.iter_mut().zip(e) {
                *gi += a * ei;
            }
        }
        let scale = 1.0 / (noise.len() as f32 * ES_SIGMA);
        es.t += 1;
        let (b1, b2) = (0.9f32, 0.999f32);
        let lr = ES_LR * (1.0 - b2.powi(es.t as i32)).sqrt() / (1.0 - b1.powi(es.t as i32));
        for i in 0..g.len() {
            let gi = g[i] * scale - ES_DECAY * es.center.w[i];
            es.m[i] = b1 * es.m[i] + (1.0 - b1) * gi;
            es.v[i] = b2 * es.v[i] + (1.0 - b2) * gi * gi;
            es.center.w[i] += lr * es.m[i] / (es.v[i].sqrt() + 1e-8);
        }
    }

    /// Runs one generation: evaluate everyone, record champions, breed the next population.
    pub fn step(&mut self) -> GenStats {
        let mut rng = SmallRng::seed_from_u64(self.seed);
        self.seed = rng.r#gen();
        let es_noise = use_es().then(|| self.es_perturb(&mut rng));

        // Shared scenarios (see SCENARIOS): the same seed, seat and three opponents for everyone.
        let (n, scenarios) = (self.pop.len(), ga_settings().1);
        let pick = |rng: &mut SmallRng, len: usize, slot: fn(usize) -> Slot| (len > 0).then(|| slot(rng.gen_range(0..len)));
        let mut jobs: Vec<([Slot; N_FIGHTERS], u64)> = Vec::new();
        for _ in 0..scenarios {
            let top = Slot::Top(rng.gen_range(0..self.top.len()));
            let recent = pick(&mut rng, self.hof.len(), Slot::Hof).unwrap_or(top);
            let old = pick(&mut rng, self.archive.len(), Slot::Archive).unwrap_or(recent);
            let mut seats = [Slot::Pop(0), top, recent, old];
            seats.shuffle(&mut rng);
            let (learner, seed) = (seats.iter().position(|s| matches!(s, Slot::Pop(_))).unwrap(), rng.r#gen());
            for i in 0..n {
                let mut slots = seats;
                slots[learner] = Slot::Pop(i);
                jobs.push((slots, seed));
            }
        }

        let shaping = (1.0 - self.generation as f32 / SHAPING_GENS).max(0.0);
        let heals = heals_at(self.curriculum_progress());
        let results: Vec<MatchResult> = jobs
            .par_iter()
            .map(|(slots, seed)| {
                let brains: [&Brain; N_FIGHTERS] = std::array::from_fn(|s| match slots[s] {
                    Slot::Pop(i) => &self.pop[i],
                    Slot::Hof(i) => &self.hof[i],
                    Slot::Archive(i) => &self.archive[i],
                    Slot::Top(i) => &self.top[i],
                });
                let mut r = SmallRng::seed_from_u64(*seed);
                let mut m = Match::new(&mut r, heals.clone());
                while !m.over {
                    m.step(brains);
                }
                let mut dmg_by_source = [0.0; 5];
                for f in &m.f {
                    for (d, s) in dmg_by_source.iter_mut().zip(f.dmg_by_source) {
                        *d += s;
                    }
                }
                MatchResult {
                    fitness: std::array::from_fn(|s| m.fitness(s, shaping)),
                    winner: m.winner,
                    on_health: m.winner.is_some() && m.timed_out(),
                    dmg_by_source,
                    self_dmg: m.f.iter().map(|f| f.self_dmg).sum(),
                    hits: m.f.iter().map(|f| f.hits).sum(),
                    headshots: m.f.iter().map(|f| f.headshots).sum(),
                    heals_taken: m.f.iter().map(|f| f.heals_taken).sum(),
                    heals_spawned: m.heals_spawned,
                }
            })
            .collect();

        let mut sum = vec![0.0f32; n];
        let mut cnt = vec![0u32; n];
        let mut stats = GenStats { generation: self.generation, matches: jobs.len() as u32, ..Default::default() };
        for ((slots, _), r) in jobs.iter().zip(results.iter()) {
            for (d, s) in stats.dmg_by_source.iter_mut().zip(r.dmg_by_source) {
                *d += s;
            }
            stats.self_dmg += r.self_dmg;
            stats.hits += r.hits;
            stats.headshots += r.headshots;
            stats.heals += r.heals_taken;
            stats.heals_spawned += r.heals_spawned;
            stats.wins_on_health += r.on_health as u32;
            stats.draws += r.winner.is_none() as u32;
            for s in 0..N_FIGHTERS {
                if let Slot::Pop(i) = slots[s] {
                    sum[i] += r.fitness[s];
                    cnt[i] += 1;
                }
            }
        }

        let fit: Vec<f32> = (0..n).map(|i| sum[i] / cnt[i].max(1) as f32).collect();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| fit[b].total_cmp(&fit[a]));
        stats.best = fit[order[0]];
        stats.avg = fit.iter().sum::<f32>() / n as f32;
        if let Some(noise) = es_noise {
            // ES: the policy is the centre brain itself; it's what the viewer, benchmark and
            // archive see.
            self.es_update(&noise, &fit);
            let center = self.es.as_ref().unwrap().center.clone();
            self.top = vec![center.clone(); N_FIGHTERS];
            self.hof.push(center);
        } else {
            self.top = order[..N_FIGHTERS].iter().map(|&i| self.pop[i].clone()).collect();
            self.hof.push(self.pop[order[0]].clone());
            self.pop = breed(&self.pop, &fit, &order, &mut rng);
        }
        if self.hof.len() > HOF_CAP {
            self.hof.remove(0);
        }
        self.generation += 1;
        if self.generation % ARCHIVE_EVERY == 0 {
            archive_champion(self.generation, &self.top[0]);
            self.archive.push(self.top[0].clone());
        }
        if self.generation % BENCH_EVERY == 0 || self.bench.is_empty() {
            self.run_benchmark();
        }
        self.history.push(stats.clone());
        append_log(&stats);
        stats
    }
}

fn append_log(s: &GenStats) {
    let path = log_path();
    let new = !path.exists();
    let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    if new {
        let _ = writeln!(f, "generation,best,avg,matches,draws,wins_on_health,fists,stab,spear_throw,rock,grenade,self_dmg,hits,headshots,heals,heals_spawned");
    }
    let d = s.dmg_by_source;
    let _ = writeln!(
        f,
        "{},{:.1},{:.1},{},{},{},{:.0},{:.0},{:.0},{:.0},{:.0},{:.0},{},{},{},{}",
        s.generation, s.best, s.avg, s.matches, s.draws, s.wins_on_health, d[0], d[1], d[2], d[3], d[4], s.self_dmg, s.hits, s.headshots, s.heals, s.heals_spawned
    );
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

fn breed(pop: &[Brain], fit: &[f32], order: &[usize], rng: &mut SmallRng) -> Vec<Brain> {
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
    /// The best few of the latest generation; the viewer pits them against each other.
    pub top: Vec<Brain>,
    pub history: Vec<GenStats>,
    pub gens_per_sec: f32,
    pub stop: bool,
    pub save_now: bool,
    pub last_save_gen: u32,
    /// Heal curriculum progress (0..1); the viewer plays by the same rules as training.
    pub curriculum: f32,
    /// Benchmark history: (generation, win rate vs the panel).
    pub bench: Vec<(u32, f32)>,
}

pub fn build_pool() -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(training_threads()).build().expect("thread pool")
}

pub fn spawn(fresh: bool) -> (Arc<Mutex<Shared>>, JoinHandle<()>) {
    let mut t = Trainer::load_or_new(fresh);
    let shared = Arc::new(Mutex::new(Shared {
        generation: t.generation,
        top: t.top.clone(),
        history: t.history.clone(),
        gens_per_sec: 0.0,
        stop: false,
        save_now: false,
        last_save_gen: t.generation,
        curriculum: t.curriculum_progress(),
        bench: t.bench.clone(),
    }));
    let s2 = shared.clone();
    let handle = std::thread::spawn(move || {
        let pool = build_pool();
        let mut rate = 0.0f32;
        let mut last_save = Instant::now();
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
            let saved = save_now || last_save.elapsed() >= AUTOSAVE_EVERY;
            if saved {
                t.save();
                last_save = Instant::now();
            }
            let mut s = s2.lock().unwrap();
            s.generation = t.generation;
            s.top = t.top.clone();
            s.curriculum = t.curriculum_progress();
            s.bench = t.bench.clone();
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
pub fn headless(gens: u32, fresh: bool, minutes: Option<f32>) {
    let began = Instant::now();
    let mut t = Trainer::load_or_new(fresh);
    let pool = build_pool();
    println!("training {gens} generations on {} threads; saving to {}", training_threads(), save_path().display());
    let mut last_save = Instant::now();
    let pct = |v: Option<f32>| v.map_or("  -".to_string(), |v| format!("{:3.0}", v * 100.0));
    for _ in 0..gens {
        if minutes.is_some_and(|m| began.elapsed().as_secs_f32() >= m * 60.0) {
            break;
        }
        let start = Instant::now();
        let s = pool.install(|| t.step());
        let shares: Vec<String> = Source::ALL.iter().map(|&src| format!("{} {}%", src.name(), pct(s.share(src)))).collect();
        println!(
            "gen {:5} | best {:6.1} avg {:6.1} | KO {}% | headshots {}% | heals {}% | {} | {:.2}s",
            s.generation, s.best, s.avg, pct(s.knockout_rate()), pct(s.headshot_rate()), pct(s.heal_pickup_rate()),
            shares.join("  "), start.elapsed().as_secs_f32()
        );
        if last_save.elapsed() >= AUTOSAVE_EVERY {
            t.save();
            last_save = Instant::now();
        }
    }
    t.save();
}

/// Champion snapshots live next to the save: `save/ffa_champions/gen_029000.json`.
pub fn archive_dir() -> PathBuf {
    let save = save_path();
    save.with_file_name(format!("{}_champions", save.file_stem().unwrap().to_string_lossy()))
}

fn archive_champion(generation: u32, brain: &Brain) {
    let dir = archive_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(text) = serde_json::to_string(brain) {
        let _ = std::fs::write(dir.join(format!("gen_{generation:06}.json")), text);
    }
}

/// Every archived champion, oldest first.
pub fn archived_champions() -> Vec<(u32, Brain)> {
    let Ok(entries) = std::fs::read_dir(archive_dir()) else {
        return Vec::new();
    };
    let mut out: Vec<(u32, Brain)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let generation = name.strip_prefix("gen_")?.strip_suffix(".json")?.parse().ok()?;
            let brain: Brain = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            brain.is_current().then_some((generation, brain))
        })
        .collect();
    out.sort_by_key(|(g, _)| *g);
    out
}

/// Watch-only: no training. Shows the brains in the save and re-reads it every 30s, so it can
/// follow a training run in another process without using its CPU budget. Never writes.
pub fn spawn_watch() -> (Arc<Mutex<Shared>>, JoinHandle<()>) {
    let read = || serde_json::from_str::<Trainer>(&std::fs::read_to_string(save_path()).ok()?).ok();
    let t = read().unwrap_or_else(Trainer::new);
    let shared = Arc::new(Mutex::new(Shared {
        generation: t.generation,
        top: t.top.clone(),
        history: t.history.clone(),
        gens_per_sec: 0.0,
        stop: false,
        save_now: false,
        last_save_gen: t.generation,
        curriculum: t.curriculum_progress(),
        bench: t.bench.clone(),
    }));
    let s2 = shared.clone();
    let handle = std::thread::spawn(move || {
        let mut last = Instant::now();
        while !s2.lock().unwrap().stop {
            std::thread::sleep(Duration::from_millis(200));
            if last.elapsed() < Duration::from_secs(30) {
                continue;
            }
            last = Instant::now();
            if let Some(t) = read() {
                let mut s = s2.lock().unwrap();
                s.generation = t.generation;
                s.curriculum = t.curriculum_progress();
                s.bench = t.bench.clone();
                s.top = t.top;
                s.history = t.history;
                s.last_save_gen = t.generation;
            }
        }
    });
    (shared, handle)
}
