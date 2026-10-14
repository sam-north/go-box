//! The room: who's in it, seats, the map, characters, and the last match's scoreboard.

use crate::app::*;
use crate::screens::{character_name, mmss, seat_name, special_name};
use crate::text::{t, tf};
use crate::platform;
use crate::ui::{self, ACCENT, BAD, DIM, INK, Ui};
use go_box_protocol::*;
use go_box_render::{self as render};
use go_box_sim::character::Character;
use go_box_sim::sim::{ArenaId, TICK_RATE};
use macroquad::prelude::*;

/// One character up close: the whole figure, its name, and what its special does.
fn preview(ui: &Ui, r: Rect, c: Character, picked: bool, portraits: &[RenderTarget]) {
    let color = render::character_color(c);
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(color.r * 0.10, color.g * 0.10, color.b * 0.10, 0.9));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 1.0, ui::EDGE);
    let k = Character::ALL.iter().position(|&x| x == c).unwrap_or(0);
    // The figure, cropped to its body so it fills the box.
    let src = Rect::new(4.0, 3.0, 18.0, 27.0);
    // (Kept smallish, so the description gets the width.)
    let ph = (r.h - ui.px(20.0)).min(ui.px(120.0));
    let pw = ph * src.w / src.h;
    draw_texture_ex(&portraits[k].texture, r.x + ui.px(12.0), r.y + (r.h - ph) / 2.0, WHITE, DrawTextureParams { dest_size: Some(vec2(pw, ph)), source: Some(src), ..Default::default() });
    let tx = r.x + pw + ui.px(28.0);
    let tw = r.x + r.w - tx - ui.px(10.0);
    ui.text(character_name(c), tx, r.y + ui.px(34.0), 30.0, color);
    if picked {
        let p = t("lobby.picked");
        ui.text(p, r.x + r.w - ui.text_width(p, 15.0) - ui.px(10.0), r.y + ui.px(22.0), 15.0, color);
    }
    let heading = match c.special() {
        Some(s) => format!("{}  -  {}", special_name(s), tf("lobby.cooldown", &[("secs", &(s.cooldown() / TICK_RATE).to_string())])),
        None => t("lobby.no_special").to_string(),
    };
    ui.text(&ui.fit(&heading, 18.0, tw), tx, r.y + ui.px(60.0), 18.0, INK);
    // Up to `text::ABOUT_MAX` characters fit (a test holds the text files to it).
    let mut ly = r.y + ui.px(82.0);
    let about: String = t(&format!("about.{}", c.id())).chars().take(crate::text::ABOUT_MAX).collect();
    for line in ui.wrap(&about, 15.0, tw) {
        ui.text(&line, tx, ly, 15.0, DIM);
        ly += ui.px(17.0);
    }
}

pub fn lobby_screen(ui: &mut Ui, app: &mut App, room: &RoomInfo, portraits: &[RenderTarget]) {
    let (sw, sh) = (screen_width(), screen_height());
    let w = ui.px(860.0).min(sw - 32.0);
    let results_h = if room.last_results.is_some() { ui.px(208.0) } else { 0.0 };
    let h = (ui.px(560.0) + results_h).min(sh - 24.0);
    let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
    ui.panel(r);
    let pad = ui.px(24.0);
    let leader = app.me == Some(room.leader);
    let mut y = r.y + ui.px(44.0);

    // Header: room code, invite link.
    ui.text(&tf("lobby.room", &[("code", &room.code)]), r.x + pad, y, 36.0, ACCENT);
    let bw = ui.px(170.0);
    if ui.button(Rect::new(r.x + w - pad - bw, y - ui.px(30.0), bw, ui.px(38.0)), t("lobby.copy_link"), true, ACCENT) {
        platform::copy(&platform::share_link(&room.code));
        app.notify(t("lobby.link_copied"));
    }
    y += ui.px(20.0);

    // Last match's scoreboard.
    if let Some(res) = &room.last_results {
        y += ui.px(8.0);
        let banner = match res.winner {
            Some(w) => tf("lobby.wins", &[("name", &seat_name(&res.seats[w].name, res.seats[w].character, res.seats[w].bot))]),
            None => t("lobby.draw").to_string(),
        };
        ui.text(&banner, r.x + pad, y + ui.px(10.0), 30.0, INK);
        let length = tf("lobby.match_length", &[("time", &mmss(res.duration))]);
        ui.text(&length, r.x + w - pad - ui.text_width(&length, 18.0), y + ui.px(10.0), 18.0, DIM);
        y += ui.px(30.0);
        // (The place, 1st..4th, sits in front of the first column.)
        let cols = [0.07, 0.32, 0.48, 0.58, 0.70, 0.82];
        let heads = [t("lobby.col_player"), t("lobby.col_character"), t("lobby.col_kills"), t("lobby.col_headshots"), t("lobby.col_damage"), t("lobby.col_down")];
        let tw = w - pad * 2.0;
        for (k, head) in heads.iter().enumerate() {
            ui.text(head, r.x + pad + cols[k] * tw, y, 18.0, DIM);
        }
        y += ui.px(8.0);
        // First to last, as the win is decided: the winner; then survivors by the health they ended
        // on, and those who went down by how long they lasted; then kills, damage dealt, headshots.
        let mut order: Vec<usize> = (0..res.seats.len()).collect();
        order.sort_by(|&ia, &ib| {
            let (a, b) = (&res.seats[ia], &res.seats[ib]);
            let won = |i: usize| res.winner == Some(i);
            let lasted = |s: &SeatResult| s.died_at.unwrap_or(f32::INFINITY);
            let left = |s: &SeatResult| if s.died_at.is_none() { s.hp } else { 0.0 };
            won(ib)
                .cmp(&won(ia))
                .then(lasted(b).total_cmp(&lasted(a)))
                .then(left(b).total_cmp(&left(a)))
                .then(b.kills.cmp(&a.kills))
                .then(b.damage_dealt.total_cmp(&a.damage_dealt))
                .then(b.headshots.cmp(&a.headshots))
        });
        for (place, i) in order.into_iter().enumerate() {
            let s = &res.seats[i];
            y += ui.px(23.0);
            let color = render::look(s.character, i).accent;
            if res.winner == Some(i) {
                draw_rectangle(r.x + pad - ui.px(6.0), y - ui.px(18.0), tw + ui.px(12.0), ui.px(23.0), Color::new(color.r, color.g, color.b, 0.18));
            }
            let fate = match s.died_at {
                Some(t) => mmss(t),
                None => tf("lobby.survived", &[("hp", &format!("{:.0}", s.hp))]),
            };
            let cells = [seat_name(&s.name, s.character, s.bot), character_name(s.character).to_string(), s.kills.to_string(), s.headshots.to_string(), format!("{:.0}", s.damage_dealt), fate];
            let places = [t("lobby.place_1"), t("lobby.place_2"), t("lobby.place_3"), t("lobby.place_4")];
            if let Some(p) = places.get(place) {
                ui.text(p, r.x + pad, y, 20.0, if place == 0 { ACCENT } else { DIM });
            }
            for (k, cell) in cells.iter().enumerate() {
                ui.text(cell, r.x + pad + cols[k] * tw, y, 20.0, if k == 0 { color } else { INK });
            }
        }
        y += ui.px(18.0);
        draw_line(r.x + pad, y, r.x + w - pad, y, 1.0, ui::EDGE);
    }

    // Who's here.
    y += ui.px(34.0);
    ui.text(t("lobby.players"), r.x + pad, y, 20.0, DIM);
    let map_x = r.x + w * 0.55;
    ui.text(t("lobby.arena"), map_x, y, 20.0, DIM);
    // The first MAX_PLAYERS have seats; anyone after them is spectating (listed by the map).
    let seated = &room.players[..room.players.len().min(MAX_PLAYERS)];
    let watching = &room.players[seated.len()..];
    let lines: Vec<String> = seated
        .iter()
        .map(|p| {
            let mut line = p.name.clone();
            if p.id == room.leader {
                line.push_str(&format!("  {}", t("lobby.leader_tag")));
            }
            if Some(p.id) == app.me {
                line.push_str(&format!("  {}", t("lobby.you_tag")));
            }
            line
        })
        .collect();
    // The character column sits just past the longest name line.
    let char_x = r.x + pad + lines.iter().map(|l| ui.text_width(l, 22.0)).fold(ui.px(200.0), f32::max) + ui.px(24.0);
    let mut py = y;
    for (i, (p, line)) in seated.iter().zip(&lines).enumerate() {
        py += ui.px(28.0);
        let color = render::look(p.character, i).accent;
        ui.text(line, r.x + pad, py, 22.0, color);
        let status = if p.ready && p.id != room.leader { t("lobby.ready_tag") } else { "" };
        ui.text(&format!("{}  {status}", character_name(p.character)), char_x, py, 20.0, if p.ready { ACCENT } else { DIM });
    }
    // Seats without a player: a bot or nobody. The leader picks; down to just you.
    for seat in seated.len()..MAX_PLAYERS {
        py += ui.px(28.0);
        let fill = room.fills[seat];
        ui.text(&tf("lobby.seat", &[("n", &(seat + 1).to_string())]), r.x + pad, py, 20.0, DIM);
        let bx = r.x + pad + ui.px(72.0);
        if leader {
            let bw = ui.px(64.0);
            for (k, (label, f)) in [(t("lobby.fill_bot"), SeatFill::Bot), (t("lobby.fill_none"), SeatFill::Empty)].into_iter().enumerate() {
                let br = Rect::new(bx + k as f32 * (bw + ui.px(6.0)), py - ui.px(20.0), bw, ui.px(26.0));
                if ui.button(br, label, true, if fill == f { ACCENT } else { ui::EDGE }) && fill != f {
                    app.send(ClientMsg::SetSeatFill { seat: seat as u8, fill: f });
                }
            }
        } else {
            let label = if fill == SeatFill::Bot { t("lobby.seat_bot") } else { t("lobby.seat_empty") };
            ui.text(label, bx, py, 20.0, DIM);
        }
    }
    // The map: buttons for the leader, a label for everyone else.
    let my = y;
    let mut dx = map_x;
    for a in ArenaId::ALL {
        let bw = ui.px(120.0);
        let rr = Rect::new(dx, my + ui.px(12.0), bw, ui.px(34.0));
        let selected = room.arena == a;
        let name = t(&format!("arena.{}", a.id()));
        if leader {
            if ui.button(rr, name, true, if selected { ACCENT } else { ui::EDGE }) && !selected {
                app.send(ClientMsg::SetArena(a));
            }
        } else if selected {
            ui.text(name, dx, my + ui.px(36.0), 22.0, INK);
        }
        dx += bw + ui.px(8.0);
    }
    // Spectators: under the map, next in line for a seat.
    if !watching.is_empty() {
        let sy = my + ui.px(76.0);
        ui.text(t("lobby.spectating"), map_x, sy, 20.0, DIM);
        let names: Vec<String> = watching.iter().map(|p| if Some(p.id) == app.me { tf("lobby.you_spectating", &[("name", &p.name)]) } else { p.name.clone() }).collect();
        let mut ly = sy + ui.px(24.0);
        for line in ui.wrap(&names.join(", "), 18.0, r.x + w - pad - map_x) {
            ui.text(&line, map_x, ly, 18.0, INK);
            ly += ui.px(20.0);
        }
    }
    y = py.max(my + ui.px(46.0)) + ui.px(20.0);

    // Character picker: small tiles (head and name) on the left; on the right, a preview of the
    // one under the mouse (or the one you picked) with its special explained.
    let mine = room.players.iter().find(|p| Some(p.id) == app.me).map(|p| p.character);
    let ch = ui.px(200.0);
    let (tw, th, gap) = (ui.px(64.0), ui.px(70.0), ui.px(6.0));
    let cols = (((w - pad * 2.0) * 0.55 + gap) / (tw + gap)).floor().max(1.0) as usize;
    let grid_w = cols.min(Character::ALL.len()) as f32 * (tw + gap) - gap;
    let mut shown = mine.unwrap_or(Character::ALL[0]);
    for (k, c) in Character::ALL.into_iter().enumerate() {
        let rr = Rect::new(r.x + pad + (k % cols) as f32 * (tw + gap), y + (k / cols) as f32 * (th + gap), tw, th);
        let color = render::character_color(c);
        let selected = mine == Some(c);
        draw_rectangle(rr.x, rr.y, rr.w, rr.h, Color::new(color.r * 0.15, color.g * 0.15, color.b * 0.15, 0.9));
        draw_rectangle_lines(rr.x, rr.y, rr.w, rr.h, if selected { 4.0 } else { 1.0 }, if selected { color } else { ui::EDGE });
        // Just the head: the top of the portrait, blown up.
        let hs = ui.px(40.0);
        let src = Rect::new(8.0, 5.0, 10.0, 10.0);
        draw_texture_ex(&portraits[k].texture, rr.x + (rr.w - hs) / 2.0, rr.y + ui.px(4.0), WHITE, DrawTextureParams { dest_size: Some(vec2(hs, hs)), source: Some(src), ..Default::default() });
        let name = ui.fit(character_name(c), 16.0, rr.w - ui.px(4.0));
        ui.text_centered(&name, rr.x + rr.w / 2.0, rr.y + rr.h - ui.px(7.0), 16.0, color);
        let (clicked, hot) = ui.area_hover(rr);
        if hot {
            shown = c;
        }
        if clicked && !selected {
            app.send(ClientMsg::SetCharacter(c));
        }
    }
    preview(ui, Rect::new(r.x + pad + grid_w + ui.px(16.0), y, w - pad * 2.0 - grid_w - ui.px(16.0), ch), shown, mine == Some(shown), portraits);
    let rows = Character::ALL.len().div_ceil(cols) as f32;
    y += ch.max(rows * (th + gap) - gap) + ui.px(20.0);

    // Play / ready / exit.
    let bh = ui.px(48.0);
    let bw = ui.px(220.0);
    let others = seated.iter().filter(|p| p.id != room.leader).count();
    let ready = seated.iter().filter(|p| p.id != room.leader && p.ready).count();
    // Exit, quietly, on the left; play (or ready up) on the right, with who's ready beside it.
    let ew = ui.px(140.0);
    if ui.ghost_button(Rect::new(r.x + pad, y, ew, bh), t("lobby.exit"), BAD) {
        app.send(ClientMsg::LeaveRoom);
    }
    let play = Rect::new(r.x + w - pad - bw, y, bw, bh);
    let note_right = play.x - ui.px(16.0);
    if leader {
        // Play, in green, once everyone else seated has said they're ready (the server holds to
        // that too).
        if ui.button(play, t("lobby.play"), ready == others, ui::GOOD) {
            app.send(ClientMsg::Start);
        }
        if others > 0 {
            let note = format!("{ready}/{others} ready");
            ui.text(&note, note_right - ui.text_width(&note, 20.0), y + bh * 0.62, 20.0, DIM);
        }
    } else if watching.iter().any(|p| Some(p.id) == app.me) {
        // Spectating: nothing to ready up for. A seat comes your way when someone leaves.
        let note = t("lobby.spectator_note");
        ui.text(note, r.x + w - pad - ui.text_width(note, 18.0), y + bh * 0.62, 18.0, DIM);
    } else {
        let me_ready = room.players.iter().any(|p| Some(p.id) == app.me && p.ready);
        let label = if me_ready { t("lobby.is_ready") } else { t("lobby.ready") };
        if ui.button(play, label, true, if me_ready { ACCENT } else { INK }) {
            app.send(ClientMsg::SetReady(!me_ready));
        }
        let note = t("lobby.leader_starts");
        ui.text(note, note_right - ui.text_width(note, 18.0), y + bh * 0.62, 18.0, DIM);
    }
    ui.text_centered(&app.controls_help(), sw / 2.0, sh - ui.px(12.0), 18.0, DIM);
}

