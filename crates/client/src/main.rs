//! Go Box client: home screen, room lobby / results overlay, and the match itself.
//!
//! Built for the browser (`scripts/build-web.sh`), it talks to the server that served the page.
//! Built natively it's a test client: `--server ws://127.0.0.1:7878/ws --name Sam --create`, and
//! a few flags for automated runs (see `testclient.rs`).

mod app;
mod attract;
mod debug;
mod game;
mod input;
mod net;
mod platform;
mod screens;
mod settings;
mod testclient;
mod text;
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
use settings::{Act, Bind, Settings};
use text::{t, tf};
use ui::{BAD, Ui};
use app::*;
use attract::Attract;
use screens::home::{connecting_screen, home_screen};
use screens::hud::{draw_aim, draw_you, hud};
use screens::lobby::lobby_screen;
use screens::settings::{settings_button, settings_screen};
use testclient::{Automation, Options, autopilot};

/// Where the opponent nearest `from` is (their middle), as of the newest snapshot: what a
/// trackpad player's throws and swings aim at.
fn nearest_foe(g: &game::Game, seat: usize, from: Vec2) -> Option<Vec2> {
    let v = g.latest()?;
    let team = v.f[seat].team(seat);
    v.f.iter().enumerate().filter(|(j, f)| *j != seat && f.present() && f.team(*j) != team).map(|(_, f)| f.center()).min_by(|a, b| a.distance(from).total_cmp(&b.distance(from)))
}

fn conf() -> Conf {
    Conf { window_title: "Go Box".into(), window_width: 1280, window_height: 720, high_dpi: true, ..Default::default() }
}

#[macroquad::main(conf)]
async fn main() {
    let opts = Options::get();
    let mut ui = Ui::new();
    let mut art = Art::new();
    let mut sfx = Sounds::load();
    let mut settings = Settings::load();
    settings.muted |= opts.mute;
    text::set_language(&settings.language);
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
    // Which portraits are drawn, for whom (their colours follow your seat), and the next to refresh.
    let mut portraits_for: Option<usize> = None;
    let mut portrait_next = 0usize;
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
        passphrase: opts.passphrase.clone().unwrap_or_else(|| platform::load_pref(PREF_PASSPHRASE)),
        notice: None,
        lost: false,
        seq: 0,
        acc: 0.0,
        controls: Controls::default(),
        view: (Vec2::ZERO, 1.0),
        settings,
        settings_open: opts.settings || opts.controls,
        settings_tab: if opts.controls { SettingsTab::Controls } else { SettingsTab::default() },
        rebinding: None,
        preview_at: 0.0,
        rng: SmallRng::seed_from_u64((get_time() * 1e6) as u64),
        was_over: false,
        // (A test client given a passphrase shows it.)
        create_open: opts.passphrase.is_some(),
    };
    if opts.create {
        app.create_room();
    } else if let Some(code) = &opts.join {
        app.enter(ClientMsg::JoinRoom { code: code.clone() });
    }
    let mut automation = Automation::new();
    let mut debug = debug::Debug::new();

    loop {
        debug.begin();
        let dt = get_frame_time().min(0.1);
        app.update_net(&mut art, &mut sfx);
        app.controls.poll();
        if let Some((act, pad)) = app.rebinding {
            // Waiting for a key or button to bind; Esc cancels.
            let captured = if pad { Bind::capture_pad(&app.controls.pad, app.controls.pad_prev) } else { Bind::capture_keyboard() };
            if is_key_pressed(KeyCode::Escape) {
                app.rebinding = None;
            } else if let Some(b) = captured {
                app.settings.bindings.set(act, b);
                app.settings.save();
                app.rebinding = None;
            }
        } else if app.controls.pressed(&app.settings.bindings, Act::Settings) {
            app.settings_open = !app.settings_open;
        }
        if !app.settings_open {
            app.rebinding = None;
        }
        sfx.master = app.settings.master;
        sfx.effects = app.settings.effects;
        sfx.muted = app.settings.muted;
        let seat = app.game.as_ref().and_then(|g| g.seat());
        let playing = app.room.as_ref().is_some_and(|r| r.phase == Phase::Playing) && app.game.as_ref().is_some_and(|g| !g.over());
        // The gamepad drives the menus whenever you're not mid-match (or the settings are up).
        let in_menus = !playing || app.settings_open;
        let nav = app.controls.nav();
        ui.begin(if in_menus { nav } else { ui::Nav::default() }, in_menus && app.controls.using_pad, in_menus);

        // Your inputs, at the sim's fixed 60/s.
        let mouse: Vec2 = mouse_position().into();
        let mouse_arena = (mouse - app.view.0) / app.view.1;
        // Clicks on the menu or its button aren't throws.
        let mouse_free = !app.settings_open && !settings_button(&ui).contains(mouse);
        let me = app.game.as_ref().and_then(|g| g.me()).cloned();
        if let (true, Some(seat), Some(me)) = (playing, seat, me) {
            app.controls.latch(&app.settings.bindings, me.holding.is_some(), mouse_free);
            if app.controls.pressed(&app.settings.bindings, Act::Jump) {
                debug.jump_pressed();
            }
            let mut ticks = 0;
            app.acc = (app.acc + dt).min(0.25);
            while app.acc >= DT {
                ticks += 1;
                app.acc -= DT;
                app.seq += 1;
                let action = if opts.autopilot {
                    let g = app.game.as_mut().unwrap();
                    let view = g.latest().cloned().map(|v| Match::from_view(v, &g.start.config, Vec::new()));
                    view.map_or_else(Action::default, |m| autopilot(&m, seat, &mut app.rng))
                } else if app.settings_open || app.game.as_ref().is_some_and(|g| g.counting_down()) {
                    // Stand still while the menu is up, and during the 3-2-1 (the server ignores input
                    // then, so predicting movement would only snap back).
                    Action::default()
                } else {
                    // On a trackpad, throws and swings aim at the nearest opponent.
                    let auto_aim = app.settings.trackpad.then(|| app.game.as_ref().and_then(|g| nearest_foe(g, seat, me.hand()))).flatten();
                    app.controls.take(&app.settings.bindings, &me, mouse_arena, mouse_free, app.settings.melee_aim, auto_aim)
                };
                let seq = app.seq;
                app.send(ClientMsg::Input { seq, action });
                let mut mine = Vec::new();
                app.game.as_mut().unwrap().local_tick(seq, action, &mut mine);
                sfx.local(&mine, me.holding);
            }
            debug.ticks(ticks);
        }

        // The arena: the match you're in, or bots fighting behind the menus.
        let shown: Option<Match> = match &mut app.game {
            Some(g) => g.frame(dt),
            None => None,
        };
        debug.observe(shown.as_ref().zip(seat).map(|(m, s)| &m.f[s]), app.game.as_ref().and_then(|g| g.latest()).map(|v| v.tick));
        let m = match &shown {
            Some(m) => {
                art.set_arena(m.config.arena);
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
                art.set_arena(attract.m.config.arena);
                art.handle_events(&attract.m);
                &attract.m
            }
        };
        art.update(dt, m);
        art.draw_sprites(m);
        set_camera(&cam);
        clear_background(BLACK);
        art.draw(m);
        if let (true, Some(seat)) = (playing, seat) {
            // (On a trackpad, the aim shown is the nearest opponent, which is where throws go.)
            let hand = m.f[seat].hand();
            let auto = app.settings.trackpad.then(|| app.game.as_ref().and_then(|g| nearest_foe(g, seat, hand))).flatten();
            let aim = auto.map(|at| (at - hand).normalize_or_zero()).or_else(|| app.controls.target(&m.f[seat], mouse_arena));
            draw_aim(m, seat, aim);
        }
        if let Some(seat) = seat {
            draw_you(m, seat);
        }

        // Character portraits for the picker: all of them when you arrive (or your seat changes),
        // then just one a frame in turn (so the scarves still sway) rather than every one, every
        // frame (a dozen render-target switches a frame is heavy on phones).
        if app.room.as_ref().is_some_and(|r| r.phase == Phase::Lobby) {
            let my_index = app.room.as_ref().and_then(|r| r.players.iter().position(|p| Some(p.id) == app.me)).unwrap_or(0);
            let all = portraits_for != Some(my_index);
            portraits_for = Some(my_index);
            portrait_next = (portrait_next + 1) % Character::ALL.len();
            for (k, c) in Character::ALL.into_iter().enumerate().filter(|&(k, _)| all || k == portrait_next) {
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
        // "Connecting..." only until the server says hello; after a refusal (wrong code or
        // passphrase) you are still connected, and back on the home screen to try again.
        match (&app.room, app.net.is_some() && app.me.is_none()) {
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
        } else if ui.button(settings_button(&ui), t("settings.button"), true, ui::EDGE) {
            app.settings_open = true;
        }
        if ui.pressed {
            sfx.play(Sfx::Click, 1.0);
        }
        // (Not while typing a name, or while a key is being rebound.)
        if ui.focus.is_none() && app.rebinding.is_none() && app.controls.pressed(&app.settings.bindings, Act::Mute) {
            app.settings.muted = !app.settings.muted;
            app.settings.save();
            sfx.muted = app.settings.muted;
            let key = app.settings.bindings.short_label(Act::Mute, false);
            app.notify(if app.settings.muted { tf("notice.sound_off", &[("key", &key)]) } else { t("notice.sound_on").to_string() });
            sfx.play(Sfx::Click, 1.0);
        }
        ui.end();
        debug.draw(&ui);
        debug.end(app.game.as_ref().filter(|_| playing).map(|g| g.debug_stats()));

        if automation.step(&mut app, shown.as_ref()) {
            break;
        }
        next_frame().await;
    }
}
