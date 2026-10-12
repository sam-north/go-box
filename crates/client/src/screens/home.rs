//! The home screen: your name, join a friend's room, or (tucked away below) create one.

use crate::app::*;
use crate::text::t;
use crate::ui::{ACCENT, DIM, Field, INK, Ui};
use go_box_protocol::*;
use macroquad::prelude::*;

pub fn connecting_screen(ui: &mut Ui) {
    ui.text_centered(t("home.connecting"), screen_width() / 2.0, screen_height() / 2.0, 32.0, INK);
}

pub fn home_screen(ui: &mut Ui, app: &mut App) {
    let (sw, sh) = (screen_width(), screen_height());
    let w = ui.px(460.0).min(sw - 32.0);
    let rowh = ui.px(46.0);
    // Opening "Create a room" makes the panel taller by a row.
    let h = ui.px(352.0) + if app.create_open { rowh + ui.px(14.0) } else { 0.0 };
    let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
    ui.panel(r);
    let cx = sw / 2.0;
    let pad = ui.px(28.0);
    ui.text_centered(t("home.title"), cx, r.y + ui.px(64.0), 64.0, ACCENT);
    ui.text_centered(t("home.tagline"), cx, r.y + ui.px(96.0), 20.0, DIM);

    let fw = w - pad * 2.0;
    let mut y = r.y + ui.px(126.0);
    ui.text(t("home.your_name"), r.x + pad, y + ui.px(4.0), 18.0, DIM);
    y += ui.px(10.0);
    let name_enter = ui.field(Field::Name, Rect::new(r.x + pad, y, fw, rowh), &mut app.name, t("home.name_placeholder"), MAX_NAME_LEN, |c| (!c.is_control()).then_some(c));
    y += rowh + ui.px(32.0);

    ui.text(t("home.join_heading"), r.x + pad, y, 18.0, DIM);
    y += ui.px(10.0);
    let bw = ui.px(120.0);
    let code_enter = ui.field(Field::Code, Rect::new(r.x + pad, y, fw - bw - ui.px(12.0), rowh), &mut app.code, t("home.code_placeholder"), CODE_LEN, |c| {
        let c = c.to_ascii_uppercase();
        CODE_ALPHABET.contains(&(c as u8)).then_some(c)
    });
    let join = ui.button(Rect::new(r.x + pad + fw - bw, y, bw, rowh), t("home.join"), app.code.len() == CODE_LEN, ACCENT);
    y += rowh + ui.px(18.0);

    // "Create a room": collapsed until opened. Creating may need the server's passphrase (the
    // cloud server asks for one; on your own machine it doesn't); it's remembered in this
    // browser.
    let header = Rect::new(r.x + pad, y, fw, ui.px(34.0));
    let (toggled, hot) = ui.area_hover(header);
    if toggled {
        app.create_open = !app.create_open;
    }
    let color = if hot { INK } else { DIM };
    let (ax, ay, a) = (header.x + ui.px(8.0), header.y + header.h / 2.0, ui.px(6.0));
    if app.create_open {
        draw_triangle(vec2(ax - a, ay - a * 0.5), vec2(ax + a, ay - a * 0.5), vec2(ax, ay + a * 0.7), color);
    } else {
        draw_triangle(vec2(ax - a * 0.5, ay - a), vec2(ax - a * 0.5, ay + a), vec2(ax + a * 0.7, ay), color);
    }
    ui.text(t("home.create_heading"), header.x + ui.px(24.0), ay + ui.px(6.0), 20.0, color);
    let mut create = false;
    if app.create_open {
        y += header.h + ui.px(8.0);
        let cw = ui.px(120.0);
        let pass_enter = ui.field(Field::Passphrase, Rect::new(r.x + pad, y, fw - cw - ui.px(12.0), rowh), &mut app.passphrase, t("home.passphrase_placeholder"), 64, |c| (!c.is_control()).then_some(c));
        create = ui.button(Rect::new(r.x + pad + fw - cw, y, cw, rowh), t("home.create"), true, ACCENT) || pass_enter;
    }

    if create {
        app.create_room();
    } else if (join || code_enter || name_enter) && app.code.len() == CODE_LEN {
        let code = app.code.clone();
        app.enter(ClientMsg::JoinRoom { code });
    }
    ui.text_centered(&app.controls_help(), cx, sh - ui.px(20.0), 18.0, DIM);
}
