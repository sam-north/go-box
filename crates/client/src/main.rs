//! Go Box client: home screen, room lobby / results overlay, and the match itself.
//!
//! Built for the browser (`scripts/build-web.sh`), it talks to the server that served the page.
//! Built natively it's a test client: `--server ws://127.0.0.1:7878/ws --name Sam --create`, and
//! a few flags for automated runs (see `testclient.rs`).

mod app;
mod attract;
mod game;
mod input;
mod net;
mod platform;
mod screens;
mod testclient;
mod ui;

use go_box_protocol::*;
use go_box_render::sound::{Sfx, Sounds};
use go_box_render::{self as render, Art};
use go_box_sim::character::Character;
use go_box_sim::sim::{Action, DT, H, Match, W};
use macroquad::prelude::*;
use ::rand::rngs::SmallRng;
use ::rand::SeedableRng;
use input::Controls;
use ui::{BAD, Ui};
use app::*;
use attract::Attract;
use screens::home::{connecting_screen, home_screen};
use screens::hud::{draw_aim, draw_you, hud};
use screens::lobby::lobby_screen;
use screens::settings::{settings_button, settings_screen};
use testclient::{Automation, Options, autopilot};

fn conf() -> Conf {
    Conf { window_title: "Go Box".into(), window_width: 1280, window_height: 720, high_dpi: true, ..Default::default() }
}

#[macroquad::main(conf)]
async fn main() {
    let opts = Options::get();
    let mut ui = Ui::new();
    let mut art = Art::new();
    let mut sfx = Sounds::load();
    sfx.muted = platform::load_pref(PREF_MUTED) == "1";
    if let Ok(v) = platform::load_pref(PREF_MASTER).parse::<f32>() {
        sfx.master = v.clamp(0.0, 1.0);
    }
    if let Ok(v) = platform::load_pref(PREF_EFFECTS).parse::<f32>() {
        sfx.effects = v.clamp(0.0, 1.0);
    }
    let rt = render_target(W as u32, H as u32);
    rt.texture.set_filter(FilterMode::Nearest);
    let mut cam = Camera2D { target: vec2(W / 2.0, H / 2.0), zoom: vec2(2.0 / W, 2.0 / H), ..Default::default() };
    cam.render_target = Some(rt.clone());
    let portraits: Vec<RenderTarget> = (0..Character::ALL.len())
        .map(|_| {
            let t = render_target(24, 30);
            t.texture.set_filter(FilterMode::Nearest);
            t
        })
        .collect();

    let mut attract = Attract::new();
    let mut app = App {
        net: None,
        hello_name: None,
        hello_sent: false,
        after_welcome: None,
        me: None,
        room: None,
        game: None,
        name: opts.name.clone().unwrap_or_else(platform::load_name),
        code: platform::url_room().unwrap_or_default(),
        notice: None,
        lost: false,
        seq: 0,
        acc: 0.0,
        controls: Controls::default(),
        view: (Vec2::ZERO, 1.0),
        settings_open: opts.settings,
        preview_at: 0.0,
        rng: SmallRng::seed_from_u64((get_time() * 1e6) as u64),
        was_over: false,
    };
    if opts.create {
        app.enter(ClientMsg::CreateRoom);
    } else if let Some(code) = &opts.join {
        app.enter(ClientMsg::JoinRoom { code: code.clone() });
    }
    let mut automation = Automation::new();

    loop {
        let dt = get_frame_time().min(0.1);
        app.update_net(&mut art, &mut sfx);
        app.controls.poll();
        if app.controls.settings_pressed() {
            app.settings_open = !app.settings_open;
        }
        let seat = app.game.as_ref().and_then(|g| g.seat());
        let playing = app.room.as_ref().is_some_and(|r| r.phase == Phase::Playing) && app.game.as_ref().is_some_and(|g| !g.over());
        // The gamepad drives the menus whenever you're not mid-match (or the settings are up).
        let in_menus = !playing || app.settings_open;
        let nav = app.controls.nav();
        ui.begin(if in_menus { nav } else { ui::Nav::default() }, in_menus && app.controls.using_pad);

        // Your inputs, at the sim's fixed 60/s.
        let mouse: Vec2 = mouse_position().into();
        let mouse_arena = (mouse - app.view.0) / app.view.1;
        // Clicks on the menu or its button aren't throws.
        let mouse_free = !app.settings_open && !settings_button(&ui).contains(mouse);
        let me = app.game.as_ref().and_then(|g| g.me()).cloned();
        if let (true, Some(seat), Some(me)) = (playing, seat, me) {
            app.controls.latch(me.holding.is_some(), mouse_free);
            app.acc = (app.acc + dt).min(0.25);
            while app.acc >= DT {
                app.acc -= DT;
                app.seq += 1;
                let action = if opts.autopilot {
                    let g = app.game.as_mut().unwrap();
                    let view = g.latest().cloned().map(|v| Match::from_view(v, Vec::new()));
                    view.map_or_else(Action::default, |m| autopilot(&m, seat, &mut app.rng))
                } else if app.settings_open {
                    // Stand still while the menu is up.
                    Action::default()
                } else {
                    app.controls.take(&me, mouse_arena, mouse_free)
                };
                let seq = app.seq;
                app.send(ClientMsg::Input { seq, action });
                let mut mine = Vec::new();
                app.game.as_mut().unwrap().local_tick(seq, action, &mut mine);
                sfx.local(&mine);
            }
        }

        // The arena: the match you're in, or bots fighting behind the menus.
        let shown: Option<Match> = match &mut app.game {
            Some(g) => g.frame(dt),
            None => None,
        };
        let m = match &shown {
            Some(m) => {
                art.handle_events(m);
                // The bots behind the menus stay silent; only a match you're in makes sound.
                sfx.events(m, &m.events, seat);
                sfx.frame(dt, m, seat);
                if m.over && !app.was_over {
                    let lost = seat.is_some_and(|s| m.winner != Some(s));
                    sfx.play(if lost { Sfx::Lose } else { Sfx::Win }, 1.0);
                }
                app.was_over = m.over;
                m
            }
            None => {
                let events = attract.frame(dt);
                attract.m.events = events;
                if std::mem::take(&mut attract.restarted) {
                    art.reset();
                }
                art.handle_events(&attract.m);
                &attract.m
            }
        };
        art.update(dt, m);
        set_camera(&cam);
        clear_background(BLACK);
        art.draw(m);
        if let (true, Some(seat)) = (playing, seat) {
            draw_aim(m, seat, app.controls.target(&m.f[seat], mouse_arena));
        }
        if let Some(seat) = seat {
            draw_you(m, seat);
        }

        // Character portraits for the picker.
        if app.room.as_ref().is_some_and(|r| r.phase == Phase::Lobby) {
            let my_index = app.room.as_ref().and_then(|r| r.players.iter().position(|p| Some(p.id) == app.me)).unwrap_or(0);
            for (k, c) in Character::ALL.into_iter().enumerate() {
                let mut pc = Camera2D { target: vec2(0.0, -10.0), zoom: vec2(2.0 / 24.0, 2.0 / 30.0), ..Default::default() };
                pc.render_target = Some(portraits[k].clone());
                set_camera(&pc);
                clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
                render::draw_portrait(c, my_index, get_time() as f32);
            }
        }

        set_default_camera();
        clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
        let scale = (screen_width() / W).min(screen_height() / H);
        // Fill the window: at 480x270 whole-pixel steps (2x, 3x) leave big borders on common screens.
        let (dw, dh) = (W * scale, H * scale);
        let (ox, oy) = (((screen_width() - dw) / 2.0).floor(), ((screen_height() - dh) / 2.0).floor());
        let in_menu = app.game.is_none() || app.room.as_ref().is_none_or(|r| r.phase == Phase::Lobby);
        let tint = if in_menu { Color::new(0.55, 0.55, 0.6, 1.0) } else { WHITE };
        draw_texture_ex(&rt.texture, ox, oy, tint, DrawTextureParams { dest_size: Some(vec2(dw, dh)), ..Default::default() });
        app.view = (vec2(ox, oy), scale);

        // Screens (under the settings menu, if it's open, they ignore the mouse).
        ui.blocked = app.settings_open;
        match (&app.room, app.net.is_some()) {
            (None, true) => connecting_screen(&mut ui),
            (None, false) => home_screen(&mut ui, &mut app),
            (Some(r), _) if r.phase == Phase::Lobby => {
                let r = r.clone();
                lobby_screen(&mut ui, &mut app, &r, &portraits);
            }
            (Some(_), _) => hud(&ui, &app, m),
        }
        if let Some((msg, t)) = &app.notice {
            if get_time() - t < 5.0 {
                let w = ui.text_width(msg, 22.0) + ui.px(40.0);
                let r = Rect::new((screen_width() - w) / 2.0, ui.px(16.0), w, ui.px(40.0));
                ui.panel(r);
                ui.text_centered(msg, screen_width() / 2.0, r.y + ui.px(27.0), 22.0, BAD);
            }
        }
        ui.blocked = false;
        if app.settings_open {
            settings_screen(&mut ui, &mut app, &mut sfx);
        } else if ui.button(settings_button(&ui), "Settings", true, ui::EDGE) {
            app.settings_open = true;
        }
        if ui.pressed {
            sfx.play(Sfx::Click, 1.0);
        }
        if ui.focus.is_none() && is_key_pressed(KeyCode::M) {
            sfx.muted = !sfx.muted;
            platform::save_pref(PREF_MUTED, if sfx.muted { "1" } else { "0" });
            app.notify(if sfx.muted { "Sound off (M to turn it back on)" } else { "Sound on" });
            sfx.play(Sfx::Click, 1.0);
        }
        ui.end();

        if automation.step(&mut app) {
            break;
        }
        next_frame().await;
    }
}
