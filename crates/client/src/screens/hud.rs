//! In the match: names and health, the clock, your special, and the marks drawn over the arena.

use crate::app::*;
use crate::screens::mmss;
use crate::ui::{ACCENT, DIM, INK, Ui};
use go_box_render::{self as render};
use go_box_sim::sim::{DT, MAX_TICKS, Match, N_FIGHTERS};
use macroquad::prelude::*;

/// Dots showing where a throw would go.
pub fn draw_aim(m: &Match, seat: usize, target: Option<Vec2>) {
    let f = &m.f[seat];
    if f.alive() && f.holding.is_some() {
        let (o, d) = (f.hand(), target.unwrap_or_else(|| f.aim_dir(0.3)));
        for k in 1..6 {
            let p = o + d * (k as f32 * 6.0);
            draw_rectangle(p.x.floor(), p.y.floor(), 1.0, 1.0, Color::new(1.0, 1.0, 1.0, 0.5 - k as f32 * 0.07));
        }
    }
}

/// A bobbing chevron over your own fighter, above the health bar, so you can always find
/// yourself.
pub fn draw_you(m: &Match, seat: usize) {
    let f = &m.f[seat];
    if !f.alive() {
        return;
    }
    let c = render::look(f.character, seat).accent;
    let bob = ((get_time() * 4.0).sin() * 1.0).round() as f32;
    let (x, y) = (f.pos.x.round(), f.pos.y.round() - 29.0 + bob);
    // Rows of a 7px-wide downward chevron, with a dark outline so it reads on any backdrop.
    let rows: [(f32, f32); 4] = [(-3.0, 7.0), (-2.0, 5.0), (-1.0, 3.0), (0.0, 1.0)];
    let shadow = Color::new(0.0, 0.0, 0.0, 0.7);
    for (k, &(dx, w)) in rows.iter().enumerate() {
        draw_rectangle(x + dx - 1.0, y + k as f32, w + 2.0, 1.0, shadow);
    }
    draw_rectangle(x - 1.0, y + 4.0, 3.0, 1.0, shadow);
    for (k, &(dx, w)) in rows.iter().enumerate() {
        draw_rectangle(x + dx, y + k as f32, w, 1.0, c);
    }
}

/// Names and health across the top, the clock, and your special at the bottom.
pub fn hud(ui: &Ui, app: &App, m: &Match) {
    let Some(g) = &app.game else {
        ui.text_centered("Joining...", screen_width() / 2.0, screen_height() / 2.0, 30.0, INK);
        return;
    };
    let sw = screen_width();
    let colw = (sw - ui.px(32.0)) / N_FIGHTERS as f32;
    for i in 0..N_FIGHTERS {
        let f = &m.f[i];
        if f.absent {
            continue;
        }
        let seat = &g.start.seats[i];
        let color = if f.alive() { render::look(f.character, i).accent } else { DIM };
        let x = ui.px(16.0) + i as f32 * colw;
        let you = g.seat() == Some(i);
        let name = if you { format!("{} (you)", seat.name) } else { seat.name.clone() };
        ui.text(&name, x, ui.px(28.0), 20.0, color);
        let bw = colw - ui.px(24.0);
        draw_rectangle(x, ui.px(36.0), bw, ui.px(6.0), Color::new(0.0, 0.0, 0.0, 0.6));
        draw_rectangle(x, ui.px(36.0), bw * (f.hp.max(0.0) / go_box_sim::sim::MAX_HP), ui.px(6.0), color);
    }
    let left = (MAX_TICKS.saturating_sub(m.tick)) as f32 * DT;
    ui.text_centered(&mmss(left.ceil()), sw / 2.0, ui.px(70.0), 28.0, INK);

    let sh = screen_height();
    match g.seat() {
        None => ui.text_centered("Watching - you'll play in the next match", sw / 2.0, sh - ui.px(24.0), 24.0, ACCENT),
        Some(s) => {
            let f = &m.f[s];
            if !f.alive() {
                ui.text_centered("You're down - watch the rest", sw / 2.0, sh - ui.px(24.0), 24.0, DIM);
            } else if let Some(sp) = f.character.special() {
                let color = render::character_color(f.character);
                let w = ui.px(260.0);
                let x = (sw - w) / 2.0;
                let y = sh - ui.px(46.0);
                let ready = f.special_cd == 0;
                let frac = 1.0 - f.special_cd as f32 / sp.cooldown() as f32;
                draw_rectangle(x, y, w, ui.px(10.0), Color::new(0.0, 0.0, 0.0, 0.6));
                draw_rectangle(x, y, w * frac, ui.px(10.0), if ready { color } else { Color::new(color.r * 0.5, color.g * 0.5, color.b * 0.5, 1.0) });
                let label = if ready { format!("[{}] {} - ready", app.controls.special_key(), sp.name()) } else { format!("{} - {:.0}s", sp.name(), (f.special_cd as f32 * DT).ceil()) };
                ui.text_centered(&label, sw / 2.0, y + ui.px(32.0), 20.0, if ready { color } else { DIM });
            } else {
                ui.text_centered(app.controls_help(), sw / 2.0, sh - ui.px(16.0), 18.0, DIM);
            }
        }
    }
}
