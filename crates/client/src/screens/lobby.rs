//! The room: who's in it, seats, bot difficulty, characters, and the last match's scoreboard.

use crate::app::*;
use crate::screens::{character_name, mmss, seat_name, special_name};
use crate::text::{t, tf};
use crate::platform;
use crate::ui::{self, ACCENT, BAD, DIM, INK, Ui};
use go_box_protocol::*;
use go_box_render::{self as render};
use go_box_sim::character::Character;
use go_box_sim::sim::TICK_RATE;
use macroquad::prelude::*;

/// The special's name and its cooldown, as two short lines for a character card.
pub fn character_lines(c: Character) -> (String, String) {
    match c.special() {
        Some(s) => (special_name(s).into(), tf("lobby.cooldown", &[("secs", &(s.cooldown() / TICK_RATE).to_string())])),
        None => (t("lobby.no_special").into(), t("lobby.classic_blurb").into()),
    }
}

pub fn lobby_screen(ui: &mut Ui, app: &mut App, room: &RoomInfo, portraits: &[RenderTarget]) {
    let (sw, sh) = (screen_width(), screen_height());
    let w = ui.px(860.0).min(sw - 32.0);
    let results_h = if room.last_results.is_some() { ui.px(240.0) } else { 0.0 };
    let h = (ui.px(470.0) + results_h).min(sh - 24.0);
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
        y += ui.px(16.0);
        let banner = match res.winner {
            Some(w) => tf("lobby.wins", &[("name", &seat_name(&res.seats[w].name, res.seats[w].character, res.seats[w].bot))]),
            None => t("lobby.draw").to_string(),
        };
        ui.text(&banner, r.x + pad, y + ui.px(10.0), 30.0, INK);
        let length = tf("lobby.match_length", &[("time", &mmss(res.duration))]);
        ui.text(&length, r.x + w - pad - ui.text_width(&length, 18.0), y + ui.px(10.0), 18.0, DIM);
        y += ui.px(36.0);
        let cols = [0.0, 0.30, 0.47, 0.57, 0.70, 0.82];
        let heads = [t("lobby.col_player"), t("lobby.col_character"), t("lobby.col_kills"), t("lobby.col_headshots"), t("lobby.col_damage"), t("lobby.col_down")];
        let tw = w - pad * 2.0;
        for (k, head) in heads.iter().enumerate() {
            ui.text(head, r.x + pad + cols[k] * tw, y, 18.0, DIM);
        }
        y += ui.px(8.0);
        for (i, s) in res.seats.iter().enumerate() {
            y += ui.px(26.0);
            let color = render::look(s.character, i).accent;
            if res.winner == Some(i) {
                draw_rectangle(r.x + pad - ui.px(6.0), y - ui.px(19.0), tw + ui.px(12.0), ui.px(25.0), Color::new(color.r, color.g, color.b, 0.18));
            }
            let fate = match s.died_at {
                Some(t) => mmss(t),
                None => tf("lobby.survived", &[("hp", &format!("{:.0}", s.hp))]),
            };
            let cells = [seat_name(&s.name, s.character, s.bot), character_name(s.character).to_string(), s.kills.to_string(), s.headshots.to_string(), format!("{:.0}", s.damage_dealt), fate];
            for (k, cell) in cells.iter().enumerate() {
                ui.text(cell, r.x + pad + cols[k] * tw, y, 20.0, if k == 0 { color } else { INK });
            }
        }
        y += ui.px(24.0);
        draw_line(r.x + pad, y, r.x + w - pad, y, 1.0, ui::EDGE);
    }

    // Who's here.
    y += ui.px(34.0);
    ui.text(t("lobby.players"), r.x + pad, y, 20.0, DIM);
    let diff_x = r.x + w * 0.55;
    ui.text(t("lobby.difficulty"), diff_x, y, 20.0, DIM);
    let lines: Vec<String> = room
        .players
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
    for (i, (p, line)) in room.players.iter().zip(&lines).enumerate() {
        py += ui.px(28.0);
        let color = render::look(p.character, i).accent;
        ui.text(line, r.x + pad, py, 22.0, color);
        let status = if p.ready && p.id != room.leader { t("lobby.ready_tag") } else { "" };
        ui.text(&format!("{}  {status}", character_name(p.character)), char_x, py, 20.0, if p.ready { ACCENT } else { DIM });
    }
    // Seats without a player: a bot or nobody. The leader picks; down to just you.
    for seat in room.players.len()..MAX_PLAYERS {
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
    // Difficulty: buttons for the leader, a label for everyone else.
    let mut dx = diff_x;
    for d in Difficulty::ALL {
        let bw = ui.px(90.0);
        let rr = Rect::new(dx, y + ui.px(12.0), bw, ui.px(34.0));
        let selected = room.difficulty == d;
        if leader {
            if ui.button(rr, t(&format!("difficulty.{}", d.id())), true, if selected { ACCENT } else { ui::EDGE }) && !selected {
                app.send(ClientMsg::SetDifficulty(d));
            }
        } else if selected {
            ui.text(t(&format!("difficulty.{}", d.id())), dx, y + ui.px(36.0), 22.0, INK);
        }
        dx += bw + ui.px(8.0);
    }
    y = py + ui.px(20.0);

    // Character picker.
    let mine = room.players.iter().find(|p| Some(p.id) == app.me).map(|p| p.character);
    let n = Character::ALL.len() as f32;
    let gap = ui.px(10.0);
    let cw = (w - pad * 2.0 - gap * (n - 1.0)) / n;
    let ch = ui.px(110.0);
    for (k, c) in Character::ALL.into_iter().enumerate() {
        let rr = Rect::new(r.x + pad + k as f32 * (cw + gap), y, cw, ch);
        let color = render::character_color(c);
        let selected = mine == Some(c);
        draw_rectangle(rr.x, rr.y, rr.w, rr.h, Color::new(color.r * 0.15, color.g * 0.15, color.b * 0.15, 0.9));
        draw_rectangle_lines(rr.x, rr.y, rr.w, rr.h, if selected { 4.0 } else { 1.0 }, if selected { color } else { ui::EDGE });
        let ph = rr.h - ui.px(16.0);
        let pw = ph * 24.0 / 30.0;
        draw_texture_ex(&portraits[k].texture, rr.x + ui.px(6.0), rr.y + ui.px(8.0), WHITE, DrawTextureParams { dest_size: Some(vec2(pw, ph)), ..Default::default() });
        let tx = rr.x + pw + ui.px(12.0);
        ui.text(character_name(c), tx, rr.y + ui.px(36.0), 26.0, color);
        let (special, cooldown) = character_lines(c);
        let mut ly = rr.y + ui.px(58.0);
        for line in ui.wrap(&special, 15.0, rr.x + rr.w - tx - ui.px(6.0)) {
            ui.text(&line, tx, ly, 15.0, INK);
            ly += ui.px(17.0);
        }
        ui.text(&cooldown, tx, ly, 15.0, DIM);
        if selected {
            ui.text(t("lobby.picked"), tx, rr.y + rr.h - ui.px(8.0), 15.0, color);
        }
        if ui.area(rr) && !selected {
            app.send(ClientMsg::SetCharacter(c));
        }
    }
    y += ch + ui.px(20.0);

    // Play / ready / exit.
    let bh = ui.px(48.0);
    let bw = ui.px(220.0);
    let again = room.last_results.is_some();
    let others = room.players.iter().filter(|p| p.id != room.leader).count();
    let ready = room.players.iter().filter(|p| p.id != room.leader && p.ready).count();
    if leader {
        let label = if again { t("lobby.play_again") } else { t("lobby.start") };
        if ui.button(Rect::new(r.x + pad, y, bw, bh), label, true, ACCENT) {
            app.send(ClientMsg::Start);
        }
        if others > 0 {
            ui.text(&format!("{ready}/{others} ready"), r.x + pad + bw + ui.px(16.0), y + bh * 0.62, 20.0, DIM);
        }
    } else {
        let me_ready = room.players.iter().any(|p| Some(p.id) == app.me && p.ready);
        let label = if me_ready { t("lobby.is_ready") } else if again { t("lobby.play_again") } else { t("lobby.ready") };
        if ui.button(Rect::new(r.x + pad, y, bw, bh), label, true, if me_ready { ACCENT } else { INK }) {
            app.send(ClientMsg::SetReady(!me_ready));
        }
        ui.text(t("lobby.leader_starts"), r.x + pad + bw + ui.px(16.0), y + bh * 0.62, 18.0, DIM);
    }
    if ui.button(Rect::new(r.x + w - pad - ui.px(140.0), y, ui.px(140.0), bh), t("lobby.exit"), true, BAD) {
        app.send(ClientMsg::LeaveRoom);
    }
    ui.text_centered(&app.controls_help(), sw / 2.0, sh - ui.px(12.0), 18.0, DIM);
}

