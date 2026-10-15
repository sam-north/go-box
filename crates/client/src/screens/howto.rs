//! "How to Play", opened from the room: what wins, items, specials, with the little scripted
//! match (`howto::HowTo`) playing in two pictures.

use crate::app::*;
use crate::howto::{ITEMS_VIEW, SPECIAL_VIEW};
use crate::screens::{character_name, mmss, special_name};
use crate::text::{t, tf};
use crate::ui::{ACCENT, DIM, EDGE, INK, Ui};
use go_box_sim::character::Character;
use go_box_sim::sim::{MAX_TICKS, TICK_RATE};
use macroquad::prelude::*;

pub fn howto_screen(ui: &mut Ui, app: &mut App, pictures: &Texture2D) {
    let (sw, sh) = (screen_width(), screen_height());
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
    let w = ui.px(760.0).min(sw - 32.0);
    let h = ui.px(520.0).min(sh - 24.0);
    let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.05, 0.06, 0.08, 1.0));
    ui.panel(r);
    let pad = ui.px(28.0);
    let mut y = r.y + ui.px(48.0);
    ui.text(t("howto.title"), r.x + pad, y, 36.0, ACCENT);
    y += ui.px(40.0);
    let time = mmss((MAX_TICKS / TICK_RATE) as f32);
    let survive = tf("howto.survive", &[("time", &time)]);
    for line in ui.wrap(&survive, 22.0, w - pad * 2.0) {
        ui.text(&line, r.x + pad, y, 22.0, INK);
        y += ui.px(26.0);
    }

    // Two columns: items on the left, specials on the right, each a line and a picture.
    y += ui.px(18.0);
    let col_w = (w - pad * 3.0) / 2.0;
    let pic_h = ui.px(190.0);
    let cols = [
        (t("howto.items"), t("howto.item_names").to_string(), ITEMS_VIEW),
        (t("howto.special"), format!("{}: {}", character_name(Character::Vid), Character::Vid.special().map_or("", special_name)), SPECIAL_VIEW),
    ];
    for (k, (heading, caption, view)) in cols.into_iter().enumerate() {
        let x = r.x + pad + k as f32 * (col_w + pad);
        let mut ly = y;
        for line in ui.wrap(heading, 19.0, col_w) {
            ui.text(&line, x, ly, 19.0, INK);
            ly += ui.px(22.0);
        }
        // Lined up under both headings, whichever wraps more.
        let py = y + ui.px(52.0);
        let pw = (pic_h * view.w / view.h).min(col_w);
        let ph = pw * view.h / view.w;
        let pr = Rect::new(x + (col_w - pw) / 2.0, py, pw, ph);
        draw_texture_ex(pictures, pr.x, pr.y, WHITE, DrawTextureParams { dest_size: Some(vec2(pw, ph)), source: Some(view), ..Default::default() });
        draw_rectangle_lines(pr.x, pr.y, pr.w, pr.h, 1.0, EDGE);
        let cap = ui.fit(&caption, 16.0, col_w);
        ui.text_centered(&cap, x + col_w / 2.0, pr.y + pr.h + ui.px(22.0), 16.0, DIM);
    }

    let bh = ui.px(42.0);
    let by = r.y + h - pad - bh;
    ui.text(t("howto.have_fun"), r.x + pad, by + bh * 0.7, 26.0, ACCENT);
    let bw = ui.px(140.0);
    if ui.button(Rect::new(r.x + w - pad - bw, by, bw, bh), t("howto.back"), true, ACCENT) {
        app.howto_open = false;
    }
}
