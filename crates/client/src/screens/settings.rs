//! The settings menu: a Sound page (volumes, mute) and a Controls page (rebind every action for
//! keyboard/mouse and for gamepads). Changes are saved as they're made.

use crate::app::*;
use crate::settings::{Act, Bindings};
use crate::text::{t, tf};
use crate::ui::{ACCENT, BAD, DIM, EDGE, INK, Slider, Ui};
use go_box_protocol::ClientMsg;
use go_box_render::sound::{Sfx, Sounds};
use macroquad::prelude::*;

/// The small button that opens the settings menu, bottom right on every screen.
pub fn settings_button(ui: &Ui) -> Rect {
    let (w, h) = (ui.px(110.0), ui.px(32.0));
    Rect::new(screen_width() - w - ui.px(10.0), screen_height() - h - ui.px(10.0), w, h)
}

pub fn settings_screen(ui: &mut Ui, app: &mut App, sfx: &mut Sounds) {
    let (sw, sh) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
    let w = ui.px(720.0).min(sw - 32.0);
    let h = ui.px(540.0).min(sh - 24.0);
    let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.05, 0.06, 0.08, 1.0));
    ui.panel(r);
    let pad = ui.px(28.0);
    let mut y = r.y + ui.px(48.0);
    ui.text(t("settings.title"), r.x + pad, y, 36.0, ACCENT);

    // Tabs.
    let tw = ui.px(130.0);
    for (k, (tab, label)) in [(SettingsTab::Sound, t("settings.tab_sound")), (SettingsTab::Controls, t("settings.tab_controls"))].into_iter().enumerate() {
        let tr = Rect::new(r.x + w - pad - (2 - k) as f32 * (tw + ui.px(8.0)) + ui.px(8.0), y - ui.px(30.0), tw, ui.px(36.0));
        if ui.button(tr, label, true, if app.settings_tab == tab { ACCENT } else { EDGE }) {
            app.settings_tab = tab;
            app.rebinding = None;
        }
    }
    y += ui.px(20.0);
    match app.settings_tab {
        SettingsTab::Sound => sound_page(ui, app, sfx, r, y),
        SettingsTab::Controls => controls_page(ui, app, r, y),
    }

    let bw = ui.px(140.0);
    if ui.button(Rect::new(r.x + w - pad - bw, r.y + h - pad - ui.px(42.0), bw, ui.px(42.0)), t("settings.close"), true, ACCENT) {
        app.settings_open = false;
    }
    // In a match: the leader can end it for everyone (scored as if time ran out); anyone else
    // can leave.
    let in_match = app.game.as_ref().is_some_and(|g| !g.over());
    if in_match {
        let leader = app.room.as_ref().is_some_and(|room| Some(room.leader) == app.me);
        let ew = ui.px(180.0);
        let er = Rect::new(r.x + w - pad - bw - ui.px(12.0) - ew, r.y + h - pad - ui.px(42.0), ew, ui.px(42.0));
        let label = if leader { t("settings.end_match") } else { t("settings.leave_match") };
        if ui.button(er, label, true, BAD) {
            app.send(if leader { ClientMsg::EndMatch } else { ClientMsg::LeaveRoom });
            app.settings_open = false;
        }
    }
}

/// Volumes (overall, then sound effects and music under it) and mute.
fn sound_page(ui: &mut Ui, app: &mut App, sfx: &mut Sounds, r: Rect, mut y: f32) {
    let pad = ui.px(28.0);
    let label_w = ui.px(190.0);
    let sx = r.x + pad + label_w;
    let slider_w = r.w - pad * 2.0 - label_w - ui.px(70.0);
    let s = &mut app.settings;
    let mut changed = false;
    for (label, id, value) in [(t("settings.volume_master"), Slider::Master, &mut s.master), (t("settings.volume_effects"), Slider::Effects, &mut s.effects), (t("settings.volume_music"), Slider::Music, &mut s.music)] {
        y += ui.px(50.0);
        ui.text(label, r.x + pad, y, 22.0, INK);
        changed |= ui.slider(id, Rect::new(sx, y - ui.px(14.0), slider_w, ui.px(12.0)), value);
        ui.text(&format!("{:.0}%", *value * 100.0), sx + slider_w + ui.px(14.0), y, 20.0, DIM);
    }
    y += ui.px(24.0);
    ui.text(t("settings.no_music_yet"), sx, y, 16.0, DIM);
    if changed {
        s.save();
        sfx.master = s.master;
        sfx.effects = s.effects;
        // Let them hear the new level (a few times a second while dragging).
        if get_time() - app.preview_at > 0.25 {
            app.preview_at = get_time();
            sfx.play(Sfx::Grab, 1.0);
        }
    }
    y += ui.px(30.0);
    let key = s.bindings.short_label(Act::Mute, false);
    let label = if s.muted { tf("settings.sound_off", &[("key", &key)]) } else { tf("settings.sound_on", &[("key", &key)]) };
    if ui.button(Rect::new(r.x + pad, y, ui.px(240.0), ui.px(40.0)), &label, true, if s.muted { BAD } else { ACCENT }) {
        s.muted = !s.muted;
        s.save();
    }
}

/// One row per action: what it's bound to on keyboard/mouse and on a gamepad, each with a
/// button to change it.
fn controls_page(ui: &mut Ui, app: &mut App, r: Rect, mut y: f32) {
    let pad = ui.px(28.0);
    let col_kb = r.x + pad + ui.px(150.0);
    let col_pad = r.x + pad + ui.px(395.0);
    let bw = ui.px(84.0);
    y += ui.px(30.0);
    ui.text(t("settings.keyboard"), col_kb, y, 18.0, DIM);
    ui.text(t("settings.controller"), col_pad, y, 18.0, DIM);
    let status = if app.controls.pad.connected { t("settings.pad_connected") } else { t("settings.pad_missing") };
    y += ui.px(18.0);
    ui.text(&ui.fit(status, 14.0, r.x + r.w - pad - col_pad), col_pad, y, 14.0, if app.controls.pad.connected { ACCENT } else { DIM });
    for act in Act::ALL {
        y += ui.px(36.0);
        ui.text(act.label(), r.x + pad, y, 20.0, INK);
        for (pad_side, x) in [(false, col_kb), (true, col_pad)] {
            let waiting = app.rebinding == Some((act, pad_side));
            let shown = if waiting { t("settings.press_key").to_string() } else { ui.fit(&app.settings.bindings.label(act, pad_side), 18.0, ui.px(140.0)) };
            ui.text(&shown, x, y, 18.0, if waiting { ACCENT } else { INK });
            let br = Rect::new(x + ui.px(150.0), y - ui.px(22.0), bw, ui.px(30.0));
            if ui.button(br, t("settings.change"), !waiting, EDGE) {
                app.rebinding = Some((act, pad_side));
            }
        }
    }
    y += ui.px(30.0);
    ui.text(t("settings.fixed_controls"), r.x + pad, y, 16.0, DIM);
    if ui.button(Rect::new(r.x + pad, r.y + r.h - pad - ui.px(42.0), ui.px(200.0), ui.px(42.0)), t("settings.reset_controls"), true, BAD) {
        app.settings.bindings = Bindings::default();
        app.settings.save();
        app.rebinding = None;
    }
}
