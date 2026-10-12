//! The maps. Every arena is the same W x H box (the render target, the camera and the bots'
//! senses all assume that size) with the same walls, ceiling and floor (`sim::SOLIDS`); what
//! differs is the one-way platforms and where each seat starts.
//!
//! Adding a map: a variant in `ArenaId`, its row in `ArenaId::def`, and its name `arena.<id>`
//! in the client's text files. Platforms should be 36-40px apart vertically (a full jump
//! clears ~48) and at least 3px from the walls; the bots were trained on the "Tiers"-style
//! layout, so very different shapes may confuse them.

use crate::sim::{Aabb, N_FIGHTERS};
use serde::{Deserialize, Serialize};

pub struct Arena {
    /// A stable id; players see the text `arena.<id>`.
    pub id: &'static str,
    /// One-way platforms: you can jump up through them and land on top.
    pub platforms: &'static [Aabb],
    /// Seat i starts somewhere in `spawn_x[i] .. spawn_x[i] + SPAWN_SPREAD` on the floor (seats
    /// left to right).
    pub spawn_x: [f32; N_FIGHTERS],
}

/// How far apart a seat's possible starting spots range.
pub const SPAWN_SPREAD: f32 = 90.0;

crate::enum_all! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
    pub enum ArenaId {
        /// Four tiers of ledges, mirrored left/right, with a long bridge on top.
        #[default]
        Tiers,
        /// Two long bridges over an open floor, with steps up at the sides.
        Bridges,
    }
}

impl ArenaId {
    pub fn def(self) -> &'static Arena {
        match self {
            ArenaId::Tiers => &TIERS,
            ArenaId::Bridges => &BRIDGES,
        }
    }

    pub fn id(self) -> &'static str {
        self.def().id
    }
}

const TIERS: Arena = Arena {
    id: "tiers",
    platforms: &[
        // Low ledges off the floor.
        Aabb::new(40.0, 214.0, 72.0, 5.0),
        Aabb::new(368.0, 214.0, 72.0, 5.0),
        // Second tier.
        Aabb::new(124.0, 178.0, 72.0, 5.0),
        Aabb::new(284.0, 178.0, 72.0, 5.0),
        // Side perches and the centre.
        Aabb::new(44.0, 142.0, 64.0, 5.0),
        Aabb::new(372.0, 142.0, 64.0, 5.0),
        Aabb::new(204.0, 142.0, 72.0, 5.0),
        // The top bridge.
        Aabb::new(168.0, 106.0, 144.0, 5.0),
    ],
    spawn_x: [20.0, 135.0, 250.0, 365.0],
};

const BRIDGES: Arena = Arena {
    id: "bridges",
    platforms: &[
        // Steps up at each side.
        Aabb::new(16.0, 216.0, 56.0, 5.0),
        Aabb::new(408.0, 216.0, 56.0, 5.0),
        Aabb::new(16.0, 180.0, 40.0, 5.0),
        Aabb::new(424.0, 180.0, 40.0, 5.0),
        // The low bridge, broken in the middle.
        Aabb::new(80.0, 180.0, 136.0, 5.0),
        Aabb::new(264.0, 180.0, 136.0, 5.0),
        // The high bridge, and a perch over the gap.
        Aabb::new(112.0, 144.0, 256.0, 5.0),
        Aabb::new(208.0, 108.0, 64.0, 5.0),
    ],
    spawn_x: [20.0, 135.0, 250.0, 365.0],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{FLOOR_Y, W, WALL};

    #[test]
    fn every_arena_fits_the_box_and_has_reachable_tiers() {
        for a in ArenaId::ALL {
            let d = a.def();
            for p in d.platforms {
                assert!(p.x >= WALL + 3.0 && p.x + p.w <= W - WALL - 3.0, "{} platform {p:?} touches a wall", d.id);
                assert!(p.y < FLOOR_Y && p.y > WALL + 20.0, "{} platform {p:?} is outside the box", d.id);
                // Something to jump up from within reach: the floor or a lower platform.
                let below = FLOOR_Y - p.y <= 48.0 || d.platforms.iter().any(|q| q.y > p.y && q.y - p.y <= 48.0 && q.x < p.x + p.w + 40.0 && q.x + q.w > p.x - 40.0);
                assert!(below, "{} platform {p:?} can't be reached", d.id);
            }
            for x in d.spawn_x {
                assert!(x >= WALL + 4.0 && x + SPAWN_SPREAD <= W - WALL - 4.0, "{} spawn at {x}", d.id);
            }
        }
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = ArenaId::ALL.iter().map(|a| a.id()).collect();
        ids.dedup();
        assert_eq!(ids.len(), ArenaId::ALL.len());
    }
}
