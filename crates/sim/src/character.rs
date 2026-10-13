//! Characters: every fighter shares the same base body (move, jump, melee, grab, throw, HP);
//! a character adds one special move on a cooldown. Each special is a `Special` strategy, so a
//! new character is a new entry in `Character` plus (optionally) a new `Special` impl.
//!
//! All tuning lives in the constants at the top of this file.

use crate::sim::{Aabb, Arena, Event, Fighter, Match, Source, DT, N_BODIES, N_FIGHTERS, SOLIDS, W};
use glam::{vec2, Vec2};
use serde::{Deserialize, Serialize};

const SECOND: u32 = crate::sim::TICK_RATE;

// Sam — Gravity Bomb. Lobbed ahead of him; it goes off when it lands, touches someone, or its
// fuse runs out. Going off, it yanks everyone on the map toward it for BOMB_CRUNCH ticks (no
// fighting it), then explodes. Within BOMB_PULL_RADIUS you're hauled right into the middle;
// farther out the yank weakens with distance (BOMB_PULL_FALLOFF): roughly 65px of drag from
// 150px away, 40px from 300px, 30px from across the arena.
const BOMB_COOLDOWN: u32 = 30 * SECOND;
/// Throw velocity (px/s, x is along Sam's facing): lands ~90px ahead on flat ground.
const BOMB_THROW: Vec2 = Vec2::new(170.0, -150.0);
const BOMB_GRAVITY: f32 = 520.0;
const BOMB_FUSE: u32 = SECOND;
pub const BOMB_PULL_RADIUS: f32 = 76.0;
/// The yank: a quarter second from the edge to the middle.
pub const BOMB_CRUNCH: u32 = 15;
const BOMB_PULL_MAX_SPEED: f32 = 420.0;
/// Beyond BOMB_PULL_RADIUS the yank's top speed scales by (BOMB_PULL_RADIUS / distance) to
/// this power.
const BOMB_PULL_FALLOFF: f32 = 0.7;
pub const BOMB_BLAST_RADIUS: f32 = 30.0;
const BOMB_DAMAGE: f32 = 34.0;
/// Damage at the blast's edge, as a fraction of the full hit.
const BOMB_EDGE: f32 = 0.4;
/// Bots hold the bomb for the first seconds of a match.
const BOT_BOMB_HOLD: u32 = 3 * SECOND;

// Austin — Pop-Up.
const POPUP_COOLDOWN: u32 = 15 * SECOND;
/// Austin never lands closer than this to a living opponent (if anywhere allows it).
const POPUP_MIN_GAP: f32 = 40.0;
/// Pop-Up only goes the way Austin is facing: this far ahead (about a third of the arena),
/// or when the wall is closer than that, anywhere ahead at least POPUP_MIN_AHEAD.
const POPUP_DISTANCE: f32 = 160.0;
const POPUP_MIN_AHEAD: f32 = 24.0;

// Seth — Mini Me: summons smaller (70%) clones of himself that fight on his side with the bot brain,
// pick things up and throw them like anyone, and go down to a single hit.
const MINI_COOLDOWN: u32 = 30 * SECOND;
const MINIS_PER_USE: usize = 2;
/// How long a mini lasts (if nothing hits it first; and it goes when Seth does).
const MINI_LIFE: u32 = 15 * SECOND;
pub const MINI_SCALE: f32 = 0.7;

// Tyler — Release the Hounds.
const HOUNDS_COOLDOWN: u32 = 30 * SECOND;
/// Each dog's speed (px/s): a little faster than a fighter's full run, so they catch runners.
const HOUNDS: [f32; 3] = [165.0, 180.0, 195.0];
const HOUND_TICKS: u32 = 5 * SECOND;
const BITE_DAMAGE: f32 = 15.0;
const BITE_EVERY: u32 = SECOND / 2;
// Corwin — Cat Herder: like the hounds, but twenty smaller cats with a lighter scratch, so they
// hurt as a swarm rather than one by one.
const CATS_COOLDOWN: u32 = 30 * SECOND;
const CATS: usize = 20;
/// The slowest and fastest cat (px/s); the rest are spread between, so the pack strings out.
const CAT_SPEEDS: (f32, f32) = (110.0, 205.0);
const CAT_TICKS: u32 = 5 * SECOND;
const SCRATCH_DAMAGE: f32 = 2.0;
/// A cat scratches each fighter at most once a second (in practice once per pass).
const SCRATCH_EVERY: u32 = SECOND;

// Justin — Sushi Table: a table drops in front of him (onto him, if he's facing a wall) and
// serves sushi to throw. It's solid from the sides and on top, never from below (see
// `sim::collide`), so it can't pin anyone.
const TABLE_COOLDOWN: u32 = 10 * SECOND;
const TABLE_TICKS: u32 = 15 * SECOND;
pub const TABLE_W: f32 = 26.0;
pub const TABLE_H: f32 = 9.0;
/// How far above the floor it appears, to drop into place.
const TABLE_DROP: f32 = 18.0;
const SUSHI_PER_TABLE: usize = 3;

// Ryan — Yanking Your Chain: a chain flies out (the way he aims, or faces) and hooks the first foe
// it touches, reeling them in to just in front of him, dazed for a moment. A miss reels back.
const CHAIN_COOLDOWN: u32 = 8 * SECOND;
pub const CHAIN_RANGE: f32 = 110.0;
const CHAIN_SPEED: f32 = 660.0;
const CHAIN_PULL_SPEED: f32 = 380.0;
const CHAIN_DAMAGE: f32 = 6.0;
/// How long the reeled-in fighter is left dazed (ticks).
const CHAIN_DAZE: u32 = 24;
/// Where a reeled-in fighter ends up: this far in front of Ryan.
const CHAIN_STOP: f32 = 10.0;
/// It gives up (and lets go) after this long, whatever is in the way.
const CHAIN_TICKS: u32 = 90;

// Jake — Into the Phone: he turns into light and goes inside his phone, which drops from his
// head to the floor. For a while he can't act or be hurt; anyone can pick the phone up and throw
// it. Then he comes back out, standing on wherever the phone is, and the phone stays behind.
const PHONE_COOLDOWN: u32 = 20 * SECOND;
pub const PHONE_TICKS: u32 = 5 * SECOND;

crate::enum_all! {
    /// Who you play. Everyone has the same body; a character adds a special (see `def`).
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
    pub enum Character {
        /// The base fighter the brains were trained as: no special.
        #[default]
        Classic,
        Sam,
        Austin,
        Tyler,
        Seth,
        Corwin,
        Justin,
        Ryan,
        Jake,
    }
}

/// What the game needs to know about a character. (Its look and sounds are in the render
/// crate's `art`; its name and special's name in the client's text files, under its id.)
///
/// Adding a character: a variant in `Character`, its row in `Character::def` (with a new
/// `Special` if it has one), its row in render's `art`, and `character.<id>` (plus
/// `special.<id>`) in the text files. The compiler and the tests point at each.
pub struct CharacterDef {
    /// A stable id (lowercase, no spaces): names it in text files and on the command line.
    pub id: &'static str,
    pub special: Option<&'static dyn Special>,
}

impl Character {
    pub fn def(self) -> &'static CharacterDef {
        match self {
            Character::Classic => &CharacterDef { id: "classic", special: None },
            Character::Sam => &CharacterDef { id: "sam", special: Some(&GravityBomb) },
            Character::Austin => &CharacterDef { id: "austin", special: Some(&PopUp) },
            Character::Tyler => &CharacterDef { id: "tyler", special: Some(&Hounds) },
            Character::Seth => &CharacterDef { id: "seth", special: Some(&MiniMe) },
            Character::Corwin => &CharacterDef { id: "corwin", special: Some(&CatHerder) },
            Character::Justin => &CharacterDef { id: "justin", special: Some(&SushiTable) },
            Character::Ryan => &CharacterDef { id: "ryan", special: Some(&YankChain) },
            Character::Jake => &CharacterDef { id: "jake", special: Some(&IntoThePhone) },
        }
    }

    pub fn id(self) -> &'static str {
        self.def().id
    }

    pub fn special(self) -> Option<&'static dyn Special> {
        self.def().special
    }
}

/// One character's special move.
pub trait Special: Sync {
    /// A stable id; players see the text `special.<id>`.
    fn id(&self) -> &'static str;
    fn cooldown(&self) -> u32;
    /// Does it. Fighter `who` is alive, not stunned, and off cooldown.
    fn activate(&self, m: &mut Match, who: usize);
    /// When a bot fires it: brains were never trained with specials, so a simple rule decides.
    fn bot_wants(&self, m: &Match, who: usize) -> bool;
}

/// Living opponents of `who` (not its own minis or summoner), in no particular order.
fn foes(m: &Match, who: usize) -> impl Iterator<Item = &Fighter> {
    (0..N_BODIES).filter(move |&j| m.foes(who, j)).map(|j| &m.f[j])
}

struct GravityBomb;

impl Special for GravityBomb {
    fn id(&self) -> &'static str {
        "gravity_bomb"
    }
    fn cooldown(&self) -> u32 {
        BOMB_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        m.effects.push(Effect::Bomb(new_bomb(&m.f[who], who)));
    }
    /// Works out where the bomb would go off and who'd be hauled into the blast (everyone
    /// within BOMB_PULL_RADIUS of it), then throws only when it pays: two or more caught, or
    /// one caught who it would finish off, or one caught while Sam is losing the fight.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        if m.tick < BOT_BOMB_HOLD {
            return false;
        }
        let at = bomb_landing(m, who);
        let caught: Vec<&Fighter> = foes(m, who).filter(|f| f.center().distance(at) < BOMB_PULL_RADIUS).collect();
        match caught.as_slice() {
            [] => false,
            [one] => one.hp <= BOMB_DAMAGE || m.f[who].hp < one.hp.min(50.0),
            _ => true,
        }
    }
}

struct PopUp;

impl Special for PopUp {
    fn id(&self) -> &'static str {
        "pop_up"
    }
    fn cooldown(&self) -> u32 {
        POPUP_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let from = m.f[who].center();
        // Only somewhere ahead of him, on any level: at least POPUP_DISTANCE ahead if there's
        // room, else anywhere ahead at least POPUP_MIN_AHEAD. Up against the wall
        // with nowhere ahead, he pops up where he stands.
        let (x, facing) = (m.f[who].pos.x, m.f[who].facing);
        let ahead = |d: f32| if facing > 0.0 { (x + d, W) } else { (0.0, x - d) };
        // The best of a handful of random standing spots there: the first far enough from
        // everyone, else whichever is farthest from its nearest opponent.
        let mut best = (f32::MIN, m.f[who].pos);
        for (lo, hi) in [ahead(POPUP_DISTANCE), ahead(POPUP_MIN_AHEAD)] {
            for _ in 0..24 {
                let Some(spot) = m.random_surface_spot_between(lo, hi) else { break };
                let gap = foes(m, who).map(|f| f.pos.distance(spot)).fold(f32::MAX, f32::min);
                if gap > best.0 {
                    best = (gap, spot);
                }
                if gap >= POPUP_MIN_GAP {
                    break;
                }
            }
            if best.0 > f32::MIN {
                break;
            }
        }
        let f = &mut m.f[who];
        f.pos = best.1;
        f.vel = Vec2::ZERO;
        f.grounded = true;
        let to = f.center();
        m.events.push(Event::Poof { pos: from });
        m.events.push(Event::Poof { pos: to });
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        in_danger(m, who)
    }
}

/// About to get hurt: a projectile closing in, a grenade about to blow, or crowded while weak.
/// (Austin pops away from it; Jake hides in his phone.)
fn in_danger(m: &Match, who: usize) -> bool {
    let me = &m.f[who];
    let c = me.center();
    let crowded = foes(m, who).any(|f| f.center().distance(c) < 24.0);
    // A projectile about to hit: lethal, not mine, close and closing.
    let incoming = m.items.iter().any(|it| it.lethal && it.owner != Some(who) && it.pos.distance(c) < 36.0 && (c - it.pos).dot(it.vel) > 0.0);
    let grenade = m.items.iter().any(|it| it.fuse.is_some_and(|t| t < 30) && it.pos.distance(c) < 30.0);
    incoming || grenade || (crowded && me.hp < 40.0)
}

struct MiniMe;

impl Special for MiniMe {
    fn id(&self) -> &'static str {
        "mini_me"
    }
    fn cooldown(&self) -> u32 {
        MINI_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let seth = m.f[who].clone();
        for k in 0..MINIS_PER_USE {
            // The mini slots after the seats; if they're all taken, fewer come.
            let Some(slot) = (N_FIGHTERS..N_BODIES).find(|&j| m.f[j].absent) else { break };
            let x = (seth.pos.x + seth.facing * (4.0 + 6.0 * k as f32)).clamp(14.0, W - 14.0);
            let mut mini = Fighter::new(x, seth.facing);
            mini.pos.y = seth.pos.y;
            // A little hop out in front, so they spread out.
            mini.vel = vec2(seth.facing * (40.0 + 40.0 * k as f32), -160.0);
            mini.grounded = false;
            mini.hp = 1.0;
            mini.scale = MINI_SCALE;
            mini.owner = Some(who);
            mini.life = Some(MINI_LIFE);
            mini.character = Character::Seth;
            m.events.push(Event::Poof { pos: mini.center() });
            m.f[slot] = mini;
        }
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].pos;
        foes(m, who).any(|f| f.pos.distance(me) < 180.0)
    }
}

struct Hounds;

impl Special for Hounds {
    fn id(&self) -> &'static str {
        "hounds"
    }
    fn cooldown(&self) -> u32 {
        HOUNDS_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let f = &m.f[who];
        for (k, &speed) in HOUNDS.iter().enumerate() {
            m.effects.push(Effect::Pet(Pet::new(Breed::Hound, f.pos + vec2(f.facing * (2.0 + 4.0 * k as f32), 0.0), f.facing, speed, who, HOUND_TICKS)));
        }
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        // Someone on my level, ahead of me, within a run's reach.
        foes(m, who).any(|f| (f.pos.y - me.pos.y).abs() < 10.0 && (f.pos.x - me.pos.x) * me.facing > 0.0 && (f.pos.x - me.pos.x).abs() < 160.0)
    }
}

struct CatHerder;

impl Special for CatHerder {
    fn id(&self) -> &'static str {
        "cat_herder"
    }
    fn cooldown(&self) -> u32 {
        CATS_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let f = &m.f[who];
        for k in 0..CATS {
            // Speeds shuffled across the pack (k * 7 mod 20), so neighbours don't run in step.
            let t = ((k * 7) % CATS) as f32 / (CATS - 1) as f32;
            let speed = CAT_SPEEDS.0 + (CAT_SPEEDS.1 - CAT_SPEEDS.0) * t;
            let x = (f.pos.x + f.facing * (1.0 + 0.6 * k as f32)).clamp(12.0, W - 12.0);
            m.effects.push(Effect::Pet(Pet::new(Breed::Cat(k as u8), vec2(x, f.pos.y), f.facing, speed, who, CAT_TICKS)));
        }
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        // Someone on my level ahead of me; the pack sweeps the whole floor.
        foes(m, who).any(|f| (f.pos.y - me.pos.y).abs() < 12.0 && (f.pos.x - me.pos.x) * me.facing > 0.0 && (f.pos.x - me.pos.x).abs() < 220.0)
    }
}

struct SushiTable;

impl Special for SushiTable {
    fn id(&self) -> &'static str {
        "sushi_table"
    }
    fn cooldown(&self) -> u32 {
        TABLE_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // One table each: a new one clears away the last (and whatever's left on it).
        let mine = |e: &Effect| matches!(e, Effect::Table(t) if t.owner == who);
        if let Some(Effect::Table(old)) = m.effects.iter().find(|e| mine(e)) {
            let old = old.clone();
            m.clear_table(&old);
        }
        m.effects.retain(|e| !mine(e));
        let f = &m.f[who];
        let half = TABLE_W / 2.0;
        let ahead = f.pos.x + f.facing * (f.half_w() + 3.0 + half);
        // Facing a wall (no room ahead): it drops onto him instead, from just over his head.
        let fits = ahead - half >= crate::sim::WALL && ahead + half <= W - crate::sim::WALL;
        let (x, y) = if fits { (ahead, f.pos.y - TABLE_DROP) } else { (f.pos.x.clamp(crate::sim::WALL + half, W - crate::sim::WALL - half), f.pos.y - f.height() - 2.0) };
        let t = Table { pos: vec2(x, y), vy: 0.0, owner: who, ticks: TABLE_TICKS, landed: false };
        m.events.push(Event::Poof { pos: vec2(x, y - TABLE_H / 2.0) });
        m.effects.push(Effect::Table(t));
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        // When empty-handed with nothing lying close by to throw, and someone to throw it at.
        let me = &m.f[who];
        let near_item = m.items.iter().any(|it| it.loose() && it.pos.distance(me.pos) < 60.0);
        me.holding.is_none() && !near_item && foes(m, who).any(|f| f.pos.distance(me.pos) < 220.0)
    }
}

/// Justin's table: drops, lands on whatever's below (fighters don't hold it up), serves sushi,
/// and is cleared away after a while.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Table {
    /// Bottom centre.
    pub pos: Vec2,
    pub vy: f32,
    pub owner: usize,
    pub ticks: u32,
    pub landed: bool,
}

impl Table {
    pub fn body(&self) -> Aabb {
        Aabb::new(self.pos.x - TABLE_W / 2.0, self.pos.y - TABLE_H, TABLE_W, TABLE_H)
    }
}

struct YankChain;

impl Special for YankChain {
    fn id(&self) -> &'static str {
        "yank_chain"
    }
    fn cooldown(&self) -> u32 {
        CHAIN_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let f = &mut m.f[who];
        // A player throws it where they aim; a bot straight ahead.
        let dir = match f.action.target {
            Some(t) if t.length_squared() > 1e-6 => t.normalize(),
            _ => vec2(f.facing, 0.0),
        };
        if dir.x.abs() > 1e-3 {
            f.facing = dir.x.signum();
        }
        let tip = chain_anchor(f);
        m.effects.push(Effect::Chain(Chain { tip, dir, owner: who, out: 0.0, hooked: None, back: false, ticks: CHAIN_TICKS }));
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        // Someone level with me, ahead, in reach but not already in my face.
        foes(m, who).any(|f| {
            let dx = (f.pos.x - me.pos.x) * me.facing;
            (f.center().y - me.center().y).abs() < 8.0 && dx > 30.0 && dx < CHAIN_RANGE - 10.0
        })
    }
}

struct IntoThePhone;

impl Special for IntoThePhone {
    fn id(&self) -> &'static str {
        "into_the_phone"
    }
    fn cooldown(&self) -> u32 {
        PHONE_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // Whatever he was holding drops where he stands.
        if let Some(k) = m.held_item(who) {
            m.items[k].held_by = None;
            m.items[k].vel = Vec2::ZERO;
        }
        let f = &mut m.f[who];
        f.holding = None;
        f.atk_timer = 0;
        f.vel = Vec2::ZERO;
        f.digital = PHONE_TICKS;
        let (center, head) = (f.center(), vec2(f.pos.x, f.pos.y - 13.0));
        let mut phone = crate::sim::Item::new(crate::sim::ItemKind::Phone, head);
        phone.vel = vec2(f.facing * 15.0, -30.0);
        phone.carries = Some(who);
        m.items.push(phone);
        m.events.push(Event::Hologram { pos: center, out: false });
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        in_danger(m, who)
    }
}

/// Ryan's chain: flying out, reeling back after a miss, or hauling in whoever it hooked.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chain {
    /// The hook end.
    pub tip: Vec2,
    pub dir: Vec2,
    pub owner: usize,
    /// How far it's flown.
    pub out: f32,
    pub hooked: Option<usize>,
    /// Missed: reeling back in.
    pub back: bool,
    pub ticks: u32,
}

/// Where the chain leaves Ryan (his chest): what the renderer draws it from, too.
pub fn chain_anchor(f: &Fighter) -> Vec2 {
    vec2(f.pos.x + f.facing * 2.0, f.pos.y - 9.0 * f.scale)
}

/// The solid things specials have put in the arena (Justin's tables), for `sim::collide`.
pub fn props(effects: &[Effect]) -> Vec<Aabb> {
    effects.iter().filter_map(|e| if let Effect::Table(t) = e { Some(t.body()) } else { None }).collect()
}

/// Sam's gravity bomb: flying, then (once it goes off) crunching everyone nearby together.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Bomb {
    pub pos: Vec2,
    pub vel: Vec2,
    pub owner: usize,
    /// Ticks of flight left before it goes off on its own.
    pub fuse: u32,
    /// None while flying; once it's gone off, the ticks left before it explodes.
    pub crunch: Option<u32>,
    /// How hard each fighter is being yanked (0..1), fixed by their distance when it went off.
    #[serde(skip)]
    pub pull: [f32; N_BODIES],
}

/// What kind of animal a `Pet` is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Breed {
    /// One of Tyler's dogs.
    Hound,
    /// One of Corwin's cats (the number picks its coat).
    Cat(u8),
}

impl Breed {
    /// Half width and height (px).
    pub fn size(self) -> (f32, f32) {
        match self {
            Breed::Hound => (3.0, 5.0),
            Breed::Cat(_) => (2.0, 4.0),
        }
    }
}

/// One of Tyler's dogs or Corwin's cats: runs along floors, turns at walls, and bites (or
/// scratches) anyone not on its owner's side.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pet {
    pub breed: Breed,
    /// Feet centre.
    pub pos: Vec2,
    pub dir: f32,
    pub speed: f32,
    pub vy: f32,
    pub owner: usize,
    pub ticks: u32,
    #[serde(skip)]
    bite_cd: [u32; N_BODIES],
}

impl Pet {
    fn new(breed: Breed, pos: Vec2, dir: f32, speed: f32, owner: usize, ticks: u32) -> Self {
        Self { breed, pos, dir, speed, vy: 0.0, owner, ticks, bite_cd: [0; N_BODIES] }
    }
    pub fn body(&self) -> Aabb {
        let (hw, h) = self.breed.size();
        Aabb::new(self.pos.x - hw, self.pos.y - h, hw * 2.0, h)
    }
    pub fn center(&self) -> Vec2 {
        vec2(self.pos.x, self.pos.y - self.breed.size().1 * 0.5)
    }
}

impl Match {
    /// Fires specials that were asked for and are ready, and counts down cooldowns.
    pub(crate) fn step_specials(&mut self, wants: [bool; N_BODIES]) {
        // Seats only: minis have no specials.
        for i in 0..N_FIGHTERS {
            let Some(special) = self.f[i].character.special() else { continue };
            let f = &mut self.f[i];
            if f.special_cd > 0 {
                f.special_cd -= 1;
                continue;
            }
            if wants[i] && f.alive() && f.hitstun == 0 {
                f.special_cd = special.cooldown();
                self.events.push(Event::Special { who: i });
                special.activate(self, i);
            }
        }
    }

    /// Advances every effect in the arena, then clears out the finished ones. Nothing happens
    /// (and nothing is drawn from the RNG) when there are none, so classic matches are
    /// unchanged.
    /// Minis count down and vanish (in a puff) when their time's up, when they're hit, or when
    /// whoever summoned them goes down; whatever they held drops.
    pub(crate) fn step_minis(&mut self) {
        for j in N_FIGHTERS..N_BODIES {
            if self.f[j].absent {
                continue;
            }
            let life = self.f[j].life.map_or(0, |t| t.saturating_sub(1));
            self.f[j].life = Some(life);
            let owner_down = self.f[j].owner.is_none_or(|o| !self.f[o].alive());
            if self.f[j].alive() && !owner_down && life > 0 {
                continue;
            }
            if let Some(k) = self.held_item(j) {
                self.items[k].held_by = None;
            }
            let pos = self.f[j].center();
            self.f[j] = Fighter::empty();
            self.events.push(Event::Poof { pos });
        }
    }

    /// Jake inside his phone goes wherever it goes; when his time's up he comes back out standing
    /// on it (and the phone stays, an ordinary item now).
    pub(crate) fn step_digital(&mut self) {
        for i in 0..N_FIGHTERS {
            if self.f[i].digital == 0 {
                continue;
            }
            let phone = self.items.iter().position(|it| it.carries == Some(i));
            let at = phone.map_or(self.f[i].pos, |k| self.items[k].pos + vec2(0.0, crate::sim::ITEM_R));
            let f = &mut self.f[i];
            f.digital -= 1;
            let hw = f.half_w();
            f.pos = vec2(at.x.clamp(crate::sim::WALL + hw, W - crate::sim::WALL - hw), at.y);
            f.vel = Vec2::ZERO;
            if f.digital == 0 {
                f.grounded = false;
                if let Some(k) = phone {
                    self.items[k].carries = None;
                }
                let pos = self.f[i].center();
                self.events.push(Event::Hologram { pos, out: true });
            }
        }
    }

    pub(crate) fn step_effects(&mut self) {
        for e in 0..self.effects.len() {
            // Stepped on a copy: an effect's step may hurt fighters (which needs the whole match).
            let mut effect = self.effects[e].clone();
            match &mut effect {
                Effect::Bomb(b) => self.step_bomb(b),
                Effect::Pet(p) => self.step_pet(p),
                Effect::Table(t) => self.step_table(t),
                Effect::Chain(c) => self.step_chain(c),
            }
            self.effects[e] = effect;
        }
        let finished: Vec<Table> = self.effects.iter().filter_map(|e| if let Effect::Table(t) = e { (t.ticks == 0).then(|| t.clone()) } else { None }).collect();
        for t in &finished {
            self.clear_table(t);
        }
        self.effects.retain(|e| !e.done());
    }

    fn step_pet(&mut self, pet: &mut Pet) {
        run_pet(pet, self.arena());
        let (body, owner, dir) = (pet.body(), pet.owner, pet.dir);
        for j in 0..N_BODIES {
            if !self.foes(owner, j) || pet.bite_cd[j] > 0 || !body.overlaps(&self.f[j].body()) {
                continue;
            }
            let pos = self.f[j].center();
            match pet.breed {
                Breed::Hound => {
                    pet.bite_cd[j] = BITE_EVERY;
                    self.events.push(Event::Bite { pos });
                    self.damage(owner, j, BITE_DAMAGE, vec2(dir * 120.0, -90.0), 8, dir, Source::Special, false);
                }
                // A scratch barely moves you: twenty of them shouldn't juggle you about.
                Breed::Cat(_) => {
                    pet.bite_cd[j] = SCRATCH_EVERY;
                    self.events.push(Event::Scratch { pos });
                    let v = self.f[j].vel;
                    self.damage(owner, j, SCRATCH_DAMAGE, vec2(v.x + dir * 25.0, v.y.min(-30.0)), 0, dir, Source::Special, false);
                }
            }
        }
        pet.bite_cd.iter_mut().for_each(|t| *t = t.saturating_sub(1));
        pet.ticks -= 1;
    }

    fn step_chain(&mut self, c: &mut Chain) {
        c.ticks -= 1;
        let owner = c.owner;
        if !self.f[owner].alive() {
            c.ticks = 0;
            return;
        }
        let anchor = chain_anchor(&self.f[owner]);
        match c.hooked {
            Some(j) => {
                if !self.f[j].alive() {
                    c.ticks = 0;
                    return;
                }
                // Haul them to just in front of Ryan, on whichever side they're coming from.
                let side = if self.f[j].pos.x >= self.f[owner].pos.x { 1.0 } else { -1.0 };
                let dest = self.f[owner].center() + vec2(side * CHAIN_STOP, 0.0);
                let to = dest - self.f[j].center();
                let v = &mut self.f[j];
                if to.length() < 4.0 || c.ticks == 0 {
                    v.vel = Vec2::ZERO;
                    v.hitstun = CHAIN_DAZE;
                    c.ticks = 0;
                } else {
                    v.vel = (to / DT).clamp_length_max(CHAIN_PULL_SPEED);
                    v.grounded = false;
                    v.hitstun = v.hitstun.max(2);
                    v.atk_timer = 0;
                }
                c.tip = self.f[j].center();
            }
            None if c.back => {
                let to = anchor - c.tip;
                if to.length() <= CHAIN_SPEED * DT {
                    c.ticks = 0;
                } else {
                    c.tip += to.normalize() * CHAIN_SPEED * DT;
                }
            }
            None => {
                c.tip += c.dir * CHAIN_SPEED * DT;
                c.out += CHAIN_SPEED * DT;
                let hook = Aabb::around(c.tip, 2.0);
                let caught = (0..N_BODIES).filter(|&j| self.foes(owner, j) && hook.overlaps(&self.f[j].body())).min_by(|&a, &b| self.f[a].center().distance(anchor).total_cmp(&self.f[b].center().distance(anchor)));
                // (Nobody can be hooked during the protected start.)
                if let Some(j) = caught.filter(|_| !self.protected()) {
                    c.hooked = Some(j);
                    c.tip = self.f[j].center();
                    self.events.push(Event::Hook { pos: c.tip });
                    self.damage(owner, j, CHAIN_DAMAGE, Vec2::ZERO, 2, c.dir.x.signum(), Source::Special, false);
                } else if c.out >= CHAIN_RANGE || SOLIDS.iter().any(|s| s.contains(c.tip)) {
                    c.back = true;
                }
            }
        }
    }

    /// A table falls until it lands on terrain (not on fighters), then serves its sushi.
    fn step_table(&mut self, t: &mut Table) {
        t.ticks -= 1;
        if t.landed {
            return;
        }
        let prev = t.pos.y;
        t.vy = (t.vy + 640.0 * DT).min(320.0);
        t.pos.y += t.vy * DT;
        let over = |x0: f32, x1: f32| t.pos.x + TABLE_W / 2.0 > x0 && t.pos.x - TABLE_W / 2.0 < x1;
        let floor = SOLIDS.iter().filter(|s| t.body().overlaps(s) && prev <= s.y + 0.01).map(|s| s.y);
        let ledge = self.arena().platforms.iter().filter(|p| over(p.x, p.x + p.w) && prev <= p.y + 0.01 && t.pos.y >= p.y).map(|p| p.y);
        let Some(top) = floor.chain(ledge).reduce(f32::min) else { return };
        t.pos.y = top;
        t.vy = 0.0;
        t.landed = true;
        self.events.push(Event::Land { pos: t.pos });
        let y = top - TABLE_H - crate::sim::ITEM_R;
        for k in 0..SUSHI_PER_TABLE {
            let dx = (k as f32 - (SUSHI_PER_TABLE - 1) as f32 / 2.0) * 7.0;
            self.items.push(crate::sim::Item::new(crate::sim::ItemKind::Sushi, vec2(t.pos.x + dx, y)));
        }
    }

    /// A table going away takes the sushi still sitting on it (nobody's touched it).
    pub(crate) fn clear_table(&mut self, t: &Table) {
        let top = t.body();
        let on_it = Aabb::new(top.x - 1.0, top.y - 4.0, top.w + 2.0, 5.0);
        self.items.retain(|it| !(it.kind == crate::sim::ItemKind::Sushi && it.held_by.is_none() && it.owner.is_none() && on_it.contains(it.pos)));
        self.events.push(Event::Poof { pos: vec2(t.pos.x, t.pos.y - TABLE_H / 2.0) });
    }

    fn step_bomb(&mut self, bomb: &mut Bomb) {
        match bomb.crunch {
            None => self.fly_bomb(bomb),
            Some(t) => self.crunch_bomb(bomb, t),
        }
    }
}

/// Something a special leaves in the arena. Each kind is a variant: its step (in
/// `step_effects`), how the bots perceive it (`threat_pos`), and how it's drawn (the render
/// crate) are all exhaustive matches, so the compiler lists what a new one needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Effect {
    /// Sam's gravity bomb.
    Bomb(Bomb),
    /// One of Tyler's dogs or Corwin's cats.
    Pet(Pet),
    /// Justin's sushi table.
    Table(Table),
    /// Ryan's chain.
    Chain(Chain),
}

impl Effect {
    /// Who made it (it never harms them, and they don't see it as a threat).
    pub fn owner(&self) -> usize {
        match self {
            Effect::Bomb(b) => b.owner,
            Effect::Pet(p) => p.owner,
            Effect::Table(t) => t.owner,
            Effect::Chain(c) => c.owner,
        }
    }

    /// Where the bots see it as a threat (None: harmless, like a table).
    pub fn threat_pos(&self) -> Option<Vec2> {
        match self {
            Effect::Bomb(b) => Some(b.pos),
            Effect::Pet(p) => Some(p.center()),
            Effect::Table(_) => None,
            Effect::Chain(c) => Some(c.tip),
        }
    }

    /// Its position (a pet's feet, a table's bottom centre).
    pub fn pos(&self) -> Vec2 {
        match self {
            Effect::Bomb(b) => b.pos,
            Effect::Pet(p) => p.pos,
            Effect::Table(t) => t.pos,
            Effect::Chain(c) => c.tip,
        }
    }

    /// Its position, for smoothing it between snapshots on the client.
    pub fn pos_mut(&mut self) -> &mut Vec2 {
        match self {
            Effect::Bomb(b) => &mut b.pos,
            Effect::Pet(p) => &mut p.pos,
            Effect::Table(t) => &mut t.pos,
            Effect::Chain(c) => &mut c.tip,
        }
    }

    fn done(&self) -> bool {
        match self {
            Effect::Bomb(b) => b.crunch == Some(0),
            Effect::Pet(p) => p.ticks == 0,
            Effect::Table(t) => t.ticks == 0,
            Effect::Chain(c) => c.ticks == 0,
        }
    }
}

impl Match {
    /// A bomb in flight: arcs under gravity and goes off on terrain, on a foe, or when its
    /// fuse runs out.
    fn fly_bomb(&mut self, bomb: &mut Bomb) {
        if !fly(bomb, &self.f, self.arena()) {
            return;
        }
        let (pos, owner) = (bomb.pos, bomb.owner);
        let pull = std::array::from_fn(|j| if !self.foes(owner, j) { 0.0 } else { pull_strength(self.f[j].center().distance(pos)) });
        bomb.vel = Vec2::ZERO;
        bomb.crunch = Some(BOMB_CRUNCH);
        bomb.pull = pull;
        self.events.push(Event::BombCrunch { pos });
    }

    /// A bomb that's gone off: yanks everyone toward the middle (they can't fight it, and the
    /// far side of the map feels it too, just less),
    /// then explodes.
    fn crunch_bomb(&mut self, bomb: &mut Bomb, left: u32) {
        let Bomb { pos, owner, pull, .. } = *bomb;
        let left = left - 1;
        bomb.crunch = Some(left);
        for j in 0..N_BODIES {
            let f = &mut self.f[j];
            if pull[j] <= 0.0 || !f.alive() {
                continue;
            }
            // Aim to arrive just as it blows; speed is capped so it's a yank, not a teleport.
            let to = pos - f.center();
            let secs = (left.max(1)) as f32 * DT;
            f.vel = (to / secs).clamp_length_max(BOMB_PULL_MAX_SPEED * pull[j]);
            f.grounded = false;
            f.hitstun = f.hitstun.max(2);
            f.atk_timer = 0;
        }
        if left > 0 {
            return;
        }
        self.events.push(Event::BombBlast { pos });
        for j in 0..N_BODIES {
            let d = self.f[j].center().distance(pos);
            if !self.foes(owner, j) || d >= BOMB_BLAST_RADIUS {
                continue;
            }
            let away = (self.f[j].center() - pos).normalize_or(vec2(0.0, -1.0));
            let dmg = BOMB_DAMAGE * (1.0 - d / BOMB_BLAST_RADIUS).max(BOMB_EDGE);
            self.damage(owner, j, dmg, away * 240.0 + vec2(0.0, -110.0), 14, if away.x == 0.0 { 1.0 } else { away.x.signum() }, Source::Special, false);
        }
    }
}

/// One tick of a bomb in flight. True when it goes off: it hit terrain or a foe (it stops
/// there), or its fuse ran out.
fn fly(bomb: &mut Bomb, fighters: &[Fighter; N_BODIES], arena: &Arena) -> bool {
    bomb.vel.y += BOMB_GRAVITY * DT;
    let prev = bomb.pos;
    bomb.pos += bomb.vel * DT;
    bomb.fuse = bomb.fuse.saturating_sub(1);
    let mut hit = bomb.fuse == 0;
    if let Some(s) = SOLIDS.iter().find(|s| s.contains(bomb.pos)) {
        // Stop on the surface it hit.
        bomb.pos = vec2(bomb.pos.x.clamp(s.x - 0.5, s.x + s.w + 0.5), bomb.pos.y);
        if prev.y <= s.y {
            bomb.pos.y = s.y - 0.5;
        } else if prev.x < s.x || prev.x > s.x + s.w {
            bomb.pos.x = prev.x;
        } else {
            bomb.pos.y = prev.y;
        }
        hit = true;
    }
    if bomb.vel.y > 0.0
        && let Some(p) = arena.platforms.iter().find(|p| bomb.pos.x > p.x && bomb.pos.x < p.x + p.w && prev.y <= p.y && bomb.pos.y >= p.y)
    {
        bomb.pos.y = p.y - 0.5;
        hit = true;
    }
    let pos = bomb.pos;
    let team = fighters[bomb.owner].team(bomb.owner);
    hit || (0..N_BODIES).any(|j| fighters[j].team(j) != team && fighters[j].alive() && fighters[j].body().overlaps(&Aabb::around(pos, 2.0)))
}

/// Where a bomb Sam threw right now would go off: flown through the same physics, everyone
/// else standing still.
fn bomb_landing(m: &Match, who: usize) -> Vec2 {
    let mut b = new_bomb(&m.f[who], who);
    while !fly(&mut b, &m.f, m.arena()) {}
    b.pos
}

fn new_bomb(f: &Fighter, who: usize) -> Bomb {
    let vel = vec2(BOMB_THROW.x * f.facing, BOMB_THROW.y) + f.vel * 0.5;
    Bomb { pos: f.hand(), vel, owner: who, fuse: BOMB_FUSE, crunch: None, pull: [0.0; N_BODIES] }
}

/// How hard a gravity bomb yanks someone `d` px away: fully within BOMB_PULL_RADIUS, then
/// weaker with distance (but never nothing).
fn pull_strength(d: f32) -> f32 {
    if d <= BOMB_PULL_RADIUS { 1.0 } else { libm::powf(BOMB_PULL_RADIUS / d, BOMB_PULL_FALLOFF) }
}

fn run_pet(dog: &mut Pet, arena: &Arena) {
    let (half_w, h) = dog.breed.size();
    dog.pos.x += dog.dir * dog.speed * DT;
    if let Some(s) = SOLIDS.iter().find(|s| dog.body().overlaps(s)) {
        dog.pos.x = if dog.dir > 0.0 { s.x - half_w } else { s.x + s.w + half_w };
        dog.dir = -dog.dir;
    }
    let prev = dog.pos.y;
    dog.vy = (dog.vy + 640.0 * DT).min(320.0);
    dog.pos.y += dog.vy * DT;
    if let Some(s) = SOLIDS.iter().find(|s| dog.body().overlaps(s)) {
        dog.pos.y = if dog.vy > 0.0 { s.y } else { s.y + s.h + h };
        dog.vy = 0.0;
    }
    if dog.vy >= 0.0 {
        for p in arena.platforms {
            let over = dog.pos.x + half_w > p.x && dog.pos.x - half_w < p.x + p.w;
            if over && prev <= p.y + 0.01 && dog.pos.y >= p.y {
                dog.pos.y = p.y;
                dog.vy = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::Brain;
    use crate::sim::{Action, MAX_HP, MatchConfig};

    /// A match with the given cast, everyone standing still on the floor at chosen x positions.
    fn staged(cast: [Character; N_FIGHTERS], xs: [f32; N_FIGHTERS]) -> Match {
        let mut m = Match::new(7, &MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, ..MatchConfig::default() });
        m.items.clear();
        for i in 0..N_FIGHTERS {
            m.f[i].character = cast[i];
            m.f[i].pos = vec2(xs[i], crate::sim::FLOOR_Y);
        }
        m
    }

    /// One tick: seat 0 is a "human" pressing only special (or nothing); the rest run an
    /// all-zero brain, which stands still.
    fn step_idle(m: &mut Match, brain: &Brain, special: bool) {
        let a = Action { special, ..Action::default() };
        m.step_with([brain; N_FIGHTERS], Some((0, a)));
    }

    fn zero_brain() -> Brain {
        Brain { w: vec![0.0; Brain::N_WEIGHTS] }
    }

    #[test]
    fn character_and_special_ids_are_unique() {
        let mut ids: Vec<&str> = Character::ALL.iter().map(|c| c.id()).chain(Character::ALL.iter().filter_map(|c| c.special()).map(|s| s.id())).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "two characters or specials share an id");
    }

    #[test]
    fn bots_use_every_special_and_matches_finish() {
        let brains = Brain::committed();
        let mut fired = [0u32; Character::ALL.len()];
        // Enough matches that every character gets a dozen seats.
        let matches = (Character::ALL.len() * 3).max(12) as u64;
        for seed in 0..matches {
            let mut m = Match::new(seed, &MatchConfig::default());
            for i in 0..N_FIGHTERS {
                m.f[i].character = Character::ALL[(seed as usize + i) % Character::ALL.len()];
            }
            let lineup: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| &brains[(seed as usize + i) % brains.len()]);
            while !m.over {
                m.step(lineup);
                for e in &m.events {
                    if let Event::Special { who } = *e {
                        fired[m.f[who].character as usize] += 1;
                    }
                }
            }
        }
        for c in Character::ALL {
            let n = fired[c as usize];
            match c.special() {
                None => assert_eq!(n, 0, "{c:?} has no special"),
                Some(_) => assert!(n > 0, "bots never used {c:?}'s special: {fired:?}"),
            }
        }
    }

    #[test]
    fn cooldown_blocks_reuse() {
        let z = zero_brain();
        let mut m = staged([Character::Austin, Character::Classic, Character::Classic, Character::Classic], [40.0, 280.0, 290.0, 300.0]);
        step_idle(&mut m, &z, true);
        assert_eq!(m.f[0].special_cd, POPUP_COOLDOWN);
        let mut uses = 1;
        for _ in 0..POPUP_COOLDOWN - 1 {
            step_idle(&mut m, &z, true);
            uses += m.events.iter().filter(|e| matches!(e, Event::Special { .. })).count();
        }
        assert_eq!(uses, 1, "no second use inside the cooldown");
        step_idle(&mut m, &z, true);
        step_idle(&mut m, &z, true);
        assert!(m.events.iter().any(|e| matches!(e, Event::Special { .. })) || m.f[0].special_cd > POPUP_COOLDOWN - 3);
    }

    #[test]
    fn austin_pops_up_ahead_and_away_from_everyone() {
        let z = zero_brain();
        for seed in 0..40 {
            let facing = if seed % 2 == 0 { 1.0 } else { -1.0 };
            let mut m = staged([Character::Austin, Character::Classic, Character::Classic, Character::Classic], [240.0, 230.0, 250.0, 260.0]);
            m.f[0].facing = facing;
            m.rng_reseed(seed);
            step_idle(&mut m, &z, true);
            let a = &m.f[0];
            assert!((a.pos.x - 240.0) * facing >= POPUP_DISTANCE, "facing {facing}, landed at x {}", a.pos.x);
            let gap = m.f[1..N_FIGHTERS].iter().map(|f| f.pos.distance(a.pos)).fold(f32::MAX, f32::min);
            assert!(gap >= POPUP_MIN_GAP, "landed {gap} from a foe");
            assert!(a.grounded && a.vel == Vec2::ZERO);
            assert_eq!(m.events.iter().filter(|e| matches!(e, Event::Poof { .. })).count(), 2);
        }
    }

    #[test]
    fn austin_near_the_wall_still_goes_ahead() {
        let z = zero_brain();
        for seed in 0..20 {
            let mut m = staged([Character::Austin, Character::Classic, Character::Classic, Character::Classic], [400.0, 100.0, 150.0, 200.0]);
            m.f[0].facing = 1.0;
            m.rng_reseed(seed);
            step_idle(&mut m, &z, true);
            assert!(m.f[0].pos.x >= 400.0 + POPUP_MIN_AHEAD, "landed at x {}", m.f[0].pos.x);
        }
    }

    #[test]
    fn austin_against_the_wall_stays_put() {
        let z = zero_brain();
        let mut m = staged([Character::Austin, Character::Classic, Character::Classic, Character::Classic], [14.0, 200.0, 300.0, 400.0]);
        m.f[0].facing = -1.0;
        step_idle(&mut m, &z, true);
        assert_eq!(m.f[0].pos.x, 14.0);
    }

    #[test]
    fn sams_bomb_yanks_everyone_nearby_together_then_blows() {
        let z = zero_brain();
        // Sam faces right; the bomb lands on (or hits) the fighter ~100px ahead, the next one
        // is 45px past that, the last is far away.
        let mut m = staged([Character::Sam, Character::Classic, Character::Classic, Character::Classic], [100.0, 200.0, 245.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        assert!(matches!(m.effects.as_slice(), [Effect::Bomb(_)]));
        let mut went_off = None;
        for t in 0..BOMB_FUSE + BOMB_CRUNCH + 2 {
            step_idle(&mut m, &z, false);
            if let Some(Event::BombCrunch { pos }) = m.events.iter().find(|e| matches!(e, Event::BombCrunch { .. })) {
                went_off = Some((t, *pos));
            }
            if m.events.iter().any(|e| matches!(e, Event::BombBlast { .. })) {
                break;
            }
        }
        let (t, pos) = went_off.expect("the bomb went off");
        assert!(t < BOMB_FUSE, "it went off on contact, not on the fuse");
        assert!(m.effects.is_empty(), "it exploded");
        // The second fighter was dragged right in and caught the blast too.
        assert!(m.f[2].center().distance(pos) < BOMB_BLAST_RADIUS, "second fighter ended {} away", m.f[2].center().distance(pos));
        assert!(m.f[1].hp < MAX_HP && m.f[2].hp < MAX_HP, "both near fighters hurt: {} {}", m.f[1].hp, m.f[2].hp);
        // The far one (~245px away) is out of the blast but still dragged a fair way in.
        assert_eq!(m.f[3].hp, MAX_HP, "out of the blast: unhurt");
        let dragged = 440.0 - m.f[3].pos.x;
        assert!(dragged > 25.0 && dragged < 70.0, "far fighter dragged {dragged}px");
        assert_eq!(m.f[0].hp, MAX_HP, "Sam is unaffected");
        assert!(m.f[0].dmg_dealt > 0.0);
    }

    #[test]
    fn sam_bots_only_bomb_when_it_pays() {
        let wants = |xs: [f32; N_FIGHTERS], hp1: f32| {
            let mut m = staged([Character::Sam, Character::Classic, Character::Classic, Character::Classic], xs);
            m.tick = BOT_BOMB_HOLD;
            m.f[0].facing = 1.0;
            m.f[1].hp = hp1;
            GravityBomb.bot_wants(&m, 0)
        };
        // The bomb hits the fighter ~100px ahead: alone and healthy, not worth it...
        assert!(!wants([100.0, 200.0, 440.0, 460.0], MAX_HP));
        // ...but worth it with a second fighter close by, or to finish a weak one.
        assert!(wants([100.0, 200.0, 240.0, 460.0], MAX_HP));
        assert!(wants([100.0, 200.0, 440.0, 460.0], 20.0));
        // Nobody near where it would land.
        assert!(!wants([100.0, 330.0, 400.0, 460.0], MAX_HP));
    }

    #[test]
    fn seths_minis_fight_for_him_and_go_down_in_one_hit() {
        let z = zero_brain();
        let mut m = staged([Character::Seth, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        let minis: Vec<usize> = (N_FIGHTERS..N_BODIES).filter(|&j| !m.f[j].absent).collect();
        assert_eq!(minis.len(), MINIS_PER_USE);
        let a = minis[0];
        assert_eq!((m.f[a].scale, m.f[a].owner, m.f[a].hp), (MINI_SCALE, Some(0), 1.0));
        assert!(m.f[a].body().h < m.f[0].body().h * 0.8, "smaller than a full fighter");
        // On Seth's side: not each other's foes, everyone else's.
        assert!(!m.foes(a, 0) && !m.foes(0, a) && !m.foes(a, minis[1]));
        assert!(m.foes(a, 1) && m.foes(1, a));
        // What a mini does counts for Seth.
        m.damage(a, 1, 10.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].dmg_dealt, 10.0);
        // One hit and it's gone (in a puff).
        m.damage(1, a, 8.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        step_idle(&mut m, &z, false);
        assert!(m.f[a].absent);
        assert!(!m.f[minis[1]].absent, "the other is still there");
        // When Seth goes down, his minis go too.
        m.damage(1, 0, MAX_HP, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        step_idle(&mut m, &z, false);
        assert!(m.f[minis[1]].absent);
    }

    #[test]
    fn minis_vanish_when_their_time_is_up() {
        let z = zero_brain();
        let mut m = staged([Character::Seth, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        step_idle(&mut m, &z, true);
        for _ in 0..MINI_LIFE - 2 {
            step_idle(&mut m, &z, false);
        }
        assert!((N_FIGHTERS..N_BODIES).any(|j| !m.f[j].absent), "still around just before the end");
        step_idle(&mut m, &z, false);
        step_idle(&mut m, &z, false);
        assert!((N_FIGHTERS..N_BODIES).all(|j| m.f[j].absent));
    }

    #[test]
    fn tylers_hounds_bite_others_then_vanish() {
        let z = zero_brain();
        let mut m = staged([Character::Tyler, Character::Classic, Character::Classic, Character::Classic], [40.0, 120.0, 200.0, 280.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        assert_eq!(m.effects.iter().filter(|e| matches!(e, Effect::Pet(_))).count(), 3);
        for _ in 0..HOUND_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert!(m.effects.is_empty(), "gone after 5s");
        assert_eq!(m.f[0].hp, MAX_HP, "dogs never bite Tyler");
        assert!(m.f[1].hp < MAX_HP, "the fighter in their path got bitten");
        assert!(m.f[0].dmg_dealt > 0.0);
    }

    #[test]
    fn corwins_cats_swarm_but_each_scratch_is_light() {
        let z = zero_brain();
        let mut m = staged([Character::Corwin, Character::Classic, Character::Classic, Character::Classic], [40.0, 120.0, 200.0, 280.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        assert_eq!(m.effects.iter().filter(|e| matches!(e, Effect::Pet(Pet { breed: Breed::Cat(_), .. }))).count(), CATS);
        let mut scratches = 0;
        for _ in 0..CAT_TICKS {
            step_idle(&mut m, &z, false);
            scratches += m.events.iter().filter(|e| matches!(e, Event::Scratch { .. })).count();
        }
        assert!(m.effects.is_empty(), "gone after 5s");
        assert_eq!(m.f[0].hp, MAX_HP, "cats never scratch Corwin");
        assert!(scratches >= CATS, "the pack ran over people: {scratches} scratches");
        let lost = MAX_HP - m.f[1].hp;
        assert!(lost > 0.0 && lost < 80.0, "swarmed, not wiped out: lost {lost}");
    }

    /// A match with just Justin (seat 0) and one other fighter, standing where given.
    fn justin_and(x0: f32, x1: f32) -> Match {
        let mut m = staged([Character::Justin, Character::Classic, Character::Classic, Character::Classic], [x0, x1, 300.0, 400.0]);
        m.remove(2);
        m.remove(3);
        m.f[0].facing = 1.0;
        m
    }

    fn table(m: &Match) -> Table {
        m.effects.iter().find_map(|e| if let Effect::Table(t) = e { Some(t.clone()) } else { None }).expect("a table")
    }

    #[test]
    fn justins_table_lands_ahead_serves_sushi_and_is_solid_from_the_side_and_top() {
        let z = zero_brain();
        let mut m = justin_and(100.0, 300.0);
        step_idle(&mut m, &z, true);
        for _ in 0..30 {
            step_idle(&mut m, &z, false);
        }
        let t = table(&m);
        assert!(t.landed && t.pos.y == crate::sim::FLOOR_Y, "landed on the floor");
        assert!(t.pos.x > 100.0 + TABLE_W / 2.0, "in front of him");
        let sushi = m.items.iter().filter(|it| it.kind == crate::sim::ItemKind::Sushi).count();
        assert_eq!(sushi, SUSHI_PER_TABLE);
        // Running at it from the right: stopped at its side.
        let run_left = Action { mv: -1.0, run: true, ..Action::default() };
        for _ in 0..120 {
            m.step_with([&z; N_FIGHTERS], Some((1, run_left)));
        }
        let side = t.body().x + TABLE_W + m.f[1].half_w();
        assert!((m.f[1].pos.x - side).abs() < 0.01, "blocked at the table's side: x {}", m.f[1].pos.x);
        // Jumping on: stands on top.
        let hop = Action { mv: -1.0, jump: true, ..Action::default() };
        for _ in 0..40 {
            m.step_with([&z; N_FIGHTERS], Some((1, hop)));
        }
        for _ in 0..10 {
            m.step_with([&z; N_FIGHTERS], Some((1, Action::default())));
        }
        assert!(m.f[1].grounded && m.f[1].pos.y == t.body().y, "standing on the table: {:?}", m.f[1].pos);
    }

    #[test]
    fn nobody_is_ever_pinned_under_a_table() {
        let z = zero_brain();
        // The table drops right onto the fighter standing just ahead of Justin.
        let trapped = || {
            let mut m = justin_and(100.0, 100.0 + 4.0 + 3.0 + TABLE_W / 2.0);
            step_idle(&mut m, &z, true);
            for _ in 0..30 {
                step_idle(&mut m, &z, false);
            }
            m
        };
        let mut m = trapped();
        let t = table(&m);
        assert!(t.landed && m.f[1].body().overlaps(&t.body()), "it fell around them");
        // They can walk out either way...
        let mut out = trapped();
        for _ in 0..60 {
            out.step_with([&z; N_FIGHTERS], Some((1, Action { mv: 1.0, ..Action::default() })));
        }
        assert!(!out.f[1].body().overlaps(&t.body()), "walked out");
        // ...or jump straight up out of it, and land on top.
        let jump = Action { jump: true, ..Action::default() };
        for _ in 0..12 {
            m.step_with([&z; N_FIGHTERS], Some((1, jump)));
        }
        for _ in 0..60 {
            m.step_with([&z; N_FIGHTERS], Some((1, Action::default())));
        }
        assert_eq!(m.f[1].pos.y, t.body().y, "jumped out and landed on top");
    }

    #[test]
    fn facing_a_wall_the_table_drops_onto_justin_and_he_walks_out() {
        let z = zero_brain();
        let mut m = justin_and(16.0, 300.0);
        m.f[0].facing = -1.0;
        step_idle(&mut m, &z, true);
        for _ in 0..30 {
            step_idle(&mut m, &z, false);
        }
        let t = table(&m);
        assert!(t.landed && m.f[0].body().overlaps(&t.body()), "on top of him");
        for _ in 0..60 {
            m.step_with([&z; N_FIGHTERS], Some((0, Action { mv: 1.0, ..Action::default() })));
        }
        assert!(!m.f[0].body().overlaps(&t.body()), "walked out from under it");
    }

    #[test]
    fn thrown_sushi_splats_and_is_gone() {
        let z = zero_brain();
        let mut m = justin_and(100.0, 200.0);
        step_idle(&mut m, &z, true);
        for _ in 0..30 {
            step_idle(&mut m, &z, false);
        }
        // Justin walks to the table and grabs a piece.
        let grab = Action { mv: 1.0, jump: true, grab: true, ..Action::default() };
        for _ in 0..60 {
            if m.f[0].holding.is_some() {
                break;
            }
            m.step_with([&z; N_FIGHTERS], Some((0, grab)));
        }
        assert_eq!(m.f[0].holding, Some(crate::sim::ItemKind::Sushi));
        for _ in 0..40 {
            step_idle(&mut m, &z, false);
        }
        // At the fighter to his right.
        let throw = Action { throw: true, target: Some(vec2(1.0, -0.3)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, throw)));
        let mut splat = false;
        for _ in 0..60 {
            step_idle(&mut m, &z, false);
            splat |= m.events.iter().any(|e| matches!(e, Event::Splat { .. }));
        }
        assert!(splat, "it splatted");
        assert!(m.f[1].hp < MAX_HP, "and hit them");
        assert_eq!(m.items.iter().filter(|it| it.kind == crate::sim::ItemKind::Sushi).count(), SUSHI_PER_TABLE - 1, "the thrown piece is gone");
    }

    #[test]
    fn ryans_chain_hauls_in_the_first_foe_ahead_and_dazes_them() {
        let z = zero_brain();
        let mut m = staged([Character::Ryan, Character::Classic, Character::Classic, Character::Classic], [100.0, 180.0, 300.0, 400.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        let mut hooked = false;
        for _ in 0..CHAIN_TICKS {
            step_idle(&mut m, &z, false);
            hooked |= m.events.iter().any(|e| matches!(e, Event::Hook { .. }));
            if m.effects.is_empty() {
                break;
            }
        }
        assert!(hooked, "hooked the fighter 80px ahead");
        assert!(m.effects.is_empty(), "the chain let go once they arrived");
        let gap = m.f[1].pos.x - m.f[0].pos.x;
        assert!(gap > 0.0 && gap < CHAIN_STOP + 6.0, "reeled in to just in front: {gap}px away");
        assert!(m.f[1].hitstun > CHAIN_DAZE / 2, "dazed");
        assert_eq!(m.f[1].hp, MAX_HP - CHAIN_DAMAGE);
        assert_eq!(m.f[2].hp, MAX_HP, "only the first one");
        assert_eq!(m.f[0].hp, MAX_HP);
    }

    #[test]
    fn ryans_chain_reels_back_after_a_miss() {
        let z = zero_brain();
        let mut m = staged([Character::Ryan, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        for _ in 0..CHAIN_TICKS {
            step_idle(&mut m, &z, false);
            assert!(!m.events.iter().any(|e| matches!(e, Event::Hook { .. })), "nobody in reach");
        }
        assert!(m.effects.is_empty(), "reeled back in");
        assert!((1..N_FIGHTERS).all(|j| m.f[j].hp == MAX_HP && m.f[j].pos.x > 290.0));
    }

    #[test]
    fn jake_hides_in_his_phone_then_comes_out_wherever_it_went() {
        use crate::sim::ItemKind;
        let z = zero_brain();
        let mut m = staged([Character::Jake, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        step_idle(&mut m, &z, true);
        assert!(m.f[0].digital > 0 && m.f[0].alive());
        let phone = |m: &Match| m.items.iter().position(|it| it.kind == ItemKind::Phone).expect("a phone");
        assert_eq!(m.items[phone(&m)].carries, Some(0));
        assert!(m.items[phone(&m)].pos.y < crate::sim::FLOOR_Y - 8.0, "dropped from about head height");
        // Inside, he's untouchable and nobody's target.
        m.damage(1, 0, 50.0, Vec2::ZERO, 10, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP);
        assert!(!m.foes(1, 0));
        // Someone picks the phone up, carries it off and drops it.
        let k = phone(&m);
        m.items[k].held_by = Some(1);
        m.items[k].vel = Vec2::ZERO;
        m.f[1].holding = Some(ItemKind::Phone);
        m.f[1].pos.x = 250.0;
        for _ in 0..PHONE_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert_eq!(m.f[0].digital, 0, "back out");
        assert!(m.events.iter().any(|e| matches!(e, Event::Hologram { out: true, .. })) || m.f[0].pos.x > 200.0);
        assert!((m.f[0].pos.x - m.f[1].hand().x).abs() < 3.0, "came out where the phone was: {}", m.f[0].pos.x);
        let k = phone(&m);
        assert_eq!(m.items[k].carries, None, "the phone stays, an ordinary item");
        m.damage(1, 0, 10.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP - 10.0, "and he can be hurt again");
    }
}
