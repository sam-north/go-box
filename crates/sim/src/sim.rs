//! Headless, deterministic arena simulation: four humans, free-for-all, starting empty-handed
//! with spears, rocks, grenades and health lying around. No hard-coded behaviour: fighters only
//! expose physical abilities; brains decide what to do with them.

use crate::brain::{Brain, IN_EXTRA, IN_HEAL, IN_ITEMS, IN_MEM, IN_NEAREST, IN_OPP, IN_SELF, IN_THREAT, MEM, N_IN, N_OUT, OUT_MEM};
use crate::character::{Character, Effect};
use glam::{vec2, Vec2};
use serde::{Deserialize, Serialize};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;
use std::ops::{Range, RangeInclusive};

pub const W: f32 = 480.0;
pub const H: f32 = 270.0;
/// Top of the floor (where everyone stands at the start); walls and ceiling are WALL thick.
pub const FLOOR_Y: f32 = 254.0;
pub const WALL: f32 = 8.0;
/// Sim ticks per second: the one place the game's clock rate is set. Everything that counts
/// ticks (cooldowns, fuses, match length, snapshot rate, client interpolation) derives from it.
pub const TICK_RATE: u32 = 60;
pub const DT: f32 = 1.0 / TICK_RATE as f32;
/// `s` seconds, in ticks.
pub const fn secs(s: u32) -> u32 {
    s * TICK_RATE
}
/// The default match length.
pub const MAX_TICKS: u32 = secs(45);
pub const N_FIGHTERS: usize = 4;

// Body.
pub const HALF_W: f32 = 4.0;
pub const BODY_H: f32 = 14.0;
/// Top slice of the body that counts as the head for headshots.
pub const HEAD_H: f32 = 4.0;
const GRAVITY: f32 = 640.0;
const MAX_FALL: f32 = 320.0;
const JUMP_V: f32 = 250.0;
const WALK: f32 = 45.0;
const RUN: f32 = 85.0;
const GROUND_ACC: f32 = 900.0;
const AIR_ACC: f32 = 420.0;
pub const MAX_HP: f32 = 100.0;

// Melee: fists, or the held item's own melee (a spear stabs); see `item.rs`.
pub const ATK_WINDUP: u32 = 5;
pub const ATK_ACTIVE: u32 = 5;
pub const ATK_TOTAL: u32 = 16;
const ATK_COOLDOWN: u32 = 18;
const HITSTUN: u32 = 12;

// Throwing.
const AIM_RANGE: f32 = 0.9;
const GRAB_COOLDOWN: u32 = 10;
const THROW_COOLDOWN: u32 = 20;
pub use crate::item::{GRENADE_FUSE, ItemKind};
use crate::item::{Blast, DROPPED_FUSE, FISTS, Hit, N_SENSED, SPAWN_ORDER};
pub const GRENADE_RADIUS: f32 = crate::item::GRENADE_BLAST.radius;
pub const ITEM_R: f32 = 1.5;
const LETHAL_SPEED: f32 = 110.0;
const OWN_GRACE: u32 = 15;

// Items lying around.
const START_SPEARS: usize = 2;
const START_ROCKS: usize = 4;
const ITEM_SPAWN_EVERY: Range<u32> = secs(5)..secs(8);
const MAX_LOOSE_ITEMS: usize = 10;

// Health pickups: restore half your health; rare and on a reachable surface.
pub const HEAL_AMOUNT: f32 = 50.0;
pub const HEAL_R: f32 = 2.5;
/// Final rules: each round gets 1 or 2 pickups (the trainer starts with more and fades to
/// this), at random times inside HEAL_WINDOW (ticks).
pub const HEALS_PER_ROUND: RangeInclusive<u32> = 1..=2;

const HEAL_WINDOW: Range<u32> = secs(6)..secs(24);

/// The sim's random number generator, named explicitly so a seed plays out the same on every
/// platform (rand's `SmallRng` is a different algorithm on 32-bit targets such as wasm).
pub type SimRng = Xoshiro256PlusPlus;

// Senses.
pub const N_RAYS: usize = 12;
pub const N_SECTORS: usize = 12;
pub const RAY_MAX: f32 = 200.0;
const VIS_MAX: f32 = 370.0;

/// Where damage came from (hit sounds differ by weapon).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Source {
    Fist,
    Stab,
    SpearThrow,
    Rock,
    Grenade,
    /// A character's special.
    Special,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Aabb {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Aabb {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn around(p: Vec2, r: f32) -> Self {
        Self::new(p.x - r, p.y - r, r * 2.0, r * 2.0)
    }
    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.x < o.x + o.w && self.x + self.w > o.x && self.y < o.y + o.h && self.y + self.h > o.y
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }
}

/// Fully solid terrain, the same in every arena: ceiling, floor, walls. (Each arena's platforms
/// are in `arena.rs`.)
pub const SOLIDS: [Aabb; 4] = [
    Aabb::new(0.0, 0.0, W, WALL),
    Aabb::new(0.0, FLOOR_Y, W, H - FLOOR_Y),
    Aabb::new(0.0, 0.0, WALL, H),
    Aabb::new(W - WALL, 0.0, WALL, H),
];

pub use crate::arena::{Arena, ArenaId};
use crate::arena::SPAWN_SPREAD;

#[derive(Clone, Copy, Default, Debug, Serialize, Deserialize)]
pub struct Action {
    pub mv: f32,
    pub run: bool,
    pub jump: bool,
    pub attack: bool,
    pub throw: bool,
    pub aim: f32,
    pub grab: bool,
    /// Use the character's special (ignored when it's cooling down or there is none).
    pub special: bool,
    /// Where a player is aiming (a direction from their hand, e.g. toward the mouse). Throws
    /// go exactly this way and swings turn to face it; `aim` is ignored. Bots leave it None.
    pub target: Option<Vec2>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fighter {
    /// Feet centre.
    pub pos: Vec2,
    pub vel: Vec2,
    pub facing: f32,
    pub grounded: bool,
    pub hp: f32,
    /// 0 = not attacking, otherwise ticks since the swing started.
    pub atk_timer: u32,
    pub cooldown: u32,
    /// Bitmask of fighters already struck by the current swing.
    #[serde(skip)]
    hit_mask: u8,
    pub hitstun: u32,
    /// What's in hand (mirrors the item's `held_by`).
    pub holding: Option<ItemKind>,
    pub hand_cd: u32,
    // Server-side only (not sent to clients): the brain's memory, the last action, and the
    // scoreboard stats (the server sends `Results` at the end instead).
    #[serde(skip)]
    pub memory: [f32; MEM],
    #[serde(skip)]
    pub action: Action,
    /// Not in this match at all (a 1v1 leaves two seats empty).
    pub absent: bool,
    pub character: Character,
    /// Ticks until the special can be used again (0 = ready).
    pub special_cd: u32,
    // Stats for the scoreboard.
    #[serde(skip)]
    pub died_at: Option<u32>,
    #[serde(skip)]
    pub dmg_dealt: f32,
    #[serde(skip)]
    pub dmg_taken: f32,
    #[serde(skip)]
    pub headshots: u32,
    #[serde(skip)]
    pub kills: u32,
}

impl Fighter {
    pub fn new(x: f32, facing: f32) -> Self {
        Self {
            pos: vec2(x, FLOOR_Y),
            vel: Vec2::ZERO,
            facing,
            grounded: true,
            hp: MAX_HP,
            atk_timer: 0,
            cooldown: 0,
            hit_mask: 0,
            hitstun: 0,
            holding: None,
            hand_cd: 0,
            memory: [0.0; MEM],
            action: Action::default(),
            absent: false,
            character: Character::Classic,
            special_cd: 0,
            died_at: None,
            dmg_dealt: 0.0,
            dmg_taken: 0.0,
            headshots: 0,
            kills: 0,
        }
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }

    pub fn body(&self) -> Aabb {
        Aabb::new(self.pos.x - HALF_W, self.pos.y - BODY_H, HALF_W * 2.0, BODY_H)
    }

    pub fn eye(&self) -> Vec2 {
        vec2(self.pos.x, self.pos.y - 10.0)
    }

    pub fn center(&self) -> Vec2 {
        vec2(self.pos.x, self.pos.y - BODY_H * 0.5)
    }

    /// Where a carried item sits: cocked over the shoulder.
    pub fn hand(&self) -> Vec2 {
        vec2(self.pos.x - self.facing * 1.5, self.pos.y - 13.0)
    }

    fn is_head(&self, y: f32) -> bool {
        let top = self.pos.y - BODY_H;
        y >= top - 1.0 && y < top + HEAD_H
    }

    pub fn attack_active(&self) -> bool {
        self.atk_timer > ATK_WINDUP && self.atk_timer <= ATK_WINDUP + ATK_ACTIVE
    }

    /// What a swing does now: the held item's melee (a spear stabs), else fists: (reach, hit).
    pub fn melee(&self) -> (f32, Hit) {
        self.holding.and_then(|k| k.def().melee).unwrap_or(FISTS)
    }

    pub fn melee_reach(&self) -> f32 {
        self.melee().0
    }

    pub fn attack_box(&self) -> Aabb {
        let reach = self.melee_reach();
        let x0 = if self.facing > 0.0 { self.pos.x + HALF_W - 2.0 } else { self.pos.x - HALF_W + 2.0 - reach };
        Aabb::new(x0, self.pos.y - BODY_H + 2.0, reach, 10.0)
    }

    pub fn aim_dir(&self, aim: f32) -> Vec2 {
        let a = aim * AIM_RANGE;
        vec2(self.facing * libm::cosf(a), -libm::sinf(a))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub kind: ItemKind,
    pub pos: Vec2,
    pub vel: Vec2,
    pub held_by: Option<usize>,
    /// Who last threw it (credited for its damage).
    pub owner: Option<usize>,
    /// Flying fast enough to hurt.
    pub lethal: bool,
    /// Ticks left before a thrown grenade explodes.
    pub fuse: Option<u32>,
    #[serde(skip)]
    age: u32,
}

impl Item {
    fn new(kind: ItemKind, pos: Vec2) -> Self {
        Self { kind, pos, vel: Vec2::ZERO, held_by: None, owner: None, lethal: false, fuse: None, age: 0 }
    }
    /// On the ground and safe to pick up.
    pub fn loose(&self) -> bool {
        self.held_by.is_none() && !self.lethal
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Event {
    Hit { pos: Vec2, dir: f32, headshot: bool, source: Source },
    Death { who: usize, pos: Vec2 },
    Jump { who: usize, pos: Vec2 },
    /// Fighter `who` started a melee swing.
    Swing { who: usize },
    Land { pos: Vec2 },
    Throw { who: usize },
    Grab { pos: Vec2 },
    Explosion { pos: Vec2 },
    Heal { who: usize, pos: Vec2 },
    Spawn { pos: Vec2 },
    /// Fighter `who` used their special.
    Special { who: usize },
    /// Austin vanishing or appearing.
    Poof { pos: Vec2 },
    /// Sam's gravity bomb going off: everyone nearby is yanked in...
    BombCrunch { pos: Vec2 },
    /// ...then it explodes.
    BombBlast { pos: Vec2 },
    /// One of Tyler's dogs biting.
    Bite { pos: Vec2 },
    /// A thrown item hitting terrain hard.
    Clatter { pos: Vec2, kind: ItemKind },
}

/// Distance along a normalised ray to the nearest terrain, capped at `max`.
pub fn terrain_ray(arena: &Arena, o: Vec2, d: Vec2, max: f32) -> f32 {
    let inv = |v: f32| if v.abs() < 1e-8 { 1e8_f32.copysign(v) } else { 1.0 / v };
    let (ix, iy) = (inv(d.x), inv(d.y));
    let mut best = max;
    for r in SOLIDS.iter().chain(arena.platforms) {
        let (tx1, tx2) = ((r.x - o.x) * ix, (r.x + r.w - o.x) * ix);
        let (ty1, ty2) = ((r.y - o.y) * iy, (r.y + r.h - o.y) * iy);
        let tmin = tx1.min(tx2).max(ty1.min(ty2)).max(0.0);
        let tmax = tx1.max(tx2).min(ty1.max(ty2));
        if tmax >= tmin {
            best = best.min(tmin);
        }
    }
    best
}

/// Direction of terrain ray `i` (evenly spaced around the circle).
pub fn ray_dir(i: usize) -> Vec2 {
    static DIRS: std::sync::LazyLock<[Vec2; N_RAYS]> = std::sync::LazyLock::new(|| {
        std::array::from_fn(|i| {
            let a = i as f32 / N_RAYS as f32 * std::f32::consts::TAU;
            vec2(libm::cosf(a), libm::sinf(a))
        })
    });
    DIRS[i]
}

/// atan2 via a polynomial, accurate to ~1e-5 rad — plenty for picking a 30° retina sector,
/// and much cheaper than the libm version.
fn fast_atan2(y: f32, x: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 && ay == 0.0 {
        return 0.0;
    }
    let z = ax.min(ay) / ax.max(ay);
    let z2 = z * z;
    let mut a = z * (0.999_977_26 + z2 * (-0.332_623_47 + z2 * (0.193_543_46 + z2 * (-0.116_432_87 + z2 * (0.052_653_32 + z2 * -0.011_721_2)))));
    if ay > ax {
        a = FRAC_PI_2 - a;
    }
    if x < 0.0 {
        a = PI - a;
    }
    if y < 0.0 { -a } else { a }
}

/// If `target` is in line of sight from `eye`, returns (proximity 0.05..1, retina sector).
fn glimpse(arena: &Arena, eye: Vec2, target: Vec2) -> Option<(f32, usize)> {
    let to = target - eye;
    let dist = to.length().max(0.001);
    if dist >= VIS_MAX || terrain_ray(arena, eye, to / dist, dist) < dist - 0.5 {
        return None;
    }
    let ang = fast_atan2(to.y, to.x) + std::f32::consts::PI;
    let sector = ((ang / std::f32::consts::TAU * N_SECTORS as f32) as usize) % N_SECTORS;
    Some(((1.0 - dist / VIS_MAX).max(0.05), sector))
}

fn mark(s: &mut [f32; N_IN], base: usize, seen: Option<(f32, usize)>) {
    if let Some((p, sector)) = seen {
        s[base + sector] = s[base + sector].max(p);
    }
}

/// How far a lit item's fuse has burnt down (0 just lit .. 1 about to go off).
fn fuse_burnt(it: &Item) -> Option<f32> {
    Some(1.0 - it.fuse? as f32 / it.kind.def().fuse? as f32)
}

/// Which of the three kinds the brains know an item looks like, as one-hot inputs.
fn one_hot(kind: Option<ItemKind>) -> [f32; N_SENSED] {
    let mut v = [0.0; N_SENSED];
    if let Some(k) = kind {
        v[k.def().sensed_as as usize] = 1.0;
    }
    v
}

/// A random spot on top of the floor or a platform — everywhere a fighter can stand. Wider
/// surfaces are proportionally more likely.
fn random_surface_spot(rng: &mut impl Rng, arena: &Arena) -> Vec2 {
    random_surface_spot_between(rng, arena, 0.0, W).unwrap_or(vec2(W / 2.0, FLOOR_Y))
}

/// Like `random_surface_spot`, but only spots with `lo <= x <= hi`. None if no surface reaches
/// into that range.
fn random_surface_spot_between(rng: &mut impl Rng, arena: &Arena, lo: f32, hi: f32) -> Option<Vec2> {
    let floor = (WALL + 6.0, W - WALL - 6.0, FLOOR_Y);
    let surfaces = std::iter::once(floor)
        .chain(arena.platforms.iter().map(|p| (p.x + 3.0, p.x + p.w - 3.0, p.y)))
        .map(|(x0, x1, y)| (x0.max(lo), x1.min(hi), y))
        .filter(|(x0, x1, _)| x1 > x0);
    let total: f32 = surfaces.clone().map(|(x0, x1, _)| x1 - x0).sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = rng.gen_range(0.0..total);
    for (x0, x1, y) in surfaces {
        if pick < x1 - x0 {
            return Some(vec2(x0 + pick, y));
        }
        pick -= x1 - x0;
    }
    None
}

/// The rules of one match: the map, how long it lasts, how many health pickups, and how it's
/// won. Everything else (physics, weapons, characters) is the same in every match.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchConfig {
    pub arena: ArenaId,
    pub max_ticks: u32,
    /// How many health pickups appear (picked at random from the range).
    pub heals: RangeInclusive<u32>,
    pub mode: Mode,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self { arena: ArenaId::default(), max_ticks: MAX_TICKS, heals: HEALS_PER_ROUND, mode: Mode::default() }
    }
}

crate::enum_all! {
    /// How a match is won. (A new mode is a variant here, its rule in `outcome`, and its name
    /// `mode.<id>` in the client's text files.)
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
    pub enum Mode {
        /// Last one standing; at the time limit, whoever has the most health. A match with only
        /// one fighter in it (practice) runs until they die or time runs out.
        #[default]
        LastStanding,
    }
}

impl Mode {
    pub fn id(self) -> &'static str {
        match self {
            Mode::LastStanding => "last_standing",
        }
    }

    /// Checked after every tick: None while the match goes on, else Some(winner) (Some(None)
    /// is a draw).
    pub fn outcome(self, m: &Match) -> Option<Option<usize>> {
        match self {
            Mode::LastStanding => {
                let alive: Vec<usize> = (0..N_FIGHTERS).filter(|&i| m.f[i].alive()).collect();
                let present = m.f.iter().filter(|f| !f.absent).count();
                let last_standing = if present > 1 { alive.len() <= 1 } else { alive.is_empty() };
                if !last_standing && m.tick < m.config.max_ticks {
                    return None;
                }
                // Last one standing, or the most health at the timeout. An exact tie is a draw.
                Some(match alive.len() {
                    0 => None,
                    1 => Some(alive[0]),
                    _ => {
                        let best = alive.iter().copied().max_by(|&a, &b| m.f[a].hp.total_cmp(&m.f[b].hp)).unwrap();
                        let tied = alive.iter().filter(|&&j| m.f[j].hp == m.f[best].hp).count() > 1;
                        (!tied).then_some(best)
                    }
                })
            }
        }
    }
}

pub struct Match {
    pub f: [Fighter; N_FIGHTERS],
    pub items: Vec<Item>,
    /// Health pickups currently lying in the arena.
    pub heals: Vec<Vec2>,
    /// Remaining heal spawn times this round, latest first.
    heal_schedule: Vec<u32>,
    next_item: u32,
    rng: SimRng,
    pub tick: u32,
    pub over: bool,
    pub winner: Option<usize>,
    pub events: Vec<Event>,
    /// What specials have left in the arena (Sam's bombs, Tyler's dogs...).
    pub effects: Vec<Effect>,
    pub config: MatchConfig,
}

impl Match {
    /// Four humans spread across the floor, empty-handed, in seat order from left to right
    /// (each somewhere in its own spot), playing by `config`. The same `seed` plays out the same
    /// match everywhere.
    pub fn new(seed: u64, config: &MatchConfig) -> Self {
        let mut rng = SimRng::seed_from_u64(seed);
        let rng = &mut rng;
        let arena = config.arena.def();
        let f = std::array::from_fn(|i| {
            let x = arena.spawn_x[i] + rng.gen_range(0.0..SPAWN_SPREAD);
            Fighter::new(x, if x < W / 2.0 { 1.0 } else { -1.0 })
        });
        let mut items = Vec::new();
        for (kind, n) in [(ItemKind::Spear, START_SPEARS), (ItemKind::Rock, START_ROCKS)] {
            for _ in 0..n {
                items.push(Item::new(kind, random_surface_spot(rng, arena) - vec2(0.0, ITEM_R)));
            }
        }
        let mut heal_schedule: Vec<u32> = (0..rng.gen_range(config.heals.clone())).map(|_| rng.gen_range(HEAL_WINDOW)).collect();
        heal_schedule.sort_unstable_by(|a, b| b.cmp(a));
        let next_item = rng.gen_range(ITEM_SPAWN_EVERY);
        Self {
            f,
            items,
            heals: Vec::new(),
            heal_schedule,
            next_item,
            rng: SimRng::seed_from_u64(rng.r#gen()),
            tick: 0,
            over: false,
            winner: None,
            events: Vec::new(),
            effects: Vec::new(),
            config: config.clone(),
        }
    }

    /// The map this match is on.
    pub fn arena(&self) -> &'static Arena {
        self.config.arena.def()
    }

    pub(crate) fn random_surface_spot_between(&mut self, lo: f32, hi: f32) -> Option<Vec2> {
        random_surface_spot_between(&mut self.rng, self.config.arena.def(), lo, hi)
    }

    /// Empties seat `i` (before the match starts): nobody plays there.
    pub fn remove(&mut self, i: usize) {
        self.f[i].hp = 0.0;
        self.f[i].absent = true;
    }

    pub fn held_item(&self, i: usize) -> Option<usize> {
        self.items.iter().position(|it| it.held_by == Some(i))
    }

    /// Everything fighter `i` perceives. Other fighters, items and pickups are only seen
    /// when in line of sight.
    pub fn senses(&self, i: usize) -> [f32; N_IN] {
        let arena = self.arena();
        let me = &self.f[i];
        let mut s = [0.0; N_IN];
        let eye = me.eye();
        for r in 0..N_RAYS {
            s[r] = 1.0 - terrain_ray(arena, eye, ray_dir(r), RAY_MAX) / RAY_MAX;
        }
        let mut nearest: Option<(f32, usize)> = None;
        for j in (0..N_FIGHTERS).filter(|&j| j != i && self.f[j].alive()) {
            let seen = glimpse(arena, eye, self.f[j].center());
            mark(&mut s, IN_OPP, seen);
            if let Some((p, _)) = seen {
                if nearest.is_none_or(|(np, _)| p > np) {
                    nearest = Some((p, j));
                }
            }
        }
        if let Some((_, j)) = nearest {
            let o = &self.f[j];
            s[IN_NEAREST] = (o.atk_timer > 0) as u8 as f32;
            s[IN_NEAREST + 1] = if (o.facing > 0.0) == (o.pos.x < me.pos.x) { 1.0 } else { -1.0 };
            s[IN_NEAREST + 2..IN_NEAREST + 5].copy_from_slice(&one_hot(o.holding));
        }
        for it in self.items.iter().filter(|it| it.held_by.is_none()) {
            let seen = glimpse(arena, eye, it.pos);
            if it.lethal || it.fuse.is_some() {
                mark(&mut s, IN_THREAT, seen);
            } else {
                mark(&mut s, IN_ITEMS + it.kind.def().sensed_as as usize * N_SECTORS, seen);
            }
        }
        for &h in &self.heals {
            mark(&mut s, IN_HEAL, glimpse(arena, eye, h));
        }
        // Brains never saw specials in training; someone else's dog or bomb reads as a threat.
        for e in self.effects.iter().filter(|e| e.owner() != i) {
            mark(&mut s, IN_THREAT, glimpse(arena, eye, e.threat_pos()));
        }
        let k = IN_SELF;
        s[k] = me.vel.x / RUN;
        s[k + 1] = me.vel.y / MAX_FALL;
        s[k + 2] = me.grounded as u8 as f32;
        s[k + 3] = me.hp / MAX_HP;
        s[k + 4] = me.facing;
        s[k + 5] = (me.atk_timer == 0 && me.cooldown == 0) as u8 as f32;
        s[k + 6] = (me.hitstun > 0) as u8 as f32;
        s[k + 7..k + 10].copy_from_slice(&one_hot(me.holding));
        s[k + 10] = (me.holding.is_some() && me.hand_cd == 0) as u8 as f32;
        let body = me.body();
        s[k + 11] = (me.holding.is_none() && self.items.iter().any(|it| it.loose() && body.overlaps(&Aabb::around(it.pos, ITEM_R)))) as u8 as f32;
        s[k + 12] = self.held_item(i).and_then(|h| fuse_burnt(&self.items[h])).unwrap_or(0.0);
        s[IN_MEM..IN_MEM + MEM].copy_from_slice(&me.memory);

        // v2 senses.
        let e = IN_EXTRA;
        if let Some((_, j)) = nearest {
            let o = &self.f[j];
            s[e] = o.hp / MAX_HP;
            s[e + 1] = ((o.pos.x - me.pos.x) / 160.0).clamp(-1.0, 1.0);
            s[e + 2] = ((o.pos.y - me.pos.y) / 90.0).clamp(-1.0, 1.0);
            s[e + 3] = (o.vel.x - me.vel.x) / (2.0 * RUN);
            s[e + 4] = ((o.vel.y - me.vel.y) / MAX_FALL).clamp(-1.0, 1.0);
        }
        s[e + 5] = (0..N_FIGHTERS).filter(|&j| j != i && self.f[j].alive()).count() as f32 / (N_FIGHTERS - 1) as f32;
        // The most dangerous projectile: where its straight-line path passes closest to me, and when.
        let c = me.center();
        let mut worst = (0.0f32, 0.0f32, Vec2::ZERO);
        for it in self.items.iter().filter(|it| it.lethal && !(it.owner == Some(i) && it.age < OWN_GRACE)) {
            let (dp, dv) = (it.pos - c, it.vel);
            let t = (-dp.dot(dv) / dv.length_squared().max(1e-3)).clamp(0.0, 1.0);
            let miss = (dp + dv * t).length();
            let danger = (1.0 - miss / 24.0).max(0.0) * (1.0 - t * 0.5);
            if danger > worst.0 {
                worst = (danger, 1.0 - t, dp.normalize_or_zero());
            }
        }
        s[e + 6] = worst.0;
        s[e + 7] = worst.1;
        s[e + 8] = worst.2.x;
        s[e + 9] = worst.2.y;
        // Live grenades: how deep inside the blast I am, weighted by how far the fuse has burned.
        s[e + 10] = self
            .items
            .iter()
            .filter_map(|it| Some((it, it.kind.def().blast?, fuse_burnt(it)?)))
            .map(|(it, b, burnt)| (1.0 - it.pos.distance(c) / b.radius).max(0.0) * burnt)
            .fold(0.0, f32::max);
        s[e + 11] = 1.0 - self.tick as f32 / self.config.max_ticks as f32;
        let best_other = (0..N_FIGHTERS).filter(|&j| j != i && self.f[j].alive()).map(|j| self.f[j].hp).fold(0.0, f32::max);
        s[e + 12] = (me.hp - best_other) / MAX_HP;
        s
    }

    pub fn step(&mut self, brains: [&Brain; N_FIGHTERS]) {
        self.step_with(brains, None);
    }

    /// Like `step`, but seat `human.0` is driven by a player's input instead of its brain.
    pub fn step_with(&mut self, brains: [&Brain; N_FIGHTERS], human: Option<(usize, Action)>) {
        self.step_humans(brains, std::array::from_fn(|i| human.filter(|&(h, _)| h == i).map(|(_, a)| a)));
    }

    /// One tick where any seat with `Some(action)` is a player and the rest think for themselves.
    pub fn step_humans(&mut self, brains: [&Brain; N_FIGHTERS], humans: [Option<Action>; N_FIGHTERS]) {
        self.events.clear();
        if self.over {
            return;
        }

        // 1. Perceive + think.
        let mut actions = [Action::default(); N_FIGHTERS];
        for i in 0..N_FIGHTERS {
            if !self.f[i].alive() {
                continue;
            }
            if let Some(a) = humans[i] {
                actions[i] = a;
                self.f[i].action = a;
                continue;
            }
            let input = self.senses(i);
            let mut out = [0.0; N_OUT];
            brains[i].forward(&input, &mut out);
            actions[i] = Action {
                mv: out[0],
                run: out[1] > 0.0,
                jump: out[2] > 0.0,
                attack: out[3] > 0.0,
                throw: out[4] > 0.0,
                aim: out[5],
                grab: out[6] > 0.0,
                special: self.f[i].special_cd == 0 && self.f[i].character.special().is_some_and(|s| s.bot_wants(self, i)),
                target: None,
            };
            self.f[i].memory.copy_from_slice(&out[OUT_MEM..]);
            self.f[i].action = actions[i];
        }

        // 2. Move.
        for i in 0..N_FIGHTERS {
            if self.f[i].absent {
                continue;
            }
            let was_grounded = self.f[i].grounded;
            let f = &mut self.f[i];
            control(f, i, actions[i], &mut self.events);
            collide(f, self.config.arena.def());
            if f.grounded && !was_grounded {
                self.events.push(Event::Land { pos: f.pos });
            }
        }

        // 2b. Specials.
        self.step_specials(actions.map(|a| a.special));

        // 3. Hands: grab or throw.
        for i in 0..N_FIGHTERS {
            self.hands(i, actions[i]);
        }

        // 4. Melee (resolved simultaneously).
        let mut hits = Vec::new();
        for i in 0..N_FIGHTERS {
            let a = &self.f[i];
            if !a.alive() || !a.attack_active() {
                continue;
            }
            for j in 0..N_FIGHTERS {
                if j != i && self.f[j].alive() && a.hit_mask & (1 << j) == 0 && a.attack_box().overlaps(&self.f[j].body()) {
                    hits.push((i, j));
                }
            }
        }
        for (i, j) in hits {
            self.f[i].hit_mask |= 1 << j;
            let a = &self.f[i];
            let dir = a.facing;
            let (_, hit) = a.melee();
            // A strike lands on the head when it comes from above, e.g. from a jump or a ledge.
            let headshot = self.f[j].is_head(a.pos.y - 8.0);
            let dmg = if headshot { hit.damage * hit.headshot_mult } else { hit.damage };
            self.damage(i, j, dmg, vec2(dir * 150.0, -140.0), HITSTUN, dir, hit.source, headshot);
        }
        for f in self.f.iter_mut() {
            if f.atk_timer > 0 {
                f.atk_timer += 1;
                if f.atk_timer > ATK_TOTAL {
                    f.atk_timer = 0;
                    f.cooldown = ATK_COOLDOWN;
                }
            } else if f.cooldown > 0 {
                f.cooldown -= 1;
            }
        }

        // 5. Items in flight, grenades, pickups, spawns.
        self.step_items();
        self.step_effects();
        self.step_heals();
        self.spawn_items();

        // 6. Is it over?
        self.tick += 1;
        if let Some(winner) = self.config.mode.outcome(self) {
            self.over = true;
            self.winner = winner;
        }
    }

    fn hands(&mut self, i: usize, a: Action) {
        let f = &mut self.f[i];
        if f.hand_cd > 0 {
            f.hand_cd -= 1;
        }
        if !f.alive() || f.hitstun > 0 || f.hand_cd > 0 {
            return;
        }
        if f.holding.is_none() {
            if !a.grab {
                return;
            }
            let body = f.body();
            let reachable = self.items.iter().enumerate().filter(|(_, it)| it.loose() && body.overlaps(&Aabb::around(it.pos, ITEM_R + 1.0)));
            if let Some((k, _)) = reachable.min_by(|(_, a), (_, b)| a.pos.distance(f.center()).total_cmp(&b.pos.distance(f.center()))) {
                self.items[k].held_by = Some(i);
                f.holding = Some(self.items[k].kind);
                f.hand_cd = GRAB_COOLDOWN;
                self.events.push(Event::Grab { pos: self.items[k].pos });
            }
        } else if a.throw {
            let k = self.held_item(i).unwrap();
            let f = &mut self.f[i];
            let dir = match a.target {
                // A player aiming (mouse or stick): throw that way, turning to face it.
                Some(t) if t.length_squared() > 1e-6 => {
                    if t.x.abs() > 1e-3 {
                        f.facing = t.x.signum();
                    }
                    t.normalize()
                }
                _ => f.aim_dir(a.aim),
            };
            f.holding = None;
            f.hand_cd = THROW_COOLDOWN;
            let it = &mut self.items[k];
            it.held_by = None;
            it.owner = Some(i);
            it.pos = f.hand();
            let def = it.kind.def();
            it.vel = dir * def.throw_speed + f.vel * 0.5;
            it.age = 0;
            // Something with a fuse is lit (and goes off later); anything else hurts on impact.
            match def.fuse {
                Some(fuse) => {
                    it.fuse.get_or_insert(fuse);
                }
                None => it.lethal = true,
            }
            self.events.push(Event::Throw { who: i });
        }
    }

    fn step_items(&mut self) {
        let mut explode = Vec::new();
        for k in 0..self.items.len() {
            if let Some(h) = self.items[k].held_by {
                self.items[k].pos = self.f[h].hand();
            } else if step_item(&mut self.items[k], self.config.arena.def()) {
                let it = &self.items[k];
                self.events.push(Event::Clatter { pos: it.pos, kind: it.kind });
            }
            if let Some(t) = self.items[k].fuse {
                if t == 0 {
                    explode.push(k);
                    continue;
                }
                self.items[k].fuse = Some(t - 1);
            }
            let it = &self.items[k];
            if !it.lethal {
                continue;
            }
            let ib = Aabb::around(it.pos, ITEM_R);
            let victim = (0..N_FIGHTERS).find(|&j| {
                let own_grace = it.owner == Some(j) && it.age < OWN_GRACE;
                self.f[j].alive() && !own_grace && ib.overlaps(&self.f[j].body())
            });
            if let Some(j) = victim {
                let dir = it.vel.x.signum();
                let src = it.owner.unwrap_or(j);
                let headshot = self.f[j].is_head(it.pos.y);
                let Some(hit) = it.kind.def().thrown else { continue };
                let dmg = if headshot { hit.damage * hit.headshot_mult } else { hit.damage };
                let it = &mut self.items[k];
                it.vel *= -0.2;
                it.lethal = false;
                self.damage(src, j, dmg, vec2(dir * 110.0, -100.0), 10, dir, hit.source, headshot);
            }
        }
        // Explode back-to-front so indices stay valid.
        explode.sort_unstable_by(|a, b| b.cmp(a));
        for k in explode {
            let it = self.items.swap_remove(k);
            if let Some(h) = it.held_by {
                self.f[h].holding = None;
            }
            if let Some(blast) = it.kind.def().blast {
                self.explode(it.pos, it.owner, blast);
            }
        }
    }

    fn explode(&mut self, pos: Vec2, owner: Option<usize>, blast: Blast) {
        self.events.push(Event::Explosion { pos });
        for j in 0..N_FIGHTERS {
            let d = self.f[j].center().distance(pos);
            if !self.f[j].alive() || d >= blast.radius {
                continue;
            }
            let away = (self.f[j].center() - pos).normalize_or(vec2(0.0, -1.0));
            let dmg = blast.damage * (1.0 - d / blast.radius).max(blast.edge);
            self.damage(owner.unwrap_or(j), j, dmg, away * 220.0 + vec2(0.0, -80.0), 14, away.x.signum(), Source::Grenade, false);
        }
        for it in self.items.iter_mut().filter(|it| it.held_by.is_none()) {
            let d = it.pos.distance(pos);
            if d < blast.radius * 1.5 {
                it.vel += (it.pos - pos).normalize_or(vec2(0.0, -1.0)) * 150.0 * (1.0 - d / (blast.radius * 1.5));
            }
        }
    }

    /// Spawns this round's health pickups at their scheduled times. Any living fighter that
    /// touches one while hurt is healed by HEAL_AMOUNT; at full health it's left alone.
    fn step_heals(&mut self) {
        while self.heal_schedule.last().is_some_and(|&t| self.tick >= t) {
            self.heal_schedule.pop();
            let pos = random_surface_spot(&mut self.rng, self.config.arena.def()) - vec2(0.0, HEAL_R + 1.0);
            self.heals.push(pos);
            self.events.push(Event::Spawn { pos });
        }
        let mut hi = 0;
        while hi < self.heals.len() {
            let hb = Aabb::around(self.heals[hi], HEAL_R);
            let taker = (0..N_FIGHTERS).find(|&j| {
                let f = &self.f[j];
                f.alive() && f.hp < MAX_HP && f.body().overlaps(&hb)
            });
            match taker {
                Some(j) => {
                    let f = &mut self.f[j];
                    f.hp += HEAL_AMOUNT.min(MAX_HP - f.hp);
                    self.events.push(Event::Heal { who: j, pos: self.heals[hi] });
                    self.heals.swap_remove(hi);
                }
                None => hi += 1,
            }
        }
    }

    /// Every few seconds a new item appears somewhere reachable: mostly rocks, some spears,
    /// and rarely a grenade.
    fn spawn_items(&mut self) {
        if self.tick < self.next_item {
            return;
        }
        self.next_item = self.tick + self.rng.gen_range(ITEM_SPAWN_EVERY);
        let loose = self.items.iter().filter(|it| it.held_by.is_none()).count();
        if loose >= MAX_LOOSE_ITEMS {
            return;
        }
        let roll: f32 = self.rng.r#gen();
        let mut total = 0.0;
        let mut kind = SPAWN_ORDER[SPAWN_ORDER.len() - 1];
        for k in SPAWN_ORDER {
            total += k.def().spawn_chance;
            let room = k.def().max_alive.is_none_or(|max| self.items.iter().filter(|it| it.kind == k).count() < max);
            if roll < total && room {
                kind = k;
                break;
            }
        }
        let pos = random_surface_spot(&mut self.rng, self.config.arena.def()) - vec2(0.0, ITEM_R);
        self.items.push(Item::new(kind, pos));
        self.events.push(Event::Spawn { pos });
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn damage(&mut self, src: usize, victim: usize, dmg: f32, knockback: Vec2, stun: u32, dir: f32, source: Source, headshot: bool) {
        if src != victim {
            let a = &mut self.f[src];
            a.dmg_dealt += dmg;
            a.headshots += headshot as u32;
        }
        let v = &mut self.f[victim];
        v.hp -= dmg;
        v.dmg_taken += dmg;
        v.vel = knockback;
        v.grounded = false;
        v.hitstun = stun;
        v.atk_timer = 0;
        let pos = v.center();
        let died = !v.alive();
        self.events.push(Event::Hit { pos, dir, headshot, source });
        if died {
            // Drop whatever was in hand. Anything with a fuse (a grenade) goes off: it arms (if it
            // wasn't already lit) and blows shortly after, on the fallen fighter's account.
            if let Some(k) = self.held_item(victim) {
                let it = &mut self.items[k];
                it.held_by = None;
                it.vel = Vec2::ZERO;
                if it.kind.def().fuse.is_some() {
                    it.fuse = Some(it.fuse.map_or(DROPPED_FUSE, |t| t.min(DROPPED_FUSE)));
                    it.owner.get_or_insert(victim);
                }
            }
            self.f[victim].holding = None;
            self.f[victim].died_at = Some(self.tick);
            self.events.push(Event::Death { who: victim, pos });
            if src != victim {
                self.f[src].kills += 1;
            }
        }
    }

}

/// Everything a viewer needs to draw a match (and the scoreboard): what the server sends
/// clients. The hidden parts of `Match` (RNG, spawn schedule) stay on the server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchView {
    pub tick: u32,
    pub f: [Fighter; N_FIGHTERS],
    pub items: Vec<Item>,
    pub heals: Vec<Vec2>,
    /// What specials have left in the arena (Sam's bombs, Tyler's dogs...).
    pub effects: Vec<Effect>,
    pub over: bool,
    pub winner: Option<usize>,
}

impl Match {
    pub fn view(&self) -> MatchView {
        MatchView {
            tick: self.tick,
            f: self.f.clone(),
            items: self.items.clone(),
            heals: self.heals.clone(),
            effects: self.effects.clone(),
            over: self.over,
            winner: self.winner,
        }
    }

    /// A drawable match rebuilt from a view and the match's rules (it can't be stepped
    /// faithfully: no spawns).
    pub fn from_view(v: MatchView, config: &MatchConfig, events: Vec<Event>) -> Self {
        Self {
            f: v.f,
            items: v.items,
            heals: v.heals,
            heal_schedule: Vec::new(),
            next_item: u32::MAX,
            rng: SimRng::seed_from_u64(0),
            tick: v.tick,
            over: v.over,
            winner: v.winner,
            events,
            effects: v.effects,
            config: config.clone(),
        }
    }
}

/// One tick of a fighter's own movement and swing timing, exactly as `Match::step` does it, but
/// without anyone else: a client runs this on its own fighter (seat `who`) to show its input
/// immediately, and the server's snapshots correct it. Its jumps and swings go in `events`.
pub fn predict(arena: &Arena, f: &mut Fighter, who: usize, a: Action, events: &mut Vec<Event>) {
    if f.absent {
        return;
    }
    control(f, who, a, events);
    collide(f, arena);
    if f.atk_timer > 0 {
        f.atk_timer += 1;
        if f.atk_timer > ATK_TOTAL {
            f.atk_timer = 0;
            f.cooldown = ATK_COOLDOWN;
        }
    } else if f.cooldown > 0 {
        f.cooldown -= 1;
    }
    if f.hand_cd > 0 {
        f.hand_cd -= 1;
    }
    f.action = a;
}

/// Moves a loose or flying item. True if it struck terrain hard this tick (for the sound).
fn step_item(it: &mut Item, arena: &Arena) -> bool {
    it.age += 1;
    let mut struck = false;
    // Flying fast enough to hurt, an item falls at its own rate (spears fly flat); else it arcs.
    let def = it.kind.def();
    let g = if it.lethal { def.flight_gravity } else { 0.8 };
    it.vel.y = (it.vel.y + GRAVITY * g * DT).min(MAX_FALL);
    let rb = |p: Vec2| Aabb::around(p, ITEM_R);
    let bounce = def.bounce;

    it.pos.x += it.vel.x * DT;
    if let Some(s) = SOLIDS.iter().find(|s| rb(it.pos).overlaps(s)) {
        it.pos.x = if it.vel.x > 0.0 { s.x - ITEM_R } else { s.x + s.w + ITEM_R };
        struck |= it.vel.x.abs() > 60.0;
        it.vel.x *= -bounce;
    }

    let prev_bottom = it.pos.y + ITEM_R;
    it.pos.y += it.vel.y * DT;
    let mut landed = None;
    if let Some(s) = SOLIDS.iter().find(|s| rb(it.pos).overlaps(s)) {
        if it.vel.y > 0.0 {
            landed = Some(s.y);
        } else {
            it.pos.y = s.y + s.h + ITEM_R;
            it.vel.y *= -0.3;
        }
    }
    if it.vel.y > 0.0 {
        for p in arena.platforms {
            let over_x = it.pos.x + ITEM_R > p.x && it.pos.x - ITEM_R < p.x + p.w;
            if over_x && prev_bottom <= p.y + 0.01 && it.pos.y + ITEM_R >= p.y {
                landed = Some(p.y);
            }
        }
    }
    if let Some(top) = landed {
        struck |= it.vel.y > 60.0;
        it.pos.y = top - ITEM_R;
        it.vel.y = if it.vel.y > 60.0 { -it.vel.y * bounce } else { 0.0 };
        it.vel.x *= 0.7;
        if it.vel.x.abs() < 4.0 {
            it.vel.x = 0.0;
        }
    }
    if it.vel.length() < LETHAL_SPEED {
        it.lethal = false;
    }
    // Belt and braces: items can never leave the box.
    it.pos.x = it.pos.x.clamp(WALL + ITEM_R, W - WALL - ITEM_R);
    it.pos.y = it.pos.y.clamp(WALL + ITEM_R, FLOOR_Y - ITEM_R);
    struck
}

fn approach(v: f32, target: f32, step: f32) -> f32 {
    if v < target { (v + step).min(target) } else { (v - step).max(target) }
}

fn control(f: &mut Fighter, who: usize, a: Action, events: &mut Vec<Event>) {
    if !f.alive() {
        f.vel.x *= 0.9;
    } else if f.hitstun > 0 {
        f.hitstun -= 1;
        f.vel.x *= 0.97;
    } else {
        let mut target = if a.mv.abs() < 0.2 { 0.0 } else { a.mv.signum() * if a.run { RUN } else { WALK } };
        if f.atk_timer > 0 && f.grounded {
            target *= 0.4;
        }
        let acc = if f.grounded { GROUND_ACC } else { AIR_ACC };
        f.vel.x = approach(f.vel.x, target, acc * DT);
        if f.atk_timer == 0 && a.mv.abs() >= 0.2 {
            f.facing = a.mv.signum();
        }
        if a.jump && f.grounded {
            f.vel.y = -JUMP_V;
            f.grounded = false;
            events.push(Event::Jump { who, pos: f.pos });
        }
        if a.attack && f.atk_timer == 0 && f.cooldown == 0 {
            f.atk_timer = 1;
            f.hit_mask = 0;
            // A player aiming swings toward their aim, whichever way they were running.
            if let Some(t) = a.target
                && t.x.abs() > 1e-3
            {
                f.facing = t.x.signum();
            }
            events.push(Event::Swing { who });
        }
    }
    // Releasing jump early cuts the arc short.
    let g = if f.vel.y < 0.0 && !a.jump { GRAVITY * 2.0 } else { GRAVITY };
    f.vel.y = (f.vel.y + g * DT).min(MAX_FALL);
}

fn collide(f: &mut Fighter, arena: &Arena) {
    f.pos.x += f.vel.x * DT;
    for s in SOLIDS.iter() {
        if f.body().overlaps(s) {
            if f.vel.x > 0.0 {
                f.pos.x = s.x - HALF_W;
            } else if f.vel.x < 0.0 {
                f.pos.x = s.x + s.w + HALF_W;
            }
            f.vel.x = 0.0;
        }
    }

    let prev_bottom = f.pos.y;
    f.pos.y += f.vel.y * DT;
    f.grounded = false;
    for s in SOLIDS.iter() {
        if f.body().overlaps(s) {
            if f.vel.y > 0.0 {
                f.pos.y = s.y;
                f.grounded = true;
            } else {
                f.pos.y = s.y + s.h + BODY_H;
            }
            f.vel.y = 0.0;
        }
    }
    if f.vel.y >= 0.0 {
        for p in arena.platforms {
            let over_x = f.pos.x + HALF_W > p.x && f.pos.x - HALF_W < p.x + p.w;
            if over_x && prev_bottom <= p.y + 0.01 && f.pos.y >= p.y {
                f.pos.y = p.y;
                f.vel.y = 0.0;
                f.grounded = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays fixed-seed matches between the committed brains and hashes every fighter's state
    /// every tick. Any change to physics, senses, the brain or the RNG stream changes the hash,
    /// so refactors (and new characters) must leave classic matches bit-identical.
    #[test]
    fn classic_matches_are_unchanged() {
        let brains = Brain::committed();
        assert_eq!(brains.len(), 5);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |x: u64| h = (h ^ x).wrapping_mul(0x100_0000_01b3);
        for seed in 0..8u64 {
            let mut m = Match::new(seed, &MatchConfig::default());
            let lineup: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| &brains[(seed as usize + i) % brains.len()]);
            while !m.over {
                m.step(lineup);
                for f in &m.f {
                    for v in [f.pos.x, f.pos.y, f.vel.x, f.vel.y, f.hp] {
                        mix(v.to_bits() as u64);
                    }
                }
                mix(m.items.len() as u64);
            }
            mix(m.tick as u64);
            mix(m.winner.map_or(9, |w| w as u64));
        }
        assert_eq!(h, CLASSIC_HASH, "classic match fingerprint changed: {h:#018x}");
    }
    // Last deliberately changed by the portable RNG (SimRng) and libm maths. It must come out
    // the same on 32-bit targets too: `cargo test -p go-box-sim --target i686-pc-windows-msvc`.
    const CLASSIC_HASH: u64 = 0x369f_a7c2_28c0_b89c;

    /// Everyone on the floor, nothing lying around, nobody thinking (all-zero brains stand still).
    fn quiet_match() -> (Match, Brain) {
        let mut m = Match::new(3, &MatchConfig { heals: 0..=0, ..MatchConfig::default() });
        m.items.clear();
        m.next_item = u32::MAX;
        for (i, x) in [60.0, 200.0, 300.0, 420.0].into_iter().enumerate() {
            m.f[i].pos = vec2(x, FLOOR_Y);
        }
        (m, Brain { w: vec![0.0; Brain::N_WEIGHTS] })
    }

    #[test]
    fn a_grenade_held_by_someone_who_dies_goes_off() {
        let (mut m, z) = quiet_match();
        m.items.push(Item { held_by: Some(1), ..Item::new(ItemKind::Grenade, m.f[1].hand()) });
        m.f[1].holding = Some(ItemKind::Grenade);
        m.f[2].pos.x = 220.0;
        m.damage(0, 1, MAX_HP, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert!(!m.f[1].alive());
        let mut boom = false;
        for _ in 0..DROPPED_FUSE + 2 {
            m.step([&z; N_FIGHTERS]);
            boom |= m.events.iter().any(|e| matches!(e, Event::Explosion { .. }));
        }
        assert!(boom, "the dropped grenade exploded");
        assert!(m.f[2].hp < MAX_HP, "and hurt the fighter standing next to it");
    }

    #[test]
    fn a_solo_match_runs_until_time_or_death() {
        let (mut m, z) = quiet_match();
        for i in 1..N_FIGHTERS {
            m.remove(i);
        }
        for _ in 0..secs(5) {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(!m.over, "alone isn't 'last one standing'");
        m.damage(0, 0, MAX_HP, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        m.step([&z; N_FIGHTERS]);
        assert!(m.over && m.winner.is_none());
    }

    #[test]
    fn a_player_throws_where_they_aim() {
        let (mut m, z) = quiet_match();
        m.items.push(Item { held_by: Some(0), ..Item::new(ItemKind::Rock, m.f[0].hand()) });
        m.f[0].holding = Some(ItemKind::Rock);
        m.f[0].facing = 1.0;
        // Aim up and back over the shoulder.
        let a = Action { throw: true, target: Some(vec2(-1.0, -1.0)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, a)));
        let v = m.items[0].vel;
        assert!(v.x < 0.0 && v.y < 0.0, "flew {v}");
        assert_eq!(m.f[0].facing, -1.0, "turned to face the throw");
    }

    #[test]
    fn fast_atan2_matches_std() {
        let mut worst = 0.0f32;
        for i in 0..3600 {
            let a = i as f32 / 3600.0 * std::f32::consts::TAU;
            for r in [0.01f32, 1.0, 300.0] {
                let (y, x) = (a.sin() * r, a.cos() * r);
                worst = worst.max((super::fast_atan2(y, x) - y.atan2(x)).abs());
            }
        }
        assert!(worst < 1e-4, "max error {worst}");
    }
}

#[cfg(test)]
impl Match {
    pub(crate) fn rng_reseed(&mut self, seed: u64) {
        self.rng = SimRng::seed_from_u64(seed);
    }
}
