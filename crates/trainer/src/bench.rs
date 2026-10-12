//! `--bench`: single-threaded timing of the simulation, so speedups can be measured.
//! Uses the current champions from the save (read-only) so fighters behave realistically.

use crate::brain::{Brain, N_OUT};
use crate::evo::Trainer;
use crate::sim::{Match, HEALS_PER_ROUND, N_FIGHTERS};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use std::hint::black_box;
use std::time::Instant;

pub fn run() {
    let t = Trainer::load_or_new(false);
    let brains: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| &t.top[i % t.top.len()]);

    // Whole matches, fixed seeds.
    let (mut ticks, start) = (0u64, Instant::now());
    for seed in 0..150u64 {
        let mut m = Match::new(&mut SmallRng::seed_from_u64(seed), HEALS_PER_ROUND);
        while !m.over {
            m.step(brains);
            ticks += 1;
        }
    }
    let per_tick = start.elapsed().as_secs_f64() * 1e6 / ticks as f64;
    println!("full step:      {per_tick:7.2} us/tick  ({ticks} ticks)");

    // Pieces, measured on a mid-match state.
    let mut m = Match::new(&mut SmallRng::seed_from_u64(7), HEALS_PER_ROUND);
    for _ in 0..400 {
        m.step(brains);
    }
    let n = 200_000;
    let start = Instant::now();
    for k in 0..n {
        black_box(m.senses(black_box(k % N_FIGHTERS)));
    }
    let senses = start.elapsed().as_secs_f64() * 1e6 / n as f64;
    let input = m.senses(0);
    let mut out = [0.0; N_OUT];
    let start = Instant::now();
    for _ in 0..n {
        brains[0].forward(black_box(&input), &mut out);
        black_box(&out);
    }
    let forward = start.elapsed().as_secs_f64() * 1e6 / n as f64;
    println!("senses x4:      {:7.2} us/tick", senses * 4.0);
    println!("brain x4:       {:7.2} us/tick", forward * 4.0);
    println!("everything else {:7.2} us/tick", per_tick - 4.0 * (senses + forward));
}

/// `--tournament a.json b.json ...`: pits the top brains of several saves against each other in
/// four-way matches (each save gets a seat, seats shuffled every match) and reports win rates.
/// Used to check which training setting actually produced better fighters.
pub fn tournament(paths: &[String], matches: usize) {
    use rand::seq::SliceRandom;
    use rand::Rng;
    use rayon::prelude::*;
    let entrants: Vec<(String, Vec<Brain>)> = paths
        .iter()
        .filter_map(|p| {
            // A full save (its top brains) or an archived champion (one brain).
            let text = std::fs::read_to_string(p).ok()?;
            if let Ok(t) = serde_json::from_str::<Trainer>(&text) {
                println!("{p}: generation {}", t.generation);
                return Some((p.clone(), t.top));
            }
            Some((p.clone(), vec![serde_json::from_str::<Brain>(&text).ok()?]))
        })
        .collect();
    assert!(!entrants.is_empty(), "no readable saves");
    // GO_BOX_THREADS can lower the thread count, e.g. to run alongside training.
    let threads = std::env::var("GO_BOX_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(crate::evo::training_threads());
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads.min(crate::evo::training_threads())).build().expect("thread pool");
    // (winner entrant, knockout?) per match; None = draw.
    let results: Vec<(Option<usize>, [usize; N_FIGHTERS])> = pool.install(|| {
        (0..matches as u64)
            .into_par_iter()
            .map(|seed| {
                let mut rng = SmallRng::seed_from_u64(seed ^ 0x5eed);
                // Four different entrants when there are enough, otherwise some get two seats.
                let mut pick: Vec<usize> = (0..entrants.len()).collect();
                pick.shuffle(&mut rng);
                let mut seats: Vec<usize> = (0..N_FIGHTERS).map(|i| pick[i % pick.len()]).collect();
                seats.shuffle(&mut rng);
                let seats: [usize; N_FIGHTERS] = std::array::from_fn(|i| seats[i]);
                let brains: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| {
                    let top = &entrants[seats[i]].1;
                    &top[rng.gen_range(0..top.len())]
                });
                let mut m = Match::new(&mut rng, HEALS_PER_ROUND);
                while !m.over {
                    m.step(brains);
                }
                (m.winner.map(|w| seats[w]), seats)
            })
            .collect()
    });
    let mut wins = vec![0u32; entrants.len()];
    let mut seats_played = vec![0u32; entrants.len()];
    for (w, seats) in &results {
        if let Some(w) = w {
            wins[*w] += 1;
        }
        for &s in seats {
            seats_played[s] += 1;
        }
    }
    println!("{matches} matches; a fair share is 25% of seats");
    for (i, (p, _)) in entrants.iter().enumerate() {
        // Win rate per seat played, so entrants with extra seats aren't favoured.
        println!("{:>6.1}% wins per seat  {p}", 100.0 * wins[i] as f32 / seats_played[i].max(1) as f32);
    }
}

/// `--duels a.json b.json ...`: every pair of champions plays `per_pair` 1v1 matches (seats
/// alternate). Prints a matrix of row-vs-column win rates; draws count as half a win.
pub fn duels(paths: &[String], per_pair: usize) {
    use rayon::prelude::*;
    let entrants: Vec<(String, Brain)> = paths
        .iter()
        .filter_map(|p| {
            let b: Brain = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
            let name = std::path::Path::new(p).file_stem()?.to_string_lossy().replace("gen_", "");
            Some((name, b))
        })
        .collect();
    let threads = std::env::var("GO_BOX_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(crate::evo::training_threads());
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads.min(crate::evo::training_threads())).build().expect("thread pool");
    let n = entrants.len();
    let pairs: Vec<(usize, usize)> = (0..n).flat_map(|a| (a + 1..n).map(move |b| (a, b))).collect();
    // Score of `a` against `b` in [0, 1].
    let scores: Vec<f32> = pool.install(|| {
        pairs
            .par_iter()
            .map(|&(a, b)| {
                let total: f32 = (0..per_pair as u64)
                    .map(|seed| {
                        let flip = seed % 2 == 1;
                        let (s0, s2) = if flip { (b, a) } else { (a, b) };
                        let brains: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| &entrants[if i < 2 { s0 } else { s2 }].1);
                        let mut m = Match::duel(&mut SmallRng::seed_from_u64(seed ^ 0xd0e1), HEALS_PER_ROUND);
                        while !m.over {
                            m.step(brains);
                        }
                        let a_seat = if flip { 2 } else { 0 };
                        match m.winner {
                            Some(w) if w == a_seat => 1.0,
                            Some(_) => 0.0,
                            None => 0.5,
                        }
                    })
                    .sum();
                total / per_pair as f32
            })
            .collect()
    });
    let mut grid = vec![vec![f32::NAN; n]; n];
    for (&(a, b), &s) in pairs.iter().zip(&scores) {
        grid[a][b] = s;
        grid[b][a] = 1.0 - s;
    }
    println!("1v1 win rate of row vs column ({per_pair} matches per pair; 50% = even)");
    print!("{:>8}", "");
    for (name, _) in &entrants {
        print!("{name:>8}");
    }
    println!("   overall");
    for (i, (name, _)) in entrants.iter().enumerate() {
        print!("{name:>8}");
        for j in 0..n {
            if i == j { print!("{:>8}", "-") } else { print!("{:>7.0}%", grid[i][j] * 100.0) }
        }
        let overall = (0..n).filter(|&j| j != i).map(|j| grid[i][j]).sum::<f32>() / (n - 1) as f32;
        println!("   {:>5.1}%", overall * 100.0);
    }
}
