//! The boss fight (`Mode::Boss`, on a map with `Arena::boss`): everyone together against the
//! Gatecrasher, a giant that smashes through the back wall during the countdown and stands in the
//! middle of the arena for the whole match, never moving. It's a body like any other (in the `BOSS`
//! slot, huge and unshakeable), so every swing, throw, blast and special hurts it; its head is a
//! headshot like anyone's. It has no brain: `step_boss` runs its attacks, each picked at
//! someone (a player, a bot or a mini) after a wind-up the client draws, so there's time to get out
//! of the way.
//!
//! Players can't hurt each other with swings, throws or specials in a boss fight (they're all on
//! one side: `sim::side`), but explosions and fire still hurt everyone they reach.

use crate::character::Effect;
use crate::sim::{secs, Aabb, Event, Fighter, Item, ItemKind, Match, Source, BOSS, DT, FLOOR_Y, GRAVITY, N_BODIES, N_FIGHTERS, W, WALL};
use glam::{vec2, Vec2};
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

/// How big the boss is (times a fighter): 56px wide, 98 tall, its head the top 28.
pub const BOSS_SCALE: f32 = 7.0;
/// Where it stands (feet centre): the middle of the floor.
pub const BOSS_X: f32 = W / 2.0;
/// A boss fight's length, and how many health pickups turn up through it.
pub const BOSS_MATCH: u32 = secs(180);
pub const BOSS_HEALS: RangeInclusive<u32> = 6..=8;
/// Its health, for each fighter in the match (alone, it's a fifth of a four-player fight's).
pub const BOSS_HP_EACH: f32 = 160.0;
/// Below this share of its health it's enraged: shorter rests, more of everything, air strikes.
pub const RAGE_AT: f32 = 0.5;

// Between attacks.
pub(crate) const REST: u32 = 40;
pub(crate) const RAGE_REST: u32 = 20;
/// The first attack, after the protected start.
const FIRST_REST: u32 = 20;
/// Into each rest (if it lasts that long), the boulder it tosses between attacks.
pub const TOSS_AT: u32 = 12;

// Slam: both fists come down on the floor. Anyone on the floor near them is hurt, and a shockwave
// runs out each way along the floor to the walls (jump it, or be up on a ledge).
const SLAM_WINDUP: u32 = 36;
const SLAM_TOTAL: u32 = 52;
/// Where the fists land, either side of its middle.
pub const FIST_X: f32 = 38.0;
const SLAM_REACH: f32 = 22.0;
const SLAM_DAMAGE: f32 = 18.0;
const WAVE_SPEED: f32 = 170.0;
const RAGE_WAVE_SPEED: f32 = 230.0;
pub const WAVE_W: f32 = 6.0;
pub const WAVE_H: f32 = 9.0;
const WAVE_DAMAGE: f32 = 12.0;

// Volley: lobs lit grenades at people (they can be picked up and thrown back).
const VOLLEY_WINDUP: u32 = 26;
const VOLLEY_TOTAL: u32 = 40;
const VOLLEY_FLIGHT: f32 = 0.9;
/// Not when there are already this many lit ones about.
const MAX_LIVE_GRENADES: usize = 6;

// Boulders: hurls rocks straight at people, one after another (they lie about afterwards, to throw
// back).
const BOULDER_WINDUP: u32 = 18;
const BOULDER_EVERY: u32 = 7;
const BOULDERS: u32 = 2;
const RAGE_BOULDERS: u32 = 3;
const BOULDER_SPEED: f32 = 250.0;
/// How much harder its boulders hit than a thrown rock.
pub const BOULDER_HIT: f32 = 1.5;
/// Not when there are already this many rocks lying about.
const MAX_ROCKS: usize = 10;

// Swat: a backhand at anyone up by its head or shoulders, on one side.
const SWAT_WINDUP: u32 = 18;
const SWAT_ACTIVE: u32 = 8;
const SWAT_TOTAL: u32 = 34;
const SWAT_DAMAGE: f32 = 18.0;

// Air strike (enraged): it roars, and a jet comes over and bombs the arena.
const STRIKE_WINDUP: u32 = 45;
const STRIKE_TOTAL: u32 = 65;

// Spray: embers out of its mouth, swept back and forth across the hall (up and over, and down
// onto both floors), a few a second. Each is a small hit; it's the sheer number of them.
const SPRAY_WINDUP: u32 = 18;
const SPRAY_TICKS: u32 = 75;
const RAGE_SPRAY_TICKS: u32 = 105;
const SPRAY_EVERY: u32 = 5;
const RAGE_SPRAY_EVERY: u32 = 4;
/// How far either side of straight up the sweep goes (radians): down past level onto the floors.
const SPRAY_ARC: f32 = 2.2;
const EMBER_SPEED: f32 = 190.0;
const EMBER_GRAVITY: f32 = 110.0;
const EMBER_DAMAGE: f32 = 5.0;
const EMBER_TICKS: u32 = secs(3);
pub const EMBER_R: f32 = 2.0;
// And all the while, whatever else it's doing, it spits a little spread of them at someone.
const SPIT_EVERY: u32 = 45;
const RAGE_SPIT_EVERY: u32 = 30;
const SPIT_SPREAD: f32 = 0.22;

// Missiles: homing, slow enough to outrun with a dash, and a small blast (which hurts anyone near).
const MISSILE_WINDUP: u32 = 20;
const MISSILE_EVERY: u32 = 10;
const MISSILES: u32 = 3;
const RAGE_MISSILES: u32 = 5;
const MISSILE_SPEED: f32 = 125.0;
const MISSILE_TURN: f32 = 1.9;
const MISSILE_TICKS: u32 = secs(4);
pub const MISSILE_BLAST: crate::item::Blast = crate::item::Blast { radius: 18.0, damage: 18.0, edge: 0.5 };

crate::enum_all! {
    /// Which boss a boss map has.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
    pub enum BossKind {
        /// A giant that smashes through the back wall and stands in the middle (this file).
        Gatecrasher,
        /// A pit monster whose mouth is the middle of the floor (`sandmaw.rs`).
        Sandmaw,
    }
}

impl BossKind {
    /// A stable id; players see the text `boss.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            BossKind::Gatecrasher => "gatecrasher",
            BossKind::Sandmaw => "sandmaw",
        }
    }

    /// Its body (what can be hit) either side of its middle, and how tall from the floor; and how
    /// much of the top of it is its head (headshots).
    pub fn size(self) -> (f32, f32, f32) {
        match self {
            BossKind::Gatecrasher => (4.0 * BOSS_SCALE, 14.0 * BOSS_SCALE, 4.0 * BOSS_SCALE),
            BossKind::Sandmaw => (crate::sandmaw::BODY_HALF_W, crate::sandmaw::BODY_H, 0.0),
        }
    }

    /// The stretch of floor that kills whoever touches it (the Sandmaw's throat), if any.
    pub fn pit(self) -> Option<(f32, f32)> {
        match self {
            BossKind::Gatecrasher => None,
            BossKind::Sandmaw => Some(crate::sandmaw::THROAT),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BossMove {
    Rest,
    Slam,
    Volley,
    Boulders,
    Swat,
    Strike,
    /// A fan of embers sprayed back and forth across the whole hall.
    Spray,
    /// Homing missiles out of its shoulders, one after another.
    Missiles,
    // The Sandmaw (`sandmaw.rs`).
    /// Tentacles bursting out of the sand under people.
    Tentacles,
    /// Sucking everyone toward its mouth.
    Inhale,
    /// Snapping shut: whoever's over its mouth is bitten, and shockwaves run out along the floor.
    Chomp,
    /// Globs of acid spat at people.
    Acid,
}

impl BossMove {
    /// (ticks of wind-up before it lands, ticks all told).
    pub fn timing(self) -> (u32, u32) {
        match self {
            BossMove::Rest => (0, 0),
            BossMove::Slam => (SLAM_WINDUP, SLAM_TOTAL),
            BossMove::Volley => (VOLLEY_WINDUP, VOLLEY_TOTAL),
            BossMove::Boulders => (BOULDER_WINDUP, BOULDER_WINDUP + BOULDER_EVERY * RAGE_BOULDERS + 10),
            BossMove::Swat => (SWAT_WINDUP, SWAT_TOTAL),
            BossMove::Strike => (STRIKE_WINDUP, STRIKE_TOTAL),
            BossMove::Spray => (SPRAY_WINDUP, SPRAY_WINDUP + RAGE_SPRAY_TICKS),
            BossMove::Missiles => (MISSILE_WINDUP, MISSILE_WINDUP + MISSILE_EVERY * RAGE_MISSILES + 10),
            mv => crate::sandmaw::timing(mv),
        }
    }
}

/// What the boss is up to (on its `Fighter`, so clients draw it).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BossState {
    pub kind: BossKind,
    pub max_hp: f32,
    pub mv: BossMove,
    /// Ticks into the move (or the rest).
    pub t: u32,
    /// Which side the move is aimed at (-1 left, 1 right): the arm that throws or swats.
    pub side: f32,
    pub enraged: bool,
    /// How long this rest lasts.
    #[serde(skip)]
    pub(crate) rest: u32,
    /// Who this swat has already hit (a bit per body).
    #[serde(skip)]
    pub(crate) hit: u16,
    /// Ticks until it next spits at someone.
    #[serde(skip)]
    pub(crate) spit: u32,
}

/// The boss as it stands in its slot (its health is set as the match starts: `size_up_boss`).
pub fn new_boss(kind: BossKind) -> Fighter {
    let mut f = Fighter::new(BOSS_X, 1.0);
    // (Its size is its kind's: `Fighter::half_w` and `height` ask it.)
    f.scale = BOSS_SCALE;
    f.boss = Some(BossState { kind, max_hp: BOSS_HP_EACH, mv: BossMove::Rest, t: 0, side: 1.0, enraged: false, rest: FIRST_REST, hit: 0, spit: SPIT_EVERY });
    f.hp = BOSS_HP_EACH;
    f
}

/// When it bursts through the wall: a third of the way through the countdown.
pub fn burst_tick(countdown: u32) -> u32 {
    countdown / 3
}

/// Its head's middle (the Sandmaw's: its mouth).
pub fn head(f: &Fighter) -> Vec2 {
    match f.boss.map(|b| b.kind) {
        Some(BossKind::Sandmaw) => vec2(f.pos.x, f.pos.y - 6.0),
        _ => vec2(f.pos.x, f.pos.y - f.height() + 14.0),
    }
}

/// Its mouth (where what it spits comes from).
pub fn mouth(f: &Fighter) -> Vec2 {
    match f.boss.map(|b| b.kind) {
        Some(BossKind::Sandmaw) => vec2(f.pos.x, f.pos.y - 8.0),
        _ => vec2(f.pos.x, f.pos.y - f.height() + 22.0),
    }
}

/// What kind of shot a `BossShot` is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ShotKind {
    /// The Gatecrasher's: flies in an arc, a small hit.
    Ember,
    /// The Gatecrasher's: homes in and blows up.
    Missile,
    /// The Sandmaw's acid: lobbed, a bigger hit, splatters on ledges too.
    Glob,
}

/// One of the boss's shots.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BossShot {
    pub pos: Vec2,
    pub vel: Vec2,
    pub kind: ShotKind,
    pub ticks: u32,
    /// What a missile is chasing.
    #[serde(skip)]
    pub target: Option<u8>,
}

/// A boss shockwave running along the floor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shockwave {
    /// Its middle, on the floor.
    pub pos: Vec2,
    /// Where it set off from (the client draws the crack it leaves behind).
    pub start: f32,
    pub dir: f32,
    pub speed: f32,
    pub ticks: u32,
    /// Who it's already hit (a bit per body).
    #[serde(skip)]
    pub hit: u16,
}

impl Shockwave {
    pub fn area(&self) -> Aabb {
        Aabb::new(self.pos.x - WAVE_W / 2.0, self.pos.y - WAVE_H, WAVE_W, WAVE_H)
    }
}

/// The velocity that takes something from `from` to `to` in `t` seconds, falling at `g`.
pub(crate) fn lob(from: Vec2, to: Vec2, t: f32, g: f32) -> Vec2 {
    let d = to - from;
    vec2(d.x / t, d.y / t - 0.5 * g * t)
}

impl Match {
    /// As the match starts: the boss's health, for however many are in it.
    pub(crate) fn size_up_boss(&mut self) {
        let n = self.f[..N_FIGHTERS].iter().filter(|f| !f.absent).count().max(1) as f32;
        let f = &mut self.f[BOSS];
        if let Some(b) = &mut f.boss {
            b.max_hp = BOSS_HP_EACH * n;
            f.hp = b.max_hp;
        }
    }

    /// Everyone the boss would go for: whoever's in the arena and on the other side.
    pub(crate) fn boss_targets(&self) -> Vec<usize> {
        (0..N_BODIES).filter(|&j| self.foes(BOSS, j)).collect()
    }

    /// One of them at random (the match's own dice).
    pub(crate) fn boss_pick(&mut self) -> Option<usize> {
        let t = self.boss_targets();
        (!t.is_empty()).then(|| t[self.roll(t.len() as u32) as usize])
    }

    /// The boss's tick: it bursts in during the countdown, then rests and attacks in turn.
    pub(crate) fn step_boss(&mut self) {
        let Some(mut b) = self.f[BOSS].boss else { return };
        if self.tick == burst_tick(self.config.countdown) {
            self.events.push(Event::BossBurst { pos: head(&self.f[BOSS]) });
        }
        // (The Sandmaw's sand is slick: whoever's on a slope slides down it, toward its mouth.)
        if b.kind == BossKind::Sandmaw && !self.counting_down() {
            self.slide_downhill();
        }
        if !self.f[BOSS].alive() || self.protected() {
            return;
        }
        if !b.enraged && self.f[BOSS].hp < b.max_hp * RAGE_AT {
            b.enraged = true;
            self.events.push(Event::BossRoar { pos: head(&self.f[BOSS]) });
        }
        if b.kind == BossKind::Sandmaw {
            self.step_sandmaw(&mut b);
            self.f[BOSS].boss = Some(b);
            return;
        }
        // Whatever else it's doing (bar spraying them already), it spits embers at someone now and then.
        if b.mv != BossMove::Spray {
            b.spit = b.spit.saturating_sub(1);
            if b.spit == 0 {
                b.spit = if b.enraged { RAGE_SPIT_EVERY } else { SPIT_EVERY };
                self.boss_spit();
            }
        }
        b.t += 1;
        match b.mv {
            BossMove::Rest => {
                // Never idle for long: between attacks it hurls a boulder at someone on the side
                // it's facing (or anyone).
                if b.t == TOSS_AT && b.enraged {
                    let side = b.side;
                    let me = self.f[BOSS].pos.x;
                    let on_side: Vec<usize> = self.boss_targets().into_iter().filter(|&j| (self.f[j].pos.x - me) * side >= 0.0).collect();
                    let pick = if on_side.is_empty() { self.boss_pick() } else { Some(on_side[self.roll(on_side.len() as u32) as usize]) };
                    if let Some(j) = pick {
                        b.side = if self.f[j].pos.x < me { -1.0 } else { 1.0 };
                        self.boss_boulder(j);
                    }
                }
                if b.t >= b.rest {
                    self.boss_choose(&mut b);
                }
            }
            mv => {
                self.boss_attack(&mut b);
                if b.t >= mv.timing().1 {
                    b.mv = BossMove::Rest;
                    b.t = 0;
                    b.rest = if b.enraged { RAGE_REST } else { REST };
                    // (The arm it'll toss its next boulder with: toward someone.)
                    b.side = match self.boss_pick() {
                        Some(j) if self.f[j].pos.x < self.f[BOSS].pos.x => -1.0,
                        _ => 1.0,
                    };
                }
            }
        }
        self.f[BOSS].boss = Some(b);
    }

    /// Picks its next attack: a swat at anyone up by its head, else something at random.
    fn boss_choose(&mut self, b: &mut BossState) {
        let me = &self.f[BOSS];
        let top = me.pos.y - me.height();
        let near_head = self.boss_targets().into_iter().map(|j| self.f[j].center()).find(|c| (c.x - me.pos.x).abs() < me.half_w() + 50.0 && c.y > top - 30.0 && c.y < top + 50.0);
        let (mv, side) = if let Some(c) = near_head {
            (BossMove::Swat, if c.x < me.pos.x { -1.0 } else { 1.0 })
        } else {
            let grenades = self.items.iter().filter(|it| it.fuse.is_some()).count();
            let plane = self.effects.iter().any(|e| matches!(e, Effect::Plane(_) | Effect::AirBomb(_)));
            let options = [
                (BossMove::Slam, 3),
                (BossMove::Volley, if grenades < MAX_LIVE_GRENADES { 1 } else { 0 }),
                (BossMove::Boulders, 1),
                (BossMove::Strike, if b.enraged && !plane { 2 } else { 0 }),
                (BossMove::Spray, 4),
                (BossMove::Missiles, 3),
            ];
            let total: u32 = options.iter().map(|o| o.1).sum();
            let mut roll = self.roll(total);
            let mv = options.iter().find(|o| if roll < o.1 { true } else { roll -= o.1; false }).map_or(BossMove::Slam, |o| o.0);
            // (Which arm: toward whoever it's going for first.)
            let side = match self.boss_pick() {
                Some(j) if self.f[j].pos.x < self.f[BOSS].pos.x => -1.0,
                _ => 1.0,
            };
            (mv, side)
        };
        *b = BossState { mv, t: 0, side, hit: 0, ..*b };
    }

    fn boss_attack(&mut self, b: &mut BossState) {
        let (windup, _) = b.mv.timing();
        let me = self.f[BOSS].clone();
        match b.mv {
            BossMove::Rest => {}
            BossMove::Slam if b.t == windup => {
                for s in [-1.0, 1.0] {
                    let at = vec2(me.pos.x + s * FIST_X, FLOOR_Y);
                    self.events.push(Event::BossSlam { pos: at });
                    let speed = if b.enraged { RAGE_WAVE_SPEED } else { WAVE_SPEED };
                    let ticks = ((W / 2.0) / speed / DT) as u32 + 10;
                    self.add_effect(Effect::Shockwave(Shockwave { pos: at + vec2(s * (SLAM_REACH - WAVE_W), 0.0), start: at.x, dir: s, speed, ticks, hit: 0 }));
                }
                for j in self.boss_targets() {
                    let f = &self.f[j];
                    let dx = f.pos.x - me.pos.x;
                    if f.grounded && f.pos.y >= FLOOR_Y - 1.0 && (dx.abs() - FIST_X).abs() < SLAM_REACH + f.half_w() {
                        let dir = if dx < 0.0 { -1.0 } else { 1.0 };
                        self.damage(BOSS, j, SLAM_DAMAGE, vec2(dir * 200.0, -200.0), 16, dir, Source::Boss, false);
                    }
                }
            }
            BossMove::Volley if b.t == windup => {
                let n = if b.enraged { 2 } else { 1 };
                for k in 0..n {
                    let Some(j) = self.boss_pick() else { break };
                    let to = self.f[j].pos - vec2(0.0, 2.0) + vec2(self.f[j].vel.x * 0.3, 0.0);
                    let side = if to.x < me.pos.x { -1.0 } else { 1.0 };
                    let from = self.boss_hand(side) + vec2(0.0, k as f32 * -6.0);
                    let mut it = Item::new(ItemKind::Grenade, from);
                    it.owner = Some(BOSS);
                    it.fuse = ItemKind::Grenade.def().fuse;
                    // (Not lethal, so it falls as a lobbed item does.)
                    it.vel = lob(from, to, VOLLEY_FLIGHT + k as f32 * 0.1, GRAVITY * 0.8);
                    self.items.push(it);
                }
                self.events.push(Event::Throw { who: BOSS });
            }
            BossMove::Boulders if b.t >= windup && (b.t - windup) % BOULDER_EVERY == 0 => {
                let n = if b.enraged { RAGE_BOULDERS } else { BOULDERS };
                if (b.t - windup) / BOULDER_EVERY >= n {
                    return;
                }
                let Some(j) = self.boss_pick() else { return };
                b.side = if self.f[j].pos.x < me.pos.x { -1.0 } else { 1.0 };
                self.boss_boulder(j);
            }
            BossMove::Swat if b.t >= windup && b.t < windup + SWAT_ACTIVE => {
                let top = me.pos.y - me.height();
                let x0 = if b.side > 0.0 { me.pos.x } else { me.pos.x - me.half_w() - 60.0 };
                let reach = Aabb::new(x0, top - 30.0, me.half_w() + 60.0, 90.0);
                for j in self.boss_targets() {
                    if b.hit & (1 << j) != 0 || !reach.overlaps(&self.f[j].body()) {
                        continue;
                    }
                    b.hit |= 1 << j;
                    self.damage(BOSS, j, SWAT_DAMAGE, vec2(b.side * 260.0, -170.0), 18, b.side, Source::Boss, false);
                }
            }
            BossMove::Spray if b.t >= windup => {
                let (every, ticks) = if b.enraged { (RAGE_SPRAY_EVERY, RAGE_SPRAY_TICKS) } else { (SPRAY_EVERY, SPRAY_TICKS) };
                let k = b.t - windup;
                if k % every == 0 && k < ticks {
                    let a = -std::f32::consts::FRAC_PI_2 + libm::sinf(k as f32 * 0.07 * b.side) * SPRAY_ARC;
                    self.boss_ember(vec2(libm::cosf(a), libm::sinf(a)));
                }
            }
            BossMove::Missiles if b.t >= windup && (b.t - windup) % MISSILE_EVERY == 0 => {
                let k = (b.t - windup) / MISSILE_EVERY;
                if k >= if b.enraged { RAGE_MISSILES } else { MISSILES } {
                    return;
                }
                let side = if k % 2 == 0 { b.side } else { -b.side };
                let from = vec2(me.pos.x + side * 34.0, me.pos.y - me.height() + 2.0);
                let target = self.boss_pick().map(|j| j as u8);
                let vel = vec2(side * 0.6, -1.0).normalize() * MISSILE_SPEED;
                self.add_effect(Effect::BossShot(BossShot { pos: from, vel, kind: ShotKind::Missile, ticks: MISSILE_TICKS, target }));
                self.events.push(Event::RocketLaunch { pos: from });
            }
            BossMove::Strike if b.t == windup => {
                self.events.push(Event::BossRoar { pos: head(&me) });
                self.air_strike(BOSS, head(&me));
            }
            _ => {}
        }
    }

    /// Hurls a boulder straight at `j`, from the hand on their side. With too many rocks lying
    /// about already, the oldest one lying loose crumbles away first.
    fn boss_boulder(&mut self, j: usize) {
        let rocks = self.items.iter().filter(|it| it.kind == ItemKind::Rock).count();
        if rocks >= MAX_ROCKS
            && let Some(k) = self.items.iter().position(|it| it.kind == ItemKind::Rock && it.loose())
        {
            let pos = self.items.remove(k).pos;
            self.events.push(Event::Land { pos });
        }
        let to = self.f[j].center();
        let side = if to.x < self.f[BOSS].pos.x { -1.0 } else { 1.0 };
        let from = self.boss_hand(side);
        let g = GRAVITY * ItemKind::Rock.def().flight_gravity;
        let t = (from.distance(to) / BOULDER_SPEED).clamp(0.3, 1.0);
        let mut it = Item::new(ItemKind::Rock, from);
        it.owner = Some(BOSS);
        it.lethal = true;
        it.vel = lob(from, to, t, g);
        self.items.push(it);
        self.events.push(Event::Throw { who: BOSS });
    }

    /// Hurts `j` a little without knocking them about: no push, no stun, and a swing they're in
    /// carries on (embers come so thick that anything more pins people in place).
    pub(crate) fn graze(&mut self, src: usize, j: usize, dmg: f32, dir: f32, source: Source) {
        let f = &self.f[j];
        let (vel, grounded, hitstun, atk) = (f.vel, f.grounded, f.hitstun, f.atk_timer);
        self.damage(src, j, dmg, vel, 0, dir, source, false);
        let f = &mut self.f[j];
        if f.alive() {
            (f.vel, f.grounded, f.hitstun, f.atk_timer) = (vel, grounded, hitstun, atk);
        }
    }

    /// An ember out of its mouth, flying `dir`.
    fn boss_ember(&mut self, dir: Vec2) {
        let from = mouth(&self.f[BOSS]);
        self.add_effect(Effect::BossShot(BossShot { pos: from, vel: dir * EMBER_SPEED, kind: ShotKind::Ember, ticks: EMBER_TICKS, target: None }));
    }

    /// A little spread of embers spat at someone (aimed high, to arc onto them).
    fn boss_spit(&mut self) {
        let Some(j) = self.boss_pick() else { return };
        let from = mouth(&self.f[BOSS]);
        let to = self.f[j].center();
        let d = to - from;
        let aim = (d + vec2(0.0, -d.length() * 0.25)).normalize_or(vec2(0.0, -1.0));
        let a = libm::atan2f(aim.y, aim.x);
        for k in [-1.0, 0.0, 1.0] {
            let a = a + k * SPIT_SPREAD;
            self.boss_ember(vec2(libm::cosf(a), libm::sinf(a)));
        }
    }

    /// An ember arcs along and burns whoever it hits (a little); it's gone on a wall, the floor or
    /// the ceiling (it flies through ledges). A missile curves after whoever it's chasing and blows
    /// up on them, a wall, or when its fuel runs out.
    pub(crate) fn step_boss_shot(&mut self, s: &mut BossShot) {
        s.ticks = s.ticks.saturating_sub(1);
        if s.kind == ShotKind::Glob {
            self.step_glob(s);
            return;
        }
        if s.kind == ShotKind::Missile {
            let chasing = s.target.map(|j| j as usize).filter(|&j| self.foes(BOSS, j));
            let target = chasing.or_else(|| self.boss_targets().into_iter().min_by(|&a, &b| self.f[a].center().distance(s.pos).total_cmp(&self.f[b].center().distance(s.pos))));
            s.target = target.map(|j| j as u8);
            if let Some(j) = target {
                let angle = |v: Vec2| libm::atan2f(v.y, v.x);
                let (have, want) = (angle(s.vel), angle(self.f[j].center() - s.pos));
                let mut turn = want - have;
                while turn > std::f32::consts::PI {
                    turn -= std::f32::consts::TAU;
                }
                while turn < -std::f32::consts::PI {
                    turn += std::f32::consts::TAU;
                }
                let a = have + turn.clamp(-MISSILE_TURN * DT, MISSILE_TURN * DT);
                s.vel = vec2(libm::cosf(a), libm::sinf(a)) * MISSILE_SPEED;
            }
            s.pos += s.vel * DT;
            let hit = Aabb::around(s.pos, 2.0);
            let struck = self.boss_targets().into_iter().any(|j| hit.overlaps(&self.f[j].body()));
            let walled = self.arena().solids().any(|w| w.contains(s.pos));
            if struck || walled || s.ticks == 0 {
                s.ticks = 0;
                self.explode(s.pos, Some(BOSS), MISSILE_BLAST);
            }
            return;
        }
        s.vel.y += EMBER_GRAVITY * DT;
        s.pos += s.vel * DT;
        if self.arena().solids().any(|w| w.contains(s.pos)) {
            s.ticks = 0;
            return;
        }
        let hit = Aabb::around(s.pos, EMBER_R);
        if let Some(j) = self.boss_targets().into_iter().find(|&j| hit.overlaps(&self.f[j].body())) {
            s.ticks = 0;
            let dir = if s.vel.x < 0.0 { -1.0 } else { 1.0 };
            self.graze(BOSS, j, EMBER_DAMAGE, dir, Source::Ember);
        }
    }

    /// Where it throws from on `side`: its hand, held out clear of its body.
    fn boss_hand(&self, side: f32) -> Vec2 {
        let me = &self.f[BOSS];
        vec2(me.pos.x + side * (me.half_w() + 10.0), me.pos.y - me.height() * 0.75)
    }

    /// A shockwave runs out along the floor to the wall, knocking down anyone on the floor it
    /// passes (once each).
    pub(crate) fn step_shockwave(&mut self, w: &mut Shockwave) {
        w.ticks = w.ticks.saturating_sub(1);
        w.pos.x += w.dir * w.speed * DT;
        if w.pos.x < WALL || w.pos.x > W - WALL {
            w.ticks = 0;
            return;
        }
        // (Up and over the sand, where the floor slopes.)
        w.pos.y = self.arena().ground_at(w.pos.x, 0.0);
        let area = w.area();
        for j in self.boss_targets() {
            let f = &self.f[j];
            if w.hit & (1 << j) != 0 || !f.grounded || (f.pos.y - w.pos.y).abs() > 3.0 || !area.overlaps(&f.body()) {
                continue;
            }
            w.hit |= 1 << j;
            self.damage(BOSS, j, WAVE_DAMAGE, vec2(w.dir * 120.0, -190.0), 14, w.dir, Source::Boss, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::Brain;
    use crate::sim::{ArenaId, MatchConfig, Mode, MAX_HP, N_FIGHTERS};

    fn boss_match() -> (Match, Brain) {
        let config = MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, unstick: false, ..MatchConfig::for_arena(ArenaId::Lair) };
        let mut m = Match::new(5, &config);
        m.items.clear();
        (m, Brain { w: vec![0.0; Brain::N_WEIGHTS] })
    }

    #[test]
    fn a_boss_map_is_a_boss_fight_and_the_boss_has_health_for_everyone() {
        let (mut m, z) = boss_match();
        assert_eq!(m.config.mode, Mode::Boss);
        m.remove(3);
        m.step([&z; N_FIGHTERS]);
        let b = m.f[BOSS].boss.unwrap();
        assert_eq!(b.max_hp, BOSS_HP_EACH * 3.0);
        assert_eq!(m.f[BOSS].hp, b.max_hp);
        // Nobody else's map has one.
        let m = Match::new(5, &MatchConfig::for_arena(ArenaId::Tiers));
        assert!(m.f[BOSS].absent && m.f[BOSS].boss.is_none() && m.config.mode == Mode::LastStanding);
    }

    #[test]
    fn players_cant_hurt_each_other_but_blasts_still_do() {
        let (mut m, _) = boss_match();
        assert!(!m.foes(0, 1) && m.foes(0, BOSS) && m.foes(BOSS, 2));
        // A thrown rock passes straight through a teammate.
        m.f[1].pos = vec2(m.f[0].pos.x + 20.0, FLOOR_Y);
        let mut it = Item::new(ItemKind::Rock, m.f[1].center());
        it.owner = Some(0);
        it.lethal = true;
        it.vel = vec2(200.0, 0.0);
        m.items.push(it);
        m.step([&Brain { w: vec![0.0; Brain::N_WEIGHTS] }; N_FIGHTERS]);
        assert_eq!(m.f[1].hp, MAX_HP, "no friendly rocks");
        // A grenade going off right by them does.
        let at = m.f[1].center();
        m.explode(at, Some(0), crate::item::GRENADE_BLAST);
        assert!(m.f[1].hp < MAX_HP, "but blasts hurt everyone");
        assert_eq!(m.f[0].kills + m.f[0].dmg_dealt as u32, 0, "and score nothing");
    }

    #[test]
    fn everything_hurts_the_boss_but_nothing_moves_it() {
        let (mut m, z) = boss_match();
        m.step([&z; N_FIGHTERS]);
        let (pos, hp) = (m.f[BOSS].pos, m.f[BOSS].hp);
        // A blast at its shoulder (well away from its middle) still reaches it.
        let shoulder = vec2(BOSS_X + 30.0, FLOOR_Y - 80.0);
        m.explode(shoulder, Some(0), crate::item::GRENADE_BLAST);
        assert!(m.f[BOSS].hp < hp, "hurt");
        assert_eq!(m.f[0].dmg_dealt, hp - m.f[BOSS].hp, "and it counts");
        for _ in 0..30 {
            m.step([&z; N_FIGHTERS]);
        }
        assert_eq!(m.f[BOSS].pos, pos, "never moves");
        // Its own blasts don't.
        let hp = m.f[BOSS].hp;
        m.explode(shoulder, Some(BOSS), crate::item::GRENADE_BLAST);
        assert_eq!(m.f[BOSS].hp, hp);
    }

    #[test]
    fn beating_the_boss_ends_the_match_and_so_does_everyone_going_down() {
        let (mut m, z) = boss_match();
        m.step([&z; N_FIGHTERS]);
        let hp = m.f[BOSS].hp;
        m.damage(2, BOSS, hp, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        m.step([&z; N_FIGHTERS]);
        assert!(m.over && m.boss_beaten() && m.winner.is_none());
        assert_eq!(m.f[2].kills, 1, "the last blow gets the kill");

        let (mut m, z) = boss_match();
        m.step([&z; N_FIGHTERS]);
        for i in 0..N_FIGHTERS {
            m.damage(BOSS, i, MAX_HP, Vec2::ZERO, 0, 1.0, Source::Boss, false);
        }
        m.step([&z; N_FIGHTERS]);
        assert!(m.over && !m.boss_beaten());
    }

    #[test]
    fn it_attacks_people_who_stand_about() {
        // Everyone standing still on the floor: before long they're all hurt.
        let (mut m, z) = boss_match();
        let mut moves = std::collections::HashSet::new();
        for _ in 0..secs(40) {
            m.step([&z; N_FIGHTERS]);
            moves.insert(format!("{:?}", m.f[BOSS].boss.unwrap().mv));
            if m.over {
                break;
            }
        }
        assert!(m.f[..N_FIGHTERS].iter().all(|f| f.hp < MAX_HP), "everyone's been hit: {:?}", m.f[..N_FIGHTERS].iter().map(|f| f.hp).collect::<Vec<_>>());
        assert!(moves.len() >= 3, "a mix of attacks: {moves:?}");
    }

    #[test]
    fn a_shockwave_is_jumped_over_or_dodged_on_a_ledge() {
        let (mut m, z) = boss_match();
        m.f[0].pos = vec2(60.0, FLOOR_Y);
        m.f[1].pos = vec2(60.0, ArenaId::Lair.def().platforms[0].y);
        m.f[2].pos.x = 420.0;
        m.f[3].pos.x = 440.0;
        m.add_effect(Effect::Shockwave(Shockwave { pos: vec2(120.0, FLOOR_Y), start: 120.0, dir: -1.0, speed: WAVE_SPEED, ticks: 200, hit: 0 }));
        for _ in 0..30 {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[0].hp < MAX_HP, "on the floor: knocked down");
        assert_eq!(m.f[1].hp, MAX_HP, "on the ledge: safe");
        // Jumping over it.
        let (mut m, z) = boss_match();
        m.f[0].pos = vec2(60.0, FLOOR_Y);
        m.add_effect(Effect::Shockwave(Shockwave { pos: vec2(100.0, FLOOR_Y), start: 100.0, dir: -1.0, speed: WAVE_SPEED, ticks: 200, hit: 0 }));
        for t in 0..40 {
            m.step_with([&z; N_FIGHTERS], Some((0, crate::sim::Action { jump: t < 20, ..Default::default() })));
        }
        assert_eq!(m.f[0].hp, MAX_HP, "jumped it");
    }

    #[test]
    fn bots_fight_the_boss() {
        let brains = Brain::committed();
        let lineup: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| &brains[i % brains.len()]);
        let mut dealt = 0.0;
        for seed in 0..3 {
            let mut m = Match::new(seed, &MatchConfig::for_arena(ArenaId::Lair));
            while !m.over {
                m.step(lineup);
            }
            dealt += m.f[..N_FIGHTERS].iter().map(|f| f.dmg_dealt).sum::<f32>();
            assert!(m.f[..N_FIGHTERS].iter().all(|f| f.kills <= 1), "nobody killed a teammate");
        }
        assert!(dealt > 300.0, "bots hurt it: {dealt}");
    }

    #[test]
    fn a_players_rocket_goes_for_the_boss_not_a_teammate() {
        let (mut m, z) = boss_match();
        m.step([&z; N_FIGHTERS]);
        // (The boss holding fire, so only the rocket can hurt anyone.)
        if let Some(b) = &mut m.f[BOSS].boss {
            (b.rest, b.spit) = (u32::MAX / 2, u32::MAX / 2);
        }
        // A teammate right in front of the shooter, the boss further off.
        m.f[0].pos = vec2(60.0, FLOOR_Y);
        m.f[1].pos = vec2(90.0, FLOOR_Y);
        m.f[2].pos.x = 440.0;
        m.f[3].pos.x = 450.0;
        let mut launcher = Item::new(ItemKind::RocketLauncher, m.f[0].hand());
        launcher.held_by = Some(0);
        m.items.push(launcher);
        m.f[0].holding = Some(ItemKind::RocketLauncher);
        let hp = m.f[BOSS].hp;
        m.step_with([&z; N_FIGHTERS], Some((0, crate::sim::Action { throw: true, target: Some(vec2(1.0, 0.0)), ..Default::default() })));
        for _ in 0..secs(3) {
            m.step([&z; N_FIGHTERS]);
            if m.f[BOSS].hp < hp {
                break;
            }
        }
        assert!(m.f[BOSS].hp < hp, "it hit the boss");
        assert_eq!(m.f[1].hp, MAX_HP, "and flew past the teammate");
    }

    #[test]
    fn a_bomb_from_the_sky_falls_straight_onto_it() {
        let (mut m, z) = boss_match();
        m.step([&z; N_FIGHTERS]);
        let hp = m.f[BOSS].hp;
        m.add_effect(Effect::AirBomb(crate::character::AirBomb { pos: vec2(BOSS_X, 20.0), vel: Vec2::ZERO, owner: 0, ticks: 600 }));
        for _ in 0..secs(2) {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[BOSS].hp < hp, "nothing over its head to stop it");
    }

    #[test]
    fn it_bursts_through_the_wall_in_the_countdown() {
        let mut m = Match::new(1, &MatchConfig::for_arena(ArenaId::Lair));
        let z = Brain { w: vec![0.0; Brain::N_WEIGHTS] };
        let mut at = None;
        while m.counting_down() {
            m.step([&z; N_FIGHTERS]);
            if m.events.iter().any(|e| matches!(e, Event::BossBurst { .. })) {
                at = Some(m.tick - 1);
            }
        }
        assert_eq!(at, Some(burst_tick(m.config.countdown)));
    }
}
