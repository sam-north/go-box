//! Headless, deterministic arena simulation: four humans, free-for-all, starting empty-handed
//! with spears, rocks, grenades and health lying around. No hard-coded behaviour: fighters only
//! expose physical abilities; brains decide what to do with them.

use crate::brain::{Brain, IN_EXTRA, IN_HEAL, IN_ITEMS, IN_MEM, IN_NEAREST, IN_OPP, IN_SELF, IN_THREAT, MEM, N_IN, N_OUT, OUT_MEM};
use crate::character::{Character, Effect, props};
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
/// The default 3-2-1 before a match, and the protected start after it.
pub const COUNTDOWN: u32 = secs(3);
pub const SPAWN_GRACE: u32 = secs(3);
/// Seats: the fighters players and bots play (4).
pub const N_FIGHTERS: usize = 4;
/// How many of Seth's minis can be out at once (they take the slots after the seats).
pub const MAX_MINIS: usize = 4;
/// Every body in a match: the seats, then the mini slots (empty until summoned).
pub const N_BODIES: usize = N_FIGHTERS + MAX_MINIS;

// Body.
pub const HALF_W: f32 = 4.0;
pub const BODY_H: f32 = 14.0;
/// Top slice of the body that counts as the head for headshots.
pub const HEAD_H: f32 = 4.0;
const GRAVITY: f32 = 640.0;
const MAX_FALL: f32 = 320.0;
const JUMP_V: f32 = 250.0;
// Moving: a quick start up to walking pace, then speed builds gradually to a full run (slow to
// fast, ~0.4s from standing); stopping and turning are snappy, so it stays controllable. In the
// air you can steer and reverse, just not as sharply as on the ground.
const WALK: f32 = 45.0;
const RUN: f32 = 150.0;
/// How much faster a hasted fighter runs (Tyler with his dogs out).
const HASTE: f32 = 1.35;
/// How fast you can move (and how high you can jump) stuck in glue.
const GLUED_SPEED: f32 = 0.3;
const GLUED_JUMP: f32 = 0.55;
// Wall jumps: off a wall while in the air (a fresh press of jump), up and away from it. Pushing
// into a wall while falling slides down it slowly.
const WALL_JUMP_V: f32 = 235.0;
const WALL_KICK: f32 = 170.0;
const WALL_JUMP_CD: u32 = 12;
const WALL_SLIDE: f32 = 70.0;
// Dash: a quick burst along the ground (or through the air, holding height), then a cooldown.
pub const DASH_SPEED: f32 = 380.0;
pub const DASH_TICKS: u32 = 9;
pub const DASH_COOLDOWN: u32 = 45;
/// px/s²: getting going (up to walking pace), then building to a run.
const GROUND_START: f32 = 900.0;
const GROUND_BUILD: f32 = 290.0;
/// Slowing down, stopping or turning around on the ground.
const GROUND_BRAKE: f32 = 1400.0;
const AIR_ACC: f32 = 600.0;
const AIR_BRAKE: f32 = 950.0;
/// The speed the bots' senses measure movement against: the run speed their brains were trained
/// with. (Kept when RUN changed, so what they feel as "running" means the same.)
const SENSE_SPEED: f32 = 85.0;
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
use crate::item::{Blast, DROPPED_FUSE, FISTS, Hit, N_SENSED, OnThrow, SPAWN_ORDER};
pub const GRENADE_RADIUS: f32 = crate::item::GRENADE_BLAST.radius;
pub const ITEM_R: f32 = 1.5;
const LETHAL_SPEED: f32 = 110.0;
const OWN_GRACE: u32 = 15;

// Items lying around.
const START_SPEARS: usize = 2;
const START_ROCKS: usize = 1;
const START_GRENADES: usize = 4;
const ITEM_SPAWN_EVERY: Range<u32> = secs(4)..secs(7);
const MAX_LOOSE_ITEMS: usize = 12;

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
    /// A piece of Justin's sushi.
    Sushi,
    /// The giant sword.
    Slash,
    /// Burning ground (a Molotov).
    Fire,
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
    /// A dash this tick (a fresh press).
    pub dash: bool,
    /// Where a mouse player's cursor is, in the arena (Austin teleports there). None with a
    /// gamepad, and for bots.
    pub cursor: Option<Vec2>,
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
    /// Size: 1 for everyone but Seth's minis (smaller; see `MINI_SCALE`).
    pub scale: f32,
    /// For a mini: the seat that summoned it (its team; credited with what it does).
    pub owner: Option<usize>,
    /// For a mini: ticks until it vanishes.
    #[serde(skip)]
    pub life: Option<u32>,
    /// Jake inside his phone: ticks until he comes back out (0 = he's here as normal). Meanwhile
    /// he can't act or be hurt, and his position follows the phone.
    pub digital: u32,
    /// Pinned in place by Kyle: ticks left. Can't move or jump (can still swing and throw).
    pub pinned: u32,
    /// Running faster (Tyler, while his dogs are out): ticks left.
    pub haste: u32,
    /// Stuck in Sam's glue: slowed right down (ticks left; the glue keeps topping it up).
    pub slowed: u32,
    /// Dashing: ticks left of the burst, and until the next one is allowed.
    pub dash_t: u32,
    pub dash_cd: u32,
    /// Jump was held last tick (a wall jump needs a fresh press), and ticks before another wall
    /// jump.
    pub jump_held: bool,
    pub wall_cd: u32,
    /// Inside a phone: which way left/right was held last tick (a fresh press switches phones).
    #[serde(skip)]
    pub phone_mv: i8,
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
            scale: 1.0,
            owner: None,
            life: None,
            digital: 0,
            pinned: 0,
            haste: 0,
            slowed: 0,
            dash_t: 0,
            dash_cd: 0,
            jump_held: false,
            wall_cd: 0,
            phone_mv: 0,
        }
    }

    /// A slot with nobody in it (a mini slot before anything is summoned).
    pub fn empty() -> Self {
        let mut f = Self::new(0.0, 1.0);
        f.hp = 0.0;
        f.absent = true;
        f
    }

    pub fn alive(&self) -> bool {
        self.hp > 0.0
    }

    /// Alive and in the arena: not tucked away inside a phone (Jake).
    pub fn present(&self) -> bool {
        self.alive() && self.digital == 0
    }

    pub fn half_w(&self) -> f32 {
        HALF_W * self.scale
    }

    pub fn height(&self) -> f32 {
        BODY_H * self.scale
    }

    /// Whose side it's on: its own seat, or for a mini the seat that summoned it.
    pub fn team(&self, me: usize) -> usize {
        self.owner.unwrap_or(me)
    }

    pub fn body(&self) -> Aabb {
        Aabb::new(self.pos.x - self.half_w(), self.pos.y - self.height(), self.half_w() * 2.0, self.height())
    }

    pub fn eye(&self) -> Vec2 {
        vec2(self.pos.x, self.pos.y - 10.0 * self.scale)
    }

    pub fn center(&self) -> Vec2 {
        vec2(self.pos.x, self.pos.y - self.height() * 0.5)
    }

    /// Where a carried item sits: cocked over the shoulder.
    pub fn hand(&self) -> Vec2 {
        vec2(self.pos.x - self.facing * 1.5 * self.scale, self.pos.y - 13.0 * self.scale)
    }

    fn is_head(&self, y: f32) -> bool {
        let top = self.pos.y - self.height();
        y >= top - 1.0 && y < top + HEAD_H * self.scale
    }

    pub fn attack_active(&self) -> bool {
        self.atk_timer > ATK_WINDUP && self.atk_timer <= ATK_WINDUP + ATK_ACTIVE
    }

    /// What a swing does now: the held item's melee (a spear stabs), else fists: (reach, hit).
    pub fn melee(&self) -> (f32, Hit) {
        self.holding.and_then(|k| k.def().melee).unwrap_or(FISTS)
    }

    pub fn melee_reach(&self) -> f32 {
        // Smaller fighters reach less, though not in proportion (a mini's arms are short).
        self.melee().0 * (0.5 + 0.5 * self.scale)
    }

    pub fn attack_box(&self) -> Aabb {
        let reach = self.melee_reach();
        let (hw, s) = (self.half_w(), self.scale);
        let x0 = if self.facing > 0.0 { self.pos.x + hw - 2.0 * s } else { self.pos.x - hw + 2.0 * s - reach };
        Aabb::new(x0, self.pos.y - self.height() + 2.0 * s, reach, 10.0 * s)
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
    /// Jake's phone with Jake inside it: whose seat.
    pub carries: Option<usize>,
    #[serde(skip)]
    age: u32,
}

impl Item {
    pub fn new(kind: ItemKind, pos: Vec2) -> Self {
        Self { kind, pos, vel: Vec2::ZERO, held_by: None, owner: None, lethal: false, fuse: None, carries: None, age: 0 }
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
    /// Sam's glue sticking to someone (or something).
    Glued { pos: Vec2 },
    /// One of Tyler's dogs biting.
    Bite { pos: Vec2 },
    /// One of Corwin's cats scratching.
    Scratch { pos: Vec2 },
    /// Thrown sushi hitting someone or something (and that's the end of it).
    Splat { pos: Vec2 },
    /// Ryan's chain hooking someone.
    Hook { pos: Vec2 },
    /// A rocket fired from a rocket launcher.
    RocketLaunch { pos: Vec2 },
    /// Fighter `who` dashed.
    Dash { who: usize, pos: Vec2 },
    /// A Molotov shattering (the ground catches fire).
    Ignite { pos: Vec2 },
    /// Fighter `who` radioing in an air strike.
    RadioCall { pos: Vec2 },
    /// The air strike's jet coming in (flying `dir`: 1 left to right).
    JetPass { dir: f32 },
    /// A bomb dropped from the jet.
    BombDrop { pos: Vec2 },
    /// Jake going into his phone (`out`: false) or coming back out of it, at his middle.
    Hologram { pos: Vec2, out: bool },
    /// Robert's lightning striking someone.
    Zap { pos: Vec2 },
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
    /// How long the match lasts once it's started (after the countdown).
    pub max_ticks: u32,
    /// The 3-2-1 before it starts: nobody can act yet.
    pub countdown: u32,
    /// After the countdown, how long nobody can be hurt (they can move, jump, grab).
    pub spawn_grace: u32,
    /// How many health pickups appear (picked at random from the range).
    pub heals: RangeInclusive<u32>,
    pub mode: Mode,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self { arena: ArenaId::default(), max_ticks: MAX_TICKS, countdown: COUNTDOWN, spawn_grace: SPAWN_GRACE, heals: HEALS_PER_ROUND, mode: Mode::default() }
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
                let present = m.f[..N_FIGHTERS].iter().filter(|f| !f.absent).count();
                let last_standing = if present > 1 { alive.len() <= 1 } else { alive.is_empty() };
                if !last_standing && m.play_ticks() < m.config.max_ticks {
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
    pub f: [Fighter; N_BODIES],
    pub items: Vec<Item>,
    /// Health pickups currently lying in the arena.
    pub heals: Vec<Vec2>,
    /// Remaining heal spawn times this round, latest first.
    heal_schedule: Vec<u32>,
    next_item: u32,
    /// How many of each kind of item have spawned this match (some are limited per match).
    spawned: [u32; ItemKind::ALL.len()],
    rng: SimRng,
    pub tick: u32,
    pub over: bool,
    pub winner: Option<usize>,
    pub events: Vec<Event>,
    /// What specials have left in the arena (Sam's glue, Tyler's dogs...).
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
            if i >= N_FIGHTERS {
                return Fighter::empty();
            }
            let x = arena.spawn_x[i] + rng.gen_range(0.0..SPAWN_SPREAD);
            Fighter::new(x, if x < W / 2.0 { 1.0 } else { -1.0 })
        });
        let mut items = Vec::new();
        for (kind, n) in [(ItemKind::Spear, START_SPEARS), (ItemKind::Rock, START_ROCKS), (ItemKind::Grenade, START_GRENADES)] {
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
            spawned: [0; ItemKind::ALL.len()],
            rng: SimRng::seed_from_u64(rng.r#gen()),
            tick: 0,
            over: false,
            winner: None,
            events: Vec::new(),
            effects: Vec::new(),
            config: config.clone(),
        }
    }

    /// Are `j` and `i` enemies: `j` is alive and on another team (a mini is on its summoner's)?
    pub fn foes(&self, i: usize, j: usize) -> bool {
        j != i && self.f[j].present() && self.f[j].team(j) != self.f[i].team(i)
    }

    /// Ends the match now, decided as if time had run out (most health wins).
    pub fn end_now(&mut self) {
        if self.over {
            return;
        }
        self.config.max_ticks = self.play_ticks();
        self.over = true;
        self.winner = self.config.mode.outcome(self).flatten();
    }

    /// Still in the 3-2-1 before the start.
    pub fn counting_down(&self) -> bool {
        self.tick < self.config.countdown
    }

    /// Ticks since the countdown ended (the match clock).
    pub fn play_ticks(&self) -> u32 {
        self.tick.saturating_sub(self.config.countdown)
    }

    /// Nobody can be hurt yet: the countdown, then the first `spawn_grace` ticks of play.
    pub fn protected(&self) -> bool {
        self.tick < self.config.countdown + self.config.spawn_grace
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
        for j in (0..N_BODIES).filter(|&j| self.foes(i, j)) {
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
        // Brains never saw specials in training; someone else's dog or glue reads as a threat.
        for e in self.effects.iter().filter(|e| e.owner() != i) {
            if let Some(p) = e.threat_pos() {
                mark(&mut s, IN_THREAT, glimpse(arena, eye, p));
            }
        }
        let k = IN_SELF;
        s[k] = me.vel.x / SENSE_SPEED;
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
            s[e + 3] = (o.vel.x - me.vel.x) / (2.0 * SENSE_SPEED);
            s[e + 4] = ((o.vel.y - me.vel.y) / MAX_FALL).clamp(-1.0, 1.0);
        }
        s[e + 5] = ((0..N_BODIES).filter(|&j| self.foes(i, j)).count() as f32 / (N_FIGHTERS - 1) as f32).min(1.0);
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
        s[e + 11] = 1.0 - self.play_ticks() as f32 / self.config.max_ticks as f32;
        let best_other = (0..N_BODIES).filter(|&j| self.foes(i, j)).map(|j| self.f[j].hp).fold(0.0, f32::max);
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

        // 1. Perceive + think (not during the countdown: nobody acts until it's over).
        let mut actions = [Action::default(); N_BODIES];
        let frozen = self.counting_down();
        for i in 0..N_BODIES {
            if !self.f[i].alive() || frozen {
                continue;
            }
            // (A player inside Jake's phone still steers: left/right picks the phone to come out of.)
            if let Some(a) = humans.get(i).copied().flatten() {
                actions[i] = a;
                self.f[i].action = a;
                continue;
            }
            if !self.f[i].present() {
                continue;
            }
            let input = self.senses(i);
            let mut out = [0.0; N_OUT];
            // A mini thinks with its summoner's brain.
            brains[self.f[i].team(i)].forward(&input, &mut out);
            actions[i] = Action {
                mv: out[0],
                run: out[1] > 0.0,
                jump: out[2] > 0.0,
                attack: out[3] > 0.0,
                throw: out[4] > 0.0,
                aim: out[5],
                grab: out[6] > 0.0,
                // (Not during the protected start: it'd be wasted on people who can't be hurt.)
                dash: false,
                cursor: None,
                special: self.f[i].special_cd == 0 && !self.protected() && self.f[i].character.special().is_some_and(|s| s.bot_wants(self, i)),
                target: None,
            };
            self.f[i].memory.copy_from_slice(&out[OUT_MEM..]);
            self.f[i].action = actions[i];
        }

        // 2. Move.
        let props = props(&self.effects);
        for i in 0..N_BODIES {
            if self.f[i].absent || self.f[i].digital > 0 {
                continue;
            }
            let was_grounded = self.f[i].grounded;
            let wall = wall_side(&self.f[i], &props);
            let f = &mut self.f[i];
            control(f, i, actions[i], wall, &mut self.events);
            collide(f, self.config.arena.def(), &props);
            if f.grounded && !was_grounded {
                self.events.push(Event::Land { pos: f.pos });
            }
        }

        // 2b. Specials.
        self.step_specials(actions.map(|a| a.special));

        // 3. Hands: grab or throw.
        for i in 0..N_BODIES {
            self.hands(i, actions[i]);
        }

        // 4. Melee (resolved simultaneously).
        let mut hits = Vec::new();
        for i in 0..N_BODIES {
            let a = &self.f[i];
            if !a.alive() || !a.attack_active() {
                continue;
            }
            for j in 0..N_BODIES {
                if self.foes(i, j) && a.hit_mask & (1 << j) == 0 && a.attack_box().overlaps(&self.f[j].body()) {
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
            let headshot = self.f[j].is_head(a.pos.y - 8.0 * a.scale);
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
        self.step_minis();
        self.step_digital();
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
        if !f.present() || f.hitstun > 0 || f.hand_cd > 0 {
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
            // A rocket launcher fires its rocket that way instead, and a radio calls in an air
            // strike; either is used up.
            match self.items[k].kind.def().on_throw {
                OnThrow::Throw => {}
                OnThrow::Rocket => {
                    let from = f.hand();
                    self.items.swap_remove(k);
                    self.effects.push(Effect::Rocket(crate::character::Rocket::new(from, dir, i)));
                    self.events.push(Event::RocketLaunch { pos: from });
                    return;
                }
                OnThrow::AirStrike => {
                    let from = f.hand();
                    self.items.swap_remove(k);
                    let plane = crate::character::Plane::new(&mut self.rng, i);
                    self.effects.push(Effect::Plane(plane));
                    self.events.push(Event::RadioCall { pos: from });
                    return;
                }
            }
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
        // Items to remove: (index, explodes) — grenades going off, sushi splatting or spoiling.
        let mut gone = Vec::new();
        let props = props(&self.effects);
        for k in 0..self.items.len() {
            let splats = self.items[k].kind.def().splats;
            if let Some(h) = self.items[k].held_by {
                self.items[k].pos = self.f[h].hand();
            } else {
                let (struck, touched) = step_item(&mut self.items[k], self.config.arena.def(), &props);
                let it = &self.items[k];
                // A thrown Molotov shatters on whatever it hits, and the ground below catches fire.
                if it.kind.def().ignites && it.owner.is_some() {
                    let ib = Aabb::around(it.pos, ITEM_R);
                    let on_someone = (0..N_BODIES).any(|j| self.f[j].present() && !(it.owner == Some(j) && it.age < OWN_GRACE) && ib.overlaps(&self.f[j].body()));
                    if touched || on_someone {
                        let (pos, owner) = (it.pos, it.owner.unwrap_or(0));
                        let fire = crate::character::Fire::at(self.arena(), &props, pos, owner);
                        self.events.push(Event::Ignite { pos });
                        self.effects.push(Effect::Fire(fire));
                        gone.push((k, false));
                        continue;
                    }
                }
                if let Some(spoil) = splats {
                    // Thrown sushi is done the moment it touches anything; uneaten, it spoils.
                    if it.owner.is_some() && touched {
                        self.events.push(Event::Splat { pos: it.pos });
                        gone.push((k, false));
                        continue;
                    }
                    if it.age > spoil {
                        gone.push((k, false));
                        continue;
                    }
                } else if struck {
                    self.events.push(Event::Clatter { pos: it.pos, kind: it.kind });
                }
            }
            if let Some(t) = self.items[k].fuse {
                if t == 0 {
                    gone.push((k, true));
                    continue;
                }
                self.items[k].fuse = Some(t - 1);
            }
            let it = &self.items[k];
            if !it.lethal {
                continue;
            }
            let ib = Aabb::around(it.pos, ITEM_R);
            let victim = (0..N_BODIES).find(|&j| {
                let own_grace = it.owner == Some(j) && it.age < OWN_GRACE;
                // Thrown things don't hit the thrower's teammates (Seth and his minis).
                let friendly = it.owner.is_some_and(|o| o != j && self.f[o].team(o) == self.f[j].team(j));
                self.f[j].present() && !own_grace && !friendly && ib.overlaps(&self.f[j].body())
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
                let pos = it.pos;
                self.damage(src, j, dmg, vec2(dir * 110.0, -100.0), 10, dir, hit.source, headshot);
                if splats.is_some() {
                    self.events.push(Event::Splat { pos });
                    gone.push((k, false));
                }
            }
        }
        // Remove back-to-front so indices stay valid.
        gone.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        for (k, explodes) in gone {
            let it = self.items.swap_remove(k);
            if let Some(h) = it.held_by {
                self.f[h].holding = None;
            }
            if let Some(blast) = it.kind.def().blast.filter(|_| explodes) {
                self.explode(it.pos, it.owner, blast);
            }
        }
    }

    pub(crate) fn explode(&mut self, pos: Vec2, owner: Option<usize>, blast: Blast) {
        self.events.push(Event::Explosion { pos });
        for j in 0..N_BODIES {
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
            // (Not minis: one hit is all they take.)
            let taker = (0..N_FIGHTERS).find(|&j| {
                let f = &self.f[j];
                f.present() && f.hp < MAX_HP && f.body().overlaps(&hb)
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

    /// Every few seconds a new item appears somewhere reachable (the odds are in `item.rs`).
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
            let room = k.def().max_alive.is_none_or(|max| self.items.iter().filter(|it| it.kind == k).count() < max)
                && k.def().per_match.is_none_or(|max| self.spawned[k as usize] < max);
            if roll < total && room {
                kind = k;
                break;
            }
        }
        let pos = random_surface_spot(&mut self.rng, self.config.arena.def()) - vec2(0.0, ITEM_R);
        self.items.push(Item::new(kind, pos));
        self.spawned[kind as usize] += 1;
        self.events.push(Event::Spawn { pos });
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn damage(&mut self, src: usize, victim: usize, dmg: f32, knockback: Vec2, stun: u32, dir: f32, source: Source, headshot: bool) {
        // Nobody can be hurt at the start (the hit just doesn't happen: no knockback either), nor
        // Jake while he's inside his phone.
        if self.protected() || self.f[victim].digital > 0 {
            return;
        }
        // What a mini does counts for whoever summoned it; hurting your own side counts for nobody.
        let credit = self.f[src].team(src);
        let scores = credit != self.f[victim].team(victim);
        if scores {
            let a = &mut self.f[credit];
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
            if scores {
                self.f[credit].kills += 1;
            }
        }
    }

}

/// Everything a viewer needs to draw a match (and the scoreboard): what the server sends
/// clients. The hidden parts of `Match` (RNG, spawn schedule) stay on the server.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatchView {
    pub tick: u32,
    pub f: [Fighter; N_BODIES],
    pub items: Vec<Item>,
    pub heals: Vec<Vec2>,
    /// What specials have left in the arena (Sam's glue, Tyler's dogs...).
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
            spawned: [0; ItemKind::ALL.len()],
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
/// `props` are the tables in the arena (`props(&view.effects)`).
pub fn predict(arena: &Arena, props: &[Aabb], f: &mut Fighter, who: usize, a: Action, events: &mut Vec<Event>) {
    if f.absent || f.digital > 0 {
        return;
    }
    control(f, who, a, wall_side(f, props), events);
    collide(f, arena, props);
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

/// Moves a loose or flying item (it can rest on `props`, Justin's tables). Returns whether it
/// struck terrain hard this tick (for the sound), and whether it touched terrain at all.
fn step_item(it: &mut Item, arena: &Arena, props: &[Aabb]) -> (bool, bool) {
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
        for p in arena.platforms.iter().chain(props) {
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
    let touched = landed.is_some() || struck || SOLIDS.iter().any(|s| Aabb::around(it.pos, ITEM_R + 0.5).overlaps(s));
    // Belt and braces: items can never leave the box.
    it.pos.x = it.pos.x.clamp(WALL + ITEM_R, W - WALL - ITEM_R);
    it.pos.y = it.pos.y.clamp(WALL + ITEM_R, FLOOR_Y - ITEM_R);
    (struck, touched)
}

fn approach(v: f32, target: f32, step: f32) -> f32 {
    if v < target { (v + step).min(target) } else { (v - step).max(target) }
}

/// How hard a fighter moving at `v` changes speed toward `target` (px/s²).
fn accel(v: f32, target: f32, grounded: bool) -> f32 {
    // Stopping, turning around, or slowing to a lower speed (e.g. while swinging).
    let braking = target == 0.0 || v * target < 0.0 || v.abs() > target.abs();
    match (grounded, braking) {
        (true, true) => GROUND_BRAKE,
        (true, false) if v.abs() < WALK => GROUND_START,
        (true, false) => GROUND_BUILD,
        (false, true) => AIR_BRAKE,
        // Speeding up in the air is no quicker than on the ground once past walking pace.
        (false, false) => if v.abs() < WALK { AIR_ACC } else { GROUND_BUILD },
    }
}

/// Which side a fighter in the air is pressed against a wall (or a table's side) on: -1 left, 1
/// right, 0 neither. (One-way ledges aren't walls.)
fn wall_side(f: &Fighter, props: &[Aabb]) -> f32 {
    if f.grounded {
        return 0.0;
    }
    let b = f.body();
    let probe = |dx: f32| {
        let r = Aabb::new(b.x + dx, b.y + 1.0, b.w, b.h - 2.0);
        SOLIDS.iter().chain(props).any(|s| r.overlaps(s))
    };
    if probe(-1.0) {
        -1.0
    } else if probe(1.0) {
        1.0
    } else {
        0.0
    }
}

fn control(f: &mut Fighter, who: usize, a: Action, wall: f32, events: &mut Vec<Event>) {
    let fresh_jump = a.jump && !f.jump_held;
    f.jump_held = a.jump;
    f.wall_cd = f.wall_cd.saturating_sub(1);
    f.dash_cd = f.dash_cd.saturating_sub(1);
    let pinned = f.pinned > 0;
    f.pinned = f.pinned.saturating_sub(1);
    let run = if f.haste > 0 { RUN * HASTE } else { RUN };
    f.haste = f.haste.saturating_sub(1);
    let glued = f.slowed > 0;
    f.slowed = f.slowed.saturating_sub(1);
    if !f.alive() {
        f.vel.x *= 0.9;
    } else if f.hitstun > 0 {
        f.hitstun -= 1;
        f.vel.x *= 0.97;
    } else {
        let mut target = if a.mv.abs() < 0.2 { 0.0 } else { a.mv.signum() * if a.run { run } else { WALK } };
        if f.atk_timer > 0 && f.grounded {
            target *= 0.4;
        }
        if glued {
            target *= GLUED_SPEED;
        }
        f.vel.x = approach(f.vel.x, target, accel(f.vel.x, target, f.grounded) * DT);
        if f.atk_timer == 0 && a.mv.abs() >= 0.2 {
            f.facing = a.mv.signum();
        }
        if a.jump && f.grounded && !pinned {
            f.vel.y = if glued { -JUMP_V * GLUED_JUMP } else { -JUMP_V };
            f.grounded = false;
            events.push(Event::Jump { who, pos: f.pos });
        }
        // Off a wall: up and away from it.
        if fresh_jump && wall != 0.0 && !f.grounded && f.wall_cd == 0 && !pinned {
            let k = if glued { GLUED_JUMP } else { 1.0 };
            f.vel = vec2(-wall * WALL_KICK * k, -WALL_JUMP_V * k);
            f.facing = -wall;
            f.wall_cd = WALL_JUMP_CD;
            events.push(Event::Jump { who, pos: f.pos });
        } else if wall != 0.0 && a.mv * wall > 0.0 && f.vel.y > WALL_SLIDE {
            // Pushing into a wall while falling: slide down it.
            f.vel.y = WALL_SLIDE;
        }
        // Dash: the way you're pushing, else the way you face.
        if a.dash && f.dash_cd == 0 && !pinned {
            if a.mv.abs() >= 0.2 {
                f.facing = a.mv.signum();
            }
            f.dash_t = DASH_TICKS;
            f.dash_cd = DASH_COOLDOWN;
            events.push(Event::Dash { who, pos: f.pos });
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
    // Pinned: stuck exactly where they are (in mid-air too), whatever hits them.
    if pinned {
        f.vel = Vec2::ZERO;
        f.dash_t = 0;
        return;
    }
    // Mid-dash: flat out, holding height (cut short by a hit).
    if f.dash_t > 0 {
        f.dash_t -= 1;
        if f.alive() && f.hitstun == 0 {
            f.vel = vec2(f.facing * DASH_SPEED * if glued { GLUED_SPEED } else { 1.0 }, 0.0);
            return;
        }
        f.dash_t = 0;
    }
    // Releasing jump early cuts the arc short.
    let g = if f.vel.y < 0.0 && !a.jump { GRAVITY * 2.0 } else { GRAVITY };
    f.vel.y = (f.vel.y + g * DT).min(MAX_FALL);
}

/// Moves a fighter and stops it at terrain. `props` (Justin's tables) block you from the sides
/// and hold you up on top, but only stop you entering them: anyone already inside one (it fell
/// on them) walks or jumps straight out, and there's no underside to bump into, so nobody can
/// ever be pinned under one.
fn collide(f: &mut Fighter, arena: &Arena, props: &[Aabb]) {
    let before = f.body();
    f.pos.x += f.vel.x * DT;
    for s in SOLIDS.iter() {
        if f.body().overlaps(s) {
            if f.vel.x > 0.0 {
                f.pos.x = s.x - f.half_w();
            } else if f.vel.x < 0.0 {
                f.pos.x = s.x + s.w + f.half_w();
            }
            f.vel.x = 0.0;
        }
    }
    for p in props {
        if f.body().overlaps(p) && !before.overlaps(p) {
            f.pos.x = if before.x + before.w <= p.x { p.x - f.half_w() } else { p.x + p.w + f.half_w() };
            f.vel.x = 0.0;
        }
    }

    let prev_bottom = f.pos.y;
    f.pos.y += f.vel.y * DT;
    // (Only something pinned stands perfectly still: it keeps its footing.)
    if f.vel.y != 0.0 {
        f.grounded = false;
    }
    for s in SOLIDS.iter() {
        if f.body().overlaps(s) {
            if f.vel.y > 0.0 {
                f.pos.y = s.y;
                f.grounded = true;
            } else {
                f.pos.y = s.y + s.h + f.height();
            }
            f.vel.y = 0.0;
        }
    }
    if f.vel.y >= 0.0 {
        for p in arena.platforms.iter().chain(props) {
            let over_x = f.pos.x + f.half_w() > p.x && f.pos.x - f.half_w() < p.x + p.w;
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
                for f in &m.f[..N_FIGHTERS] {
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
    // Last deliberately changed by one rock a match and the new items. It must
    // come out the same on 32-bit targets too: `cargo test -p go-box-sim --target i686-pc-windows-msvc`.
    const CLASSIC_HASH: u64 = 0x183d_2237_c2dd_a31e;

    /// Everyone on the floor, nothing lying around, nobody thinking (all-zero brains stand still).
    fn quiet_match() -> (Match, Brain) {
        let mut m = Match::new(3, &MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, ..MatchConfig::default() });
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
    fn nobody_acts_in_the_countdown_and_nobody_is_hurt_just_after() {
        let mut m = Match::new(3, &MatchConfig { heals: 0..=0, ..MatchConfig::default() });
        m.items.clear();
        let z = Brain { w: vec![0.0; Brain::N_WEIGHTS] };
        let run = Action { mv: 1.0, run: true, ..Action::default() };
        let x0 = m.f[0].pos.x;
        for _ in 0..COUNTDOWN {
            m.step_with([&z; N_FIGHTERS], Some((0, run)));
        }
        assert_eq!(m.f[0].pos.x, x0, "frozen during the 3-2-1");
        m.step_with([&z; N_FIGHTERS], Some((0, run)));
        assert!(m.f[0].pos.x > x0, "moving once it's over");
        m.damage(1, 0, 50.0, vec2(100.0, -100.0), 10, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP, "protected for the first seconds");
        assert_eq!(m.f[0].hitstun, 0, "not even knocked about");
        while m.protected() {
            m.step([&z; N_FIGHTERS]);
        }
        m.damage(1, 0, 50.0, Vec2::ZERO, 10, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP - 50.0);
        assert_eq!(m.play_ticks(), SPAWN_GRACE, "the match clock started after the countdown");
    }

    #[test]
    fn running_builds_from_slow_to_fast_and_turns_quickly() {
        let arena = ArenaId::default().def();
        let mut f = Fighter::new(100.0, 1.0);
        let run = Action { mv: 1.0, run: true, ..Action::default() };
        let speed_after = |f: &mut Fighter, a: Action, ticks: u32| {
            for _ in 0..ticks {
                predict(arena, &[], f, 0, a, &mut Vec::new());
            }
            f.vel.x
        };
        let early = speed_after(&mut f, run, secs(1) / 10);
        assert!(early >= WALK && early < RUN * 0.5, "a quick start, then building: {early} after 0.1s");
        let later = speed_after(&mut f, run, secs(1) * 35 / 100);
        assert_eq!(later, RUN, "full speed within ~0.45s");
        // Turning around: from full speed one way to moving the other in well under 0.2s.
        let back = speed_after(&mut f, Action { mv: -1.0, ..run }, secs(1) * 15 / 100);
        assert!(back < 0.0, "turned around: {back}");
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
    fn a_rocket_homes_in_and_hits_hard() {
        let (mut m, z) = quiet_match();
        m.items.push(Item { held_by: Some(0), ..Item::new(ItemKind::RocketLauncher, m.f[0].hand()) });
        m.f[0].holding = Some(ItemKind::RocketLauncher);
        // Fired well above the fighter 140px away: it curves down onto them.
        let a = Action { throw: true, target: Some(vec2(1.0, -0.6)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, a)));
        assert!(m.items.is_empty(), "the launcher is used up");
        let mut boom = false;
        for _ in 0..secs(3) {
            m.step([&z; N_FIGHTERS]);
            boom |= m.events.iter().any(|e| matches!(e, Event::Explosion { .. }));
        }
        assert!(boom && m.effects.is_empty());
        assert!(m.f[1].hp < MAX_HP - 40.0, "a big hit: {} left", m.f[1].hp);
        assert_eq!(m.f[0].hp, MAX_HP, "the shooter is well clear");
    }

    #[test]
    fn at_most_two_rocket_launchers_a_match() {
        let (mut m, _) = quiet_match();
        let mut launchers = 0;
        for _ in 0..2000 {
            m.next_item = m.tick;
            m.spawn_items();
            launchers += m.items.iter().filter(|it| it.kind == ItemKind::RocketLauncher).count();
            m.items.clear();
        }
        assert!(launchers > 0 && launchers <= 2, "{launchers}");
    }

    #[test]
    fn wall_jumps_kick_off_and_up_on_a_fresh_press() {
        let (mut m, z) = quiet_match();
        // In the air, pressed against the left wall.
        m.f[0].pos = vec2(WALL + HALF_W, 200.0);
        m.f[0].grounded = false;
        let left = Action { mv: -1.0, ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, left)));
        assert!(m.f[0].vel.y <= WALL_SLIDE, "sliding down the wall");
        m.step_with([&z; N_FIGHTERS], Some((0, Action { jump: true, ..left })));
        assert!(m.f[0].vel.y < -150.0 && m.f[0].vel.x > 100.0, "kicked up and away: {}", m.f[0].vel);
        // Held, not pressed again: no second kick.
        m.f[0].pos.x = WALL + HALF_W;
        m.f[0].vel = Vec2::ZERO;
        for _ in 0..WALL_JUMP_CD + 2 {
            m.step_with([&z; N_FIGHTERS], Some((0, Action { jump: true, ..left })));
        }
        assert!(m.f[0].vel.y > 0.0, "holding jump doesn't climb");
    }

    #[test]
    fn a_dash_bursts_ahead_then_cools_down() {
        let (mut m, z) = quiet_match();
        m.f[0].facing = 1.0;
        let x0 = m.f[0].pos.x;
        m.step_with([&z; N_FIGHTERS], Some((0, Action { dash: true, ..Action::default() })));
        for _ in 0..DASH_TICKS {
            m.step_with([&z; N_FIGHTERS], Some((0, Action::default())));
        }
        let gone = m.f[0].pos.x - x0;
        assert!(gone > 45.0, "dashed {gone}px");
        m.step_with([&z; N_FIGHTERS], Some((0, Action { dash: true, ..Action::default() })));
        assert_eq!(m.f[0].dash_t, 0, "not again so soon");
        assert!(!m.events.iter().any(|e| matches!(e, Event::Dash { .. })));
    }

    #[test]
    fn only_one_rock_a_match() {
        let (mut m, _) = quiet_match();
        let m0 = Match::new(3, &MatchConfig::default());
        assert_eq!(m0.items.iter().filter(|it| it.kind == ItemKind::Rock).count(), 1, "one at the start");
        for _ in 0..3000 {
            m.next_item = m.tick;
            m.spawn_items();
            assert!(m.items.iter().all(|it| it.kind != ItemKind::Rock), "no rock ever spawns");
            m.items.clear();
        }
    }

    #[test]
    fn a_molotov_sets_the_ground_burning() {
        let (mut m, z) = quiet_match();
        m.items.push(Item { held_by: Some(0), ..Item::new(ItemKind::Molotov, m.f[0].hand()) });
        m.f[0].holding = Some(ItemKind::Molotov);
        // Lobbed at the floor just ahead.
        let a = Action { throw: true, target: Some(vec2(1.0, -0.2)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, a)));
        let mut lit = false;
        for _ in 0..60 {
            m.step([&z; N_FIGHTERS]);
            lit |= m.events.iter().any(|e| matches!(e, Event::Ignite { .. }));
        }
        assert!(lit, "it shattered");
        let fire = m.effects.iter().find_map(|e| if let Effect::Fire(f) = e { Some(f.clone()) } else { None }).expect("fire");
        assert_eq!(fire.at.y, FLOOR_Y, "on the floor");
        // Someone walks into it and gets burnt.
        m.f[1].pos.x = fire.at.x;
        let hp = m.f[1].hp;
        for _ in 0..30 {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[1].hp < hp, "burnt");
    }

    #[test]
    fn an_air_strike_flies_over_and_bombs_the_arena() {
        let (mut m, z) = quiet_match();
        m.items.push(Item { held_by: Some(0), ..Item::new(ItemKind::AirStrike, m.f[0].hand()) });
        m.f[0].holding = Some(ItemKind::AirStrike);
        m.step_with([&z; N_FIGHTERS], Some((0, Action { throw: true, ..Action::default() })));
        assert!(m.events.iter().any(|e| matches!(e, Event::RadioCall { .. })), "radioed in");
        let (mut drops, mut booms, mut jet) = (0, 0, false);
        for _ in 0..secs(6) {
            m.step([&z; N_FIGHTERS]);
            jet |= m.events.iter().any(|e| matches!(e, Event::JetPass { .. }));
            drops += m.events.iter().filter(|e| matches!(e, Event::BombDrop { .. })).count();
            booms += m.events.iter().filter(|e| matches!(e, Event::Explosion { .. })).count();
        }
        assert!(jet && (5..=7).contains(&drops), "the jet came and dropped {drops}");
        assert_eq!(booms, drops, "every bomb went off");
        assert!(m.effects.is_empty(), "and it's all over");
    }

    #[test]
    fn the_sword_reaches_further_and_hits_harder_than_fists() {
        let (mut m, z) = quiet_match();
        m.f[0].pos.x = m.f[1].pos.x - 20.0;
        m.f[0].facing = 1.0;
        m.items.push(Item { held_by: Some(0), ..Item::new(ItemKind::Sword, m.f[0].hand()) });
        m.f[0].holding = Some(ItemKind::Sword);
        m.step_with([&z; N_FIGHTERS], Some((0, Action { attack: true, ..Action::default() })));
        for _ in 0..ATK_TOTAL {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(MAX_HP - m.f[1].hp >= 22.0, "a slash from 20px away: {}", m.f[1].hp);
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
