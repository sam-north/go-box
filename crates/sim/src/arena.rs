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

use crate::boss::BossKind;
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
    /// Ground rising from the floor (the Dune's slopes, as fine steps): solid like walls, but
    /// fighters walk up and down it (`sim::collide` steps them up, and keeps them on it going down)
    /// and things turn up on top of it.
    pub hills: &'static [Aabb],
    /// A campfire on the floor, burning all match long (like a Molotov's fire, but nobody's):
    /// each match puts it somewhere new, always with open sky above (`MatchConfig::campfire`).
    pub campfire: bool,
    /// A boss fight: everyone together against the boss in the middle (`boss.rs`;
    /// `MatchConfig::for_arena` makes it one).
    pub boss: Option<BossKind>,
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
        /// "Desktop": app windows floating on a wallpaper. Each window's title bar and bottom
        /// edge are ledges (drop through the title bar to land inside), its sides are walls.
        Desktop,
        /// "Courtyard Cafe": a coffee shop on the left, a courtyard on the right, a wall between them with a gap at the floor.
        Cafe,
        /// "Gatecrasher": a boss fight. A dungeon hall the boss smashes into through the back wall,
        /// standing in the middle; ledges climb either side up to its head, and nothing hangs over it (air strikes reach it).
        Lair,
        /// "Sandmaw": a boss fight in the desert. The middle of the floor is the monster's mouth (touch
        /// its throat and you're swallowed); skiffs hover either side, none over it.
        Dune,
    }
}

impl Arena {
    /// How high the ground is under something `half_w` either side of `x`: the floor, or the top of
    /// the hill there (the highest it overlaps).
    pub fn ground_at(&self, x: f32, half_w: f32) -> f32 {
        self.hills.iter().filter(|h| h.x < x + half_w && h.x + h.w > x - half_w).map(|h| h.y).fold(crate::sim::FLOOR_Y, f32::min)
    }

    /// The stretch of floor that swallows whoever touches it (a pit boss's mouth), if any.
    pub fn pit(&self) -> Option<(f32, f32)> {
        self.boss.and_then(|k| k.pit())
    }

    /// Everything solid: the box (ceiling, floor, side walls) and this map's own walls.
    pub fn solids(&self) -> impl Iterator<Item = &Aabb> + Clone {
        crate::sim::SOLIDS.iter().chain(self.walls).chain(self.hills)
    }
}

/// A fingerprint of every map's layout (ledges, walls, starting spots, whether it has a
/// campfire): the server sends its own, so a client built with different maps (an old browser tab) knows to reload
/// rather than play on walls the server doesn't have.
pub fn layout_fingerprint() -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for a in ArenaId::ALL {
        let d = a.def();
        let boxes = d.platforms.iter().chain(d.walls).chain(d.hills).flat_map(|s| [s.x, s.y, s.w, s.h]);
        for v in boxes.chain(d.spawn_x).chain([d.campfire as u8 as f32, d.boss.map_or(0.0, |k| 1.0 + k as u8 as f32)]) {
            h = (h ^ v.to_bits() as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

impl ArenaId {
    pub fn def(self) -> &'static Arena {
        match self {
            ArenaId::Tiers => &TIERS,
            ArenaId::Bridges => &BRIDGES,
            ArenaId::Forest => &FOREST,
            ArenaId::Desktop => &DESKTOP,
            ArenaId::Cafe => &CAFE,
            ArenaId::Lair => &LAIR,
            ArenaId::Dune => &DUNE,
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
    hills: &[],
    campfire: false,
    boss: None,
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
    hills: &[],
    campfire: false,
    boss: None,
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
    hills: &[],
    campfire: true,
    boss: None,
    // (Clear of the trunks: nobody starts in one.)
    spawn_x: [28.0, 130.0, 254.0, 356.0],
};

/// The Desktop's windows, whole (title bar to bottom edge), in the order the renderer dresses
/// them: a terminal, a chat, a Jira board, VS Code, a calendar, and a settings notification
/// up in the top right corner. Each is a title bar
/// and a bottom edge (`WINDOW_BAR` thick, ledges) with a wall (`WINDOW_SIDE` wide) down each
/// side between them; `DESKTOP`'s ledges and walls are exactly these (a test holds them to it).
/// (The chat is tall and narrow, up under the notification: climbing it takes wall jumps.)
pub const DESKTOP_WINDOWS: [Aabb; 6] = [
    Aabb::new(250.0, 178.0, 120.0, 50.0),
    Aabb::new(409.0, 72.0, 60.0, 147.0),
    Aabb::new(24.0, 172.0, 150.0, 44.0),
    Aabb::new(40.0, 90.0, 116.0, 52.0),
    Aabb::new(256.0, 98.0, 90.0, 46.0),
    Aabb::new(349.0, 30.0, 120.0, 30.0),
];
pub const WINDOW_BAR: f32 = 5.0;
pub const WINDOW_SIDE: f32 = 4.0;

const DESKTOP: Arena = Arena {
    id: "desktop",
    platforms: &[
        // Each window's title bar and bottom edge, in `DESKTOP_WINDOWS` order.
        Aabb::new(250.0, 178.0, 120.0, 5.0),
        Aabb::new(250.0, 223.0, 120.0, 5.0),
        Aabb::new(409.0, 72.0, 60.0, 5.0),
        Aabb::new(409.0, 214.0, 60.0, 5.0),
        Aabb::new(24.0, 172.0, 150.0, 5.0),
        Aabb::new(24.0, 211.0, 150.0, 5.0),
        Aabb::new(40.0, 90.0, 116.0, 5.0),
        Aabb::new(40.0, 137.0, 116.0, 5.0),
        Aabb::new(256.0, 98.0, 90.0, 5.0),
        Aabb::new(256.0, 139.0, 90.0, 5.0),
        Aabb::new(349.0, 30.0, 120.0, 5.0),
        Aabb::new(349.0, 55.0, 120.0, 5.0),
    ],
    // Each window's sides, between its title bar and bottom edge.
    walls: &[
        Aabb::new(250.0, 183.0, 4.0, 40.0),
        Aabb::new(366.0, 183.0, 4.0, 40.0),
        Aabb::new(409.0, 77.0, 4.0, 137.0),
        Aabb::new(465.0, 77.0, 4.0, 137.0),
        Aabb::new(24.0, 177.0, 4.0, 34.0),
        Aabb::new(170.0, 177.0, 4.0, 34.0),
        Aabb::new(40.0, 95.0, 4.0, 42.0),
        Aabb::new(152.0, 95.0, 4.0, 42.0),
        Aabb::new(256.0, 103.0, 4.0, 36.0),
        Aabb::new(342.0, 103.0, 4.0, 36.0),
        Aabb::new(349.0, 35.0, 4.0, 20.0),
        Aabb::new(465.0, 35.0, 4.0, 20.0),
    ],
    hills: &[],
    campfire: false,
    boss: None,
    spawn_x: [20.0, 135.0, 250.0, 365.0],
};

// The Courtyard Cafe's pieces, named because the renderer dresses each kind differently.
pub const CAFE_BAR: Aabb = Aabb::new(8.0, 218.0, 92.0, 36.0);
/// The cafe's side wall, ceiling down to a gap just tall enough to walk through into the courtyard.
pub const CAFE_DIVIDER: Aabb = Aabb::new(288.0, 0.0, 8.0, 230.0);
pub const CAFE_TABLES: [Aabb; 2] = [Aabb::new(122.0, 218.0, 40.0, 5.0), Aabb::new(206.0, 218.0, 40.0, 5.0)];
/// On the brick over the bar, low then high.
pub const CAFE_SHELVES: [Aabb; 2] = [Aabb::new(14.0, 182.0, 52.0, 5.0), Aabb::new(40.0, 146.0, 56.0, 5.0)];
/// Along the front window, with potted plants.
pub const CAFE_PLANT_SHELF: Aabb = Aabb::new(150.0, 182.0, 72.0, 5.0);
/// Hung from the ductwork on ropes, the lower one first.
pub const CAFE_HANGING: [Aabb; 2] = [Aabb::new(176.0, 146.0, 64.0, 5.0), Aabb::new(96.0, 110.0, 72.0, 5.0)];
pub const COURTYARD_TABLES: [Aabb; 2] = [Aabb::new(314.0, 218.0, 28.0, 5.0), Aabb::new(416.0, 218.0, 28.0, 5.0)];
pub const COURTYARD_AWNING: Aabb = Aabb::new(300.0, 182.0, 44.0, 5.0);
/// The stair landing, the apartments' balcony, then the two top balconies (left building, right).
pub const COURTYARD_BALCONIES: [Aabb; 4] = [Aabb::new(428.0, 182.0, 36.0, 5.0), Aabb::new(352.0, 146.0, 40.0, 5.0), Aabb::new(300.0, 110.0, 44.0, 5.0), Aabb::new(428.0, 110.0, 36.0, 5.0)];

const CAFE: Arena = Arena {
    id: "cafe",
    platforms: &[
        CAFE_TABLES[0],
        CAFE_TABLES[1],
        CAFE_SHELVES[0],
        CAFE_SHELVES[1],
        CAFE_PLANT_SHELF,
        CAFE_HANGING[0],
        CAFE_HANGING[1],
        COURTYARD_TABLES[0],
        COURTYARD_TABLES[1],
        COURTYARD_AWNING,
        COURTYARD_BALCONIES[0],
        COURTYARD_BALCONIES[1],
        COURTYARD_BALCONIES[2],
        COURTYARD_BALCONIES[3],
    ],
    walls: &[CAFE_BAR, CAFE_DIVIDER],
    hills: &[],
    campfire: false,
    boss: None,
    // Two in the cafe (clear of the bar), two in the courtyard.
    spawn_x: [106.0, 190.0, 300.0, 376.0],
};

/// The Gatecrasher's hall. The boss stands in the middle of the floor (x 212..268, up to y 156: see
/// `boss.rs`), so the ledges climb either side: the long ones at y 184 run right up to it, at head
/// height. Nothing is over it, so an air strike's bombs fall straight onto it.
const LAIR: Arena = Arena {
    id: "lair",
    platforms: &[
        // Low steps off the floor, by the walls.
        Aabb::new(20.0, 218.0, 56.0, 5.0),
        Aabb::new(404.0, 218.0, 56.0, 5.0),
        // Up to its head.
        Aabb::new(84.0, 184.0, 120.0, 5.0),
        Aabb::new(276.0, 184.0, 120.0, 5.0),
        // Perches by the walls, and high ledges either side.
        Aabb::new(20.0, 148.0, 52.0, 5.0),
        Aabb::new(408.0, 148.0, 52.0, 5.0),
        Aabb::new(96.0, 120.0, 72.0, 5.0),
        Aabb::new(312.0, 120.0, 72.0, 5.0),
    ],
    walls: &[],
    hills: &[],
    campfire: false,
    boss: Some(BossKind::Gatecrasher),
    // Two each side of it, clear of its feet.
    spawn_x: [14.0, 110.0, 280.0, 376.0],
};

/// How steeply the Dune's sand slopes down to the Sandmaw's mouth (30 degrees: tan 30), and where
/// each slope meets the flat at the bottom.
pub const DUNE_SLOPE: f32 = 0.577_35;
pub const DUNE_FOOT: (f32, f32) = (176.0, 304.0);
const DUNE_STEP: f32 = 4.0;
const DUNE_STEPS: usize = 42;

/// Where the Dune's sand is at `x`, as drawn (a smooth line; underfoot it's `DUNE_HILLS`, fine steps
/// that follow it within a couple of pixels). None on the flat round the mouth.
pub fn dune_sand_y(x: f32) -> Option<f32> {
    let (l, r) = DUNE_FOOT;
    let run = if x < l { l - x } else if x > r { x - r } else { return None };
    Some(crate::sim::FLOOR_Y - run * DUNE_SLOPE)
}

/// The Dune's slopes as steps `DUNE_STEP` wide, each as tall as the slope at its middle, from each
/// foot out to the wall.
const fn dune_hills() -> [Aabb; DUNE_STEPS * 2] {
    let mut out = [Aabb::new(0.0, 0.0, 0.0, 0.0); DUNE_STEPS * 2];
    let mut i = 0;
    while i < DUNE_STEPS {
        let rise = (i as f32 * DUNE_STEP + DUNE_STEP / 2.0) * DUNE_SLOPE;
        let top = crate::sim::FLOOR_Y - rise;
        out[i] = Aabb::new(DUNE_FOOT.0 - DUNE_STEP * (i + 1) as f32, top, DUNE_STEP, rise);
        out[DUNE_STEPS + i] = Aabb::new(DUNE_FOOT.1 + DUNE_STEP * i as f32, top, DUNE_STEP, rise);
        i += 1;
    }
    out
}

pub const DUNE_HILLS: [Aabb; DUNE_STEPS * 2] = dune_hills();

/// The Dune's skiffs: (left, right, height at the end nearer its wall). Each slopes down toward the
/// middle at `SKIFF_SLOPE` (20 degrees), its deck slick like the sand. The ledges themselves are
/// `DUNE_SKIFF_STEPS` (fine steps following the deck; the renderer draws the deck).
pub const DUNE_SKIFFS: [(f32, f32, f32); 6] = [(96.0, 168.0, 156.0), (24.0, 80.0, 100.0), (104.0, 160.0, 112.0), (312.0, 384.0, 156.0), (400.0, 456.0, 100.0), (320.0, 376.0, 112.0)];
pub const SKIFF_SLOPE: f32 = 0.364;
const SKIFF_STEPS: usize = (18 + 14 + 14) * 2;

/// Where a Dune skiff's deck is at `x` (sloping down toward the middle).
pub const fn skiff_deck(skiff: (f32, f32, f32), x: f32) -> f32 {
    let (l, r, high) = skiff;
    let run = if l < crate::sim::W / 2.0 { x - l } else { r - x };
    high + run * SKIFF_SLOPE
}

const fn dune_skiff_steps() -> [Aabb; SKIFF_STEPS] {
    let mut out = [Aabb::new(0.0, 0.0, 0.0, 0.0); SKIFF_STEPS];
    let (mut n, mut s) = (0, 0);
    while s < DUNE_SKIFFS.len() {
        let (l, r, _) = DUNE_SKIFFS[s];
        let mut x = l;
        while x < r {
            out[n] = Aabb::new(x, skiff_deck(DUNE_SKIFFS[s], x + DUNE_STEP / 2.0), DUNE_STEP, 5.0);
            n += 1;
            x += DUNE_STEP;
        }
        s += 1;
    }
    out
}

pub const DUNE_SKIFF_STEPS: [Aabb; SKIFF_STEPS] = dune_skiff_steps();

/// The Sandmaw's desert. Its mouth is the middle of the floor (its throat, x 192..288, swallows
/// whoever touches it; see `sandmaw.rs`), and the sand slopes down to it from both walls at 30
/// degrees, so anything that rolls or slides goes its way. Skiffs hover over the slopes and stop
/// short of its mouth: grenades and Molotovs dropped or lobbed from them fall into it.
const DUNE: Arena = Arena {
    id: "dune",
    // Each side: a long skiff low over the slope toward the mouth, a perch by the wall above it,
    // and a high skiff over toward the mouth again.
    platforms: &DUNE_SKIFF_STEPS,
    walls: &[],
    hills: &DUNE_HILLS,
    campfire: false,
    boss: Some(BossKind::Sandmaw),
    // Two each side of its mouth.
    spawn_x: [14.0, 90.0, 300.0, 376.0],
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
                let below = FLOOR_Y - p.y <= 48.0 || d.platforms.iter().chain(d.walls).chain(d.hills).any(|q| q.y > p.y && q.y - p.y <= 48.0 && q.x < p.x + p.w + 40.0 && q.x + q.w > p.x - 40.0);
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
                    assert!(!a.def().walls.iter().chain(a.def().hills).any(|w| f.body().overlaps(w)), "{} seed {seed}: a fighter starts in a wall at {}", a.id(), f.pos);
                    assert!(!m.config.campfire_area().is_some_and(|c| f.body().overlaps(&c)), "{} seed {seed}: a fighter starts in the campfire at {}", a.id(), f.pos);
                }
                for _ in 0..50 {
                    let p = m.random_surface_spot_between(0.0, W).unwrap() - glam::vec2(0.0, ITEM_R);
                    assert!(!a.def().walls.iter().chain(a.def().hills).any(|w| crate::sim::Aabb::around(p, ITEM_R).overlaps(w)), "{} seed {seed}: a spot in a wall at {p}", a.id());
                }
            }
        }
    }

    #[test]
    fn the_desktops_ledges_and_walls_are_its_windows() {
        let d = ArenaId::Desktop.def();
        for (k, w) in DESKTOP_WINDOWS.iter().enumerate() {
            let bottom = w.y + w.h - WINDOW_BAR;
            assert_eq!(d.platforms[2 * k], Aabb::new(w.x, w.y, w.w, WINDOW_BAR), "window {k}'s title bar");
            assert_eq!(d.platforms[2 * k + 1], Aabb::new(w.x, bottom, w.w, WINDOW_BAR), "window {k}'s bottom edge");
            let side = |x: f32| Aabb::new(x, w.y + WINDOW_BAR, WINDOW_SIDE, bottom - w.y - WINDOW_BAR);
            assert_eq!(d.walls[2 * k], side(w.x), "window {k}'s left side");
            assert_eq!(d.walls[2 * k + 1], side(w.x + w.w - WINDOW_SIDE), "window {k}'s right side");
        }
        assert_eq!(d.walls.len(), 2 * DESKTOP_WINDOWS.len());
    }

    #[test]
    fn the_campfire_moves_about_but_always_has_open_sky_above() {
        use crate::sim::{Match, MatchConfig};
        for a in ArenaId::ALL {
            let mut spots = Vec::new();
            for seed in 0..60 {
                let m = Match::new(seed, &MatchConfig { arena: a, ..MatchConfig::default() });
                let Some(fire) = m.config.campfire_area() else {
                    assert!(!a.def().campfire, "{} has a campfire", a.id());
                    continue;
                };
                // Nothing above it, however high: no ledge, no wall.
                let over = |s: &Aabb| s.x < fire.x + fire.w && s.x + s.w > fire.x;
                assert!(!a.def().platforms.iter().chain(a.def().walls).any(over), "{} seed {seed}: something over the campfire at {}", a.id(), fire.x);
                spots.push(fire.x as i32);
            }
            if a.def().campfire {
                spots.sort();
                spots.dedup();
                assert!(spots.len() > 20, "{}: only {} different spots", a.id(), spots.len());
            }
        }
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = ArenaId::ALL.iter().map(|a| a.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ArenaId::ALL.len());
    }
}
