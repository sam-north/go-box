//! Go Box client: home screen, room lobby / results overlay, and the match itself.
//!
//! Built for the browser (`scripts/build-web.sh`), it talks to the server that served the page.
//! Built natively it's a test client: `--server ws://127.0.0.1:7878/ws --name Sam --create`, and
//! a few flags for automated runs (see `Options`).

mod game;
mod input;
mod net;
mod platform;
mod ui;

use game::Game;
use go_box_protocol::*;
use go_box_render::sound::{Sfx, Sounds};
use go_box_render::{self as render, Art};
use go_box_sim::brain::Brain;
use go_box_sim::character::Character;
use go_box_sim::sim::{Action, DT, H, HEALS_PER_ROUND, MAX_TICKS, Match, N_FIGHTERS, TICK_RATE, W};
use macroquad::prelude::*;
use net::{Net, State};
use ::rand::rngs::SmallRng;
use ::rand::{Rng, SeedableRng};
use std::sync::OnceLock;
use input::{Controls, KEYS_HELP, PAD_HELP};
use ui::{ACCENT, BAD, DIM, Field, INK, Slider, Ui};

/// Command-line options (native test client only; the browser uses the defaults).
#[derive(Default)]
pub struct Options {
    pub server: String,
    pub name: Option<String>,
    pub join: Option<String>,
    create: bool,
    character: Option<Character>,
    /// As leader, start once this many players are in the room.
    start_when: Option<usize>,
    /// Play with simple scripted inputs (for automated tests).
    autopilot: bool,
    /// Write the room code here once in a room (so another test client can join it).
    code_file: Option<String>,
    shots: Option<String>,
    shot_every: f32,
    quit_after: Option<f32>,
    /// Open the settings menu at startup (for screenshots).
    settings: bool,
    /// Start with sound off.
    pub mute: bool,
}

static OPTIONS: OnceLock<Options> = OnceLock::new();

impl Options {
    pub fn get() -> &'static Options {
        OPTIONS.get_or_init(Options::parse)
    }

    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let val = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
        let has = |k: &str| args.iter().any(|a| a == k);
        Options {
            server: val("--server").unwrap_or_else(|| format!("ws://127.0.0.1:{DEFAULT_PORT}{WS_PATH}")),
            name: val("--name"),
            join: val("--join"),
            create: has("--create"),
            character: val("--character").and_then(|c| Character::ALL.into_iter().find(|x| x.name().eq_ignore_ascii_case(&c))),
            start_when: val("--start-when").and_then(|v| v.parse().ok()),
            autopilot: has("--autopilot"),
            code_file: val("--code-file"),
            shots: val("--shots"),
            shot_every: val("--shot-every").and_then(|v| v.parse().ok()).unwrap_or(3.0),
            quit_after: val("--quit-after").and_then(|v| v.parse().ok()),
            mute: has("--mute"),
            settings: has("--settings"),
        }
    }
}

fn conf() -> Conf {
    Conf { window_title: "Go Box".into(), window_width: 1280, window_height: 720, high_dpi: true, ..Default::default() }
}

/// Bots fighting behind the menus.
struct Attract {
    m: Match,
    brain: Brain,
    rng: SmallRng,
    acc: f32,
    pause: f32,
    /// A new match began since the arena art was last cleared.
    restarted: bool,
}

impl Attract {
    fn new() -> Self {
        let brain = Brain::from_json(include_str!("../../../brains/gen_219000.json")).expect("embedded brain");
        let mut rng = SmallRng::seed_from_u64((get_time() * 1e6) as u64 ^ 0x9e37_79b9);
        let m = Self::fresh(&mut rng);
        Self { m, brain, rng, acc: 0.0, pause: 0.0, restarted: false }
    }

    fn fresh(rng: &mut SmallRng) -> Match {
        let mut m = Match::new(rng, HEALS_PER_ROUND);
        for f in m.f.iter_mut() {
            f.character = Character::ALL[rng.gen_range(0..Character::ALL.len())];
        }
        m
    }

    /// Steps in real time; returns the events of the ticks run.
    fn frame(&mut self, dt: f32) -> Vec<go_box_sim::sim::Event> {
        let mut events = Vec::new();
        if self.m.over {
            self.pause += dt;
            if self.pause > 2.0 {
                self.m = Self::fresh(&mut self.rng);
                self.pause = 0.0;
                self.restarted = true;
            }
            return events;
        }
        self.acc += dt;
        let b = &self.brain;
        while self.acc >= DT && !self.m.over {
            self.m.step([b, b, b, b]);
            events.extend_from_slice(&self.m.events);
            self.acc -= DT;
        }
        events
    }
}

/// What the test client's autopilot does: chase the nearest opponent, swing, throw, use the
/// special now and then.
fn autopilot(m: &Match, seat: usize, rng: &mut SmallRng) -> Action {
    let me = &m.f[seat];
    let foe = (0..N_FIGHTERS).filter(|&j| j != seat && m.f[j].alive()).min_by(|&a, &b| me.pos.distance(m.f[a].pos).total_cmp(&me.pos.distance(m.f[b].pos)));
    let dx = foe.map_or(0.0, |j| m.f[j].pos.x - me.pos.x);
    Action {
        mv: if dx.abs() < 6.0 { 0.0 } else { dx.signum() },
        run: true,
        jump: rng.gen_bool(0.02),
        attack: dx.abs() < 14.0,
        throw: me.holding.is_some() && rng.gen_bool(0.03),
        aim: 0.3,
        grab: me.holding.is_none(),
        special: rng.gen_bool(0.01),
        target: None,
    }
}

struct App {
    net: Option<Net>,
    /// The name this connection said hello with.
    hello_name: Option<String>,
    hello_sent: bool,
    /// Sent as soon as the server welcomes us.
    after_welcome: Option<ClientMsg>,
    me: Option<PlayerId>,
    room: Option<RoomInfo>,
    game: Option<Game>,
    name: String,
    code: String,
    notice: Option<(String, f64)>,
    lost: bool,
    seq: u32,
    acc: f32,
    controls: Controls,
    /// Arena-to-screen mapping of the last frame drawn: (top-left on screen, pixels per unit).
    view: (Vec2, f32),
    /// The settings menu is open.
    settings_open: bool,
    /// When the volume preview last played.
    preview_at: f64,
    rng: SmallRng,
    code_written: bool,
    /// The shown match had ended last frame (to play the result sound once).
    was_over: bool,
}

impl App {
    fn notify(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), get_time()));
    }

    /// Sends `msg` (CreateRoom / JoinRoom), connecting first if needed.
    fn enter(&mut self, msg: ClientMsg) {
        let name = self.name.trim().to_string();
        let name = if name.is_empty() { "Player".to_string() } else { name };
        platform::save_name(&name);
        let reuse = self.me.is_some() && self.hello_name.as_deref() == Some(&name) && self.net.as_mut().is_some_and(|n| n.state() == State::Open);
        if reuse {
            self.net.as_mut().unwrap().send(&msg);
        } else {
            self.net = Some(Net::connect());
            self.me = None;
            self.hello_name = Some(name);
            self.hello_sent = false;
            self.after_welcome = Some(msg);
            self.lost = false;
        }
    }

    fn send(&mut self, msg: ClientMsg) {
        if let Some(n) = &mut self.net {
            n.send(&msg);
        }
    }

    fn update_net(&mut self, art: &mut Art, sfx: &mut Sounds) {
        let Some(net) = &mut self.net else { return };
        match net.state() {
            State::Connecting => return,
            State::Closed => {
                if !self.lost {
                    self.lost = true;
                    let msg = if self.me.is_some() { "Lost connection to the server." } else { "Can't reach the server." };
                    self.notify(msg);
                }
                self.net = None;
                self.me = None;
                self.room = None;
                self.game = None;
                art.reset();
                return;
            }
            State::Open => {}
        }
        if !self.hello_sent {
            let name = self.hello_name.clone().unwrap_or_default();
            net.send(&ClientMsg::Hello { version: PROTOCOL_VERSION, name });
            self.hello_sent = true;
        }
        for msg in net.poll() {
            match msg {
                ServerMsg::Welcome { you } => {
                    self.me = Some(you);
                    if let Some(m) = self.after_welcome.take() {
                        net.send(&m);
                    }
                }
                ServerMsg::Error(e) => self.notice = Some((e, get_time())),
                ServerMsg::Room(r) => {
                    platform::set_url_room(Some(&r.code));
                    if let Some(c) = Options::get().character
                        && self.room.is_none()
                    {
                        net.send(&ClientMsg::SetCharacter(c));
                    }
                    self.room = Some(r);
                }
                ServerMsg::LeftRoom => {
                    platform::set_url_room(None);
                    self.room = None;
                    self.game = None;
                    art.reset();
                }
                ServerMsg::MatchStart(s) => {
                    self.game = Some(Game::new(s));
                    self.acc = 0.0;
                    self.was_over = false;
                    art.reset();
                    sfx.reset();
                    sfx.play(Sfx::Start, 1.0);
                }
                ServerMsg::Snapshot(s) => {
                    if let Some(g) = &mut self.game {
                        g.on_snapshot(s);
                    }
                }
            }
        }
    }
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
        code_written: false,
        was_over: false,
    };
    if opts.create {
        app.enter(ClientMsg::CreateRoom);
    } else if let Some(code) = &opts.join {
        app.enter(ClientMsg::JoinRoom { code: code.clone() });
    }
    let (mut next_shot, mut shot_n) = (opts.shot_every, 0);

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

        // Test-client automation.
        if let Some(r) = &app.room {
            if let Some(path) = &opts.code_file
                && !app.code_written
            {
                let _ = std::fs::write(path, &r.code);
                app.code_written = true;
            }
            let leader = app.me == Some(r.leader);
            if leader && r.phase == Phase::Lobby && r.last_results.is_none() && opts.start_when.is_some_and(|n| r.players.len() >= n) {
                app.send(ClientMsg::Start);
            }
        }
        if let Some(prefix) = &opts.shots
            && get_time() as f32 >= next_shot
        {
            shot_n += 1;
            get_screen_data().export_png(&format!("{prefix}_{shot_n:02}.png"));
            next_shot += opts.shot_every;
        }
        if opts.quit_after.is_some_and(|q| get_time() as f32 >= q) {
            break;
        }
        next_frame().await;
    }
}

/// Dots showing where a throw would go.
fn draw_aim(m: &Match, seat: usize, target: Option<Vec2>) {
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
fn draw_you(m: &Match, seat: usize) {
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

// Settings saved in the browser.
const PREF_MUTED: &str = "go-box-muted";
const PREF_MASTER: &str = "go-box-volume-master";
const PREF_EFFECTS: &str = "go-box-volume-effects";

impl App {
    /// The controls line for whichever device you're using.
    fn controls_help(&self) -> &'static str {
        if self.controls.using_pad { PAD_HELP } else { KEYS_HELP }
    }
}

/// The small button that opens the settings menu, bottom right on every screen.
fn settings_button(ui: &Ui) -> Rect {
    let (w, h) = (ui.px(110.0), ui.px(32.0));
    Rect::new(screen_width() - w - ui.px(10.0), screen_height() - h - ui.px(10.0), w, h)
}

/// Volume sliders (saved in the browser), mute, and the controls for keyboard and gamepad.
fn settings_screen(ui: &mut Ui, app: &mut App, sfx: &mut Sounds) {
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

fn connecting_screen(ui: &mut Ui) {
    let t = "Connecting...";
    ui.text_centered(t, screen_width() / 2.0, screen_height() / 2.0, 32.0, INK);
}

fn home_screen(ui: &mut Ui, app: &mut App) {
    let (sw, sh) = (screen_width(), screen_height());
    let w = ui.px(460.0).min(sw - 32.0);
    let h = ui.px(400.0);
    let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
    ui.panel(r);
    let cx = sw / 2.0;
    let pad = ui.px(28.0);
    ui.text_centered("GO BOX", cx, r.y + ui.px(64.0), 64.0, ACCENT);
    ui.text_centered("4-player arena. Bots fill the empty seats.", cx, r.y + ui.px(96.0), 20.0, DIM);

    let fw = w - pad * 2.0;
    let rowh = ui.px(46.0);
    let mut y = r.y + ui.px(126.0);
    ui.text("Your name", r.x + pad, y + ui.px(4.0), 18.0, DIM);
    y += ui.px(10.0);
    let name_enter = ui.field(Field::Name, Rect::new(r.x + pad, y, fw, rowh), &mut app.name, "Type your name", MAX_NAME_LEN, |c| (!c.is_control()).then_some(c));
    y += rowh + ui.px(18.0);
    let create = ui.button(Rect::new(r.x + pad, y, fw, rowh), "Create a room", true, ACCENT);
    y += rowh + ui.px(32.0);
    ui.text("Or join a friend's room", r.x + pad, y, 18.0, DIM);
    y += ui.px(10.0);
    let bw = ui.px(120.0);
    let code_enter = ui.field(Field::Code, Rect::new(r.x + pad, y, fw - bw - ui.px(12.0), rowh), &mut app.code, "Room code", CODE_LEN, |c| {
        let c = c.to_ascii_uppercase();
        CODE_ALPHABET.contains(&(c as u8)).then_some(c)
    });
    let join = ui.button(Rect::new(r.x + pad + fw - bw, y, bw, rowh), "Join", app.code.len() == CODE_LEN, ACCENT);
    if create || (name_enter && app.code.len() != CODE_LEN) {
        app.enter(ClientMsg::CreateRoom);
    } else if (join || code_enter || name_enter) && app.code.len() == CODE_LEN {
        let code = app.code.clone();
        app.enter(ClientMsg::JoinRoom { code });
    }
    ui.text_centered(app.controls_help(), cx, sh - ui.px(20.0), 18.0, DIM);
}

/// The special's name and its cooldown, as two short lines for a character card.
fn character_lines(c: Character) -> (String, String) {
    match c.special() {
        Some(s) => (s.name().into(), format!("every {}s", s.cooldown() / TICK_RATE)),
        None => ("No special".into(), "the original".into()),
    }
}

fn mmss(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

fn lobby_screen(ui: &mut Ui, app: &mut App, room: &RoomInfo, portraits: &[RenderTarget]) {
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
    ui.text(&format!("ROOM {}", room.code), r.x + pad, y, 36.0, ACCENT);
    let bw = ui.px(170.0);
    if ui.button(Rect::new(r.x + w - pad - bw, y - ui.px(30.0), bw, ui.px(38.0)), "Copy invite link", true, ACCENT) {
        platform::copy(&platform::share_link(&room.code));
        app.notify("Invite link copied");
    }
    y += ui.px(20.0);

    // Last match's scoreboard.
    if let Some(res) = &room.last_results {
        y += ui.px(16.0);
        let banner = match res.winner {
            Some(w) => format!("{} wins!", res.seats[w].name),
            None => "Draw!".to_string(),
        };
        ui.text(&banner, r.x + pad, y + ui.px(10.0), 30.0, INK);
        ui.text(&format!("match length {}", mmss(res.duration)), r.x + w - pad - ui.text_width("match length 0:00", 18.0), y + ui.px(10.0), 18.0, DIM);
        y += ui.px(36.0);
        let cols = [0.0, 0.30, 0.47, 0.57, 0.70, 0.82];
        let heads = ["Player", "Character", "Kills", "Headshots", "Damage", "Went down"];
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
                None => format!("survived, {:.0} HP", s.hp),
            };
            let cells = [s.name.clone(), s.character.name().to_string(), s.kills.to_string(), s.headshots.to_string(), format!("{:.0}", s.damage_dealt), fate];
            for (k, cell) in cells.iter().enumerate() {
                ui.text(cell, r.x + pad + cols[k] * tw, y, 20.0, if k == 0 { color } else { INK });
            }
        }
        y += ui.px(24.0);
        draw_line(r.x + pad, y, r.x + w - pad, y, 1.0, ui::EDGE);
    }

    // Who's here.
    y += ui.px(34.0);
    ui.text("Players", r.x + pad, y, 20.0, DIM);
    let diff_x = r.x + w * 0.55;
    ui.text("Bot difficulty", diff_x, y, 20.0, DIM);
    let lines: Vec<String> = room
        .players
        .iter()
        .map(|p| {
            let mut line = p.name.clone();
            if p.id == room.leader {
                line.push_str("  (leader)");
            }
            if Some(p.id) == app.me {
                line.push_str("  - you");
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
        let status = if p.ready && p.id != room.leader { "ready" } else { "" };
        ui.text(&format!("{}  {status}", p.character.name()), char_x, py, 20.0, if p.ready { ACCENT } else { DIM });
    }
    // Seats without a player: a bot or nobody. The leader picks; down to just you.
    for seat in room.players.len()..MAX_PLAYERS {
        py += ui.px(28.0);
        let fill = room.fills[seat];
        ui.text(&format!("Seat {}", seat + 1), r.x + pad, py, 20.0, DIM);
        let bx = r.x + pad + ui.px(72.0);
        if leader {
            let bw = ui.px(64.0);
            for (k, (label, f)) in [("Bot", SeatFill::Bot), ("None", SeatFill::Empty)].into_iter().enumerate() {
                let br = Rect::new(bx + k as f32 * (bw + ui.px(6.0)), py - ui.px(20.0), bw, ui.px(26.0));
                if ui.button(br, label, true, if fill == f { ACCENT } else { ui::EDGE }) && fill != f {
                    app.send(ClientMsg::SetSeatFill { seat: seat as u8, fill: f });
                }
            }
        } else {
            let label = if fill == SeatFill::Bot { "bot" } else { "empty" };
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
            if ui.button(rr, d.name(), true, if selected { ACCENT } else { ui::EDGE }) && !selected {
                app.send(ClientMsg::SetDifficulty(d));
            }
        } else if selected {
            ui.text(d.name(), dx, y + ui.px(36.0), 22.0, INK);
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
        ui.text(c.name(), tx, rr.y + ui.px(36.0), 26.0, color);
        let (special, cooldown) = character_lines(c);
        let mut ly = rr.y + ui.px(58.0);
        for line in ui.wrap(&special, 15.0, rr.x + rr.w - tx - ui.px(6.0)) {
            ui.text(&line, tx, ly, 15.0, INK);
            ly += ui.px(17.0);
        }
        ui.text(&cooldown, tx, ly, 15.0, DIM);
        if selected {
            ui.text("picked", tx, rr.y + rr.h - ui.px(8.0), 15.0, color);
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
        let label = if again { "Play again" } else { "Start match" };
        if ui.button(Rect::new(r.x + pad, y, bw, bh), label, true, ACCENT) {
            app.send(ClientMsg::Start);
        }
        if others > 0 {
            ui.text(&format!("{ready}/{others} ready"), r.x + pad + bw + ui.px(16.0), y + bh * 0.62, 20.0, DIM);
        }
    } else {
        let me_ready = room.players.iter().any(|p| Some(p.id) == app.me && p.ready);
        let label = if me_ready { "Ready!" } else if again { "Play again" } else { "Ready" };
        if ui.button(Rect::new(r.x + pad, y, bw, bh), label, true, if me_ready { ACCENT } else { INK }) {
            app.send(ClientMsg::SetReady(!me_ready));
        }
        ui.text("The leader starts the match", r.x + pad + bw + ui.px(16.0), y + bh * 0.62, 18.0, DIM);
    }
    if ui.button(Rect::new(r.x + w - pad - ui.px(140.0), y, ui.px(140.0), bh), "Exit", true, BAD) {
        app.send(ClientMsg::LeaveRoom);
    }
    ui.text_centered(app.controls_help(), sw / 2.0, sh - ui.px(12.0), 18.0, DIM);
}

/// Names and health across the top, the clock, and your special at the bottom.
fn hud(ui: &Ui, app: &App, m: &Match) {
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
