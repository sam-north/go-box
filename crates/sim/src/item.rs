//! The things lying around the arena, as data: one `ItemDef` per `ItemKind`. Adding a weapon is
//! a variant, its row in `ItemKind::def`, a place in `SPAWN_ORDER`, and how it's drawn and
//! sounds (the render crate's `draw_item` and `Clatter` sound, which the compiler points to).
//!
//! The bots' brains were trained on exactly three kinds of item, so every item says which of
//! those it looks like to them (`sensed_as`). A new item never changes the brains' inputs.

use crate::sim::{Source, secs};
use serde::{Deserialize, Serialize};

crate::enum_all! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
    pub enum ItemKind {
        Spear,
        Rock,
        Grenade,
        /// Justin's sushi: only ever served on his table, thrown like a rock, and gone in a splat.
        Sushi,
        /// Jake's phone: he hides in it for a while, and it stays behind as something to throw.
        Phone,
        /// Fires one homing rocket, then it's spent. Rare: at most a couple a match.
        RocketLauncher,
    }
}

/// What an item looks like to the bots: one of the three kinds their brains were trained on.
/// Fixed forever; the number is the slot in the brain's inputs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sensed {
    Spear = 0,
    Rock = 1,
    Grenade = 2,
}

/// How many `Sensed` kinds there are (the brains' inputs have a slot for each).
pub const N_SENSED: usize = 3;

/// Damage done by something: base damage, the multiplier for a headshot, and what to call it.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub damage: f32,
    pub headshot_mult: f32,
    pub source: Source,
}

/// An explosion: full damage at the centre, falling off to `edge` (a fraction) at `radius`.
#[derive(Clone, Copy, Debug)]
pub struct Blast {
    pub radius: f32,
    pub damage: f32,
    pub edge: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub sensed_as: Sensed,
    pub throw_speed: f32,
    /// Gravity (as a fraction of a fighter's) while flying fast enough to hurt.
    pub flight_gravity: f32,
    /// How much speed it keeps bouncing off terrain.
    pub bounce: f32,
    /// What it does to someone it hits in flight (None: thrown, it doesn't hurt on impact).
    pub thrown: Option<Hit>,
    /// Held, a swing uses it (reach, hit) instead of fists.
    pub melee: Option<(f32, Hit)>,
    /// Thrown, it lights a fuse of this many ticks and then explodes.
    pub fuse: Option<u32>,
    pub blast: Option<Blast>,
    /// Chance (0..1) that a new item is this kind, and how many can exist at once.
    pub spawn_chance: f32,
    pub max_alive: Option<usize>,
    /// Thrown, it's used up when it hits anyone or anything (a splat), and left lying around
    /// it spoils after this many ticks.
    pub splats: Option<u32>,
    /// At most this many spawn in a whole match.
    pub per_match: Option<u32>,
    /// "Throwing" it fires a homing rocket instead, and the launcher is used up.
    pub launches: bool,
}

// Headshots multiply damage: rocks "hitting the right spot" are devastating.
const HEADSHOT_MULT: f32 = 2.0;

const SPEAR: ItemDef = ItemDef {
    sensed_as: Sensed::Spear,
    throw_speed: 280.0,
    // Thrown spears fly flat; everything else arcs.
    flight_gravity: 0.35,
    bounce: 0.15,
    thrown: Some(Hit { damage: 28.0, headshot_mult: HEADSHOT_MULT, source: Source::SpearThrow }),
    melee: Some((14.0, Hit { damage: 16.0, headshot_mult: HEADSHOT_MULT, source: Source::Stab })),
    fuse: None,
    blast: None,
    spawn_chance: 0.18,
    max_alive: None,
    splats: None,
    per_match: None,
    launches: false,
};

const ROCK: ItemDef = ItemDef {
    sensed_as: Sensed::Rock,
    throw_speed: 230.0,
    flight_gravity: 0.8,
    bounce: 0.35,
    thrown: Some(Hit { damage: 10.0, headshot_mult: 3.0, source: Source::Rock }),
    melee: None,
    fuse: None,
    blast: None,
    // Whatever's left over.
    spawn_chance: 0.14,
    max_alive: None,
    splats: None,
    per_match: None,
    launches: false,
};

const GRENADE: ItemDef = ItemDef {
    sensed_as: Sensed::Grenade,
    throw_speed: 180.0,
    flight_gravity: 0.8,
    bounce: 0.45,
    thrown: None,
    melee: None,
    fuse: Some(secs(1)),
    blast: Some(Blast { radius: 34.0, damage: 85.0, edge: 0.35 }),
    spawn_chance: 0.62,
    max_alive: Some(8),
    splats: None,
    per_match: None,
    launches: false,
};

const SUSHI: ItemDef = ItemDef {
    sensed_as: Sensed::Rock,
    throw_speed: 230.0,
    flight_gravity: 0.8,
    bounce: 0.0,
    thrown: Some(Hit { damage: 10.0, headshot_mult: 3.0, source: Source::Sushi }),
    melee: None,
    fuse: None,
    blast: None,
    // Never spawns on its own: Justin serves it.
    spawn_chance: 0.0,
    max_alive: None,
    splats: Some(secs(15)),
    per_match: None,
    launches: false,
};

const PHONE: ItemDef = ItemDef {
    sensed_as: Sensed::Rock,
    throw_speed: 230.0,
    flight_gravity: 0.8,
    bounce: 0.3,
    thrown: Some(Hit { damage: 10.0, headshot_mult: 3.0, source: Source::Rock }),
    melee: None,
    fuse: None,
    blast: None,
    // Never spawns on its own: Jake drops it.
    spawn_chance: 0.0,
    max_alive: None,
    splats: None,
    per_match: None,
    launches: false,
};

const ROCKET_LAUNCHER: ItemDef = ItemDef {
    // To the bots it's something to throw at people, which is what firing it amounts to.
    sensed_as: Sensed::Spear,
    throw_speed: 0.0,
    flight_gravity: 0.8,
    bounce: 0.2,
    thrown: None,
    melee: None,
    fuse: None,
    blast: None,
    spawn_chance: 0.06,
    max_alive: Some(1),
    splats: None,
    per_match: Some(2),
    launches: true,
};

/// A rocket's explosion.
pub const ROCKET_BLAST: Blast = Blast { radius: 26.0, damage: 60.0, edge: 0.4 };

/// Fists, when not holding something with its own melee: (reach, hit).
pub const FISTS: (f32, Hit) = (9.0, Hit { damage: 8.0, headshot_mult: HEADSHOT_MULT, source: Source::Fist });

/// A grenade dropped by someone who dies while holding it goes off this soon (or sooner, if it
/// was already lit).
pub const DROPPED_FUSE: u32 = 30;

/// The order spawn chances are rolled in: a random number in 0..1 picks the first kind whose
/// running total of chances it falls under (skipping kinds already at their limit). The chances
/// add up to 1.
pub const SPAWN_ORDER: [ItemKind; 4] = [ItemKind::Grenade, ItemKind::RocketLauncher, ItemKind::Spear, ItemKind::Rock];

impl ItemKind {
    pub const fn def(self) -> &'static ItemDef {
        match self {
            ItemKind::Spear => &SPEAR,
            ItemKind::Rock => &ROCK,
            ItemKind::Grenade => &GRENADE,
            ItemKind::Sushi => &SUSHI,
            ItemKind::Phone => &PHONE,
            ItemKind::RocketLauncher => &ROCKET_LAUNCHER,
        }
    }
}

/// The grenade's blast and fuse, for drawing and senses that are about grenades in particular.
pub const GRENADE_BLAST: Blast = match GRENADE.blast {
    Some(b) => b,
    None => panic!(),
};
pub const GRENADE_FUSE: u32 = match GRENADE.fuse {
    Some(f) => f,
    None => panic!(),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_spawns_from_the_table() {
        for k in ItemKind::ALL.into_iter().filter(|k| k.def().spawn_chance > 0.0) {
            assert!(SPAWN_ORDER.contains(&k), "{k:?} isn't in SPAWN_ORDER");
        }
        let total: f32 = SPAWN_ORDER.iter().map(|k| k.def().spawn_chance).sum();
        assert!((total - 1.0).abs() < 1e-6, "spawn chances add up to {total}");
        assert!(SPAWN_ORDER.last().unwrap().def().max_alive.is_none(), "the last kind must always be able to spawn");
    }

    #[test]
    fn exploding_items_have_a_fuse() {
        for k in ItemKind::ALL {
            assert_eq!(k.def().fuse.is_some(), k.def().blast.is_some(), "{k:?}");
        }
    }
}
