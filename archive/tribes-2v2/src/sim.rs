//! Headless, deterministic arena simulation. No rendering, no hard-coded behaviour:
//! fighters only expose physical abilities; brains decide what to do with them.

use crate::brain::{Brain, IN_ALLY, IN_EXTRA, IN_HEAL, IN_MEM, IN_ROCK, IN_THREAT, MEM, N_IN, N_OUT, OUT_MEM};
use macroquad::math::{vec2, Vec2};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

pub const W: f32 = 320.0;
pub const H: f32 = 180.0;
pub const DT: f32 = 1.0 / 60.0;
pub const MAX_TICKS: u32 = 60 * 30;
pub const N_FIGHTERS: usize = 4;

// Body (identical for every kind).
pub const HALF_W: f32 = 4.0;
pub const BODY_H: f32 = 14.0;
const GRAVITY: f32 = 640.0;
const MAX_FALL: f32 = 320.0;
const JUMP_V: f32 = 250.0;
const WALK: f32 = 45.0;
const RUN: f32 = 85.0;
const GROUND_ACC: f32 = 900.0;
const AIR_ACC: f32 = 420.0;

// Melee (frame data shared; damage and reach depend on kind).
pub const MAX_HP: f32 = 100.0;
pub const ATK_WINDUP: u32 = 5;
pub const ATK_ACTIVE: u32 = 5;
pub const ATK_TOTAL: u32 = 16;
const ATK_COOLDOWN: u32 = 18;
const HITSTUN: u32 = 12;

// Ranged.
pub const VARIANT_CHANCE: f64 = 0.35;
/// Rocks and acid hit much harder than any melee, so ranged play is worth prioritising.
const RANGED_DAMAGE: f32 = 24.0;
const SALIVA_TICKS: f32 = 120.0;
const THROW_COOLDOWN: u32 = 20;
const THROW_SPEED: f32 = 230.0;
const SPIT_SPEED: f32 = 190.0;
const AIM_RANGE: f32 = 0.9;
pub const ROCK_R: f32 = 1.5;
const ROCK_LETHAL_SPEED: f32 = 110.0;
const N_ROCKS: usize = 5;
const OWN_ROCK_GRACE: u32 = 15;

// Health pickups: restore half your health; rare (1-2 per round) and on a reachable surface.
pub const HEAL_AMOUNT: f32 = 50.0;
pub const HEAL_R: f32 = 2.5;
/// Final rules: each round gets 1 or 2 pickups (the trainer starts with more and fades to this),
/// at random times inside HEAL_WINDOW (ticks).
pub const HEALS_PER_ROUND: std::ops::RangeInclusive<usize> = 1..=2;
const HEAL_WINDOW: std::ops::Range<u32> = 60 * 6..60 * 24;

// Senses.
pub const N_RAYS: usize = 12;
pub const N_SECTORS: usize = 12;
pub const RAY_MAX: f32 = 200.0;
const VIS_MAX: f32 = 370.0;
const CLOSE_DIST: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Spear,
    Thrower,
    Biter,
    Spitter,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tribe {
    Humans,
    Creatures,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Spear, Kind::Thrower, Kind::Biter, Kind::Spitter];

    pub fn index(self) -> usize {
        self as usize
    }
    pub fn tribe(self) -> Tribe {
        match self {
            Kind::Spear | Kind::Thrower => Tribe::Humans,
            Kind::Biter | Kind::Spitter => Tribe::Creatures,
        }
    }
    pub fn ranged(self) -> bool {
        matches!(self, Kind::Thrower | Kind::Spitter)
    }
    pub fn name(self) -> &'static str {
        match self {
            Kind::Spear => "spear",
            Kind::Thrower => "thrower",
            Kind::Biter => "biter",
            Kind::Spitter => "spitter",
        }
    }
    /// Spear and bite hit a little harder than the variants' plain punch/claw.
    fn melee_damage(self) -> f32 {
        if self.ranged() { 10.0 } else { 14.0 }
    }
    pub fn melee_reach(self) -> f32 {
        if self.ranged() { 9.0 } else { 14.0 }
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

fn in_terrain(p: Vec2) -> bool {
    SOLIDS.iter().chain(PLATFORMS.iter()).any(|r| r.contains(p))
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Action {
    pub mv: f32,
    pub run: bool,
    pub jump: bool,
    pub melee: bool,
    pub ranged: bool,
    pub aim: f32,
}

#[derive(Clone, Debug)]
pub struct Fighter {
    pub kind: Kind,
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
    pub holding: Option<usize>,
    pub throw_cd: u32,
    /// Spitter's acid reserve, 0..1; can spit at 1.
    pub saliva: f32,
    pub memory: [f32; MEM],
    pub action: Action,
    pub dmg_enemy: f32,
    pub dmg_ally: f32,
    pub dmg_taken: f32,
    pub kills: u32,
    pub ally_kills: u32,
    pub close_ticks: u32,
    pub healed: f32,
    pub heals_taken: u32,
}

impl Fighter {
    fn new(kind: Kind, x: f32, facing: f32) -> Self {
        Self {
            kind,
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
            throw_cd: 0,
            saliva: 1.0,
            memory: [0.0; MEM],
            action: Action::default(),
            dmg_enemy: 0.0,
            dmg_ally: 0.0,
            dmg_taken: 0.0,
            kills: 0,
            ally_kills: 0,
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

    pub fn hand(&self) -> Vec2 {
        vec2(self.pos.x - self.facing * 1.5, self.pos.y - 13.0)
    }

    pub fn attack_active(&self) -> bool {
        self.atk_timer > ATK_WINDUP && self.atk_timer <= ATK_WINDUP + ATK_ACTIVE
    }

    pub fn attack_box(&self) -> Aabb {
        let reach = self.kind.melee_reach();
        let x0 = if self.facing > 0.0 { self.pos.x + HALF_W - 2.0 } else { self.pos.x - HALF_W + 2.0 - reach };
        Aabb::new(x0, self.pos.y - BODY_H + 2.0, reach, 10.0)
    }

    fn ranged_ready(&self) -> f32 {
        match self.kind {
            Kind::Thrower => (self.holding.is_some() && self.throw_cd == 0) as u8 as f32,
            Kind::Spitter => self.saliva,
            _ => 0.0,
        }
    }

    fn aim_dir(&self, aim: f32) -> Vec2 {
        let a = aim * AIM_RANGE;
        vec2(self.facing * a.cos(), -a.sin())
    }
}

#[derive(Clone, Debug)]
pub struct Rock {
    pub pos: Vec2,
    pub vel: Vec2,
    pub held_by: Option<usize>,
    pub owner: Option<usize>,
    pub lethal: bool,
    age: u32,
}

#[derive(Clone, Debug)]
pub struct Acid {
    pub pos: Vec2,
    pub vel: Vec2,
    pub owner: usize,
    age: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Hit { victim: usize, pos: Vec2, dir: f32 },
    Death { who: usize, pos: Vec2 },
    Jump { pos: Vec2 },
    Land { pos: Vec2 },
    Throw { who: usize },
    Spit { who: usize },
    Splash { pos: Vec2 },
    Heal { who: usize, pos: Vec2 },
    HealSpawn { pos: Vec2 },
}

/// Distance along a normalised ray to the nearest terrain, capped at `max`.
pub fn terrain_ray(o: Vec2, d: Vec2, max: f32) -> f32 {
    let mut best = max;
    for r in SOLIDS.iter().chain(PLATFORMS.iter()) {
        if let Some(t) = ray_aabb(o, d, r) {
            best = best.min(t);
        }
    }
    best
}

fn ray_aabb(o: Vec2, d: Vec2, r: &Aabb) -> Option<f32> {
    let inv = |v: f32| if v.abs() < 1e-8 { 1e8_f32.copysign(v) } else { 1.0 / v };
    let (ix, iy) = (inv(d.x), inv(d.y));
    let (tx1, tx2) = ((r.x - o.x) * ix, (r.x + r.w - o.x) * ix);
    let (ty1, ty2) = ((r.y - o.y) * iy, (r.y + r.h - o.y) * iy);
    let tmin = tx1.min(tx2).max(ty1.min(ty2));
    let tmax = tx1.max(tx2).min(ty1.max(ty2));
    (tmax >= tmin.max(0.0)).then_some(tmin.max(0.0))
}

pub fn ray_dir(i: usize) -> Vec2 {
    let a = i as f32 / N_RAYS as f32 * std::f32::consts::TAU;
    vec2(a.cos(), a.sin())
}

/// If `target` is in line of sight from `eye`, returns (proximity 0.05..1, retina sector).
fn glimpse(eye: Vec2, target: Vec2) -> Option<(f32, usize)> {
    let to = target - eye;
    let dist = to.length().max(0.001);
    if dist >= VIS_MAX || terrain_ray(eye, to / dist, dist) < dist - 0.5 {
        return None;
    }
    let ang = to.y.atan2(to.x) + std::f32::consts::PI;
    let sector = ((ang / std::f32::consts::TAU * N_SECTORS as f32) as usize) % N_SECTORS;
    Some(((1.0 - dist / VIS_MAX).max(0.05), sector))
}

fn mark(s: &mut [f32; N_IN], base: usize, seen: Option<(f32, usize)>) {
    if let Some((p, sector)) = seen {
        s[base + sector] = s[base + sector].max(p);
    }
}

pub fn random_kinds(rng: &mut impl Rng) -> [Kind; N_FIGHTERS] {
    let mut pick = |base, variant| if rng.gen_bool(VARIANT_CHANCE) { variant } else { base };
    [
        pick(Kind::Spear, Kind::Thrower),
        pick(Kind::Spear, Kind::Thrower),
        pick(Kind::Biter, Kind::Spitter),
        pick(Kind::Biter, Kind::Spitter),
    ]
}

pub struct Match {
    pub f: [Fighter; N_FIGHTERS],
    pub rocks: Vec<Rock>,
    pub acid: Vec<Acid>,
    /// Health pickups currently lying in the arena.
    pub heals: Vec<Vec2>,
    /// Remaining spawn times this round, latest first.
    heal_schedule: Vec<u32>,
    pub heals_spawned: u32,
    rng: SmallRng,
    pub tick: u32,
    pub over: bool,
    pub winner: Option<Tribe>,
    pub events: Vec<Event>,
}

/// A random spot on top of the floor or a platform — everywhere a fighter can stand.
fn random_surface_spot(rng: &mut impl Rng) -> Vec2 {
    let floor = (14.0, 306.0, 164.0);
    let surfaces = std::iter::once(floor).chain(PLATFORMS.iter().map(|p| (p.x + 3.0, p.x + p.w - 3.0, p.y)));
    let (x0, x1, y) = surfaces.clone().nth(rng.gen_range(0..1 + PLATFORMS.len())).unwrap();
    vec2(rng.gen_range(x0..x1), y)
}

impl Match {
    /// Fighters 0-1 are humans, 2-3 creatures. Each tribe starts on a random side.
    /// `heals` is how many health pickups this round gets (picked at random from the range).
    pub fn new(rng: &mut impl Rng, kinds: [Kind; N_FIGHTERS], heals: std::ops::RangeInclusive<usize>) -> Self {
        let humans_left = rng.gen_bool(0.5);
        let f: [Fighter; N_FIGHTERS] = std::array::from_fn(|i| {
            let left = (i < 2) == humans_left;
            let zone = if left { 20.0 } else { 200.0 };
            let x = zone + rng.gen_range(0.0..48.0) + if i % 2 == 1 { 52.0 } else { 0.0 };
            Fighter::new(kinds[i], x, if left { 1.0 } else { -1.0 })
        });
        let rocks = (0..N_ROCKS)
            .map(|_| {
                let pos = random_surface_spot(rng) - vec2(0.0, ROCK_R);
                Rock { pos, vel: Vec2::ZERO, held_by: None, owner: None, lethal: false, age: 0 }
            })
            .collect();
        let mut heal_schedule: Vec<u32> = (0..rng.gen_range(heals)).map(|_| rng.gen_range(HEAL_WINDOW)).collect();
        heal_schedule.sort_unstable_by(|a, b| b.cmp(a));
        Self {
            f,
            rocks,
            acid: Vec::new(),
            heals: Vec::new(),
            heal_schedule,
            heals_spawned: 0,
            rng: SmallRng::seed_from_u64(rng.r#gen()),
            tick: 0,
            over: false,
            winner: None,
            events: Vec::new(),
        }
    }

    fn allies(&self, i: usize, j: usize) -> bool {
        self.f[i].kind.tribe() == self.f[j].kind.tribe()
    }

    /// Everything fighter `i` perceives. Other fighters, rocks and projectiles are only
    /// seen when in line of sight; allies and enemies land on separate "retinas".
    pub fn senses(&self, i: usize) -> [f32; N_IN] {
        let me = &self.f[i];
        let mut s = [0.0; N_IN];
        let eye = me.eye();
        for r in 0..N_RAYS {
            s[r] = 1.0 - terrain_ray(eye, ray_dir(r), RAY_MAX) / RAY_MAX;
        }
        let mut nearest: Option<(f32, usize)> = None;
        for j in 0..N_FIGHTERS {
            if j == i || !self.f[j].alive() {
                continue;
            }
            let seen = glimpse(eye, self.f[j].center());
            if self.allies(i, j) {
                mark(&mut s, IN_ALLY, seen);
            } else {
                mark(&mut s, N_RAYS, seen);
                if let Some((p, _)) = seen {
                    if nearest.is_none_or(|(np, _)| p > np) {
                        nearest = Some((p, j));
                    }
                }
            }
        }
        let k = N_RAYS + N_SECTORS;
        if let Some((_, j)) = nearest {
            let o = &self.f[j];
            s[k] = (o.atk_timer > 0) as u8 as f32;
            s[k + 1] = if (o.facing > 0.0) == (o.pos.x < me.pos.x) { 1.0 } else { -1.0 };
            s[IN_EXTRA + 2] = o.kind.ranged() as u8 as f32;
        }
        let k = k + 2;
        s[k] = me.vel.x / RUN;
        s[k + 1] = me.vel.y / MAX_FALL;
        s[k + 2] = me.grounded as u8 as f32;
        s[k + 3] = me.hp / MAX_HP;
        s[k + 4] = me.facing;
        s[k + 5] = (me.atk_timer == 0 && me.cooldown == 0) as u8 as f32;
        s[k + 6] = (me.hitstun > 0) as u8 as f32;
        s[IN_MEM..IN_MEM + MEM].copy_from_slice(&me.memory);
        for r in &self.rocks {
            if r.held_by.is_some() {
                continue;
            }
            let seen = glimpse(eye, r.pos);
            mark(&mut s, if r.lethal { IN_THREAT } else { IN_ROCK }, seen);
        }
        for a in &self.acid {
            if a.owner != i {
                mark(&mut s, IN_THREAT, glimpse(eye, a.pos));
            }
        }
        s[IN_EXTRA] = me.holding.is_some() as u8 as f32;
        s[IN_EXTRA + 1] = me.ranged_ready();
        for &h in &self.heals {
            mark(&mut s, IN_HEAL, glimpse(eye, h));
        }
        s
    }

    pub fn step(&mut self, brains: [&Brain; N_FIGHTERS]) {
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
            let input = self.senses(i);
            let mut out = [0.0; N_OUT];
            brains[i].forward(&input, &mut out);
            actions[i] = Action {
                mv: out[0],
                run: out[1] > 0.0,
                jump: out[2] > 0.0,
                melee: out[3] > 0.0,
                ranged: out[4] > 0.0,
                aim: out[5],
            };
            self.f[i].memory.copy_from_slice(&out[OUT_MEM..]);
            self.f[i].action = actions[i];
        }

        // 2. Move.
        for i in 0..N_FIGHTERS {
            let was_grounded = self.f[i].grounded;
            let f = &mut self.f[i];
            control(f, actions[i], &mut self.events);
            collide(f);
            if f.grounded && !was_grounded {
                self.events.push(Event::Land { pos: f.pos });
            }
        }

        // 3. Pick up, throw, spit.
        for i in 0..N_FIGHTERS {
            self.ranged(i, actions[i]);
        }

        // 4. Melee (resolved simultaneously; can hit anyone, allies included).
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
            let dir = self.f[i].facing;
            self.damage(i, j, self.f[i].kind.melee_damage(), vec2(dir * 150.0, -140.0), HITSTUN, dir);
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

        // 5. Health pickups, then projectiles.
        self.step_heals();
        self.step_rocks();
        self.step_acid();

        // 6. Bookkeeping.
        for i in 0..N_FIGHTERS {
            let close = (0..N_FIGHTERS)
                .any(|j| !self.allies(i, j) && self.f[j].alive() && self.f[i].center().distance(self.f[j].center()) < CLOSE_DIST);
            if close && self.f[i].alive() {
                self.f[i].close_ticks += 1;
            }
        }
        self.tick += 1;
        let humans = self.f[..2].iter().any(|f| f.alive());
        let creatures = self.f[2..].iter().any(|f| f.alive());
        if !humans || !creatures || self.tick >= MAX_TICKS {
            self.over = true;
            self.winner = match (humans, creatures) {
                (true, false) => Some(Tribe::Humans),
                (false, true) => Some(Tribe::Creatures),
                (false, false) => None,
                // Time's up: the tribe with more health left wins. Only an exact tie is a draw.
                (true, true) => {
                    let (h, c) = (self.tribe_health(Tribe::Humans), self.tribe_health(Tribe::Creatures));
                    if h > c {
                        Some(Tribe::Humans)
                    } else if c > h {
                        Some(Tribe::Creatures)
                    } else {
                        None
                    }
                }
            };
        }
    }

    /// Total health of a tribe's living fighters.
    pub fn tribe_health(&self, tribe: Tribe) -> f32 {
        self.f.iter().filter(|f| f.kind.tribe() == tribe && f.alive()).map(|f| f.hp).sum()
    }

    /// True if the match ended on the clock rather than by wiping out a tribe.
    pub fn timed_out(&self) -> bool {
        self.over && self.f[..2].iter().any(|f| f.alive()) && self.f[2..].iter().any(|f| f.alive())
    }

    fn ranged(&mut self, i: usize, a: Action) {
        let f = &mut self.f[i];
        if f.throw_cd > 0 {
            f.throw_cd -= 1;
        }
        if f.kind == Kind::Spitter {
            f.saliva = (f.saliva + 1.0 / SALIVA_TICKS).min(1.0);
        }
        if !f.alive() || f.hitstun > 0 {
            return;
        }
        match f.kind {
            Kind::Thrower => {
                if f.holding.is_none() {
                    let body = f.body();
                    if let Some(r) = self.rocks.iter().position(|r| r.held_by.is_none() && !r.lethal && body.overlaps(&Aabb::around(r.pos, ROCK_R))) {
                        self.rocks[r].held_by = Some(i);
                        f.holding = Some(r);
                        f.throw_cd = THROW_COOLDOWN;
                    }
                } else if a.ranged && f.throw_cd == 0 {
                    let r = f.holding.take().unwrap();
                    let dir = f.aim_dir(a.aim);
                    f.throw_cd = THROW_COOLDOWN;
                    let rock = &mut self.rocks[r];
                    *rock = Rock { pos: f.hand(), vel: dir * THROW_SPEED + f.vel * 0.5, held_by: None, owner: Some(i), lethal: true, age: 0 };
                    self.events.push(Event::Throw { who: i });
                }
            }
            Kind::Spitter if a.ranged && f.saliva >= 1.0 => {
                f.saliva = 0.0;
                let dir = f.aim_dir(a.aim);
                self.acid.push(Acid { pos: f.eye() + vec2(f.facing * 4.0, 2.0), vel: dir * SPIT_SPEED, owner: i, age: 0 });
                self.events.push(Event::Spit { who: i });
            }
            _ => {}
        }
    }

    fn step_rocks(&mut self) {
        for ri in 0..self.rocks.len() {
            if let Some(h) = self.rocks[ri].held_by {
                self.rocks[ri].pos = self.f[h].hand();
                continue;
            }
            step_rock(&mut self.rocks[ri]);
            let r = &self.rocks[ri];
            if !r.lethal {
                continue;
            }
            let rb = Aabb::around(r.pos, ROCK_R);
            let victim = (0..N_FIGHTERS).find(|&j| {
                let own_grace = r.owner == Some(j) && r.age < OWN_ROCK_GRACE;
                self.f[j].alive() && !own_grace && rb.overlaps(&self.f[j].body())
            });
            if let Some(j) = victim {
                let dir = r.vel.x.signum();
                let src = r.owner.unwrap_or(j);
                let r = &mut self.rocks[ri];
                r.vel *= -0.25;
                r.lethal = false;
                self.damage(src, j, RANGED_DAMAGE, vec2(dir * 110.0, -100.0), 10, dir);
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
            self.events.push(Event::HealSpawn { pos });
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

    fn step_acid(&mut self) {
        let mut ai = 0;
        while ai < self.acid.len() {
            let a = &mut self.acid[ai];
            a.vel.y += GRAVITY * 0.45 * DT;
            a.pos += a.vel * DT;
            a.age += 1;
            let (pos, vel, owner) = (a.pos, a.vel, a.owner);
            let victim = (0..N_FIGHTERS).find(|&j| j != owner && self.f[j].alive() && self.f[j].body().contains(pos));
            if let Some(j) = victim {
                let dir = vel.x.signum();
                self.damage(owner, j, RANGED_DAMAGE, vec2(dir * 70.0, -60.0), 8, dir);
            }
            if victim.is_some() || in_terrain(pos) || self.acid[ai].age > 240 {
                self.events.push(Event::Splash { pos });
                self.acid.swap_remove(ai);
            } else {
                ai += 1;
            }
        }
    }

    fn damage(&mut self, src: usize, victim: usize, dmg: f32, knockback: Vec2, stun: u32, dir: f32) {
        let ally = self.allies(src, victim);
        if src != victim {
            if ally {
                self.f[src].dmg_ally += dmg;
            } else {
                self.f[src].dmg_enemy += dmg;
            }
        }
        let v = &mut self.f[victim];
        v.hp -= dmg;
        v.dmg_taken += dmg;
        v.vel = knockback;
        v.grounded = false;
        v.hitstun = stun;
        v.atk_timer = 0;
        let pos = v.center();
        self.events.push(Event::Hit { victim, pos, dir });
        if !v.alive() {
            if let Some(r) = v.holding.take() {
                self.rocks[r].held_by = None;
                self.rocks[r].vel = Vec2::ZERO;
            }
            self.events.push(Event::Death { who: victim, pos });
            if src != victim {
                if ally {
                    self.f[src].ally_kills += 1;
                } else {
                    self.f[src].kills += 1;
                }
            }
        }
    }

    /// Outcome-based score. Hurting an ally costs exactly what hurting an enemy earns, and
    /// the whole tribe shares in a win (by wipe-out, or by having more health at the timeout),
    /// so loyalty and teamwork have to be learned. `shaping` (0..1) is an optional "get close
    /// to an enemy" hint the trainer fades out early on.
    pub fn fitness(&self, i: usize, shaping: f32) -> f32 {
        let me = &self.f[i];
        // Healing cancels out the damage-taken penalty for the health it actually restored.
        let mut s = me.dmg_enemy - me.dmg_ally - 0.5 * (me.dmg_taken - me.healed);
        s += 60.0 * (me.kills as f32 - me.ally_kills as f32);
        if !me.alive() {
            s -= 40.0;
        }
        match self.winner {
            Some(t) if t == me.kind.tribe() => s += 80.0 + 40.0 * (1.0 - self.tick as f32 / MAX_TICKS as f32),
            Some(_) => s -= 20.0,
            None => {}
        }
        s + shaping * me.close_ticks as f32 * 0.03
    }
}

fn step_rock(r: &mut Rock) {
    r.age += 1;
    r.vel.y = (r.vel.y + GRAVITY * 0.8 * DT).min(MAX_FALL);
    let rb = |p: Vec2| Aabb::around(p, ROCK_R);

    r.pos.x += r.vel.x * DT;
    if let Some(s) = SOLIDS.iter().find(|s| rb(r.pos).overlaps(s)) {
        r.pos.x = if r.vel.x > 0.0 { s.x - ROCK_R } else { s.x + s.w + ROCK_R };
        r.vel.x *= -0.4;
    }

    let prev_bottom = r.pos.y + ROCK_R;
    r.pos.y += r.vel.y * DT;
    let mut landed = None;
    if let Some(s) = SOLIDS.iter().find(|s| rb(r.pos).overlaps(s)) {
        if r.vel.y > 0.0 {
            landed = Some(s.y);
        } else {
            r.pos.y = s.y + s.h + ROCK_R;
            r.vel.y *= -0.3;
        }
    }
    if r.vel.y > 0.0 {
        for p in PLATFORMS.iter() {
            let over_x = r.pos.x + ROCK_R > p.x && r.pos.x - ROCK_R < p.x + p.w;
            if over_x && prev_bottom <= p.y + 0.01 && r.pos.y + ROCK_R >= p.y {
                landed = Some(p.y);
            }
        }
    }
    if let Some(top) = landed {
        r.pos.y = top - ROCK_R;
        r.vel.y = if r.vel.y > 60.0 { -r.vel.y * 0.35 } else { 0.0 };
        r.vel.x *= 0.7;
        if r.vel.x.abs() < 4.0 {
            r.vel.x = 0.0;
        }
    }
    if r.vel.length() < ROCK_LETHAL_SPEED {
        r.lethal = false;
    }
    // Belt and braces: rocks can never leave the box.
    r.pos.x = r.pos.x.clamp(8.0 + ROCK_R, 312.0 - ROCK_R);
    r.pos.y = r.pos.y.clamp(8.0 + ROCK_R, 164.0 - ROCK_R);
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
        if a.melee && f.atk_timer == 0 && f.cooldown == 0 {
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
