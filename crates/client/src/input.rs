//! Your controls: keyboard + mouse, or a gamepad (a PS4/PS5/Xbox pad in the browser). What each
//! key or button does comes from your `Bindings` (Settings > Controls); this turns them into
//! the sim's `Action` each tick.
//!
//! Players always run. Moving also works with the gamepad's left stick, and aiming follows the
//! mouse or the right stick; neither is rebindable. In menus the gamepad moves a highlight with
//! the d-pad or left stick and Cross presses it (also fixed, so you can't lock yourself out).
//!
//! The hand button is one button: held while empty-handed it picks up what you're touching, a
//! fresh press while holding something throws it. Presses are latched until a sim tick takes
//! them (a fast frame may run no ticks).

use crate::platform::{self, Pad};
use crate::settings::{Act, Bindings};
use crate::ui::Nav;
use go_box_sim::sim::{Action, Fighter};
use macroquad::prelude::*;

// Gamepad buttons used for menu navigation (standard mapping).
const PAD_CROSS: u32 = 0;
const PAD_UP: u32 = 12;
const PAD_DOWN: u32 = 13;
const PAD_LEFT: u32 = 14;
const PAD_RIGHT: u32 = 15;

const STICK_DEADZONE: f32 = 0.3;
/// The right stick has to be pushed this far to aim (else throws go straight ahead).
const AIM_DEADZONE: f32 = 0.4;
/// Holding the stick in a menu steps the highlight again after this long.
const NAV_REPEAT: f64 = 0.28;

#[derive(Default)]
pub struct Controls {
    pub pad: Pad,
    /// Gamepad buttons held last frame (to spot fresh presses).
    pub pad_prev: u32,
    /// The gamepad was used more recently than the keyboard/mouse.
    pub using_pad: bool,
    last_mouse: Vec2,
    throw: bool,
    special: bool,
    /// A dash pressed since the last tick: -1 left, 1 right, 0 the way you're going.
    dash: Option<f32>,
    /// Down was tapped since the last tick (a quick tap still drops you through).
    down_tap: bool,
    /// The left stick's menu direction last frame, and when it may step again.
    stick_nav: Option<Vec2>,
    stick_next: f64,
}

impl Controls {
    /// Reads the devices; call once per frame before anything else asks.
    pub fn poll(&mut self) {
        self.pad_prev = self.pad.buttons;
        self.pad = platform::gamepad();
        let mouse: Vec2 = mouse_position().into();
        let pad_active = self.pad.connected && (self.pad.buttons != 0 || self.pad.left.length() > STICK_DEADZONE || self.pad.right.length() > AIM_DEADZONE);
        let kb_active = mouse.distance(self.last_mouse) > 2.0 || get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left) || is_mouse_button_pressed(MouseButton::Right);
        if pad_active {
            self.using_pad = true;
        } else if kb_active {
            self.using_pad = false;
        }
        self.last_mouse = mouse;
    }

    fn pad_pressed(&self, b: u32) -> bool {
        self.pad.buttons & (1 << b) != 0 && self.pad_prev & (1 << b) == 0
    }

    /// `act` was pressed this frame (on any device).
    pub fn pressed(&self, b: &Bindings, act: Act) -> bool {
        b.pressed(act, &self.pad, self.pad_prev, true)
    }

    /// Menu navigation: a d-pad press, or the left stick pushed (repeating while held), and
    /// Cross to press.
    pub fn nav(&mut self) -> Nav {
        let mut dir = [(PAD_UP, vec2(0.0, -1.0)), (PAD_DOWN, vec2(0.0, 1.0)), (PAD_LEFT, vec2(-1.0, 0.0)), (PAD_RIGHT, vec2(1.0, 0.0))]
            .into_iter()
            .find(|&(b, _)| self.pad_pressed(b))
            .map(|(_, d)| d);
        let s = self.pad.left;
        let stick = if s.length() < 0.6 {
            None
        } else if s.x.abs() > s.y.abs() {
            Some(vec2(s.x.signum(), 0.0))
        } else {
            Some(vec2(0.0, s.y.signum()))
        };
        let now = get_time();
        if stick != self.stick_nav || (stick.is_some() && now >= self.stick_next) {
            if stick.is_some() {
                dir = dir.or(stick);
                self.stick_next = now + if stick != self.stick_nav { NAV_REPEAT * 1.5 } else { NAV_REPEAT };
            }
            self.stick_nav = stick;
        }
        Nav { dir, activate: self.pad_pressed(PAD_CROSS) }
    }

    /// Remembers this frame's presses. `mouse_free` is false when the mouse is busy with the
    /// UI (a menu is open, or it's over a button).
    pub fn latch(&mut self, b: &Bindings, holding: bool, mouse_free: bool) {
        self.throw |= holding && b.pressed(Act::Hand, &self.pad, self.pad_prev, mouse_free);
        self.special |= b.pressed(Act::Special, &self.pad, self.pad_prev, mouse_free);
        self.down_tap |= b.pressed(Act::Down, &self.pad, self.pad_prev, mouse_free);
        for (act, d) in [(Act::Dash, 0.0), (Act::DashLeft, -1.0), (Act::DashRight, 1.0)] {
            if b.pressed(act, &self.pad, self.pad_prev, mouse_free) {
                self.dash = Some(d);
            }
        }
    }

    /// This tick's action for fighter `me`. `mouse_arena` is the mouse in arena coordinates.
    /// `melee_aim`: a swing goes toward your aim; off, the way you face. `auto_aim`: where to aim
    /// instead of the mouse or stick (a trackpad player's nearest opponent).
    pub fn take(&mut self, b: &Bindings, me: &Fighter, mouse_arena: Vec2, mouse_free: bool, melee_aim: bool, auto_aim: Option<Vec2>) -> Action {
        let down = |act| b.down(act, &self.pad, mouse_free);
        let mut mv = down(Act::Right) as i32 as f32 - down(Act::Left) as i32 as f32;
        if mv == 0.0 && self.pad.connected && self.pad.left.x.abs() > STICK_DEADZONE {
            mv = self.pad.left.x.signum();
        }
        let mut a = Action {
            mv,
            run: true,
            jump: down(Act::Jump),
            attack: down(Act::Melee),
            throw: self.throw,
            aim: 0.3,
            grab: me.holding.is_none() && down(Act::Hand),
            special: self.special,
            // (Auto-aimed: at them, lobbed a little high so arcing throws land.)
            target: match auto_aim {
                Some(at) => {
                    let d = at - me.hand();
                    Some((d + vec2(0.0, -d.length() * 0.3)).normalize_or(vec2(me.facing, 0.0)))
                }
                None => self.target(me, mouse_arena),
            },
            dash: self.dash,
            down: self.down_tap || down(Act::Down) || (self.pad.connected && self.pad.left.y > 0.6),
            // (Austin teleports to the cursor; with a gamepad there isn't one.)
            cursor: (!self.using_pad).then_some(mouse_arena),
        };
        // Swinging without aiming it: no aim this tick (the sim then swings the way you face), unless
        // a throw or special that wants it goes off too.
        if !melee_aim && a.attack && !a.throw && !a.special {
            a.target = None;
        }
        self.throw = false;
        self.special = false;
        self.dash = None;
        self.down_tap = false;
        a
    }

    /// Where you're aiming, as a direction from your hand: toward the mouse, or along the
    /// right stick (None = straight ahead, the sim's default arc).
    pub fn target(&self, me: &Fighter, mouse_arena: Vec2) -> Option<Vec2> {
        if self.using_pad {
            (self.pad.right.length() > AIM_DEADZONE).then(|| self.pad.right.normalize())
        } else {
            let d = mouse_arena - me.hand();
            (d.length() > 1.0).then(|| d.normalize())
        }
    }
}
