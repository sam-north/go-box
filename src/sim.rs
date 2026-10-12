//! Headless, deterministic arena simulation: four humans, free-for-all, starting empty-handed
//! with spears, rocks, grenades and health lying around. No hard-coded behaviour: fighters only
//! expose physical abilities; brains decide what to do with them.

use crate::brain::{Brain, IN_EXTRA, IN_HEAL, IN_ITEMS, IN_MEM, IN_NEAREST, IN_OPP, IN_SELF, IN_THREAT, MEM, N_IN, N_OUT, OUT_MEM};
use macroquad::math::{vec2, Vec2};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::ops::{Range, RangeInclusive};

pub const W: f32 = 320.0;
pub const H: f32 = 180.0;
pub const DT: f32 = 1.0 / 60.0;
pub const MAX_TICKS: u32 = 60 * 30;
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

// Melee: fists when empty-handed (or holding a rock/grenade), a stab when holding a spear.
pub const ATK_WINDUP: u32 = 5;
pub const ATK_ACTIVE: u32 = 5;
pub const ATK_TOTAL: u32 = 16;
const ATK_COOLDOWN: u32 = 18;
const HITSTUN: u32 = 12;
const FIST_DAMAGE: f32 = 8.0;
const FIST_REACH: f32 = 9.0;
const STAB_DAMAGE: f32 = 16.0;
const STAB_REACH: f32 = 14.0;

// Throwing.
const AIM_RANGE: f32 = 0.9;
const GRAB_COOLDOWN: u32 = 10;
const THROW_COOLDOWN: u32 = 20;
const SPEAR_THROW_DAMAGE: f32 = 28.0;
const ROCK_DAMAGE: f32 = 10.0;
/// Headshots multiply damage: rocks "hitting the right spot" are devastating.
const HEADSHOT_MULT: f32 = 2.0;
const ROCK_HEADSHOT_MULT: f32 = 3.0;
pub const GRENADE_FUSE: u32 = 90;
pub const GRENADE_RADIUS: f32 = 28.0;
const GRENADE_DAMAGE: f32 = 45.0;
pub const ITEM_R: f32 = 1.5;
const LETHAL_SPEED: f32 = 110.0;
const OWN_GRACE: u32 = 15;

// Items lying around.
const START_SPEARS: usize = 2;
const START_ROCKS: usize = 4;
const ITEM_SPAWN_EVERY: Range<u32> = 60 * 5..60 * 8;
const MAX_LOOSE_ITEMS: usize = 10;
const MAX_GRENADES: usize = 1;

// Health pickups: restore half your health; rare and on a reachable surface.
pub const HEAL_AMOUNT: f32 = 50.0;
pub const HEAL_R: f32 = 2.5;
/// Final rules: each round gets 1 or 2 pickups (the trainer starts with more and fades to
/// this), at random times inside HEAL_WINDOW (ticks).
pub const HEALS_PER_ROUND: RangeInclusive<usize> = 1..=2;
const HEAL_WINDOW: Range<u32> = 60 * 6..60 * 24;

// Senses.
pub const N_RAYS: usize = 12;
pub const N_SECTORS: usize = 12;
pub const RAY_MAX: f32 = 200.0;
const VIS_MAX: f32 = 370.0;
const CLOSE_DIST: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemKind {
    Spear,
    Rock,
    Grenade,
}

impl ItemKind {
    pub fn index(self) -> usize {
        self as usize
    }
    fn throw_speed(self) -> f32 {
        match self {
            ItemKind::Spear => 280.0,
            ItemKind::Rock => 230.0,
            ItemKind::Grenade => 180.0,
        }
    }
}

/// Where damage came from, for stats.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Fist,
    Stab,
    SpearThrow,
    Rock,
    Grenade,
}

impl Source {
    pub const ALL: [Source; 5] = [Source::Fist, Source::Stab, Source::SpearThrow, Source::Rock, Source::Grenade];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Source::Fist => "fists",
            Source::Stab => "spear stab",
            Source::SpearThrow => "spear throw",
            Source::Rock => "rock",
            Source::Grenade => "grenade",
        }
    }
}

#[derive(Clone, Copy, Debug)]
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

/// Fully solid terrain: ceiling, floor, walls.
pub const SOLIDS: [Aabb; 4] = [
    Aabb::new(0.0, 0.0, W, 8.0),
    Aabb::new(0.0, 164.0, W, 16.0),
    Aabb::new(0.0, 0.0, 8.0, H),
    Aabb::new(312.0, 0.0, 8.0, H),
];

/// One-way platforms: you can jump up through them and land on top.
pub const PLATFORMS: [Aabb; 3] = [
    Aabb::new(52.0, 124.0, 64.0, 5.0),
    Aabb::new(204.0, 124.0, 64.0, 5.0),
    Aabb::new(128.0, 88.0, 64.0, 5.0),
];

#[derive(Clone, Copy, Default, Debug)]
pub struct Action {
    pub mv: f32,
    pub run: bool,
    pub jump: bool,
    pub attack: bool,
    pub throw: bool,
    pub aim: f32,
    pub grab: bool,
}

#[derive(Clone, Debug)]
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
    hit_mask: u8,
    pub hitstun: u32,
    /// What's in hand (mirrors the item's `held_by`).
    pub holding: Option<ItemKind>,
    pub hand_cd: u32,
    pub memory: [f32; MEM],
    pub action: Action,
    /// Not in this match at all (a 1v1 leaves two seats empty).
    pub absent: bool,
    // Stats.
    pub dmg_dealt: f32,
    pub dmg_taken: f32,
    pub self_dmg: f32,
    pub dmg_by_source: [f32; 5],
    pub hits: u32,
    pub headshots: u32,
    pub kills: u32,
    pub close_ticks: u32,
    pub healed: f32,
    pub heals_taken: u32,
}

impl Fighter {
    fn new(x: f32, facing: f32) -> Self {
        Self {
            pos: vec2(x, 164.0),
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
            dmg_dealt: 0.0,
            dmg_taken: 0.0,
            self_dmg: 0.0,
            dmg_by_source: [0.0; 5],
            hits: 0,
            headshots: 0,
            kills: 0,
            close_ticks: 0,
            healed: 0.0,
            heals_taken: 0,
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

    pub fn melee_reach(&self) -> f32 {
        if self.holding == Some(ItemKind::Spear) { STAB_REACH } else { FIST_REACH }
    }

    pub fn attack_box(&self) -> Aabb {
        let reach = self.melee_reach();
        let x0 = if self.facing > 0.0 { self.pos.x + HALF_W - 2.0 } else { self.pos.x - HALF_W + 2.0 - reach };
        Aabb::new(x0, self.pos.y - BODY_H + 2.0, reach, 10.0)
    }

    pub fn aim_dir(&self, aim: f32) -> Vec2 {
        let a = aim * AIM_RANGE;
        vec2(self.facing * a.cos(), -a.sin())
    }
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Hit { pos: Vec2, dir: f32, headshot: bool },
    Death { pos: Vec2 },
    Jump { pos: Vec2 },
    Land { pos: Vec2 },
    Throw { who: usize },
    Grab { pos: Vec2 },
    Explosion { pos: Vec2 },
    Heal { who: usize, pos: Vec2 },
    Spawn { pos: Vec2 },
}

/// Distance along a normalised ray to the nearest terrain, capped at `max`.
pub fn terrain_ray(o: Vec2, d: Vec2, max: f32) -> f32 {
    let inv = |v: f32| if v.abs() < 1e-8 { 1e8_f32.copysign(v) } else { 1.0 / v };
    let (ix, iy) = (inv(d.x), inv(d.y));
    let mut best = max;
    for r in SOLIDS.iter().chain(PLATFORMS.iter()) {
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
            vec2(a.cos(), a.sin())
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
fn glimpse(eye: Vec2, target: Vec2) -> Option<(f32, usize)> {
    let to = target - eye;
    let dist = to.length().max(0.001);
    if dist >= VIS_MAX || terrain_ray(eye, to / dist, dist) < dist - 0.5 {
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

fn one_hot(kind: Option<ItemKind>) -> [f32; 3] {
    let mut v = [0.0; 3];
    if let Some(k) = kind {
        v[k.index()] = 1.0;
    }
    v
}

/// A random spot on top of the floor or a platform — everywhere a fighter can stand.
fn random_surface_spot(rng: &mut impl Rng) -> Vec2 {
    let floor = (14.0, 306.0, 164.0);
    let surfaces = std::iter::once(floor).chain(PLATFORMS.iter().map(|p| (p.x + 3.0, p.x + p.w - 3.0, p.y)));
    let (x0, x1, y) = surfaces.clone().nth(rng.gen_range(0..1 + PLATFORMS.len())).unwrap();
    vec2(rng.gen_range(x0..x1), y)
}

pub struct Match {
    pub f: [Fighter; N_FIGHTERS],
    pub items: Vec<Item>,
    /// Health pickups currently lying in the arena.
    pub heals: Vec<Vec2>,
    /// Remaining heal spawn times this round, latest first.
    heal_schedule: Vec<u32>,
    pub heals_spawned: u32,
    next_item: u32,
    rng: SmallRng,
    pub tick: u32,
    pub over: bool,
    pub winner: Option<usize>,
    pub events: Vec<Event>,
}

impl Match {
    /// Four humans spread across the floor, empty-handed. `heals` is how many health pickups
    /// this round gets (picked at random from the range).
    pub fn new(rng: &mut impl Rng, heals: RangeInclusive<usize>) -> Self {
        let mut zones = [20.0, 95.0, 175.0, 250.0];
        for i in (1..zones.len()).rev() {
            zones.swap(i, rng.gen_range(0..=i));
        }
        let f = std::array::from_fn(|i| {
            let x = zones[i] + rng.gen_range(0.0..50.0);
            Fighter::new(x, if x < W / 2.0 { 1.0 } else { -1.0 })
        });
        let mut items = Vec::new();
        for (kind, n) in [(ItemKind::Spear, START_SPEARS), (ItemKind::Rock, START_ROCKS)] {
            for _ in 0..n {
                items.push(Item::new(kind, random_surface_spot(rng) - vec2(0.0, ITEM_R)));
            }
        }
        let mut heal_schedule: Vec<u32> = (0..rng.gen_range(heals)).map(|_| rng.gen_range(HEAL_WINDOW)).collect();
        heal_schedule.sort_unstable_by(|a, b| b.cmp(a));
        let next_item = rng.gen_range(ITEM_SPAWN_EVERY);
        Self {
            f,
            items,
            heals: Vec::new(),
            heal_schedule,
            heals_spawned: 0,
            next_item,
            rng: SmallRng::seed_from_u64(rng.r#gen()),
            tick: 0,
            over: false,
            winner: None,
            events: Vec::new(),
        }
    }

    /// A 1v1 in seats 0 and 2; the other two seats are empty.
    pub fn duel(rng: &mut impl Rng, heals: RangeInclusive<usize>) -> Self {
        let mut m = Self::new(rng, heals);
        for i in [1, 3] {
            m.f[i].hp = 0.0;
            m.f[i].absent = true;
        }
        m
    }

    pub fn held_item(&self, i: usize) -> Option<usize> {
        self.items.iter().position(|it| it.held_by == Some(i))
    }

    /// Everything fighter `i` perceives. Other fighters, items and pickups are only seen
    /// when in line of sight.
    pub fn senses(&self, i: usize) -> [f32; N_IN] {
        let me = &self.f[i];
        let mut s = [0.0; N_IN];
        let eye = me.eye();
        for r in 0..N_RAYS {
            s[r] = 1.0 - terrain_ray(eye, ray_dir(r), RAY_MAX) / RAY_MAX;
        }
        let mut nearest: Option<(f32, usize)> = None;
        for j in (0..N_FIGHTERS).filter(|&j| j != i && self.f[j].alive()) {
            let seen = glimpse(eye, self.f[j].center());
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
            let seen = glimpse(eye, it.pos);
            if it.lethal || it.fuse.is_some() {
                mark(&mut s, IN_THREAT, seen);
            } else {
                mark(&mut s, IN_ITEMS + it.kind.index() * N_SECTORS, seen);
            }
        }
        for &h in &self.heals {
            mark(&mut s, IN_HEAL, glimpse(eye, h));
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
        s[k + 12] = self.held_item(i).and_then(|h| self.items[h].fuse).map_or(0.0, |t| 1.0 - t as f32 / GRENADE_FUSE as f32);
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
            .filter_map(|it| it.fuse.map(|t| (it, t)))
            .map(|(it, t)| (1.0 - it.pos.distance(c) / GRENADE_RADIUS).max(0.0) * (1.0 - t as f32 / GRENADE_FUSE as f32))
            .fold(0.0, f32::max);
        s[e + 11] = 1.0 - self.tick as f32 / MAX_TICKS as f32;
        let best_other = (0..N_FIGHTERS).filter(|&j| j != i && self.f[j].alive()).map(|j| self.f[j].hp).fold(0.0, f32::max);
        s[e + 12] = (me.hp - best_other) / MAX_HP;
        s
    }

    pub fn step(&mut self, brains: [&Brain; N_FIGHTERS]) {
        self.step_with(brains, None);
    }

    /// Like `step`, but seat `human.0` is driven by a player's input instead of its brain.
    pub fn step_with(&mut self, brains: [&Brain; N_FIGHTERS], human: Option<(usize, Action)>) {
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
            if let Some((_, a)) = human.filter(|&(h, _)| h == i) {
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
            control(f, actions[i], &mut self.events);
            collide(f);
            if f.grounded && !was_grounded {
                self.events.push(Event::Land { pos: f.pos });
            }
        }

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
            let (base, source) = if a.holding == Some(ItemKind::Spear) { (STAB_DAMAGE, Source::Stab) } else { (FIST_DAMAGE, Source::Fist) };
            // A strike lands on the head when it comes from above, e.g. from a jump or a ledge.
            let headshot = self.f[j].is_head(a.pos.y - 8.0);
            let dmg = if headshot { base * HEADSHOT_MULT } else { base };
            self.damage(i, j, dmg, vec2(dir * 150.0, -140.0), HITSTUN, dir, source, headshot);
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
        self.step_heals();
        self.spawn_items();

        // 6. Bookkeeping.
        for i in 0..N_FIGHTERS {
            let close = (0..N_FIGHTERS)
                .any(|j| j != i && self.f[j].alive() && self.f[i].center().distance(self.f[j].center()) < CLOSE_DIST);
            if close && self.f[i].alive() {
                self.f[i].close_ticks += 1;
            }
        }
        self.tick += 1;
        let alive: Vec<usize> = (0..N_FIGHTERS).filter(|&i| self.f[i].alive()).collect();
        if alive.len() <= 1 || self.tick >= MAX_TICKS {
            self.over = true;
            // Last one standing, or the most health at the timeout. An exact tie is a draw.
            self.winner = match alive.len() {
                0 => None,
                1 => Some(alive[0]),
                _ => {
                    let best = alive.iter().copied().max_by(|&a, &b| self.f[a].hp.total_cmp(&self.f[b].hp)).unwrap();
                    let tied = alive.iter().filter(|&&j| self.f[j].hp == self.f[best].hp).count() > 1;
                    (!tied).then_some(best)
                }
            };
        }
    }

    /// True if the match ended on the clock rather than with one survivor.
    pub fn timed_out(&self) -> bool {
        self.over && self.f.iter().filter(|f| f.alive()).count() > 1
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
            let dir = f.aim_dir(a.aim);
            f.holding = None;
            f.hand_cd = THROW_COOLDOWN;
            let it = &mut self.items[k];
            it.held_by = None;
            it.owner = Some(i);
            it.pos = f.hand();
            it.vel = dir * it.kind.throw_speed() + f.vel * 0.5;
            it.age = 0;
            if it.kind == ItemKind::Grenade {
                it.fuse.get_or_insert(GRENADE_FUSE);
            } else {
                it.lethal = true;
            }
            self.events.push(Event::Throw { who: i });
        }
    }

    fn step_items(&mut self) {
        let mut explode = Vec::new();
        for k in 0..self.items.len() {
            if let Some(h) = self.items[k].held_by {
                self.items[k].pos = self.f[h].hand();
            } else {
                step_item(&mut self.items[k]);
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
                let (base, mult, source) = match it.kind {
                    ItemKind::Spear => (SPEAR_THROW_DAMAGE, HEADSHOT_MULT, Source::SpearThrow),
                    _ => (ROCK_DAMAGE, ROCK_HEADSHOT_MULT, Source::Rock),
                };
                let dmg = if headshot { base * mult } else { base };
                let it = &mut self.items[k];
                it.vel *= -0.2;
                it.lethal = false;
                self.damage(src, j, dmg, vec2(dir * 110.0, -100.0), 10, dir, source, headshot);
            }
        }
        // Explode back-to-front so indices stay valid.
        explode.sort_unstable_by(|a, b| b.cmp(a));
        for k in explode {
            let it = self.items.swap_remove(k);
            if let Some(h) = it.held_by {
                self.f[h].holding = None;
            }
            self.explode(it.pos, it.owner);
        }
    }

    fn explode(&mut self, pos: Vec2, owner: Option<usize>) {
        self.events.push(Event::Explosion { pos });
        for j in 0..N_FIGHTERS {
            let d = self.f[j].center().distance(pos);
            if !self.f[j].alive() || d >= GRENADE_RADIUS {
                continue;
            }
            let away = (self.f[j].center() - pos).normalize_or(vec2(0.0, -1.0));
            let dmg = GRENADE_DAMAGE * (1.0 - d / GRENADE_RADIUS).max(0.15);
            self.damage(owner.unwrap_or(j), j, dmg, away * 220.0 + vec2(0.0, -80.0), 14, away.x.signum(), Source::Grenade, false);
        }
        for it in self.items.iter_mut().filter(|it| it.held_by.is_none()) {
            let d = it.pos.distance(pos);
            if d < GRENADE_RADIUS * 1.5 {
                it.vel += (it.pos - pos).normalize_or(vec2(0.0, -1.0)) * 150.0 * (1.0 - d / (GRENADE_RADIUS * 1.5));
            }
        }
    }

    /// Spawns this round's health pickups at their scheduled times. Any living fighter that
    /// touches one while hurt is healed by HEAL_AMOUNT; at full health it's left alone.
    fn step_heals(&mut self) {
        while self.heal_schedule.last().is_some_and(|&t| self.tick >= t) {
            self.heal_schedule.pop();
            let pos = random_surface_spot(&mut self.rng) - vec2(0.0, HEAL_R + 1.0);
            self.heals.push(pos);
            self.heals_spawned += 1;
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
                    let gained = HEAL_AMOUNT.min(MAX_HP - f.hp);
                    f.hp += gained;
                    f.healed += gained;
                    f.heals_taken += 1;
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
        let grenades = self.items.iter().filter(|it| it.kind == ItemKind::Grenade).count();
        let roll: f32 = self.rng.r#gen();
        let kind = if roll < 0.15 && grenades < MAX_GRENADES {
            ItemKind::Grenade
        } else if roll < 0.5 {
            ItemKind::Spear
        } else {
            ItemKind::Rock
        };
        let pos = random_surface_spot(&mut self.rng) - vec2(0.0, ITEM_R);
        self.items.push(Item::new(kind, pos));
        self.events.push(Event::Spawn { pos });
    }

    #[allow(clippy::too_many_arguments)]
    fn damage(&mut self, src: usize, victim: usize, dmg: f32, knockback: Vec2, stun: u32, dir: f32, source: Source, headshot: bool) {
        if src == victim {
            self.f[victim].self_dmg += dmg;
        } else {
            let a = &mut self.f[src];
            a.dmg_dealt += dmg;
            a.dmg_by_source[source.index()] += dmg;
            a.hits += 1;
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
        self.events.push(Event::Hit { pos, dir, headshot });
        if died {
            // Drop whatever was in hand.
            if let Some(k) = self.held_item(victim) {
                self.items[k].held_by = None;
                self.items[k].vel = Vec2::ZERO;
            }
            self.f[victim].holding = None;
            self.events.push(Event::Death { pos });
            if src != victim {
                self.f[src].kills += 1;
            }
        }
    }

    /// Outcome-based score: damage dealt, damage taken (offset by health restored), kills,
    /// dying, and winning (last one standing, or most health at the timeout). Nothing rewards
    /// any particular weapon or style. `shaping` (0..1) is an optional "get close to someone"
    /// hint the trainer fades out early on.
    pub fn fitness(&self, i: usize, shaping: f32) -> f32 {
        let me = &self.f[i];
        let mut s = me.dmg_dealt - 0.5 * (me.dmg_taken - me.healed) + 60.0 * me.kills as f32;
        if !me.alive() {
            s -= 40.0;
        }
        if self.winner == Some(i) {
            s += 80.0 + 40.0 * (1.0 - self.tick as f32 / MAX_TICKS as f32);
        }
        s + shaping * me.close_ticks as f32 * 0.03
    }
}

fn step_item(it: &mut Item) {
    it.age += 1;
    // Thrown spears fly flat; everything else arcs.
    let g = if it.kind == ItemKind::Spear && it.lethal { 0.35 } else { 0.8 };
    it.vel.y = (it.vel.y + GRAVITY * g * DT).min(MAX_FALL);
    let rb = |p: Vec2| Aabb::around(p, ITEM_R);
    let bounce = match it.kind {
        ItemKind::Spear => 0.15,
        ItemKind::Rock => 0.35,
        ItemKind::Grenade => 0.45,
    };

    it.pos.x += it.vel.x * DT;
    if let Some(s) = SOLIDS.iter().find(|s| rb(it.pos).overlaps(s)) {
        it.pos.x = if it.vel.x > 0.0 { s.x - ITEM_R } else { s.x + s.w + ITEM_R };
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
        for p in PLATFORMS.iter() {
            let over_x = it.pos.x + ITEM_R > p.x && it.pos.x - ITEM_R < p.x + p.w;
            if over_x && prev_bottom <= p.y + 0.01 && it.pos.y + ITEM_R >= p.y {
                landed = Some(p.y);
            }
        }
    }
    if let Some(top) = landed {
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
    it.pos.x = it.pos.x.clamp(8.0 + ITEM_R, 312.0 - ITEM_R);
    it.pos.y = it.pos.y.clamp(8.0 + ITEM_R, 164.0 - ITEM_R);
}

fn approach(v: f32, target: f32, step: f32) -> f32 {
    if v < target { (v + step).min(target) } else { (v - step).max(target) }
}

fn control(f: &mut Fighter, a: Action, events: &mut Vec<Event>) {
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
            events.push(Event::Jump { pos: f.pos });
        }
        if a.attack && f.atk_timer == 0 && f.cooldown == 0 {
            f.atk_timer = 1;
            f.hit_mask = 0;
        }
    }
    // Releasing jump early cuts the arc short.
    let g = if f.vel.y < 0.0 && !a.jump { GRAVITY * 2.0 } else { GRAVITY };
    f.vel.y = (f.vel.y + g * DT).min(MAX_FALL);
}

fn collide(f: &mut Fighter) {
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
        for p in PLATFORMS.iter() {
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
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("brains");
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        names.sort();
        let brains: Vec<Brain> = names.iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect();
        assert_eq!(brains.len(), 5);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |x: u64| h = (h ^ x).wrapping_mul(0x100_0000_01b3);
        for seed in 0..8u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut m = Match::new(&mut rng, HEALS_PER_ROUND);
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
    const CLASSIC_HASH: u64 = 0x9fec_dcd7_3934_23f2;

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
