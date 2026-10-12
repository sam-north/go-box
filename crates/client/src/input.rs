//! Your controls: keyboard + mouse, or a gamepad (a PS4/PS5/Xbox pad in the browser).
//!
//! Players always run. Your left hand only moves and jumps; the hand actions live on the mouse.
//!
//! | | Keyboard + mouse | Gamepad |
//! |---|---|---|
//! | move | A/D (or arrows) | left stick / d-pad |
//! | jump | W or Space | Cross (A) |
//! | aim | mouse | right stick (else straight ahead) |
//! | grab / throw | left click (or E) | R1 / R2 / Circle (B) |
//! | melee | right click (or R) | Square (X) |
//! | special | Shift (or F, Q) | Triangle (Y) / L1 / L2 |
//! | settings | Esc | Options (Start) |
//!
//! "Grab / throw" is one hand button: held while empty-handed it picks up what you're touching,
//! a fresh press while holding something throws it. Presses are latched until a sim tick
//! takes them (a fast frame may run no ticks).

use crate::platform::{self, Pad};
use go_box_sim::sim::{Action, Fighter};
use macroquad::prelude::*;

// Standard gamepad button numbers (the browser's "standard" mapping).
pub const PAD_CROSS: u32 = 0;
pub const PAD_CIRCLE: u32 = 1;
pub const PAD_SQUARE: u32 = 2;
pub const PAD_TRIANGLE: u32 = 3;
pub const PAD_L1: u32 = 4;
pub const PAD_R1: u32 = 5;
pub const PAD_L2: u32 = 6;
pub const PAD_R2: u32 = 7;
pub const PAD_OPTIONS: u32 = 9;
pub const PAD_UP: u32 = 12;
pub const PAD_LEFT: u32 = 14;
pub const PAD_RIGHT: u32 = 15;

const STICK_DEADZONE: f32 = 0.3;
/// The right stick has to be pushed this far to aim (else throws go straight ahead).
const AIM_DEADZONE: f32 = 0.4;

fn bit(b: u32) -> u32 {
    1 << b
}

#[derive(Default)]
pub struct Controls {
    pad: Pad,
    /// Gamepad buttons held last frame (to spot fresh presses).
    pad_prev: u32,
    /// The gamepad was used more recently than the keyboard/mouse.
    pub using_pad: bool,
    last_mouse: Vec2,
    throw: bool,
    special: bool,
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

    fn pad_down(&self, b: u32) -> bool {
        self.pad.buttons & bit(b) != 0
    }

    pub fn pad_pressed(&self, b: u32) -> bool {
        self.pad_down(b) && self.pad_prev & bit(b) == 0
    }

    /// Esc, or Options on the pad.
    pub fn settings_pressed(&self) -> bool {
        is_key_pressed(KeyCode::Escape) || self.pad_pressed(PAD_OPTIONS)
    }

    /// Remembers this frame's presses. `mouse_free` is false when the mouse is busy with the
    /// UI (a menu is open, or it's over a button).
    pub fn latch(&mut self, holding: bool, mouse_free: bool) {
        let click = mouse_free && is_mouse_button_pressed(MouseButton::Left);
        let hand = click || is_key_pressed(KeyCode::E) || [PAD_R1, PAD_R2, PAD_CIRCLE].iter().any(|&b| self.pad_pressed(b));
        self.throw |= holding && hand;
        let keys = [KeyCode::LeftShift, KeyCode::RightShift, KeyCode::F, KeyCode::Q];
        self.special |= keys.iter().any(|&k| is_key_pressed(k)) || [PAD_TRIANGLE, PAD_L1, PAD_L2].iter().any(|&b| self.pad_pressed(b));
    }

    /// This tick's action for fighter `me`. `mouse_arena` is the mouse in arena coordinates.
    pub fn take(&mut self, me: &Fighter, mouse_arena: Vec2, mouse_free: bool) -> Action {
        let key = |keys: &[KeyCode]| keys.iter().any(|&k| is_key_down(k));
        let holding = me.holding.is_some();
        let mut mv = key(&[KeyCode::D, KeyCode::Right]) as i32 as f32 - key(&[KeyCode::A, KeyCode::Left]) as i32 as f32;
        if mv == 0.0 && self.pad.connected {
            let dpad = self.pad_down(PAD_RIGHT) as i32 as f32 - self.pad_down(PAD_LEFT) as i32 as f32;
            mv = if dpad != 0.0 { dpad } else if self.pad.left.x.abs() > STICK_DEADZONE { self.pad.left.x.signum() } else { 0.0 };
        }
        let mouse_down = |b| mouse_free && is_mouse_button_down(b);
        let a = Action {
            mv,
            run: true,
            jump: key(&[KeyCode::W, KeyCode::Space, KeyCode::Up]) || self.pad_down(PAD_CROSS) || self.pad_down(PAD_UP),
            attack: mouse_down(MouseButton::Right) || key(&[KeyCode::R]) || self.pad_down(PAD_SQUARE),
            throw: self.throw,
            aim: 0.3,
            grab: !holding && (mouse_down(MouseButton::Left) || key(&[KeyCode::E]) || [PAD_R1, PAD_R2, PAD_CIRCLE].iter().any(|&b| self.pad_down(b))),
            special: self.special,
            target: self.target(me, mouse_arena),
        };
        self.throw = false;
        self.special = false;
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

    pub fn pad_connected(&self) -> bool {
        self.pad.connected
    }

    /// The key or button that fires the special, for the HUD.
    pub fn special_key(&self) -> &'static str {
        if self.using_pad { "Triangle" } else { "Shift" }
    }
}

pub const KEYS_HELP: &str = "A/D move   W/Space jump   Mouse aim   Left click grab/throw   Right click attack   Shift special   Esc settings";
pub const PAD_HELP: &str = "Stick move   Cross jump   Right stick aim   R1 grab/throw   Square attack   Triangle special   Options settings";
