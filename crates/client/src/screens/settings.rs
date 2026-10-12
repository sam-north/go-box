//! The settings menu: volumes, mute, and the controls.

use crate::app::*;
use crate::input::{KEYS_HELP, PAD_HELP};
use crate::platform;
use crate::ui::{ACCENT, BAD, DIM, INK, Slider, Ui};
use go_box_render::sound::{Sfx, Sounds};
use macroquad::prelude::*;

pub fn settings_button(ui: &Ui) -> Rect {
    let (w, h) = (ui.px(110.0), ui.px(32.0));
    Rect::new(screen_width() - w - ui.px(10.0), screen_height() - h - ui.px(10.0), w, h)
}

/// Volume sliders (saved in the browser), mute, and the controls for keyboard and gamepad.
pub fn settings_screen(ui: &mut Ui, app: &mut App, sfx: &mut Sounds) {
    let (sw, sh) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
    let w = ui.px(640.0).min(sw - 32.0);
    let h = ui.px(470.0).min(sh - 24.0);
    let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.05, 0.06, 0.08, 1.0));
    ui.panel(r);
    let pad = ui.px(28.0);
    let mut y = r.y + ui.px(48.0);
    ui.text("SETTINGS", r.x + pad, y, 36.0, ACCENT);

    // Volumes: overall, and sound effects under it (music will get its own slider here).
    let label_w = ui.px(170.0);
    let sx = r.x + pad + label_w;
    let slider_w = w - pad * 2.0 - label_w - ui.px(70.0);
    let mut changed = false;
    for (label, id, value) in [("Overall volume", Slider::Master, &mut sfx.master), ("Sound effects", Slider::Effects, &mut sfx.effects)] {
        y += ui.px(46.0);
        ui.text(label, r.x + pad, y, 22.0, INK);
        changed |= ui.slider(id, Rect::new(sx, y - ui.px(14.0), slider_w, ui.px(12.0)), value);
        ui.text(&format!("{:.0}%", *value * 100.0), sx + slider_w + ui.px(14.0), y, 20.0, DIM);
    }
    y += ui.px(36.0);
    ui.text("Music", r.x + pad, y, 22.0, DIM);
    ui.text("none yet", sx, y, 18.0, DIM);
    if changed {
        platform::save_pref(PREF_MASTER, &format!("{:.2}", sfx.master));
        platform::save_pref(PREF_EFFECTS, &format!("{:.2}", sfx.effects));
        // Let them hear the new level (a few times a second while dragging).
        if get_time() - app.preview_at > 0.25 {
            app.preview_at = get_time();
            sfx.play(Sfx::Grab, 1.0);
        }
    }
    y += ui.px(22.0);
    let label = if sfx.muted { "Sound: off (M)" } else { "Sound: on (M)" };
    if ui.button(Rect::new(r.x + pad, y, ui.px(200.0), ui.px(38.0)), label, true, if sfx.muted { BAD } else { ACCENT }) {
        sfx.muted = !sfx.muted;
        platform::save_pref(PREF_MUTED, if sfx.muted { "1" } else { "0" });
    }

    // Controls.
    y += ui.px(76.0);
    let tw = w - pad * 2.0;
    for (title, help) in [("Keyboard + mouse", KEYS_HELP), ("Controller", PAD_HELP)] {
        ui.text(title, r.x + pad, y, 22.0, INK);
        if title == "Controller" {
            let status = if app.controls.pad_connected() { "connected" } else { "none found (press a button on it)" };
            ui.text(status, r.x + pad + ui.text_width(title, 22.0) + ui.px(14.0), y, 18.0, DIM);
        }
        for line in ui.wrap(&help.split("   ").collect::<Vec<_>>().join(",  "), 18.0, tw) {
            y += ui.px(24.0);
            ui.text(&line, r.x + pad, y, 18.0, DIM);
        }
        y += ui.px(40.0);
    }
    let bw = ui.px(140.0);
    if ui.button(Rect::new(r.x + w - pad - bw, r.y + h - pad - ui.px(42.0), bw, ui.px(42.0)), "Close", true, ACCENT) {
        app.settings_open = false;
    }
}
