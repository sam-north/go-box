//! The client's screens, drawn over the arena.

pub mod home;
pub mod hud;
pub mod lobby;
pub mod settings;

/// Seconds as m:ss.
pub fn mmss(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}
