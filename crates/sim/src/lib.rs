//! The game itself, shared by the server and the browser client.

pub mod brain;
pub mod boss;
pub mod sandmaw;
pub mod arena;
pub mod character;
pub mod item;
pub mod sim;

/// Re-exported so other crates name the same `Vec2` the sim uses.
pub use glam;

/// Declares a fieldless enum together with `ALL`, every variant in declaration order, so a new
/// variant can't be left out of it (lists like that drive menus, tables and tests).
#[macro_export]
macro_rules! enum_all {
    ($(#[$m:meta])* $vis:vis enum $name:ident { $($(#[$vm:meta])* $v:ident),+ $(,)? }) => {
        $(#[$m])* $vis enum $name { $($(#[$vm])* $v),+ }
        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: [$name; [$($name::$v),+].len()] = [$($name::$v),+];
        }
    };
}
