//! The client's screens, drawn over the arena.

pub mod home;
pub mod hud;
pub mod howto;
pub mod lobby;
pub mod settings;

/// Seconds as m:ss.
pub fn mmss(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

use crate::text::{t, tf};
use go_box_sim::character::{Character, Special};

/// A character's name, in the player's language.
pub fn character_name(c: Character) -> &'static str {
    t(&format!("character.{}", c.id()))
}

pub fn special_name(s: &dyn Special) -> &'static str {
    t(&format!("special.{}", s.id()))
}

/// What to call whoever is in a seat: the player's name, or "Sam (bot)" for a bot.
pub fn seat_name(name: &str, character: Character, bot: bool) -> String {
    if bot { tf("seat.bot", &[("character", character_name(character))]) } else { name.to_string() }
}
