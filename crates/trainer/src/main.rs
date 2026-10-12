mod bench;
use go_box_sim::brain;
mod evo;
use go_box_render as render;
use go_box_sim::sim;

use ::rand::rngs::SmallRng;
use ::rand::SeedableRng;
use macroquad::prelude::*;
use brain::Brain;
use go_box_sim::character::Character;
use sim::{Action, Match, Source, DT, H, HEALS_PER_ROUND, N_FIGHTERS, W};

const USAGE: &str = "go-box [--fresh] [--headless <generations> [--minutes M]] [--watch] [--compare] [--play] [--duel] [--cast] [--screenshot <prefix.png>]";

/// Compare mode: who sits in each seat (by scarf colour: RED, BLUE, GOLD, VIOLET), as
/// "generations before the current one". `None` is an untrained brain (generation 1).
const COMPARE_SEATS: [Option<u32>; N_FIGHTERS] = [Some(10_000), Some(20_000), Some(0), None];

/// The brains and labels for a compare-mode match: the current champion, archived champions
/// from about 10k and 20k generations earlier, and a fresh untrained brain.
fn compare_lineup(current: &Brain, generation: u32, rng: &mut SmallRng) -> (Vec<Brain>, Vec<String>) {
    let archive = evo::archived_champions();
    let mut brains = Vec::new();
    let mut labels = Vec::new();
    for seat in COMPARE_SEATS {
        let (brain, label) = match seat {
            Some(0) => (current.clone(), format!("gen {generation}")),
            Some(ago) => {
                // The newest archived champion at least `ago` generations old.
                let target = generation.saturating_sub(ago);
                match archive.iter().rev().find(|(g, _)| *g <= target) {
                    Some((g, b)) => (b.clone(), format!("gen {g}")),
                    // Champions are only archived from gen ~29k on, so early on there's none yet.
                    None => (Brain::random(rng), format!("no gen {target} saved yet: untrained")),
                }
            }
            None => (Brain::random(rng), "gen 1".into()),
        };
        brains.push(brain);
        labels.push(label);
    }
    (brains, labels)
}

fn conf() -> Conf {
    Conf { window_title: "Go Box".into(), window_width: 1280, window_height: 720, ..Default::default() }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fresh = args.iter().any(|a| a == "--fresh");
    let value_of = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--tournament") {
        let paths: Vec<String> = args[i + 1..].iter().take_while(|a| !a.starts_with("--")).cloned().collect();
        bench::tournament(&paths, value_of("--matches").and_then(|n| n.parse().ok()).unwrap_or(2000));
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--duels") {
        let paths: Vec<String> = args[i + 1..].iter().take_while(|a| !a.starts_with("--")).cloned().collect();
        bench::duels(&paths, value_of("--matches").and_then(|n| n.parse().ok()).unwrap_or(1000));
        return;
    }
    if args.iter().any(|a| a == "--bench") {
        bench::run();
        return;
    }
    if let Some(n) = value_of("--headless") {
        evo::headless(n.parse().unwrap_or(100), fresh, value_of("--minutes").and_then(|m| m.parse().ok()));
        return;
    }
    let screenshot = value_of("--screenshot");
    let start = [("--compare", KeyCode::C), ("--play", KeyCode::V), ("--duel", KeyCode::D)].into_iter().find(|(f, _)| args.iter().any(|a| a == f)).map(|(_, k)| k);
    macroquad::Window::from_config(conf(), viewer(fresh, screenshot, start, args.iter().any(|a| a == "--watch"), args.iter().any(|a| a == "--cast")));
}

/// Play mode: the seat you control (RED); the champion sits in GOLD.
const PLAYER: usize = 0;

/// Your record against the champion in play mode.
struct Play {
    wins: u32,
    losses: u32,
    draws: u32,
}

/// Keyboard controls: A/D move, Shift run, Space jump, R melee, E grab (hold) / throw (press while
/// holding something), W/S aim up/down.
fn player_action(holding: bool, throw: bool) -> Action {
    let down = |keys: &[KeyCode]| keys.iter().any(|&k| is_key_down(k));
    let mv = down(&[KeyCode::D, KeyCode::Right]) as i32 as f32 - down(&[KeyCode::A, KeyCode::Left]) as i32 as f32;
    let aim = if down(&[KeyCode::W, KeyCode::Up]) { 1.0 } else if down(&[KeyCode::S, KeyCode::Down]) { -0.6 } else { 0.3 };
    Action {
        mv,
        run: down(&[KeyCode::LeftShift, KeyCode::RightShift]),
        jump: down(&[KeyCode::Space]),
        attack: down(&[KeyCode::R]),
        throw,
        aim,
        grab: !holding && down(&[KeyCode::E]),
        special: down(&[KeyCode::F]),
    }
}

/// Running tally for compare mode.
struct Compare {
    labels: Vec<String>,
    wins: [u32; N_FIGHTERS],
    draws: u32,
    /// 1v1 instead: the current champion (GOLD) against a random archived champion (RED).
    duel: bool,
}

async fn viewer(fresh: bool, screenshot: Option<String>, start_mode: Option<KeyCode>, watch: bool, cast: bool) {
    // --cast: seats play Sam, Austin, Tyler and Classic (you are Sam in play mode, F = special).
    let apply_cast = |m: &mut Match| {
        if cast {
            for (f, c) in m.f.iter_mut().zip([Character::Sam, Character::Austin, Character::Tyler, Character::Classic]) {
                f.character = c;
            }
        }
    };
    prevent_quit();
    let (shared, trainer) = if watch { evo::spawn_watch() } else { evo::spawn(fresh) };
    let rt = render_target(W as u32, H as u32);
    rt.texture.set_filter(FilterMode::Nearest);
    let mut cam = Camera2D { target: vec2(W / 2.0, H / 2.0), zoom: vec2(2.0 / W, 2.0 / H), ..Default::default() };
    cam.render_target = Some(rt.clone());

    let mut art = render::Art::new();
    // Sound only in play mode: training at speed would be a racket.
    let mut sfx = render::sound::Sounds::load();
    let mut rng = SmallRng::from_entropy();
    // The viewer pits the best few of the latest generation against each other, under the
    // heal rules training is currently using.
    let snapshot = || {
        let s = shared.lock().unwrap();
        (s.top.clone(), s.generation, s.curriculum)
    };
    let (mut top, mut match_gen, curriculum) = snapshot();
    let mut m = Match::new(&mut rng, evo::heals_at(curriculum));
    apply_cast(&mut m);
    let (mut acc, mut end_timer, mut frames) = (0.0f32, 0.0f32, 0u32);
    let mut throw_queued = false;
    let mut speed = 1.0f32;
    let mut debug = false;
    let mut paused = false;
    // Compare mode: champions from different generations fight, with a running win tally.
    // Play mode: you (RED) against the latest champion (GOLD), 1v1.
    let mut compare: Option<Compare> = None;
    let mut play: Option<Play> = None;
    let mut switch = start_mode;
    let mut tallied = false;

    loop {
        for k in [KeyCode::C, KeyCode::V, KeyCode::D] {
            // D is a movement key while playing.
            if is_key_pressed(k) && !(k == KeyCode::D && play.is_some()) {
                switch = Some(k);
            }
        }
        if is_quit_requested() || is_key_pressed(KeyCode::Escape) {
            shared.lock().unwrap().stop = true;
            break;
        }
        if is_key_pressed(KeyCode::Tab) {
            debug = !debug;
        }
        if is_key_pressed(KeyCode::P) {
            paused = !paused;
        }
        // In play mode the letter keys are the controls, and the game runs at normal speed.
        if play.is_none() {
            if is_key_pressed(KeyCode::S) {
                shared.lock().unwrap().save_now = true;
            }
            for (k, v) in [(KeyCode::Key1, 1.0), (KeyCode::Key2, 2.0), (KeyCode::Key3, 4.0), (KeyCode::Key4, 8.0)] {
                if is_key_pressed(k) {
                    speed = v;
                }
            }
        }
        if m.over && !tallied {
            if let Some(c) = &mut compare {
                match m.winner {
                    Some(w) => c.wins[w] += 1,
                    None => c.draws += 1,
                }
            }
            if let Some(p) = &mut play {
                match m.winner {
                    Some(PLAYER) => p.wins += 1,
                    Some(_) => p.losses += 1,
                    None => p.draws += 1,
                }
            }
            tallied = true;
        }
        let new_match_key = play.is_none() && is_key_pressed(KeyCode::R);
        if switch.is_some() || new_match_key || (m.over && end_timer > 2.5) {
            // C toggles compare mode and V toggles play mode; switching to one leaves the other.
            match switch.take() {
                Some(k @ (KeyCode::C | KeyCode::D)) => {
                    play = None;
                    let duel = k == KeyCode::D;
                    compare = match compare {
                        Some(c) if c.duel == duel => None,
                        _ => Some(Compare { labels: Vec::new(), wins: [0; N_FIGHTERS], draws: 0, duel }),
                    };
                }
                Some(_) => {
                    compare = None;
                    play = match play {
                        Some(_) => None,
                        None => Some(Play { wins: 0, losses: 0, draws: 0 }),
                    };
                    speed = 1.0;
                }
                None => {}
            }
            let curriculum;
            (top, match_gen, curriculum) = snapshot();
            m = if let Some(c) = compare.as_mut().filter(|c| c.duel) {
                let archive = evo::archived_champions();
                let (g, old) = archive[::rand::Rng::gen_range(&mut rng, 0..archive.len())].clone();
                c.labels = vec![format!("gen {g}"), String::new(), format!("gen {match_gen} (current)"), String::new()];
                top = vec![old.clone(), old, top[0].clone(), top[0].clone()];
                Match::duel(&mut rng, HEALS_PER_ROUND)
            } else if let Some(c) = &mut compare {
                (top, c.labels) = compare_lineup(&top[0], match_gen, &mut rng);
                Match::new(&mut rng, HEALS_PER_ROUND)
            } else if play.is_some() {
                // The champion takes the GOLD seat; RED is you.
                top = vec![top[0].clone(); N_FIGHTERS];
                Match::duel(&mut rng, HEALS_PER_ROUND)
            } else {
                Match::new(&mut rng, evo::heals_at(curriculum))
            };
            apply_cast(&mut m);
            tallied = false;
            art.reset();
            end_timer = 0.0;
            acc = 0.0;
        }

        let dt = if paused { 0.0 } else { get_frame_time().min(0.1) * speed };
        if m.over {
            end_timer += dt / speed.max(1.0);
        } else {
            acc += dt;
            // E throws only on a fresh press, so holding E to pick something up doesn't also throw
            // it. The press is kept until a tick runs (a fast frame may run no ticks).
            let holding = m.f[PLAYER].holding.is_some();
            throw_queued |= holding && is_key_pressed(KeyCode::E);
            let human = play.is_some().then(|| (PLAYER, player_action(holding, throw_queued)));
            if acc >= DT {
                throw_queued = false;
            }
            while acc >= DT && !m.over {
                m.step_with(std::array::from_fn(|i| &top[i % top.len()]), human);
                art.handle_events(&m);
                if play.is_some() {
                    sfx.events(&m, &m.events, None);
                }
                acc -= DT;
            }
        }
        art.update(dt, &m);
        if play.is_some() {
            sfx.frame(dt, &m, Some(PLAYER));
        }

        set_camera(&cam);
        clear_background(BLACK);
        art.draw(&m, debug);
        if play.is_some() {
            // Show where a throw would go.
            let f = &m.f[PLAYER];
            if f.alive() && f.holding.is_some() {
                let (o, d) = (f.hand(), f.aim_dir(player_action(true, false).aim));
                for k in 1..6 {
                    let p = o + d * (k as f32 * 6.0);
                    draw_rectangle(p.x.floor(), p.y.floor(), 1.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.5 - k as f32 * 0.07));
                }
            }
        }

        set_default_camera();
        clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
        let scale = (screen_width() / W).min(screen_height() / H);
        let (dw, dh) = (W * scale, H * scale);
        let (ox, oy) = (((screen_width() - dw) / 2.0).floor(), ((screen_height() - dh) / 2.0).floor());
        draw_texture_ex(&rt.texture, ox, oy, WHITE, DrawTextureParams { dest_size: Some(vec2(dw, dh)), ..Default::default() });
        draw_hud(&shared.lock().unwrap(), &m, match_gen, compare.as_ref(), play.as_ref(), speed, paused, ox, oy, dw, dh);

        frames += 1;
        if let Some(path) = &screenshot {
            // Capture a few frames spread over ~10s, then quit (used for quick visual checks).
            if frames % 200 == 0 {
                get_screen_data().export_png(&path.replace(".png", &format!("_{}.png", frames / 200)));
            }
            if frames >= 600 {
                shared.lock().unwrap().stop = true;
                break;
            }
        }
        next_frame().await;
    }
    let _ = trainer.join();
}

/// Colours for each damage source in the "how they fight" chart.
fn source_color(s: Source) -> Color {
    match s {
        Source::Fist => Color::new(0.85, 0.80, 0.72, 1.0),
        Source::Stab => Color::new(0.80, 0.55, 0.30, 1.0),
        Source::SpearThrow => Color::new(0.95, 0.35, 0.25, 1.0),
        Source::Rock => Color::new(0.60, 0.62, 0.68, 1.0),
        Source::Grenade => Color::new(0.55, 0.85, 0.30, 1.0),
        Source::Special => Color::new(0.45, 0.85, 0.55, 1.0),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_hud(s: &evo::Shared, m: &Match, match_gen: u32, compare: Option<&Compare>, play: Option<&Play>, speed: f32, paused: bool, ox: f32, oy: f32, dw: f32, dh: f32) {
    let dim = Color::new(0.72, 0.74, 0.78, 0.85);
    let (tx, ty) = (ox + 12.0, oy + 22.0);
    let grey = Color::new(0.4, 0.4, 0.42, 0.8);
    if let Some(p) = play {
        draw_text(
            &format!("YOU vs GEN {match_gen} CHAMPION   you {} - {} champion   ({} draws)", p.wins, p.losses, p.draws),
            tx, ty, 20.0, dim,
        );
        for (row, (i, name)) in [(PLAYER, "YOU".to_string()), (2, format!("GEN {match_gen}"))].into_iter().enumerate() {
            let f = &m.f[i];
            draw_text(&format!("{name} {:.0}", f.hp.max(0.0)), tx, ty + 22.0 + row as f32 * 18.0, 18.0, if f.alive() { render::ACCENTS[i] } else { grey });
        }
    } else if let Some(c) = compare {
        // Compare mode: one line per seat with its generation and wins so far.
        let title = if c.duel { "1v1: CURRENT CHAMPION vs A RANDOM OLD CHAMPION   (RED wins = old champions combined)" } else { "COMPARING GENERATIONS" };
        draw_text(&format!("{title}   (save at gen {})", s.generation), tx, ty, 20.0, dim);
        let played = c.wins.iter().sum::<u32>() + c.draws;
        for (row, i) in (0..N_FIGHTERS).filter(|&i| !m.f[i].absent).enumerate() {
            let f = &m.f[i];
            let pct = if played > 0 { c.wins[i] as f32 / played as f32 * 100.0 } else { 0.0 };
            let label = format!(
                "{:<6} {:<34} wins {:>3} ({:>3.0}%)   hp {:.0}",
                render::NAMES[i], c.labels.get(i).map_or("", |l| l.as_str()), c.wins[i], pct, f.hp.max(0.0)
            );
            draw_text(&label, tx, ty + 22.0 + row as f32 * 18.0, 18.0, if f.alive() { render::ACCENTS[i] } else { grey });
        }
        draw_text(&format!("{played} matches, {} draws", c.draws), tx, ty + 22.0 + 4.0 * 18.0, 16.0, dim);
    } else {
        draw_text(
            &if s.gens_per_sec > 0.0 {
                format!("GEN {}   {:.1} gen/s   {} threads   watching gen {}'s top 4", s.generation, s.gens_per_sec, evo::training_threads(), match_gen)
            } else {
                format!("GEN {}   watch-only (not training)   watching gen {}'s top 4", s.generation, match_gen)
            },
            tx, ty, 20.0, dim,
        );
        // Roster, coloured by scarf; the dead are greyed out.
        let mut rx = tx;
        for i in 0..N_FIGHTERS {
            let f = &m.f[i];
            let label = format!("{} {:.0}", render::NAMES[i], f.hp.max(0.0));
            draw_text(&label, rx, ty + 22.0, 18.0, if f.alive() { render::ACCENTS[i] } else { grey });
            rx += measure_text(&label, None, 18, 1.0).width + 14.0;
        }
    }

    let secs = m.tick as f32 / 60.0;
    let name = |w: usize| match (play.is_some(), w) {
        (true, PLAYER) => "YOU".to_string(),
        (true, _) => "CHAMPION".to_string(),
        _ => render::NAMES[w].to_string(),
    };
    let wins = |w: usize| if play.is_some() && w == PLAYER { "YOU WIN".to_string() } else { format!("{} WINS", name(w)) };
    let status = match (m.over, m.winner) {
        (false, _) => format!("{secs:4.1}s"),
        (true, Some(w)) if m.timed_out() => format!("{} ON HEALTH", wins(w)),
        (true, Some(w)) => wins(w),
        (true, None) => "DRAW".into(),
    };
    let size = if m.over { 40.0 } else { 22.0 };
    let tw = measure_text(&status, None, size as u16, 1.0).width;
    let sc = match (m.over, m.winner) {
        (true, Some(w)) => render::ACCENTS[w],
        _ => Color::new(0.9, 0.9, 0.92, 0.9),
    };
    draw_text(&status, ox + dw / 2.0 - tw / 2.0, if m.over { oy + dh * 0.4 } else { ty }, size, sc);

    let recent = &s.history[s.history.len().saturating_sub(300)..];

    // Strength: win rate against the fixed benchmark panel (25% = as good as the panel). Unlike
    // fitness, this is an absolute measure, so a rising line means real improvement.
    let (gw, gh) = (300.0, 70.0);
    let (gx, gy) = (ox + dw - gw - 14.0, oy + dh - gh - 30.0);
    draw_rectangle(gx, gy, gw, gh, Color::new(0.0, 0.0, 0.0, 0.35));
    let gold = Color::new(0.95, 0.85, 0.55, 1.0);
    let ymax = 0.6;
    let y_of = |v: f32| gy + gh - (v / ymax).clamp(0.0, 1.0) * gh;
    draw_line(gx, y_of(0.25), gx + gw, y_of(0.25), 1.0, Color::new(1.0, 1.0, 1.0, 0.25));
    let b = &s.bench;
    if b.len() >= 2 {
        // Smoothed over 4 benchmarks (each one is ~600 matches, about +/-2%).
        let sm: Vec<f32> = (0..b.len()).map(|i| b[i.saturating_sub(3)..=i].iter().map(|p| p.1).sum::<f32>() / (i - i.saturating_sub(3) + 1) as f32).collect();
        for i in 1..b.len() {
            let x = |i: usize| gx + i as f32 / (b.len() - 1) as f32 * gw;
            draw_line(x(i - 1), y_of(sm[i - 1]), x(i), y_of(sm[i]), 1.5, gold);
        }
    }
    let latest = b.last().map_or("-".to_string(), |p| format!("{:.0}% at gen {}", p.1 * 100.0, p.0));
    draw_text(&format!("strength vs panel: {latest}"), gx, gy - 6.0, 16.0, gold);
    draw_text(&format!("25% line = equal   autosaved at gen {}", s.last_save_gen), gx, gy + gh + 16.0, 16.0, dim);

    // How they fight: share of damage dealt by each weapon (0..100%).
    let (cx, cy) = (ox + 12.0, oy + dh - gh - 54.0);
    let shares: Vec<(fn(&evo::GenStats) -> Option<f32>, Color)> = vec![
        (|g| g.share(Source::Fist), source_color(Source::Fist)),
        (|g| g.share(Source::Stab), source_color(Source::Stab)),
        (|g| g.share(Source::SpearThrow), source_color(Source::SpearThrow)),
        (|g| g.share(Source::Rock), source_color(Source::Rock)),
        (|g| g.share(Source::Grenade), source_color(Source::Grenade)),
    ];
    chart(recent, &shares, cx, cy, gw, gh, Some(1.0));
    let mut lx = cx;
    draw_text("damage by:", lx, cy - 6.0, 16.0, dim);
    lx += measure_text("damage by:", None, 16, 1.0).width + 6.0;
    let last = s.history.last();
    for src in Source::ALL {
        let pct = last.and_then(|g| g.share(src)).map_or("-".into(), |v| format!("{:.0}%", v * 100.0));
        let label = format!("{} {pct}", src.name());
        draw_text(&label, lx, cy - 6.0, 16.0, source_color(src));
        lx += measure_text(&label, None, 16, 1.0).width + 8.0;
    }
    // Other lessons, smoothed over the last 10 generations.
    let avg10 = |f: fn(&evo::GenStats) -> Option<f32>| {
        let v: Vec<f32> = recent.iter().rev().take(10).filter_map(f).collect();
        (!v.is_empty()).then(|| v.iter().sum::<f32>() / v.len() as f32)
    };
    let pct = |v: Option<f32>| v.map_or("-".to_string(), |v| format!("{:.0}%", v * 100.0));
    let heals = evo::heals_at(s.curriculum);
    let curriculum = if s.curriculum < 1.0 { format!(" (curriculum {:.0}%)", s.curriculum * 100.0) } else { String::new() };
    draw_text(
        &format!(
            "headshots {}   heals grabbed {}   knockouts {}   heals/round {}-{}{}",
            pct(avg10(evo::GenStats::headshot_rate)),
            pct(avg10(evo::GenStats::heal_pickup_rate)),
            pct(avg10(evo::GenStats::knockout_rate)),
            heals.start(),
            heals.end(),
            curriculum
        ),
        cx, cy + gh + 16.0, 16.0, dim,
    );

    let help = if paused {
        "PAUSED  [P] resume"
    } else if play.is_some() {
        "[A/D] move  [Shift] run  [Space] jump  [R] melee  [E] grab / throw  [W/S] aim up/down  [V] stop playing"
    } else { "[1-4] speed  [P] pause  [R] new match  [C] compare generations  [D] watch 1v1s  [V] play the champion  [Tab] senses  [S] save  [Esc] save+quit" };
    draw_text(help, ox + 12.0, oy + dh - 12.0, 16.0, dim);
    if speed > 1.0 {
        draw_text(&format!("x{speed}"), ox + dw - 60.0, oy + 22.0, 18.0, dim);
    }
}

/// A small line chart of per-generation series, each smoothed over 10 generations. With
/// `max`, the y axis runs 0..max; otherwise it fits the data.
fn chart(hist: &[evo::GenStats], series: &[(fn(&evo::GenStats) -> Option<f32>, Color)], x: f32, y: f32, w: f32, h: f32, max: Option<f32>) {
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.35));
    let smoothed: Vec<Vec<f32>> = series
        .iter()
        .map(|(f, _)| {
            let raw: Vec<f32> = hist.iter().map(|g| f(g).unwrap_or(0.0)).collect();
            (0..raw.len()).map(|i| raw[i.saturating_sub(9)..=i].iter().sum::<f32>() / (i - i.saturating_sub(9) + 1) as f32).collect()
        })
        .collect();
    let n = hist.len();
    if n < 2 {
        return;
    }
    let (lo, hi) = match max {
        Some(m) => (0.0, m),
        None => smoothed.iter().flatten().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v))),
    };
    let span = (hi - lo).max(1e-3);
    let pt = |i: usize, v: f32| vec2(x + i as f32 / (n - 1) as f32 * w, y + h - ((v - lo) / span).clamp(0.0, 1.0) * h);
    for (vals, (_, color)) in smoothed.iter().zip(series) {
        for i in 1..n {
            let (a, b) = (pt(i - 1, vals[i - 1]), pt(i, vals[i]));
            draw_line(a.x, a.y, b.x, b.y, 1.5, *color);
        }
    }
}
