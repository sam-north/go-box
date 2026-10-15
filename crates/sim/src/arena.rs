//! The maps. Every arena is the same W x H box (the render target, the camera and the bots'
//! senses all assume that size) with the same walls, ceiling and floor (`sim::SOLIDS`); what
//! differs is the one-way platforms, any solid blocks of its own (`walls`: to stand on and
//! wall-jump off), and where each seat starts.
//!
//! Adding a map: a variant in `ArenaId`, its row in `ArenaId::def`, and its name `arena.<id>`
//! in the client's text files. Platforms should be 36-40px apart vertically (a full jump
//! clears ~48) and at least 3px from the walls; the bots were trained on the "Tiers"-style
//! layout, so very different shapes may confuse them.
//! The client shades every map's backdrop around its ledges and walls (`make_background` in the
//! render crate) so they stand out; a new backdrop gets that for free.

use crate::sim::{Aabb, N_FIGHTERS};
use serde::{Deserialize, Serialize};

pub struct Arena {
    /// A stable id; players see the text `arena.<id>`.
    pub id: &'static str,
    /// One-way platforms: you can jump up through them and land on top.
    pub platforms: &'static [Aabb],
    /// Solid blocks besides the box's walls, floor and ceiling: nothing passes through them, you
    /// can stand on top, and wall-jump off their sides.
    pub walls: &'static [Aabb],
    /// Seat i starts somewhere in `spawn_x[i] .. spawn_x[i] + SPAWN_SPREAD` on the floor (seats
    /// left to right).
    pub spawn_x: [f32; N_FIGHTERS],
}

/// How far apart a seat's possible starting spots range.
pub const SPAWN_SPREAD: f32 = 90.0;

crate::enum_all! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
    pub enum ArenaId {
        /// "Classic": four tiers of ledges, mirrored left/right, with a long bridge on top.
        #[default]
        Tiers,
        /// "City": two long bridges over an open floor, with steps up at the sides, and two steel
        /// pillars hanging from the high bridge to wall-jump up between.
        Bridges,
        /// "Forest": tall trees up each side with branches all the way up, and a short one in
        /// the middle (a branch each way) that you jump or wall-jump over, not walk through.
        Forest,
    }
}

impl Arena {
    /// Everything solid: the box (ceiling, floor, side walls) and this map's own walls.
    pub fn solids(&self) -> impl Iterator<Item = &Aabb> + Clone {
        crate::sim::SOLIDS.iter().chain(self.walls)
    }
}

impl ArenaId {
    pub fn def(self) -> &'static Arena {
        match self {
            ArenaId::Tiers => &TIERS,
            ArenaId::Bridges => &BRIDGES,
            ArenaId::Forest => &FOREST,
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
    walls: &[],
    spawn_x: [20.0, 135.0, 250.0, 365.0],
};

/// (Shown as "City": its id is older than its name.)
const BRIDGES: Arena = Arena {
    id: "bridges",
    platforms: &[
        // Steps up at each side.
        Aabb::new(16.0, 216.0, 56.0, 5.0),
        Aabb::new(408.0, 216.0, 56.0, 5.0),
        Aabb::new(16.0, 180.0, 40.0, 5.0),
        Aabb::new(424.0, 180.0, 40.0, 5.0),
        // The low bridge, broken wide in the middle.
        Aabb::new(80.0, 180.0, 112.0, 5.0),
        Aabb::new(288.0, 180.0, 112.0, 5.0),
        // The high bridge, and a perch over the gap.
        Aabb::new(112.0, 144.0, 256.0, 5.0),
        Aabb::new(208.0, 108.0, 64.0, 5.0),
    ],
    // Two pillars hanging from the high bridge, high enough off the floor to walk under: wall-jump
    // up the shaft between them (or off their outsides) and through the bridge.
    walls: &[Aabb::new(204.0, 149.0, 8.0, 73.0), Aabb::new(268.0, 149.0, 8.0, 73.0)],
    spawn_x: [20.0, 135.0, 250.0, 365.0],
};

/// The trunks: the side trees stand in place of the box's walls, floor to ceiling, the middle
/// one up to its two branches. (The renderer draws the trees from these, so they're named.)
pub const FOREST_TRUNKS: [Aabb; 3] = [Aabb::new(0.0, 0.0, 16.0, 254.0), Aabb::new(464.0, 0.0, 16.0, 254.0), Aabb::new(234.0, 214.0, 12.0, 40.0)];

const FOREST: Arena = Arena {
    id: "forest",
    platforms: &[
        // Branches off the left tree, all the way up, then the same off the right one.
        Aabb::new(16.0, 214.0, 32.0, 5.0),
        Aabb::new(16.0, 178.0, 42.0, 5.0),
        Aabb::new(16.0, 142.0, 30.0, 5.0),
        Aabb::new(16.0, 106.0, 46.0, 5.0),
        Aabb::new(16.0, 70.0, 28.0, 5.0),
        Aabb::new(432.0, 214.0, 32.0, 5.0),
        Aabb::new(422.0, 178.0, 42.0, 5.0),
        Aabb::new(434.0, 142.0, 30.0, 5.0),
        Aabb::new(418.0, 106.0, 46.0, 5.0),
        Aabb::new(436.0, 70.0, 28.0, 5.0),
        // The middle tree, short: just a branch each way off the top of its trunk.
        Aabb::new(208.0, 214.0, 26.0, 5.0),
        Aabb::new(246.0, 214.0, 26.0, 5.0),
    ],
    walls: &FOREST_TRUNKS,
    // (Clear of the trunks: none can start inside one.)
    spawn_x: [28.0, 130.0, 254.0, 356.0],
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
                let below = FLOOR_Y - p.y <= 48.0 || d.platforms.iter().chain(d.walls).any(|q| q.y > p.y && q.y - p.y <= 48.0 && q.x < p.x + p.w + 40.0 && q.x + q.w > p.x - 40.0);
                assert!(below, "{} platform {p:?} can't be reached", d.id);
            }
            for x in d.spawn_x {
                assert!(x >= WALL + 4.0 && x + SPAWN_SPREAD <= W - WALL - 4.0, "{} spawn at {x}", d.id);
            }
        }
    }

    #[test]
    fn nobody_starts_and_nothing_drops_inside_a_wall() {
        use crate::sim::{ITEM_R, Match, MatchConfig};
        for a in ArenaId::ALL {
            for seed in 0..40 {
                let mut m = Match::new(seed, &MatchConfig { arena: a, ..MatchConfig::default() });
                for f in &m.f[..N_FIGHTERS] {
                    assert!(!a.def().walls.iter().any(|w| f.body().overlaps(w)), "{} seed {seed}: a fighter starts in a wall at {}", a.id(), f.pos);
                }
                for _ in 0..50 {
                    let p = m.random_surface_spot_between(0.0, W).unwrap() - glam::vec2(0.0, ITEM_R);
                    assert!(!a.def().walls.iter().any(|w| crate::sim::Aabb::around(p, ITEM_R).overlaps(w)), "{} seed {seed}: a spot in a wall at {p}", a.id());
                }
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
