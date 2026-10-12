mod brain;
mod evo;
mod render;
mod sim;

use macroquad::prelude::*;
use ::rand::rngs::SmallRng;
use ::rand::SeedableRng;
use sim::{random_kinds, Kind, Match, Tribe, DT, H, W};

/// A match under the heal rules training is currently using; `showcase` fields one of each kind.
fn new_match(rng: &mut SmallRng, curriculum: f32, showcase: bool) -> Match {
    let kinds = if showcase { Kind::ALL } else { random_kinds(rng) };
    Match::new(rng, kinds, evo::heals_at(curriculum))
}

const USAGE: &str = "box-arena [--fresh] [--headless <generations>] [--screenshot <prefix.png>]";

fn conf() -> Conf {
    Conf { window_title: "Box Arena".into(), window_width: 1280, window_height: 720, ..Default::default() }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fresh = args.iter().any(|a| a == "--fresh");
    let value_of = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    if let Some(n) = value_of("--headless") {
        evo::headless(n.parse().unwrap_or(100), fresh);
        return;
    }
    let screenshot = value_of("--screenshot");
    macroquad::Window::from_config(conf(), viewer(fresh, screenshot));
}

async fn viewer(fresh: bool, screenshot: Option<String>) {
    prevent_quit();
    let (shared, trainer) = evo::spawn(fresh);
    let rt = render_target(W as u32, H as u32);
    rt.texture.set_filter(FilterMode::Nearest);
    let mut cam = Camera2D { target: vec2(W / 2.0, H / 2.0), zoom: vec2(2.0 / W, 2.0 / H), ..Default::default() };
    cam.render_target = Some(rt.clone());

    let mut art = render::Art::new();
    let mut rng = SmallRng::from_entropy();
    let snapshot = || {
        let s = shared.lock().unwrap();
        (s.champs.clone(), s.generation, s.curriculum)
    };
    let (mut champs, mut match_gen, mut curriculum) = snapshot();
    let mut m = new_match(&mut rng, curriculum, false);
    let (mut acc, mut end_timer, mut frames) = (0.0f32, 0.0f32, 0u32);
    let mut speed = 1.0f32;
    let mut debug = false;
    let mut paused = false;

    loop {
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
        if is_key_pressed(KeyCode::S) {
            shared.lock().unwrap().save_now = true;
        }
        for (k, v) in [(KeyCode::Key1, 1.0), (KeyCode::Key2, 2.0), (KeyCode::Key3, 4.0), (KeyCode::Key4, 8.0)] {
            if is_key_pressed(k) {
                speed = v;
            }
        }
        // [V] (and screenshot mode) shows one of each kind; otherwise rosters are random like in training.
        let showcase = is_key_pressed(KeyCode::V) || (screenshot.is_some() && frames == 0);
        let restart = showcase || is_key_pressed(KeyCode::R) || (m.over && end_timer > 2.5);
        if restart {
            (champs, match_gen, curriculum) = snapshot();
            m = new_match(&mut rng, curriculum, showcase);
            art.reset();
            end_timer = 0.0;
            acc = 0.0;
        }

        let dt = if paused { 0.0 } else { get_frame_time().min(0.1) * speed };
        if m.over {
            end_timer += dt / speed.max(1.0);
        } else {
            acc += dt;
            while acc >= DT && !m.over {
                m.step(std::array::from_fn(|i| &champs[m.f[i].kind.index()]));
                art.handle_events(&m);
                acc -= DT;
            }
        }
        art.update(dt, &m);

        set_camera(&cam);
        clear_background(BLACK);
        art.draw(&m, debug);

        set_default_camera();
        clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
        let scale = (screen_width() / W).min(screen_height() / H).floor().max(1.0);
        let (dw, dh) = (W * scale, H * scale);
        let (ox, oy) = (((screen_width() - dw) / 2.0).floor(), ((screen_height() - dh) / 2.0).floor());
        draw_texture_ex(&rt.texture, ox, oy, WHITE, DrawTextureParams { dest_size: Some(vec2(dw, dh)), ..Default::default() });
        draw_hud(&shared.lock().unwrap(), &m, match_gen, speed, paused, ox, oy, dw, dh);

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

#[allow(clippy::too_many_arguments)]
fn draw_hud(s: &evo::Shared, m: &Match, match_gen: u32, speed: f32, paused: bool, ox: f32, oy: f32, dw: f32, dh: f32) {
    let dim = Color::new(0.72, 0.74, 0.78, 0.85);
    let (tx, ty) = (ox + 12.0, oy + 22.0);
    draw_text(
        &format!("GEN {}   {:.1} gen/s   {} threads   watching gen {} champions", s.generation, s.gens_per_sec, evo::training_threads(), match_gen),
        tx, ty, 20.0, dim,
    );
    if let Some(last) = s.history.last() {
        draw_text(
            &format!("last gen  humans won {}   creatures won {}   draws {}", last.h_wins, last.c_wins, last.draws),
            tx, ty + 20.0, 18.0, dim,
        );
    }
    // Roster for the match on screen, coloured by kind.
    let mut rx = tx;
    for (i, f) in m.f.iter().enumerate() {
        let label = if i == 2 { format!("vs  {}", f.kind.name()) } else { f.kind.name().to_string() };
        let c = if f.alive() { render::accent(f.kind) } else { Color::new(0.4, 0.4, 0.42, 0.8) };
        draw_text(&label, rx, ty + 40.0, 18.0, c);
        rx += measure_text(&label, None, 18, 1.0).width + 12.0;
    }

    let secs = m.tick as f32 / 60.0;
    let status = match (m.over, m.winner) {
        (false, _) => format!("{secs:4.1}s"),
        (true, Some(Tribe::Humans)) => if m.timed_out() { "HUMANS WIN ON HEALTH".into() } else { "HUMANS WIN".into() },
        (true, Some(Tribe::Creatures)) => if m.timed_out() { "CREATURES WIN ON HEALTH".into() } else { "CREATURES WIN".into() },
        (true, None) => "DRAW".into(),
    };
    let size = if m.over { 40.0 } else { 22.0 };
    let tw = measure_text(&status, None, size as u16, 1.0).width;
    draw_text(&status, ox + dw / 2.0 - tw / 2.0, if m.over { oy + dh * 0.4 } else { ty }, size, Color::new(0.9, 0.9, 0.92, 0.9));

    // Fitness sparkline (average fitness per generation for each kind, last 300 gens).
    let (gw, gh) = (300.0, 70.0);
    let (gx, gy) = (ox + dw - gw - 14.0, oy + dh - gh - 30.0);
    draw_rectangle(gx, gy, gw, gh, Color::new(0.0, 0.0, 0.0, 0.35));
    let hist = &s.history[s.history.len().saturating_sub(300)..];
    if hist.len() > 1 {
        let (lo, hi) = hist.iter().flat_map(|g| g.avg).fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let span = (hi - lo).max(1.0);
        let pt = |i: usize, v: f32| vec2(gx + i as f32 / (hist.len() - 1) as f32 * gw, gy + gh - (v - lo) / span * gh);
        for kind in Kind::ALL {
            let k = kind.index();
            for i in 1..hist.len() {
                let (a, b) = (pt(i - 1, hist[i - 1].avg[k]), pt(i, hist[i].avg[k]));
                draw_line(a.x, a.y, b.x, b.y, 1.5, render::accent(kind));
            }
        }
    }
    let mut lx = gx;
    draw_text("avg fitness:", lx, gy - 6.0, 16.0, dim);
    lx += measure_text("avg fitness:", None, 16, 1.0).width + 8.0;
    for kind in Kind::ALL {
        draw_text(kind.name(), lx, gy - 6.0, 16.0, render::accent(kind));
        lx += measure_text(kind.name(), None, 16, 1.0).width + 8.0;
    }
    // Lessons: is each behaviour actually being learned? Smoothed trends over recent generations.
    let recent = &s.history[s.history.len().saturating_sub(300)..];
    let lessons: [(&str, &str, fn(&evo::GenStats) -> Option<f32>, Color); 3] = [
        ("friendly fire", "want low", evo::GenStats::friendly_fire, Color::new(0.95, 0.45, 0.35, 1.0)),
        ("heals grabbed", "want high", evo::GenStats::heal_pickup_rate, Color::new(0.38, 0.86, 1.0, 1.0)),
        ("wins by wipe-out", "rest on health", evo::GenStats::wipeout_rate, Color::new(0.85, 0.82, 0.6, 1.0)),
    ];
    let (lx, mut ly) = (ox + 12.0, oy + dh - 150.0);
    let heals = evo::heals_at(s.curriculum);
    let (lo, hi) = (heals.start(), heals.end());
    let curriculum = if s.curriculum < 1.0 {
        format!("heals per round {lo}-{hi}  (curriculum {:.0}%, fading to 1-2)", s.curriculum * 100.0)
    } else {
        format!("heals per round {lo}-{hi}")
    };
    draw_text(&curriculum, lx, ly - 6.0, 16.0, dim);
    for (label, hint, metric, color) in lessons {
        let values: Vec<f32> = recent.iter().filter_map(metric).collect();
        draw_lesson(label, hint, &values, lx, ly, color, dim);
        ly += 28.0;
    }

    let saved = format!("autosaved at gen {}", s.last_save_gen);
    draw_text(&saved, gx, gy + gh + 16.0, 16.0, dim);

    let help = if paused { "PAUSED  [P] resume" } else { "[1-4] speed  [P] pause  [R] new match  [V] one of each  [Tab] senses  [S] save  [Esc] save+quit" };
    draw_text(help, ox + 12.0, oy + dh - 12.0, 16.0, dim);
    if speed > 1.0 {
        draw_text(&format!("x{speed}"), ox + 12.0, oy + dh - 32.0, 18.0, dim);
    }
}

/// One lesson trend: a 10-generation moving average drawn as a sparkline (0..100%), plus its
/// current value.
#[allow(clippy::too_many_arguments)]
fn draw_lesson(label: &str, hint: &str, values: &[f32], x: f32, y: f32, color: Color, dim: Color) {
    let (w, h) = (150.0, 22.0);
    draw_rectangle(x, y, w, h, Color::new(0.0, 0.0, 0.0, 0.35));
    let smooth: Vec<f32> = (0..values.len())
        .map(|i| {
            let from = i.saturating_sub(9);
            values[from..=i].iter().sum::<f32>() / (i - from + 1) as f32
        })
        .collect();
    let pt = |i: usize, v: f32| vec2(x + i as f32 / (smooth.len() - 1) as f32 * w, y + h - v.clamp(0.0, 1.0) * h);
    for i in 1..smooth.len() {
        let (a, b) = (pt(i - 1, smooth[i - 1]), pt(i, smooth[i]));
        draw_line(a.x, a.y, b.x, b.y, 1.5, color);
    }
    let value = smooth.last().map_or("--".to_string(), |v| format!("{:.0}%", v * 100.0));
    draw_text(&format!("{label} {value}"), x + w + 8.0, y + 12.0, 16.0, color);
    draw_text(hint, x + w + 8.0, y + 24.0, 14.0, dim);
}
