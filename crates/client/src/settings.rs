//! Your settings, saved in the browser as one JSON blob: volumes, mute, language, and the
//! controls (every action can be rebound, separately for keyboard/mouse and for gamepads).
//!
//! To add an action: a variant in `Act` (plus `ALL` and `id`), its default keys in
//! `Bindings::default`, and its name `act.<id>` in the text files. To add a setting: a field
//! here with a default; saved settings without it get the default.

use crate::platform::{self, Pad};
use crate::text::{t, tf};
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

/// Where the settings are saved (localStorage key).
const PREF_KEY: &str = "go-box-settings";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Overall volume (0..1): everything the game plays.
    pub master: f32,
    /// Sound effects, under `master`.
    pub effects: f32,
    /// Music, under `master` (there's no music yet).
    pub music: f32,
    pub muted: bool,
    /// A language code from the text files, e.g. "en-US".
    pub language: String,
    pub bindings: Bindings,
}

impl Default for Settings {
    fn default() -> Self {
        Self { master: 0.8, effects: 1.0, music: 0.7, muted: false, language: "en-US".into(), bindings: Bindings::default() }
    }
}

impl Settings {
    /// The saved settings, or the defaults (nothing saved, or saved by an incompatible version).
    pub fn load() -> Self {
        let saved = platform::load_pref(PREF_KEY);
        let mut s: Settings = serde_json::from_str(&saved).unwrap_or_default();
        s.master = s.master.clamp(0.0, 1.0);
        s.effects = s.effects.clamp(0.0, 1.0);
        s.music = s.music.clamp(0.0, 1.0);
        if saved.is_empty() && platform::load_pref("go-box-muted") == "1" {
            // Saved by the version before settings were one blob.
            s.muted = true;
        }
        s
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            platform::save_pref(PREF_KEY, &json);
        }
    }
}

/// Something you can do, and bind keys or buttons to. (Moving with the left stick and aiming
/// with the mouse or right stick aren't bindable.)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Act {
    Left,
    Right,
    Jump,
    /// Grab when empty-handed, throw when holding something.
    Hand,
    Melee,
    Special,
    Settings,
    Mute,
}

impl Act {
    /// In the order the controls are listed.
    pub const ALL: [Act; 8] = [Act::Left, Act::Right, Act::Jump, Act::Hand, Act::Melee, Act::Special, Act::Settings, Act::Mute];

    /// A stable id; players see the text `act.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Act::Left => "left",
            Act::Right => "right",
            Act::Jump => "jump",
            Act::Hand => "hand",
            Act::Melee => "melee",
            Act::Special => "special",
            Act::Settings => "settings",
            Act::Mute => "mute",
        }
    }

    pub fn label(self) -> &'static str {
        t(&format!("act.{}", self.id()))
    }
}

/// One key, mouse button or gamepad button.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Bind {
    /// A key, by its `KeyCode` number (only keys in `KEY_NAMES` can be bound).
    Key(u16),
    /// 0 left, 1 right, 2 middle.
    Mouse(u8),
    /// A gamepad button by its standard-mapping number (see `PAD_NAMES`).
    Pad(u8),
}

/// The keys that can be bound, and what they're called: a literal label, or `key.<id>` for a
/// name looked up in the text files.
pub const KEY_NAMES: &[(KeyCode, &str)] = &[
    (KeyCode::A, "A"), (KeyCode::B, "B"), (KeyCode::C, "C"), (KeyCode::D, "D"), (KeyCode::E, "E"), (KeyCode::F, "F"),
    (KeyCode::G, "G"), (KeyCode::H, "H"), (KeyCode::I, "I"), (KeyCode::J, "J"), (KeyCode::K, "K"), (KeyCode::L, "L"),
    (KeyCode::M, "M"), (KeyCode::N, "N"), (KeyCode::O, "O"), (KeyCode::P, "P"), (KeyCode::Q, "Q"), (KeyCode::R, "R"),
    (KeyCode::S, "S"), (KeyCode::T, "T"), (KeyCode::U, "U"), (KeyCode::V, "V"), (KeyCode::W, "W"), (KeyCode::X, "X"),
    (KeyCode::Y, "Y"), (KeyCode::Z, "Z"),
    (KeyCode::Key0, "0"), (KeyCode::Key1, "1"), (KeyCode::Key2, "2"), (KeyCode::Key3, "3"), (KeyCode::Key4, "4"),
    (KeyCode::Key5, "5"), (KeyCode::Key6, "6"), (KeyCode::Key7, "7"), (KeyCode::Key8, "8"), (KeyCode::Key9, "9"),
    (KeyCode::Comma, ","), (KeyCode::Period, "."), (KeyCode::Slash, "/"), (KeyCode::Semicolon, ";"),
    (KeyCode::Apostrophe, "'"), (KeyCode::LeftBracket, "["), (KeyCode::RightBracket, "]"), (KeyCode::Minus, "-"),
    (KeyCode::Equal, "="), (KeyCode::Backslash, "\\"),
    (KeyCode::Space, "key.space"), (KeyCode::Enter, "key.enter"), (KeyCode::Tab, "key.tab"), (KeyCode::Escape, "key.escape"),
    (KeyCode::Backspace, "key.backspace"), (KeyCode::LeftShift, "key.shift"), (KeyCode::RightShift, "key.right_shift"),
    (KeyCode::LeftControl, "key.ctrl"), (KeyCode::RightControl, "key.right_ctrl"), (KeyCode::LeftAlt, "key.alt"),
    (KeyCode::RightAlt, "key.right_alt"), (KeyCode::Up, "key.up"), (KeyCode::Down, "key.down"), (KeyCode::Left, "key.left"),
    (KeyCode::Right, "key.right"),
];

/// Gamepad buttons by standard-mapping number; players see the text `pad.<name>`. (PlayStation
/// names: Cross is A on an Xbox pad, Circle is B, Square is X, Triangle is Y.)
pub const PAD_NAMES: [&str; 17] = ["cross", "circle", "square", "triangle", "l1", "r1", "l2", "r2", "share", "options", "l3", "r3", "up", "down", "left", "right", "home"];

fn key_code(code: u16) -> Option<KeyCode> {
    KEY_NAMES.iter().map(|(k, _)| *k).find(|k| *k as u16 == code)
}

fn mouse_button(b: u8) -> MouseButton {
    match b {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        _ => MouseButton::Middle,
    }
}

impl Bind {
    pub fn label(self) -> String {
        match self {
            Bind::Key(code) => {
                let name = KEY_NAMES.iter().find(|(k, _)| *k as u16 == code).map_or("?", |(_, n)| n);
                if name.starts_with("key.") { t(name).to_string() } else { name.to_string() }
            }
            Bind::Mouse(0) => t("mouse.left").to_string(),
            Bind::Mouse(1) => t("mouse.right").to_string(),
            Bind::Mouse(_) => t("mouse.middle").to_string(),
            Bind::Pad(b) => PAD_NAMES.get(b as usize).map_or_else(|| tf("pad.button", &[("n", &b.to_string())]), |n| t(&format!("pad.{n}")).to_string()),
        }
    }

    /// Held now. Mouse buttons only count when `mouse` is true (the mouse isn't busy with a menu).
    fn down(self, pad: &Pad, mouse: bool) -> bool {
        match self {
            Bind::Key(code) => key_code(code).is_some_and(is_key_down),
            Bind::Mouse(b) => mouse && is_mouse_button_down(mouse_button(b)),
            Bind::Pad(b) => pad.buttons & (1 << b) != 0,
        }
    }

    /// Pressed this frame (`pad_prev` is last frame's gamepad buttons).
    fn pressed(self, pad: &Pad, pad_prev: u32, mouse: bool) -> bool {
        match self {
            Bind::Key(code) => key_code(code).is_some_and(is_key_pressed),
            Bind::Mouse(b) => mouse && is_mouse_button_pressed(mouse_button(b)),
            Bind::Pad(b) => pad.buttons & (1 << b) != 0 && pad_prev & (1 << b) == 0,
        }
    }

    /// The key or mouse button pressed this frame, if it can be bound.
    pub fn capture_keyboard() -> Option<Bind> {
        if let Some(k) = get_last_key_pressed()
            && key_code(k as u16).is_some()
        {
            return Some(Bind::Key(k as u16));
        }
        [MouseButton::Left, MouseButton::Right, MouseButton::Middle].into_iter().position(is_mouse_button_pressed).map(|b| Bind::Mouse(b as u8))
    }

    /// The gamepad button pressed this frame.
    pub fn capture_pad(pad: &Pad, pad_prev: u32) -> Option<Bind> {
        (0..PAD_NAMES.len() as u8).find(|&b| pad.buttons & (1 << b) != 0 && pad_prev & (1 << b) == 0).map(Bind::Pad)
    }
}

/// What each action is bound to, on keyboard/mouse and on gamepads.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Bindings {
    pub keys: Vec<(Act, Vec<Bind>)>,
    pub pad: Vec<(Act, Vec<Bind>)>,
}

impl Default for Bindings {
    fn default() -> Self {
        use Bind::{Key, Mouse, Pad};
        let k = |c: KeyCode| Key(c as u16);
        Self {
            keys: vec![
                (Act::Left, vec![k(KeyCode::A), k(KeyCode::Left)]),
                (Act::Right, vec![k(KeyCode::D), k(KeyCode::Right)]),
                (Act::Jump, vec![k(KeyCode::W), k(KeyCode::Space), k(KeyCode::Up)]),
                (Act::Hand, vec![Mouse(0), k(KeyCode::E)]),
                (Act::Melee, vec![Mouse(1), k(KeyCode::R)]),
                (Act::Special, vec![k(KeyCode::LeftShift), k(KeyCode::RightShift), k(KeyCode::F), k(KeyCode::Q)]),
                (Act::Settings, vec![k(KeyCode::Escape)]),
                (Act::Mute, vec![k(KeyCode::M)]),
            ],
            pad: vec![
                (Act::Left, vec![Pad(14)]),
                (Act::Right, vec![Pad(15)]),
                (Act::Jump, vec![Pad(0), Pad(12)]),
                (Act::Hand, vec![Pad(2), Pad(7)]),
                (Act::Melee, vec![Pad(1), Pad(5)]),
                (Act::Special, vec![Pad(3), Pad(6), Pad(4)]),
                (Act::Settings, vec![Pad(9)]),
                (Act::Mute, vec![]),
            ],
        }
    }
}

impl Bindings {
    /// What `act` is bound to (falling back to the default if these bindings predate it).
    pub fn get(&self, act: Act, pad: bool) -> Vec<Bind> {
        let find = |b: &Bindings| (if pad { &b.pad } else { &b.keys }).iter().find(|(a, _)| *a == act).map(|(_, v)| v.clone());
        find(self).or_else(|| find(&Bindings::default())).unwrap_or_default()
    }

    /// Binds `act` to just `bind` (on its side: keyboard/mouse or gamepad).
    pub fn set(&mut self, act: Act, bind: Bind) {
        let list = if matches!(bind, Bind::Pad(_)) { &mut self.pad } else { &mut self.keys };
        list.retain(|(a, _)| *a != act);
        list.push((act, vec![bind]));
    }

    pub fn down(&self, act: Act, pad: &Pad, mouse: bool) -> bool {
        [false, true].into_iter().flat_map(|side| self.get(act, side)).any(|b| b.down(pad, mouse))
    }

    pub fn pressed(&self, act: Act, pad: &Pad, pad_prev: u32, mouse: bool) -> bool {
        [false, true].into_iter().flat_map(|side| self.get(act, side)).any(|b| b.pressed(pad, pad_prev, mouse))
    }

    /// "A/Left" etc.: what `act` is bound to on one side, or "-" if nothing.
    pub fn label(&self, act: Act, pad: bool) -> String {
        let binds = self.get(act, pad);
        if binds.is_empty() { "-".into() } else { binds.iter().map(|b| b.label()).collect::<Vec<_>>().join("/") }
    }

    /// The first thing `act` is bound to, for short hints like "[Shift] Gravity Bomb".
    pub fn short_label(&self, act: Act, pad: bool) -> String {
        self.get(act, pad).first().map_or_else(|| "-".into(), |b| b.label())
    }

    /// One line of controls for the HUD and menus, from the current bindings.
    pub fn help(&self, pad: bool) -> String {
        let s = |a| self.short_label(a, pad);
        let moving = if pad { t("help.stick").to_string() } else { format!("{}/{}", s(Act::Left), s(Act::Right)) };
        let aim = if pad { t("help.right_stick") } else { t("help.mouse") };
        [
            tf("help.move", &[("keys", &moving)]),
            tf("help.jump", &[("keys", &s(Act::Jump))]),
            tf("help.aim", &[("keys", aim)]),
            tf("help.hand", &[("keys", &s(Act::Hand))]),
            tf("help.melee", &[("keys", &s(Act::Melee))]),
            tf("help.special", &[("keys", &s(Act::Special))]),
            tf("help.settings", &[("keys", &s(Act::Settings))]),
        ]
        .join("   ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_has_default_bindings_and_a_unique_id() {
        let d = Bindings::default();
        for a in Act::ALL {
            assert!(!d.get(a, false).is_empty(), "{a:?} has no keyboard binding");
        }
        let mut ids: Vec<_> = Act::ALL.iter().map(|a| a.id()).collect();
        ids.dedup();
        assert_eq!(ids.len(), Act::ALL.len());
    }

    #[test]
    fn default_keys_are_bindable_keys() {
        for (_, binds) in Bindings::default().keys {
            for b in binds {
                if let Bind::Key(code) = b {
                    assert!(key_code(code).is_some(), "key {code:#x} isn't in KEY_NAMES");
                }
            }
        }
    }

    #[test]
    fn rebinding_replaces_one_side_and_survives_a_round_trip() {
        let mut s = Settings::default();
        s.bindings.set(Act::Jump, Bind::Key(KeyCode::K as u16));
        s.bindings.set(Act::Melee, Bind::Pad(5));
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.bindings.get(Act::Jump, false), vec![Bind::Key(KeyCode::K as u16)]);
        assert_eq!(back.bindings.get(Act::Jump, true), Bindings::default().get(Act::Jump, true), "the pad side is untouched");
        assert_eq!(back.bindings.get(Act::Melee, true), vec![Bind::Pad(5)]);
    }

    #[test]
    fn settings_from_an_older_version_fill_in_defaults() {
        let s: Settings = serde_json::from_str(r#"{"master":0.5,"bindings":{"keys":[["Jump",[{"Key":75}]]]}}"#).unwrap();
        assert_eq!(s.master, 0.5);
        assert_eq!(s.effects, 1.0);
        assert_eq!(s.bindings.get(Act::Melee, false), Bindings::default().get(Act::Melee, false));
        assert!(serde_json::from_str::<Settings>("not json").is_err());
    }
}
