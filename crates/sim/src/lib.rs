//! The game itself, shared by the server and the browser client.

pub mod brain;
pub mod character;
pub mod item;
pub mod sim;

/// Re-exported so other crates name the same `Vec2` the sim uses.
pub use glam;
