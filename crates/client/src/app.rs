//! The client's state: the connection, the room, the match, your choices; and handling what
//! the server says.

use crate::game::Game;
use crate::input::Controls;
use crate::settings::{Act, Settings};
use crate::text::t;
use crate::net::{Net, State};
use crate::platform;
use crate::testclient::Options;
use go_box_protocol::*;
use go_box_render::sound::{Sfx, Sounds};
use go_box_render::Art;
use macroquad::prelude::*;
use ::rand::rngs::SmallRng;

pub struct App {
    pub net: Option<Net>,
    /// The name this connection said hello with.
    pub hello_name: Option<String>,
    pub hello_sent: bool,
    /// Sent as soon as the server welcomes us.
    pub after_welcome: Option<ClientMsg>,
    pub me: Option<PlayerId>,
    pub room: Option<RoomInfo>,
    pub game: Option<Game>,
    pub name: String,
    pub code: String,
    /// The passphrase for creating rooms (remembered in this browser).
    pub passphrase: String,
    pub notice: Option<(String, f64)>,
    pub lost: bool,
    pub seq: u32,
    pub acc: f32,
    pub controls: Controls,
    /// Arena-to-screen mapping of the last frame drawn: (top-left on screen, pixels per unit).
    pub view: (Vec2, f32),
    pub settings: Settings,
    /// The settings menu is open, and which page of it.
    pub settings_open: bool,
    pub settings_tab: SettingsTab,
    /// Waiting for a key (or, if true, a gamepad button) to bind to this action.
    pub rebinding: Option<(Act, bool)>,
    /// When the volume preview last played.
    pub preview_at: f64,
    pub rng: SmallRng,
    /// The shown match had ended last frame (to play the result sound once).
    pub was_over: bool,
}

impl App {
    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), get_time()));
    }

    /// Sends `msg` (CreateRoom / JoinRoom), connecting first if needed.
    pub fn enter(&mut self, msg: ClientMsg) {
        let name = self.name.trim().to_string();
        let name = if name.is_empty() { t("home.default_name").to_string() } else { name };
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

    /// Creates a room with the remembered passphrase.
    pub fn create_room(&mut self) {
        platform::save_pref(PREF_PASSPHRASE, &self.passphrase);
        let passphrase = self.passphrase.clone();
        self.enter(ClientMsg::CreateRoom { passphrase });
    }

    pub fn send(&mut self, msg: ClientMsg) {
        if let Some(n) = &mut self.net {
            n.send(&msg);
        }
    }

    pub fn update_net(&mut self, art: &mut Art, sfx: &mut Sounds) {
        let Some(net) = &mut self.net else { return };
        match net.state() {
            State::Connecting => return,
            State::Closed => {
                // The server may have said why it closed the connection (e.g. this page is out of
                // date): show that rather than a plain "lost connection".
                let refused = match net.close_code() {
                    Some(CLOSE_OUT_OF_DATE) => Some(Refusal::OutOfDate),
                    Some(CLOSE_SERVER_FULL) => Some(Refusal::ServerFull),
                    _ => None,
                };
                if let Some(r) = refused {
                    self.notice = Some((t(&format!("refusal.{}", r.id())).to_string(), get_time()));
                }
                let mut explained = refused.is_some();
                for msg in net.poll() {
                    if let ServerMsg::Error(e) = msg {
                        self.notice = Some((t(&format!("refusal.{}", e.id())).to_string(), get_time()));
                        explained = true;
                    }
                }
                if !self.lost && !explained {
                    self.lost = true;
                    let msg = if self.me.is_some() { t("notice.connection_lost") } else { t("notice.server_unreachable") };
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
                ServerMsg::Error(e) => self.notice = Some((t(&format!("refusal.{}", e.id())).to_string(), get_time())),
                ServerMsg::Room(r) => {
                    platform::set_url_room(Some(&r.code));
                    if let Some(c) = Options::get().character
                        && self.room.is_none()
                    {
                        net.send(&ClientMsg::SetCharacter(c));
                    }
                    if let Some(a) = Options::get().arena
                        && self.room.is_none()
                    {
                        net.send(&ClientMsg::SetArena(a));
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

/// Where the room passphrase is remembered (localStorage).
pub const PREF_PASSPHRASE: &str = "go-box-passphrase";

impl App {
    /// The controls line for whichever device you're using.
    pub fn controls_help(&self) -> String {
        self.settings.bindings.help(self.controls.using_pad)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Sound,
    Controls,
}

