//! In the match: names and health, the clock, your special, and the marks drawn over the arena.

use crate::app::*;
use crate::screens::{character_name, mmss, seat_name, special_name};
use crate::settings::{Act, CONTROL_GROUPS};
use crate::text::{t, tf};
use crate::ui::{ACCENT, DIM, INK, Ui};
use go_box_render::{self as render};
use go_box_sim::sim::{DT, Match, N_FIGHTERS, TICK_RATE};
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

/// Names, health and specials (compact, top left), the clock, and the 3-2-1.
pub fn hud(ui: &Ui, app: &App, m: &Match) {
    let Some(g) = &app.game else {
        ui.text_centered(t("hud.joining"), screen_width() / 2.0, screen_height() / 2.0, 30.0, INK);
        return;
    };
    names_over_heads(ui, app, m);
    let sw = screen_width();
    // Compact, in the top-left of the arena (under half its width), so it never runs off the edge.
    let (origin, scale) = app.view;
    let arena_w = go_box_sim::sim::W * scale;
    let left = origin.x.max(0.0) + ui.px(12.0);
    let top = origin.y.max(0.0);
    let colw = arena_w * 0.46 / N_FIGHTERS as f32;
    for i in 0..N_FIGHTERS {
        let f = &m.f[i];
        if f.absent {
            continue;
        }
        let seat = &g.start.seats[i];
        let color = if f.alive() { render::look(f.character, i).accent } else { DIM };
        let x = left + i as f32 * colw;
        let you = g.seat() == Some(i);
        let name = seat_name(&seat.name, f.character, seat.player.is_none());
        let name = if you { tf("hud.you", &[("name", &name)]) } else { name };
        let bw = colw - ui.px(26.0);
        ui.text(&ui.fit(&name, 15.0, bw + ui.px(14.0)), x, top + ui.px(22.0), 15.0, color);
        let by = top + ui.px(28.0);
        draw_rectangle(x, by, bw, ui.px(4.0), Color::new(0.0, 0.0, 0.0, 0.6));
        draw_rectangle(x, by, bw * (f.hp.max(0.0) / go_box_sim::sim::MAX_HP), ui.px(4.0), color);
        // Their special: a little clock filling up as it recharges, solid when it's ready.
        if let Some(sp) = f.special().filter(|_| f.alive()) {
            let ready = 1.0 - f.special_cd as f32 / sp.cooldown() as f32;
            special_pie(vec2(x + bw + ui.px(11.0), by + ui.px(2.0)), ui.px(7.0), ready, render::character_color(f.character));
        }
        // Yours: the special's recharge bar and what it says, just under your health. (The Chosen One inside
        // someone: theirs, which is the one his key fires.)
        let own = f.host.filter(|_| f.digital > 0).map_or(f, |j| &m.f[j as usize]);
        if you && f.alive()
            && let Some(sp) = own.special()
        {
            let c = render::character_color(own.character);
            let ready = own.special_cd == 0;
            let frac = 1.0 - own.special_cd as f32 / sp.cooldown() as f32;
            let sy = by + ui.px(8.0);
            draw_rectangle(x, sy, bw, ui.px(3.0), Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(x, sy, bw * frac, ui.px(3.0), if ready { c } else { Color::new(c.r * 0.5, c.g * 0.5, c.b * 0.5, 1.0) });
            let key = app.settings.bindings.short_label(Act::Special, app.controls.using_pad);
            let secs = format!("{:.0}", (own.special_cd as f32 * DT).ceil());
            let label = if ready { tf("hud.special_ready", &[("key", &key), ("special", special_name(sp))]) } else { tf("hud.special_cooling", &[("special", special_name(sp)), ("secs", &secs)]) };
            ui.text(&ui.fit(&label, 13.0, colw * 1.6), x, sy + ui.px(15.0), 13.0, if ready { c } else { DIM });
        }
        // Ryan (Hobbyist): which special he picked up this match, under his column.
        if let Some(sp) = f.special().filter(|_| f.hobby.is_some()) {
            let hy = by + if you { ui.px(39.0) } else { ui.px(18.0) };
            let line = tf("hud.hobby", &[("special", special_name(sp))]);
            ui.text(&ui.fit(&line, 13.0, colw * 1.6), x, hy, 13.0, color);
        }
    }
    // As the match starts, if you're Ryan: what you've picked up, big, so you know before you use it.
    if let Some(s) = g.seat()
        && let Some(sp) = m.f[s].special().filter(|_| m.f[s].hobby.is_some())
        && m.play_ticks() < 3 * TICK_RATE
    {
        let (sw, sh) = (screen_width(), screen_height());
        let c = render::character_color(m.f[s].character);
        let line = tf("hud.hobby_banner", &[("special", special_name(sp))]);
        let y = sh * 0.24;
        let o = ui.px(2.0);
        for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
            ui.text_centered(&line, sw / 2.0 + dx, y + dy, 30.0, Color::new(0.0, 0.0, 0.0, 0.9));
        }
        ui.text_centered(&line, sw / 2.0, y, 30.0, c);
        let sub = t("hud.hobby_sub");
        ui.text_centered(sub, sw / 2.0, y + ui.px(26.0), 16.0, DIM);
    }
    // Whole seconds left, counted in ticks (float maths made 45s show as 0:46).
    let left = m.config.max_ticks.saturating_sub(m.play_ticks()).div_ceil(TICK_RATE);
    if !m.counting_down() && (1..=render::sound::ENDING_SECS).contains(&left) {
        // The last seconds: the clock swells and flashes red on each tick, then settles.
        // How much of this second is still to run (1 just after the tick, 0 just before the next).
        let ticks_left = m.config.max_ticks - m.play_ticks();
        let rest = ((ticks_left + TICK_RATE - 1) % TICK_RATE) as f32 / TICK_RATE as f32;
        let pulse = ((rest - 0.67) / 0.33).max(0.0);
        let color = Color::new(1.0, 0.35 + 0.4 * (1.0 - pulse), 0.30 + 0.4 * (1.0 - pulse), 1.0);
        ui.text_centered_scaled(&mmss(left as f32), sw / 2.0, ui.px(72.0 + 10.0 * pulse), 34.0, 1.0 + 26.0 / 34.0 * pulse, color);
    } else {
        ui.text_centered(&mmss(left as f32), sw / 2.0, ui.px(70.0), 28.0, INK);
    }

    // 3, 2, 1 in the middle of the screen, then GO! for a moment.
    let sh = screen_height();
    if m.counting_down() {
        let n = (m.config.countdown - m.tick).div_ceil(TICK_RATE);
        let pulse = ((m.config.countdown - m.tick) % TICK_RATE) as f32 / TICK_RATE as f32;
        ui.text_centered_scaled(&n.to_string(), sw / 2.0, sh * 0.45, 90.0, 1.0 + 40.0 / 90.0 * pulse, ACCENT);
    } else if m.play_ticks() < TICK_RATE * 2 / 3 {
        ui.text_centered(t("hud.go"), sw / 2.0, sh * 0.45, 100.0, ACCENT);
    }

    match g.seat() {
        None => {
            // Past the seats in the room: spectating, waiting for one to free up. Otherwise you just
            // joined mid-match and play in the next.
            let place = app.room.as_ref().and_then(|r| r.players.iter().position(|p| Some(p.id) == app.me));
            let msg = if place.is_some_and(|k| k >= go_box_protocol::MAX_PLAYERS) { t("hud.spectating") } else { t("hud.watching") };
            ui.text_centered(msg, sw / 2.0, sh - ui.px(24.0), 24.0, ACCENT)
        }
        Some(s) => {
            let f = &m.f[s];
            if !f.alive() {
                ui.text_centered(t("hud.down"), sw / 2.0, sh - ui.px(24.0), 24.0, DIM);
            } else if f.special().is_some() {
                // (Your special's bar is up by your health, top left.)
            } else {
                ui.text_centered(&app.controls_help(), sw / 2.0, sh - ui.px(16.0), 18.0, DIM);
            }
        }
    }
}

/// The protected start: "Invulnerable", centred on `at` (the text's baseline), with a pie running
/// down until it ends. Over every fighter (yours a bit bigger).
fn invulnerable(ui: &Ui, m: &Match, at: Vec2, size: f32) {
    if m.counting_down() || !m.protected() {
        return;
    }
    let left = (m.config.countdown + m.config.spawn_grace - m.tick) as f32 / m.config.spawn_grace.max(1) as f32;
    let shield = Color::new(0.55, 0.85, 1.0, 1.0);
    let label = t("hud.invulnerable");
    let r = ui.px(size * 0.38);
    let gap = ui.px(size * 0.38);
    let lw = ui.text_width(label, size);
    let x0 = at.x - (lw + gap + r * 2.0) / 2.0;
    let o = ui.px(1.0);
    for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
        ui.text(label, x0 + dx, at.y + dy, size, Color::new(0.0, 0.0, 0.0, 0.8));
    }
    ui.text(label, x0, at.y, size, shield);
    special_pie(vec2(x0 + lw + gap + r, at.y - ui.px(size * 0.3)), r, left.min(0.999), shield);
}

/// A recharge clock: an outlined circle filling clockwise from 12 o'clock with `ready` (0..1), and
/// a solid disc (with a soft halo) once it's full.
fn special_pie(c: Vec2, r: f32, ready: f32, color: Color) {
    // Outline and glow scale with the size (a tiny clock shouldn't be all border).
    let line = (r * 0.18).clamp(1.0, 2.0);
    let halo = (r * 0.2).max(1.0);
    if ready >= 1.0 {
        draw_circle(c.x, c.y, r + halo, Color::new(color.r, color.g, color.b, 0.25));
        draw_circle(c.x, c.y, r, color);
        draw_circle_lines(c.x, c.y, r, line * 0.75, Color::new(1.0, 1.0, 1.0, 0.8));
        return;
    }
    draw_circle(c.x, c.y, r + halo * 0.75, Color::new(0.0, 0.0, 0.0, 0.75));
    draw_circle(c.x, c.y, r, Color::new(0.10, 0.10, 0.12, 0.9));
    let fill = Color::new(color.r * 0.85, color.g * 0.85, color.b * 0.85, 1.0);
    let steps = 32;
    let end = ready.clamp(0.0, 1.0) * std::f32::consts::TAU;
    let at = |a: f32| c + vec2(a.sin(), -a.cos()) * r;
    for k in 0..steps {
        let (a0, a1) = (end * k as f32 / steps as f32, end * (k + 1) as f32 / steps as f32);
        draw_triangle(c, at(a0), at(a1), fill);
    }
    draw_circle_lines(c.x, c.y, r, line, color);
}

/// Everyone's name over their head. Others' are small and faded so they don't pull the eye;
/// yours is bigger, in your colour and outlined, sitting above your arrow, so you always know
/// which one is you.
fn names_over_heads(ui: &Ui, app: &App, m: &Match) {
    let Some(g) = &app.game else { return };
    let (origin, scale) = app.view;
    let to_screen = |p: Vec2| origin + p * scale;
    // Michelle's freeze: a pie in its colour beside each frozen fighter's feet, running down until
    // they're free.
    for f in m.f.iter().filter(|f| f.pinned > 0 && f.present()) {
        let left = f.pinned as f32 / go_box_sim::character::PIN_TICKS as f32;
        special_pie(to_screen(f.pos + vec2(13.0 * f.scale, -3.0)), ui.px(4.0), left.min(0.999), Color::new(0.66, 0.62, 1.0, 1.0));
    }
    for i in 0..N_FIGHTERS {
        let f = &m.f[i];
        if !f.alive() {
            continue;
        }
        // A small recharge clock just right of the health bar floating over them.
        if let Some(sp) = f.special().filter(|_| f.present()) {
            let ready = 1.0 - f.special_cd as f32 / sp.cooldown() as f32;
            special_pie(to_screen(f.pos + vec2(8.5, -21.5)), ui.px(3.5), ready, render::character_color(f.character));
        }
        let seat = &g.start.seats[i];
        let name = seat_name(&seat.name, f.character, seat.player.is_none());
        if g.seat() == Some(i) {
            // Your name, then (small and quiet) your character, above the arrow (which sits above
            // the health bar).
            let p = to_screen(f.pos - vec2(0.0, 38.0));
            let pc = to_screen(f.pos - vec2(0.0, 31.0));
            // Your colour, lifted toward white so even a dark one (Sam's green) reads on the dark arena.
            let a = render::look(f.character, i).accent;
            let color = Color::new(a.r + (1.0 - a.r) * 0.45, a.g + (1.0 - a.g) * 0.45, a.b + (1.0 - a.b) * 0.45, 1.0);
            let o = ui.px(2.0);
            for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (-o, -o), (o, o), (-o, o), (o, -o)] {
                ui.text_centered(&name, p.x + dx, p.y + dy, 26.0, Color::new(0.0, 0.0, 0.0, 0.9));
            }
            ui.text_centered(&name, p.x, p.y, 26.0, color);
            let who = character_name(f.character);
            let o = ui.px(1.0);
            for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o)] {
                ui.text_centered(who, pc.x + dx, pc.y + dy, 13.0, Color::new(0.0, 0.0, 0.0, 0.7));
            }
            ui.text_centered(who, pc.x, pc.y, 13.0, Color::new(0.80, 0.81, 0.84, 0.75));
            invulnerable(ui, m, vec2(p.x, p.y - ui.px(26.0)), 16.0);
        } else {
            let p = to_screen(f.pos - vec2(0.0, 25.0));
            ui.text_centered(&name, p.x, p.y, 14.0, Color::new(0.85, 0.86, 0.88, 0.5));
            invulnerable(ui, m, vec2(p.x, p.y - ui.px(16.0)), 12.0);
        }
    }
}

/// The controls overlay's header (clicking it opens and closes the overlay): where it is, so a
/// click there isn't also a grab in the game.
pub fn controls_header(ui: &Ui) -> Rect {
    let w = ui.px(280.0);
    Rect::new(screen_width() - w - ui.px(8.0), ui.px(60.0), w, ui.px(24.0))
}

/// In a match: the controls, in the same groups and order as on the Controls page (movement,
/// then actions), with what each is bound to on what you're playing with; or, collapsed, just
/// its header. Returns where it ends (the debug overlay goes under it). Not `collapsible` (on the
/// home screen), it's always open.
pub fn controls_overlay(ui: &mut Ui, app: &mut App, collapsible: bool) -> f32 {
    let head = controls_header(ui);
    let open = app.settings.controls_overlay || !collapsible;
    let pad_side = app.controls.using_pad;
    let groups = &CONTROL_GROUPS[..2];
    let line = ui.px(17.0);
    let rows: usize = if open { groups.iter().map(|(_, acts)| acts.len() + 1).sum() } else { 0 };
    let h = head.h + rows as f32 * line + if open { ui.px(8.0) } else { 0.0 };
    draw_rectangle(head.x, head.y, head.w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    let title = if !collapsible { t("controls.title").to_string() } else { format!("{}  {}", t("controls.title"), if open { "[-]" } else { "[+]" }) };
    ui.text(&title, head.x + ui.px(8.0), head.y + ui.px(17.0), 15.0, ACCENT);
    let (clicked, _) = ui.area_hover(head);
    if clicked && collapsible {
        app.settings.controls_overlay = !open;
        app.settings.save();
    }
    if open {
        let mut y = head.y + head.h - ui.px(4.0);
        for (group, acts) in groups {
            y += line;
            ui.text(t(group), head.x + ui.px(8.0), y, 14.0, DIM);
            for &act in *acts {
                y += line;
                ui.text(act.label(), head.x + ui.px(16.0), y, 14.0, INK);
                let keys = ui.fit(&app.settings.bindings.label(act, pad_side), 14.0, head.w * 0.42);
                ui.text(&keys, head.x + head.w - ui.px(8.0) - ui.text_width(&keys, 14.0), y, 14.0, ACCENT);
            }
        }
    }
    head.y + h
}
