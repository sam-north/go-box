//! Characters: every fighter shares the same base body (move, jump, melee, grab, throw, HP);
//! a character adds one special move on a cooldown. Each special is a `Special` strategy, so a
//! new character is a new entry in `Character` plus (optionally) a new `Special` impl.
//!
//! All tuning lives in the constants at the top of this file.

use crate::sim::{Aabb, Arena, Event, Fighter, Match, Source, DT, N_BODIES, N_FIGHTERS, SOLIDS, W};
use glam::{vec2, Vec2};
use serde::{Deserialize, Serialize};

const SECOND: u32 = crate::sim::TICK_RATE;

// Sam — Glue Gun: fires a glob of glue (like Noita's glue gunners). Whoever it hits is stuck: slowed
// right down and tethered to the nearest surface by an elastic strand for a few seconds. Strain far
// enough and the strand snaps. A glob that hits terrain leaves a blob there, and anyone who runs
// into a blob or a strand is stuck to it too.
const GLUE_COOLDOWN: u32 = 8 * SECOND;
const GLUE_SPEED: f32 = 260.0;
const GLUE_GRAVITY: f32 = 260.0;
/// A glob that hits nothing for this long just dries up.
const GLUE_FLIGHT: u32 = SECOND * 6 / 5;
pub const GLUE_HOLD: u32 = 5 * SECOND;
const GLUE_DAMAGE: f32 = 4.0;
/// The strand's slack, how far it stretches before it snaps, and how hard it pulls back (px/s² per
/// px of stretch).
pub const TETHER_REST: f32 = 6.0;
pub const TETHER_BREAK: f32 = 48.0;
const TETHER_SPRING: f32 = 20.0;
/// How close to a blob (or a strand) you have to come to get stuck in it.
const GLUE_CATCH: f32 = 4.0;

// Austin — Pop-Up.
const POPUP_COOLDOWN: u32 = 6 * SECOND;
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
/// How far (sideways) a cat can smell sushi from.
const CAT_SNIFF: f32 = 100.0;
/// A dog or cat on the ground with a ledge just above and ahead of it leaps up onto it now and then
/// (this chance a tick), so they don't all just run the floor.
const PET_HOP_CHANCE: f64 = 0.04;
/// How high a ledge they'll leap for, and how far ahead they look for one.
const PET_HOP_HEIGHT: f32 = 44.0;
const PET_HOP_AHEAD: f32 = 36.0;

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

// Vid — The Chosen One: he goes inside the opponent nearest the cursor and drives their body
// for a few seconds (they're drawn as falling code), then steps back out.
const CHOSEN_COOLDOWN: u32 = 20 * SECOND;
pub const RIDE_TICKS: u32 = 3 * SECOND;

// Robert — Forked Lightning: a bolt forks out to every foe in the arena at once (no need to see
// them), hurting most up close and least far away.
const LIGHTNING_COOLDOWN: u32 = 25 * SECOND;
const LIGHTNING_MAX: f32 = 30.0;
const LIGHTNING_MIN: f32 = 5.0;
/// From LIGHTNING_MAX at point blank down to LIGHTNING_MIN at this distance and beyond.
const LIGHTNING_REACH: f32 = 300.0;
/// How long the bolts stay on screen.
const LIGHTNING_TICKS: u32 = 20;

// Kyle — Organize: pins every foe exactly where they are for a few seconds.
const ORGANIZE_COOLDOWN: u32 = 20 * SECOND;
pub const PIN_TICKS: u32 = 3 * SECOND;

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
        Robert,
        Kyle,
        Vid,
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
            Character::Sam => &CharacterDef { id: "sam", special: Some(&GlueGun) },
            Character::Austin => &CharacterDef { id: "austin", special: Some(&PopUp) },
            Character::Tyler => &CharacterDef { id: "tyler", special: Some(&Hounds) },
            Character::Seth => &CharacterDef { id: "seth", special: Some(&MiniMe) },
            Character::Corwin => &CharacterDef { id: "corwin", special: Some(&CatHerder) },
            Character::Justin => &CharacterDef { id: "justin", special: Some(&SushiTable) },
            Character::Ryan => &CharacterDef { id: "ryan", special: Some(&YankChain) },
            Character::Jake => &CharacterDef { id: "jake", special: Some(&IntoThePhone) },
            Character::Robert => &CharacterDef { id: "robert", special: Some(&ForkedLightning) },
            Character::Kyle => &CharacterDef { id: "kyle", special: Some(&Organize) },
            Character::Vid => &CharacterDef { id: "vid", special: Some(&ChosenOne) },
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

struct GlueGun;

impl Special for GlueGun {
    fn id(&self) -> &'static str {
        "glue_gun"
    }
    fn cooldown(&self) -> u32 {
        GLUE_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let f = &mut m.f[who];
        // A player fires where they aim; a bot straight ahead (a touch upward, for the drop).
        let dir = match f.action.target {
            Some(t) if t.length_squared() > 1e-6 => t.normalize(),
            _ => vec2(f.facing, -0.08).normalize(),
        };
        if dir.x.abs() > 1e-3 {
            f.facing = dir.x.signum();
        }
        let glob = Glue { pos: f.hand(), vel: dir * GLUE_SPEED + f.vel * 0.3, owner: who, stuck: false, held: Vec::new(), ticks: GLUE_FLIGHT };
        m.effects.push(Effect::Glue(glob));
    }
    /// Someone ahead of him, near his level, in range.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        foes(m, who).any(|f| {
            let dx = (f.pos.x - me.pos.x) * me.facing;
            (f.pos.y - me.pos.y).abs() < 24.0 && dx > 20.0 && dx < 180.0
        })
    }
}

/// Sam's glue: a glob in flight, then (stuck to something) a blob holding whoever it caught.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Glue {
    /// The glob; once stuck, the blob the strands are anchored to.
    pub pos: Vec2,
    pub vel: Vec2,
    pub owner: usize,
    pub stuck: bool,
    /// Who it's holding (seats and mini slots).
    pub held: Vec<u8>,
    /// Flight time left, then (stuck) how long it holds.
    pub ticks: u32,
}

/// The point on any terrain (or table) nearest to `p`: where glue anchors someone it hits.
fn nearest_surface(arena: &Arena, props: &[Aabb], p: Vec2) -> Vec2 {
    SOLIDS
        .iter()
        .chain(arena.platforms)
        .chain(props)
        .map(|r| vec2(p.x.clamp(r.x, r.x + r.w), p.y.clamp(r.y, r.y + r.h)))
        .min_by(|a, b| a.distance(p).total_cmp(&b.distance(p)))
        .unwrap_or(p)
}

/// How far `p` is from the segment `a`-`b`.
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
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
        // A mouse player pops up right where the cursor is (in mid-air too: then he drops). Not
        // inside a wall or a table, though: squeezed out of the wall, or stood on the table.
        if let Some(c) = m.f[who].action.cursor {
            let f = &m.f[who];
            let (hw, h) = (f.half_w(), f.height());
            let x = c.x.clamp(crate::sim::WALL + hw, W - crate::sim::WALL - hw);
            let mut y = (c.y + h / 2.0).clamp(crate::sim::WALL + h + 1.0, crate::sim::FLOOR_Y);
            for p in props(&m.effects) {
                if Aabb::new(x - hw, y - h, hw * 2.0, h).overlaps(&p) {
                    y = p.y;
                }
            }
            let f = &mut m.f[who];
            f.pos = vec2(x, y);
            f.vel = Vec2::ZERO;
            f.grounded = false;
            let to = f.center();
            m.events.push(Event::Poof { pos: from });
            m.events.push(Event::Poof { pos: to });
            return;
        }
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
        // And Tyler runs with them: faster for as long as they're out.
        m.f[who].haste = HOUND_TICKS;
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
        // Facing a wall (no room ahead): it appears over his head instead. Either way it stays put
        // right where it appears: a floating platform, not something that falls.
        let fits = ahead - half >= crate::sim::WALL && ahead + half <= W - crate::sim::WALL;
        let (x, y) = if fits { (ahead, f.pos.y - TABLE_DROP) } else { (f.pos.x.clamp(crate::sim::WALL + half, W - crate::sim::WALL - half), f.pos.y - f.height() - 2.0) };
        let t = Table { pos: vec2(x, y), owner: who, ticks: TABLE_TICKS };
        m.events.push(Event::Poof { pos: vec2(x, y - TABLE_H / 2.0) });
        // Sushi served on top.
        let top = y - TABLE_H - crate::sim::ITEM_R;
        for k in 0..SUSHI_PER_TABLE {
            let dx = (k as f32 - (SUSHI_PER_TABLE - 1) as f32 / 2.0) * 7.0;
            m.items.push(crate::sim::Item::new(crate::sim::ItemKind::Sushi, vec2(x + dx, top)));
        }
        m.effects.push(Effect::Table(t));
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        // When empty-handed with nothing lying close by to throw, and someone to throw it at.
        let me = &m.f[who];
        let near_item = m.items.iter().any(|it| it.loose() && it.pos.distance(me.pos) < 60.0);
        me.holding.is_none() && !near_item && foes(m, who).any(|f| f.pos.distance(me.pos) < 220.0)
    }
}

/// Justin's table: appears in front of him and stays exactly there (a platform, in mid-air if
/// need be) with sushi on top, until it's cleared away.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Table {
    /// Bottom centre.
    pub pos: Vec2,
    pub owner: usize,
    pub ticks: u32,
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

struct ForkedLightning;

impl Special for ForkedLightning {
    fn id(&self) -> &'static str {
        "forked_lightning"
    }
    fn cooldown(&self) -> u32 {
        LIGHTNING_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let from = m.f[who].hand();
        let targets: Vec<usize> = (0..N_BODIES).filter(|&j| m.foes(who, j)).collect();
        let mut to = Vec::new();
        for j in targets {
            let c = m.f[j].center();
            let d = c.distance(from);
            let away = (c - from).normalize_or(vec2(0.0, -1.0));
            to.push(c);
            m.events.push(Event::Zap { pos: c });
            m.damage(who, j, lightning_damage(d), vec2(away.x * 70.0, -70.0), 8, if away.x < 0.0 { -1.0 } else { 1.0 }, Source::Special, false);
        }
        m.effects.push(Effect::Lightning(Lightning { from, to, owner: who, seed: m.tick, ticks: LIGHTNING_TICKS }));
    }
    /// When it would do 30 or more all told, or finish someone off.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let from = m.f[who].hand();
        let hits: Vec<(f32, f32)> = foes(m, who).map(|f| (lightning_damage(f.center().distance(from)), f.hp)).collect();
        hits.iter().map(|h| h.0).sum::<f32>() >= 30.0 || hits.iter().any(|&(dmg, hp)| dmg >= hp)
    }
}

fn lightning_damage(d: f32) -> f32 {
    LIGHTNING_MIN + (LIGHTNING_MAX - LIGHTNING_MIN) * (1.0 - d / LIGHTNING_REACH).max(0.0)
}

/// Robert's bolts, from his hand to where each foe was when it struck (for drawing; the damage
/// is done the moment it's cast).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lightning {
    pub from: Vec2,
    pub to: Vec<Vec2>,
    pub owner: usize,
    /// Shapes the forks.
    pub seed: u32,
    pub ticks: u32,
}

/// Kyle's organize while it lasts: where he cast it (the shockwave starts there) and for how long.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Order {
    pub at: Vec2,
    pub owner: usize,
    pub ticks: u32,
}

struct Organize;

impl Special for Organize {
    fn id(&self) -> &'static str {
        "organize"
    }
    fn cooldown(&self) -> u32 {
        ORGANIZE_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // Only his opponents: Kyle (and anyone on his side) moves freely.
        let caught: Vec<usize> = (0..N_BODIES).filter(|&j| m.foes(who, j)).collect();
        for j in caught {
            m.f[j].pinned = PIN_TICKS;
            m.f[j].vel = Vec2::ZERO;
        }
        let at = m.f[who].center();
        m.effects.push(Effect::Order(Order { at, owner: who, ticks: PIN_TICKS }));
    }
    /// Someone close enough to hit while they're stuck.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].center();
        foes(m, who).any(|f| f.center().distance(me) < 50.0)
    }
}

struct ChosenOne;

impl Special for ChosenOne {
    fn id(&self) -> &'static str {
        "chosen_one"
    }
    fn cooldown(&self) -> u32 {
        CHOSEN_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // The opponent nearest the cursor (or, without one, nearest him) who isn't already ridden.
        let near = m.f[who].action.cursor.unwrap_or(m.f[who].center());
        let target = (0..N_FIGHTERS).filter(|&j| m.foes(who, j) && m.f[j].ridden == 0).min_by(|&a, &b| m.f[a].center().distance(near).total_cmp(&m.f[b].center().distance(near)));
        let Some(j) = target else { return };
        if let Some(k) = m.held_item(who) {
            m.items[k].held_by = None;
            m.items[k].vel = Vec2::ZERO;
        }
        let from = m.f[who].center();
        let f = &mut m.f[who];
        f.holding = None;
        f.atk_timer = 0;
        f.vel = Vec2::ZERO;
        f.digital = RIDE_TICKS;
        f.host = Some(j as u8);
        m.f[j].ridden = RIDE_TICKS;
        m.events.push(Event::Matrix { pos: from, out: false });
        m.events.push(Event::Matrix { pos: m.f[j].center(), out: false });
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].center();
        foes(m, who).any(|f| f.center().distance(me) < 180.0)
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

// Rockets (from the rocket launcher item): steady speed, turning toward the nearest foe.
const ROCKET_SPEED: f32 = 200.0;
/// How fast it can turn (radians a second): enough to curve onto someone, not to loop back.
const ROCKET_TURN: f32 = 3.2;
const ROCKET_TICKS: u32 = 3 * SECOND;

/// A homing rocket.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rocket {
    pub pos: Vec2,
    pub vel: Vec2,
    pub owner: usize,
    pub ticks: u32,
}

impl Rocket {
    pub fn new(pos: Vec2, dir: Vec2, owner: usize) -> Self {
        Self { pos, vel: dir.normalize_or(vec2(1.0, 0.0)) * ROCKET_SPEED, owner, ticks: ROCKET_TICKS }
    }
}

// Molotovs: the burning strip they leave, and what standing in it costs.
const FIRE_HALF: f32 = 20.0;
pub const FIRE_H: f32 = 7.0;
const FIRE_TICKS: u32 = 4 * SECOND;
const FIRE_DAMAGE: f32 = 5.0;
/// How often the fire burns each fighter standing in it.
const FIRE_EVERY: u32 = SECOND / 2;

/// Burning ground: a strip along the surface a Molotov came down on.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fire {
    /// Middle of the strip, on the surface.
    pub at: Vec2,
    pub half_w: f32,
    pub owner: usize,
    pub ticks: u32,
    #[serde(skip)]
    burn_cd: [u32; N_BODIES],
}

impl Fire {
    /// The fire from a Molotov shattering at `pos`: on whatever surface is below (or under) it,
    /// spread along it but not past its ends.
    pub fn at(arena: &Arena, props: &[Aabb], pos: Vec2, owner: usize) -> Self {
        let probe = vec2(pos.x, pos.y - 2.0);
        let mut y = probe.y + crate::sim::terrain_ray(arena, probe, vec2(0.0, 1.0), crate::sim::H);
        for p in props.iter().filter(|p| pos.x >= p.x && pos.x <= p.x + p.w && p.y >= probe.y) {
            y = y.min(p.y);
        }
        let ledge = arena.platforms.iter().chain(props).find(|p| (p.y - y).abs() < 0.5 && pos.x >= p.x && pos.x <= p.x + p.w);
        let (lo, hi) = match ledge {
            Some(p) => (p.x, p.x + p.w),
            None => (crate::sim::WALL, W - crate::sim::WALL),
        };
        let (x0, x1) = ((pos.x - FIRE_HALF).max(lo), (pos.x + FIRE_HALF).min(hi));
        Self { at: vec2((x0 + x1) / 2.0, y), half_w: (x1 - x0) / 2.0, owner, ticks: FIRE_TICKS, burn_cd: [0; N_BODIES] }
    }

    pub fn area(&self) -> Aabb {
        Aabb::new(self.at.x - self.half_w, self.at.y - FIRE_H, self.half_w * 2.0, FIRE_H)
    }
}

// Air strikes: the jet crosses the top of the arena once, dropping bombs at random points.
const PLANE_SPEED: f32 = 210.0;
pub const PLANE_Y: f32 = 24.0;
/// The radio call before the jet arrives.
const PLANE_DELAY: u32 = SECOND;
const AIR_BOMB_GRAVITY: f32 = 420.0;

/// An air strike's jet: on its way after the radio call, then one pass across.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plane {
    pub pos: Vec2,
    pub dir: f32,
    pub owner: usize,
    /// Ticks until it arrives.
    pub delay: u32,
    /// Where (x) it still has to drop bombs, in the order it reaches them.
    #[serde(skip)]
    drops: Vec<f32>,
    pub ticks: u32,
}

impl Plane {
    /// A strike called in by `owner`: from a random side, with 5-7 bombs at random points.
    pub fn new(rng: &mut impl rand::Rng, owner: usize) -> Self {
        let dir = if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
        let mut drops: Vec<f32> = (0..rng.gen_range(5..=7)).map(|_| rng.gen_range(40.0..W - 40.0)).collect();
        drops.sort_by(|a, b| (a * dir).total_cmp(&(b * dir)));
        let x = if dir > 0.0 { -30.0 } else { W + 30.0 };
        Self { pos: vec2(x, PLANE_Y), dir, owner, delay: PLANE_DELAY, drops, ticks: 30 * SECOND }
    }
}

/// A bomb falling from the jet: it explodes on whatever it hits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AirBomb {
    pub pos: Vec2,
    pub vel: Vec2,
    pub owner: usize,
    pub ticks: u32,
}

/// The solid things specials have put in the arena (Justin's tables), for `sim::collide`.
pub fn props(effects: &[Effect]) -> Vec<Aabb> {
    effects.iter().filter_map(|e| if let Effect::Table(t) = e { Some(t.body()) } else { None }).collect()
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
    /// A cat that's found sushi and is sitting eating it (it's forgotten about fighting).
    pub eating: bool,
    #[serde(skip)]
    bite_cd: [u32; N_BODIES],
}

impl Pet {
    fn new(breed: Breed, pos: Vec2, dir: f32, speed: f32, owner: usize, ticks: u32) -> Self {
        Self { breed, pos, dir, speed, vy: 0.0, owner, ticks, eating: false, bite_cd: [0; N_BODIES] }
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
            if wants[i] && f.present() && f.hitstun == 0 {
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
    /// Vid inside someone: he goes where their body goes, and when his time's up (or they go
    /// down) he steps out beside them.
    fn step_riding(&mut self, i: usize) {
        let j = self.f[i].host.unwrap_or(0) as usize;
        let host_ok = self.f[j].present();
        let f = &mut self.f[i];
        f.digital = if host_ok { f.digital - 1 } else { 0 };
        f.vel = Vec2::ZERO;
        let left = f.digital;
        self.f[j].ridden = left;
        let h = &self.f[j];
        let (hx, hy) = (h.pos.x, h.pos.y);
        if left > 0 {
            self.f[i].pos = vec2(hx, hy);
            return;
        }
        // Out beside them, on whichever side has more room.
        let side = if hx < W / 2.0 { 1.0 } else { -1.0 };
        let f = &mut self.f[i];
        let hw = f.half_w();
        f.pos = vec2((hx + side * 10.0).clamp(crate::sim::WALL + hw, W - crate::sim::WALL - hw), hy);
        f.host = None;
        f.grounded = false;
        let pos = f.center();
        self.events.push(Event::Matrix { pos, out: true });
    }

    pub(crate) fn step_digital(&mut self) {
        for i in 0..N_FIGHTERS {
            if self.f[i].digital == 0 {
                continue;
            }
            if self.f[i].host.is_some() {
                self.step_riding(i);
                continue;
            }
            // A fresh press of left or right hops to the next phone that way (wrapping round): any
            // phone in the match, his or another Jake's, that nobody else is inside.
            let mv = self.f[i].action.mv;
            let dir: i8 = if mv > 0.2 { 1 } else if mv < -0.2 { -1 } else { 0 };
            let pressed = dir != 0 && dir != self.f[i].phone_mv;
            self.f[i].phone_mv = dir;
            if pressed {
                let mut phones: Vec<usize> = (0..self.items.len()).filter(|&k| self.items[k].kind == crate::sim::ItemKind::Phone && self.items[k].carries.is_none_or(|c| c == i)).collect();
                phones.sort_by(|&a, &b| self.items[a].pos.x.total_cmp(&self.items[b].pos.x));
                if let Some(at) = phones.iter().position(|&k| self.items[k].carries == Some(i))
                    && phones.len() > 1
                {
                    let n = phones.len() as i32;
                    let next = phones[((at as i32 + dir as i32).rem_euclid(n)) as usize];
                    self.items[phones[at]].carries = None;
                    self.items[next].carries = Some(i);
                }
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
                Effect::Glue(g) => self.step_glue(g),
                Effect::Pet(p) => self.step_pet(p),
                Effect::Table(t) => self.step_table(t),
                Effect::Chain(c) => self.step_chain(c),
                Effect::Lightning(l) => l.ticks = l.ticks.saturating_sub(1),
                Effect::Order(o) => o.ticks = o.ticks.saturating_sub(1),
                Effect::Rocket(r) => self.step_rocket(r),
                Effect::Fire(f) => self.step_fire(f),
                Effect::Plane(p) => self.step_plane(p),
                Effect::AirBomb(b) => self.step_air_bomb(b),
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
        // Cats can't resist sushi: one lying close by (on the floor or up on a table) and they all
        // crowd round it, each taking its own spot, and forget about scratching anyone.
        let goal = match pet.breed {
            Breed::Cat(k) => self
                .items
                .iter()
                .filter(|it| it.kind == crate::sim::ItemKind::Sushi && it.held_by.is_none() && !it.lethal)
                .filter(|it| (it.pos.x - pet.pos.x).abs() < CAT_SNIFF && (-45.0..8.0).contains(&(it.pos.y - pet.pos.y)))
                .min_by(|a, b| a.pos.distance(pet.pos).total_cmp(&b.pos.distance(pet.pos)))
                .map(|it| vec2(it.pos.x + ((k % 7) as f32 - 3.0) * 1.5, it.pos.y + crate::sim::ITEM_R)),
            Breed::Hound => None,
        };
        let hop = goal.is_none() && pet.vy == 0.0 && self.chance(PET_HOP_CHANCE);
        run_pet(pet, self.arena(), &props(&self.effects), goal, hop);
        if goal.is_some() {
            pet.bite_cd.iter_mut().for_each(|t| *t = t.saturating_sub(1));
            pet.ticks -= 1;
            return;
        }
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

    /// Burning ground: anyone standing in it (the thrower too) is burnt every so often.
    fn step_fire(&mut self, fire: &mut Fire) {
        fire.ticks = fire.ticks.saturating_sub(1);
        let area = fire.area();
        for j in 0..N_BODIES {
            fire.burn_cd[j] = fire.burn_cd[j].saturating_sub(1);
            let f = &self.f[j];
            if !f.present() || fire.burn_cd[j] > 0 || !area.overlaps(&f.body()) {
                continue;
            }
            fire.burn_cd[j] = FIRE_EVERY;
            // A little hop off the flames rather than a knock across the room.
            let v = vec2(f.vel.x * 0.5, -90.0);
            let dir = if f.vel.x < 0.0 { -1.0 } else { 1.0 };
            self.damage(fire.owner, j, FIRE_DAMAGE, v, 0, dir, Source::Fire, false);
        }
    }

    /// The jet: waits out the radio call, then crosses once, dropping its bombs as it reaches
    /// each point.
    fn step_plane(&mut self, p: &mut Plane) {
        if p.delay > 0 {
            p.delay -= 1;
            if p.delay == 0 {
                self.events.push(Event::JetPass { dir: p.dir });
            }
            return;
        }
        p.pos.x += p.dir * PLANE_SPEED * DT;
        while p.drops.first().is_some_and(|&x| (p.pos.x - x) * p.dir >= 0.0) {
            p.drops.remove(0);
            let pos = p.pos + vec2(0.0, 4.0);
            self.events.push(Event::BombDrop { pos });
            self.effects.push(Effect::AirBomb(AirBomb { pos, vel: vec2(p.dir * PLANE_SPEED * 0.3, 0.0), owner: p.owner, ticks: 10 * SECOND }));
        }
        let gone = if p.dir > 0.0 { p.pos.x > W + 40.0 } else { p.pos.x < -40.0 };
        if gone {
            p.ticks = 0;
        }
    }

    /// A falling bomb: explodes (big) on the ground, a ledge, a table or anyone it hits.
    fn step_air_bomb(&mut self, b: &mut AirBomb) {
        let prev = b.pos;
        b.vel.y += AIR_BOMB_GRAVITY * DT;
        b.pos += b.vel * DT;
        b.ticks = b.ticks.saturating_sub(1);
        let hit = Aabb::around(b.pos, 2.0);
        let on_someone = (0..N_BODIES).any(|j| self.f[j].present() && hit.overlaps(&self.f[j].body()));
        let props = props(&self.effects);
        let solid = SOLIDS.iter().chain(props.iter()).any(|s| s.contains(b.pos));
        let ledge = self.arena().platforms.iter().any(|p| b.pos.x > p.x && b.pos.x < p.x + p.w && prev.y <= p.y && b.pos.y >= p.y);
        if on_someone || solid || ledge || b.ticks == 0 {
            b.ticks = 0;
            self.explode(b.pos, Some(b.owner), crate::item::AIR_BOMB_BLAST);
        }
    }

    /// A rocket curves toward the nearest foe and blows up on whoever it touches, on a wall (or a
    /// table), or when its fuel runs out. (It flies over the one-way ledges.)
    fn step_rocket(&mut self, r: &mut Rocket) {
        r.ticks = r.ticks.saturating_sub(1);
        let target = (0..N_BODIES).filter(|&j| self.foes(r.owner, j)).map(|j| self.f[j].center()).min_by(|a, b| a.distance(r.pos).total_cmp(&b.distance(r.pos)));
        if let Some(t) = target {
            // (libm, not std: the platform's own trig differs between Windows and Linux in the last
            // bits, and matches must play out identically everywhere.)
            let angle = |v: Vec2| libm::atan2f(v.y, v.x);
            let (have, want) = (angle(r.vel), angle(t - r.pos));
            let mut turn = want - have;
            while turn > std::f32::consts::PI {
                turn -= std::f32::consts::TAU;
            }
            while turn < -std::f32::consts::PI {
                turn += std::f32::consts::TAU;
            }
            let a = have + turn.clamp(-ROCKET_TURN * DT, ROCKET_TURN * DT);
            r.vel = vec2(libm::cosf(a), libm::sinf(a)) * ROCKET_SPEED;
        }
        r.pos += r.vel * DT;
        let hit = Aabb::around(r.pos, 2.0);
        let struck = (0..N_BODIES).any(|j| self.foes(r.owner, j) && hit.overlaps(&self.f[j].body()));
        let walled = SOLIDS.iter().chain(props(&self.effects).iter()).any(|s| s.contains(r.pos));
        if struck || walled || r.ticks == 0 {
            r.ticks = 0;
            self.explode(r.pos, Some(r.owner), crate::item::ROCKET_BLAST);
        }
    }

    /// A table just stands there (in mid-air, if that's where it appeared) until it's cleared away.
    fn step_table(&mut self, t: &mut Table) {
        t.ticks -= 1;
    }

    /// A table going away takes the sushi still sitting on it (nobody's touched it).
    pub(crate) fn clear_table(&mut self, t: &Table) {
        let top = t.body();
        let on_it = Aabb::new(top.x - 1.0, top.y - 4.0, top.w + 2.0, 5.0);
        self.items.retain(|it| !(it.kind == crate::sim::ItemKind::Sushi && it.held_by.is_none() && it.owner.is_none() && on_it.contains(it.pos)));
        self.events.push(Event::Poof { pos: vec2(t.pos.x, t.pos.y - TABLE_H / 2.0) });
    }

    fn step_glue(&mut self, g: &mut Glue) {
        g.ticks = g.ticks.saturating_sub(1);
        if !g.stuck {
            self.fly_glue(g);
        } else {
            self.hold_glue(g);
        }
    }

    /// A glob in flight: sticks to the first foe it touches (anchoring them to the nearest surface)
    /// or to terrain.
    fn fly_glue(&mut self, g: &mut Glue) {
        let prev = g.pos;
        g.vel.y += GLUE_GRAVITY * DT;
        g.pos += g.vel * DT;
        let props = props(&self.effects);
        let hit = Aabb::around(g.pos, 2.0);
        let victim = (0..N_BODIES).find(|&j| self.foes(g.owner, j) && hit.overlaps(&self.f[j].body()));
        if let Some(j) = victim.filter(|_| !self.protected()) {
            let c = self.f[j].center();
            g.pos = nearest_surface(self.arena(), &props, c);
            g.stuck = true;
            g.held = vec![j as u8];
            g.ticks = GLUE_HOLD;
            self.events.push(Event::Glued { pos: c });
            self.damage(g.owner, j, GLUE_DAMAGE, Vec2::ZERO, 0, g.vel.x.signum(), Source::Special, false);
            return;
        }
        let in_solid = SOLIDS.iter().chain(props.iter()).any(|s| s.contains(g.pos));
        let on_ledge = g.vel.y > 0.0 && self.arena().platforms.iter().any(|p| g.pos.x > p.x && g.pos.x < p.x + p.w && prev.y <= p.y && g.pos.y >= p.y);
        if in_solid || on_ledge {
            g.pos = nearest_surface(self.arena(), &props, prev);
            g.stuck = true;
            g.ticks = GLUE_HOLD;
            self.events.push(Event::Glued { pos: g.pos });
        }
    }

    /// Stuck glue: holds its catches on elastic strands (slowed right down; strain too far and the
    /// strand snaps), and catches anyone who blunders into the blob or a strand.
    fn hold_glue(&mut self, g: &mut Glue) {
        let anchor = g.pos;
        for j in 0..N_BODIES {
            if g.held.contains(&(j as u8)) || !self.foes(g.owner, j) {
                continue;
            }
            let f = &self.f[j];
            let blob = f.body().overlaps(&Aabb::around(anchor, GLUE_CATCH));
            let strand = g.held.iter().any(|&h| segment_distance(f.center(), anchor, self.f[h as usize].center()) < f.half_w() + 1.0);
            if blob || strand {
                g.held.push(j as u8);
                self.events.push(Event::Glued { pos: f.center() });
            }
        }
        g.held.retain(|&h| {
            let f = &mut self.f[h as usize];
            let d = f.center() - anchor;
            let dist = d.length();
            if !f.present() || dist > TETHER_BREAK {
                return false;
            }
            f.slowed = f.slowed.max(2);
            if dist > TETHER_REST {
                f.vel -= d / dist * TETHER_SPRING * (dist - TETHER_REST) * DT;
            }
            true
        });
    }
}

/// Something a special leaves in the arena. Each kind is a variant: its step (in
/// `step_effects`), how the bots perceive it (`threat_pos`), and how it's drawn (the render
/// crate) are all exhaustive matches, so the compiler lists what a new one needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Effect {
    /// Sam's glue: a glob in flight, or stuck somewhere holding people.
    Glue(Glue),
    /// One of Tyler's dogs or Corwin's cats.
    Pet(Pet),
    /// Justin's sushi table.
    Table(Table),
    /// Ryan's chain.
    Chain(Chain),
    /// Robert's lightning.
    Lightning(Lightning),
    /// Kyle's organize, while it holds (drawn on him, so it's clear who did it).
    Order(Order),
    /// A homing rocket (from a rocket launcher, not a special).
    Rocket(Rocket),
    /// Burning ground, from a Molotov.
    Fire(Fire),
    /// An air strike's jet, and a bomb it dropped.
    Plane(Plane),
    AirBomb(AirBomb),
}

impl Effect {
    /// Who made it (it never harms them, and they don't see it as a threat).
    pub fn owner(&self) -> usize {
        match self {
            Effect::Glue(g) => g.owner,
            Effect::Pet(p) => p.owner,
            Effect::Table(t) => t.owner,
            Effect::Chain(c) => c.owner,
            Effect::Lightning(l) => l.owner,
            Effect::Order(o) => o.owner,
            Effect::Rocket(r) => r.owner,
            Effect::Fire(f) => f.owner,
            Effect::Plane(p) => p.owner,
            Effect::AirBomb(b) => b.owner,
        }
    }

    /// Where the bots see it as a threat (None: harmless, like a table).
    pub fn threat_pos(&self) -> Option<Vec2> {
        match self {
            Effect::Glue(g) => Some(g.pos),
            Effect::Pet(p) => Some(p.center()),
            Effect::Table(_) => None,
            Effect::Chain(c) => Some(c.tip),
            Effect::Lightning(_) => None,
            Effect::Order(_) => None,
            Effect::Rocket(r) => Some(r.pos),
            Effect::Fire(f) => Some(f.at),
            Effect::Plane(_) => None,
            Effect::AirBomb(b) => Some(b.pos),
        }
    }

    /// Its position (a pet's feet, a table's bottom centre).
    pub fn pos(&self) -> Vec2 {
        match self {
            Effect::Glue(g) => g.pos,
            Effect::Pet(p) => p.pos,
            Effect::Table(t) => t.pos,
            Effect::Chain(c) => c.tip,
            Effect::Lightning(l) => l.from,
            Effect::Order(o) => o.at,
            Effect::Rocket(r) => r.pos,
            Effect::Fire(f) => f.at,
            Effect::Plane(p) => p.pos,
            Effect::AirBomb(b) => b.pos,
        }
    }

    /// Its position, for smoothing it between snapshots on the client.
    pub fn pos_mut(&mut self) -> &mut Vec2 {
        match self {
            Effect::Glue(g) => &mut g.pos,
            Effect::Pet(p) => &mut p.pos,
            Effect::Table(t) => &mut t.pos,
            Effect::Chain(c) => &mut c.tip,
            Effect::Lightning(l) => &mut l.from,
            Effect::Order(o) => &mut o.at,
            Effect::Rocket(r) => &mut r.pos,
            Effect::Fire(f) => &mut f.at,
            Effect::Plane(p) => &mut p.pos,
            Effect::AirBomb(b) => &mut b.pos,
        }
    }

    fn done(&self) -> bool {
        match self {
            Effect::Glue(g) => g.ticks == 0,
            Effect::Pet(p) => p.ticks == 0,
            Effect::Table(t) => t.ticks == 0,
            Effect::Chain(c) => c.ticks == 0,
            Effect::Lightning(l) => l.ticks == 0,
            Effect::Order(o) => o.ticks == 0,
            Effect::Rocket(r) => r.ticks == 0,
            Effect::Fire(f) => f.ticks == 0,
            Effect::Plane(p) => p.ticks == 0,
            Effect::AirBomb(b) => b.ticks == 0,
        }
    }
}

/// One tick of a pet running the floors (landing on ledges and tables). With `hop` it leaps for a
/// ledge just above and ahead, if there is one. With a `goal` (a cat after sushi) it heads there
/// instead, hops up to it if it's on a table, and sits once it's arrived.
fn run_pet(dog: &mut Pet, arena: &Arena, props: &[Aabb], goal: Option<Vec2>, hop: bool) {
    let (half_w, h) = dog.breed.size();
    let grounded = dog.vy == 0.0;
    dog.eating = false;
    // A leap up onto the nearest ledge just above and ahead (if there is one in reach).
    if hop && grounded {
        let ahead = |p: &&Aabb| {
            let (near, far) = if dog.dir > 0.0 { (dog.pos.x, dog.pos.x + PET_HOP_AHEAD) } else { (dog.pos.x - PET_HOP_AHEAD, dog.pos.x) };
            p.x < far && p.x + p.w > near
        };
        let above = |p: &&Aabb| p.y < dog.pos.y - 4.0 && dog.pos.y - p.y <= PET_HOP_HEIGHT;
        if let Some(p) = arena.platforms.iter().chain(props).filter(ahead).filter(above).max_by(|a, b| a.y.total_cmp(&b.y)) {
            dog.vy = -libm::sqrtf(2.0 * 640.0 * (dog.pos.y - p.y + 6.0));
        }
    }
    let step = match goal {
        None => dog.dir * dog.speed * DT,
        Some(g) => {
            let dx = g.x - dog.pos.x;
            if dx.abs() > 1.0 {
                dog.dir = dx.signum();
            }
            // Up on a table above: hop.
            if grounded && g.y < dog.pos.y - 4.0 && dx.abs() < 14.0 {
                dog.vy = -libm::sqrtf(2.0 * 640.0 * (dog.pos.y - g.y + 4.0));
            }
            dog.eating = dx.abs() <= 1.0 && (g.y - dog.pos.y).abs() < 6.0;
            dx.clamp(-dog.speed * DT, dog.speed * DT)
        }
    };
    dog.pos.x += step;
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
        for p in arena.platforms.iter().chain(props) {
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
                        assert!(m.tick > m.config.countdown + m.config.spawn_grace, "a bot used its special during the protected start");
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
    fn justins_table_stays_in_the_air_where_it_appears_with_sushi_on_top() {
        let z = zero_brain();
        let mut m = justin_and(100.0, 300.0);
        step_idle(&mut m, &z, true);
        let at = table(&m).pos;
        assert!(at.x > 100.0 + TABLE_W / 2.0, "in front of him");
        assert!(at.y < crate::sim::FLOOR_Y - 10.0, "up in the air");
        for _ in 0..60 {
            step_idle(&mut m, &z, false);
        }
        let t = table(&m);
        assert_eq!(t.pos, at, "it hasn't moved");
        let sushi: Vec<_> = m.items.iter().filter(|it| it.kind == crate::sim::ItemKind::Sushi).collect();
        assert_eq!(sushi.len(), SUSHI_PER_TABLE);
        assert!(sushi.iter().all(|it| (it.pos.y + crate::sim::ITEM_R - t.body().y).abs() < 0.5), "sitting on top");
    }

    #[test]
    fn the_floating_table_is_a_platform_and_never_pins_anyone() {
        let z = zero_brain();
        // The fighter standing right where the table appears: under it or in it, they walk away.
        let make = || {
            let mut m = justin_and(100.0, 100.0 + 4.0 + 3.0 + TABLE_W / 2.0);
            step_idle(&mut m, &z, true);
            m
        };
        let mut m = make();
        let t = table(&m);
        let x0 = m.f[1].pos.x;
        for _ in 0..60 {
            m.step_with([&z; N_FIGHTERS], Some((1, Action { mv: 1.0, ..Action::default() })));
        }
        assert!(m.f[1].pos.x > x0 + 20.0, "walked away");
        // Jumping up from underneath goes through it and lands on top.
        let mut m = make();
        for _ in 0..12 {
            m.step_with([&z; N_FIGHTERS], Some((1, Action { jump: true, ..Action::default() })));
        }
        for _ in 0..60 {
            m.step_with([&z; N_FIGHTERS], Some((1, Action::default())));
        }
        assert!(m.f[1].grounded && m.f[1].pos.y == t.body().y, "standing on the table: {:?}", m.f[1].pos);
    }

    #[test]
    fn facing_a_wall_the_table_appears_over_justin() {
        let z = zero_brain();
        let mut m = justin_and(16.0, 300.0);
        m.f[0].facing = -1.0;
        step_idle(&mut m, &z, true);
        let t = table(&m);
        assert!(t.body().y + t.body().h <= m.f[0].body().y, "over his head");
        for _ in 0..60 {
            m.step_with([&z; N_FIGHTERS], Some((0, Action { mv: 1.0, ..Action::default() })));
        }
        assert!(m.f[0].pos.x > 40.0, "walked out from under it");
    }

    #[test]
    fn corwins_cats_drop_everything_for_sushi() {
        let z = zero_brain();
        let mut m = staged([Character::Corwin, Character::Classic, Character::Classic, Character::Classic], [40.0, 300.0, 380.0, 440.0]);
        m.f[0].facing = 1.0;
        // Sushi on the floor between the cats and the fighters.
        m.items.push(crate::sim::Item::new(crate::sim::ItemKind::Sushi, vec2(120.0, crate::sim::FLOOR_Y - crate::sim::ITEM_R)));
        step_idle(&mut m, &z, true);
        for _ in 0..CAT_TICKS - 5 {
            step_idle(&mut m, &z, false);
        }
        let cats: Vec<Pet> = m.effects.iter().filter_map(|e| if let Effect::Pet(p) = e { Some(p.clone()) } else { None }).collect();
        assert_eq!(cats.len(), CATS);
        assert!(cats.iter().all(|c| (c.pos.x - 120.0).abs() < 8.0), "all crowded round the sushi");
        assert!(cats.iter().filter(|c| c.eating).count() > CATS / 2, "eating");
        assert_eq!(m.f[1].hp, MAX_HP, "and nobody got scratched");
    }

    #[test]
    fn austin_pops_up_where_the_cursor_is() {
        let z = zero_brain();
        let mut m = staged([Character::Austin, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        let a = Action { special: true, cursor: Some(vec2(250.0, 150.0)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, a)));
        let f = &m.f[0];
        assert!((f.pos.x - 250.0).abs() < 0.5 && (f.center().y - 150.0).abs() < 4.0, "at the cursor: {:?}", f.pos);
        // Into the floor: stood on it instead.
        let mut m = staged([Character::Austin, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        let a = Action { special: true, cursor: Some(vec2(200.0, 268.0)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, a)));
        assert_eq!(m.f[0].pos.y, crate::sim::FLOOR_Y);
    }

    #[test]
    fn thrown_sushi_splats_and_is_gone() {
        let z = zero_brain();
        let mut m = justin_and(100.0, 165.0);
        step_idle(&mut m, &z, true);
        // Justin takes a piece off his table.
        let k = m.items.iter().position(|it| it.kind == crate::sim::ItemKind::Sushi).unwrap();
        m.items[k].held_by = Some(0);
        m.f[0].holding = Some(crate::sim::ItemKind::Sushi);
        for _ in 0..30 {
            step_idle(&mut m, &z, false);
        }
        // At the fighter to his right.
        let throw = Action { throw: true, target: Some(vec2(1.0, -0.25)), ..Action::default() };
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

    #[test]
    fn roberts_lightning_hits_everyone_hardest_up_close() {
        let z = zero_brain();
        let mut m = staged([Character::Robert, Character::Classic, Character::Classic, Character::Classic], [100.0, 130.0, 250.0, 460.0]);
        step_idle(&mut m, &z, true);
        let lost: Vec<f32> = (1..N_FIGHTERS).map(|j| MAX_HP - m.f[j].hp).collect();
        assert!(lost[0] > lost[1] && lost[1] > lost[2], "closer hurts more: {lost:?}");
        assert!(lost[0] > 25.0 && lost[2] >= LIGHTNING_MIN, "{lost:?}");
        assert_eq!(m.f[0].hp, MAX_HP);
        assert!(matches!(m.effects.as_slice(), [Effect::Lightning(l)] if l.to.len() == 3), "a bolt to each");
    }

    #[test]
    fn kyle_pins_everyone_else_where_they_stand_for_three_seconds() {
        let z = zero_brain();
        let mut m = staged([Character::Kyle, Character::Classic, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
        m.f[2].pos.y = 150.0;
        m.f[2].grounded = false;
        step_idle(&mut m, &z, true);
        let (x1, y2) = (m.f[1].pos.x, m.f[2].pos.y);
        let run = Action { mv: 1.0, run: true, jump: true, ..Action::default() };
        let brains = [&z; N_FIGHTERS];
        for _ in 0..PIN_TICKS - 1 {
            let mut humans = [None; N_FIGHTERS];
            humans[0] = Some(run);
            humans[1] = Some(run);
            m.step_humans(brains, humans);
        }
        assert_eq!(m.f[1].pos.x, x1, "pinned: can't run");
        assert_eq!(m.f[2].pos.y, y2, "pinned in mid-air");
        assert!(m.f[0].pos.x > 150.0, "Kyle moves freely");
        assert_eq!(m.f[0].pinned, 0, "Kyle isn't pinned by his own organize");
        for _ in 0..30 {
            let mut humans = [None; N_FIGHTERS];
            humans[1] = Some(run);
            m.step_humans(brains, humans);
        }
        assert!(m.f[1].pos.x > x1 + 5.0, "free again after 3s");
        assert!(m.f[2].pos.y > y2, "and falling");
    }

    #[test]
    fn tyler_runs_faster_while_his_dogs_are_out() {
        let z = zero_brain();
        let top_speed = |special: bool| {
            let mut m = staged([Character::Tyler, Character::Classic, Character::Classic, Character::Classic], [40.0, 300.0, 380.0, 440.0]);
            m.f[0].facing = 1.0;
            step_idle(&mut m, &z, special);
            let run = Action { mv: 1.0, run: true, ..Action::default() };
            for _ in 0..60 {
                m.step_with([&z; N_FIGHTERS], Some((0, run)));
            }
            m.f[0].vel.x
        };
        let (normal, hasted) = (top_speed(false), top_speed(true));
        assert!(hasted > normal * 1.3, "{normal} -> {hasted}");
    }

    fn glue_of(m: &Match) -> Option<Glue> {
        m.effects.iter().find_map(|e| if let Effect::Glue(g) = e { Some(g.clone()) } else { None })
    }

    #[test]
    fn sams_glue_tethers_whoever_it_hits_and_wears_off() {
        let z = zero_brain();
        let mut m = staged([Character::Sam, Character::Classic, Character::Classic, Character::Classic], [100.0, 180.0, 380.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        let mut glued = false;
        for _ in 0..60 {
            step_idle(&mut m, &z, false);
            glued |= m.events.iter().any(|e| matches!(e, Event::Glued { .. }));
            if glue_of(&m).is_some_and(|g| g.stuck) {
                break;
            }
        }
        let g = glue_of(&m).expect("glue");
        assert!(glued && g.stuck && g.held == vec![1], "stuck to the fighter it hit: {g:?}");
        assert!(m.f[1].hp < MAX_HP);
        // Trying to run away: slowed right down, held near the anchor.
        let x0 = m.f[1].pos.x;
        let run = Action { mv: 1.0, run: true, ..Action::default() };
        for _ in 0..SECOND / 2 {
            m.step_with([&z; N_FIGHTERS], Some((1, run)));
        }
        let moved = m.f[1].pos.x - x0;
        assert!(moved < 25.0, "barely got anywhere: {moved}px in half a second");
        assert!(m.f[1].slowed > 0, "still stuck");
        // It wears off.
        for _ in 0..GLUE_HOLD {
            step_idle(&mut m, &z, false);
        }
        assert!(glue_of(&m).is_none());
        for _ in 0..30 {
            m.step_with([&z; N_FIGHTERS], Some((1, run)));
        }
        assert!(m.f[1].vel.x > 100.0, "running freely again: {}", m.f[1].vel.x);
    }

    #[test]
    fn glue_on_the_floor_catches_whoever_runs_into_it_and_a_hard_pull_snaps_it() {
        let z = zero_brain();
        let mut m = staged([Character::Sam, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        m.effects.push(Effect::Glue(Glue { pos: vec2(200.0, crate::sim::FLOOR_Y), vel: Vec2::ZERO, owner: 0, stuck: true, held: Vec::new(), ticks: GLUE_HOLD }));
        let left = Action { mv: -1.0, run: true, ..Action::default() };
        for _ in 0..90 {
            m.step_with([&z; N_FIGHTERS], Some((1, left)));
        }
        assert_eq!(glue_of(&m).unwrap().held, vec![1], "ran into it and got stuck");
        assert!(m.f[1].pos.x > 150.0, "and held there: x {}", m.f[1].pos.x);
        // A big knock (say, a grenade) yanks them past the strand's limit: it snaps.
        m.f[1].vel = vec2(-600.0, -200.0);
        for _ in 0..10 {
            step_idle(&mut m, &z, false);
        }
        assert!(glue_of(&m).unwrap().held.is_empty(), "the strand snapped");
    }

    #[test]
    fn a_glob_that_misses_sticks_where_it_lands() {
        let z = zero_brain();
        let mut m = staged([Character::Sam, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        m.f[0].facing = -1.0;
        step_idle(&mut m, &z, true);
        for _ in 0..GLUE_FLIGHT {
            step_idle(&mut m, &z, false);
            if glue_of(&m).is_some_and(|g| g.stuck) {
                break;
            }
        }
        let g = glue_of(&m).expect("still there");
        assert!(g.stuck && g.held.is_empty(), "stuck to the terrain: {g:?}");
    }

    #[test]
    fn jake_hops_between_phones_with_left_and_right() {
        use crate::sim::{ItemKind, Item};
        let z = zero_brain();
        let mut m = staged([Character::Jake, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        // Two phones already lying about from before.
        m.items.push(Item::new(ItemKind::Phone, vec2(50.0, crate::sim::FLOOR_Y - crate::sim::ITEM_R)));
        m.items.push(Item::new(ItemKind::Phone, vec2(250.0, crate::sim::FLOOR_Y - crate::sim::ITEM_R)));
        step_idle(&mut m, &z, true);
        assert_eq!(m.items.iter().filter(|it| it.kind == ItemKind::Phone).count(), 3);
        for _ in 0..20 {
            step_idle(&mut m, &z, false);
        }
        let right = Action { mv: 1.0, ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, right)));
        assert!((m.f[0].pos.x - 250.0).abs() < 1.0, "hopped right to the far phone: {}", m.f[0].pos.x);
        // Held: no more hopping. Released and pressed again: wraps round to the leftmost.
        m.step_with([&z; N_FIGHTERS], Some((0, right)));
        assert!((m.f[0].pos.x - 250.0).abs() < 1.0);
        step_idle(&mut m, &z, false);
        m.step_with([&z; N_FIGHTERS], Some((0, right)));
        assert!((m.f[0].pos.x - 50.0).abs() < 1.0, "wrapped round: {}", m.f[0].pos.x);
        for _ in 0..PHONE_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert_eq!(m.f[0].digital, 0);
        assert!((m.f[0].pos.x - 50.0).abs() < 2.0, "came out of the phone he chose");
        assert_eq!(m.items.iter().filter(|it| it.kind == ItemKind::Phone).count(), 3, "every phone stays");
    }

    #[test]
    fn vid_rides_the_opponent_nearest_the_cursor_then_steps_out() {
        let z = zero_brain();
        let mut m = staged([Character::Vid, Character::Classic, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
        // The cursor is nearest the fighter at 300.
        let a = Action { special: true, cursor: Some(vec2(290.0, 240.0)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, a)));
        assert_eq!(m.f[0].host, Some(2));
        assert!(m.f[2].ridden > 0 && !m.f[0].present());
        // Inside, Vid can't be hurt; his input drives their body.
        m.damage(1, 0, 50.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP);
        let x2 = m.f[2].pos.x;
        let left = Action { mv: -1.0, run: true, ..Action::default() };
        for _ in 0..SECOND {
            m.step_with([&z; N_FIGHTERS], Some((0, left)));
        }
        assert!(m.f[2].pos.x < x2 - 40.0, "Vid ran their body left: {}", m.f[2].pos.x);
        assert!((m.f[0].pos.x - m.f[2].pos.x).abs() < 0.5, "he goes where it goes");
        for _ in 0..RIDE_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert!(m.f[0].present() && m.f[0].host.is_none() && m.f[2].ridden == 0, "out again");
        let gap = (m.f[0].pos.x - m.f[2].pos.x).abs();
        assert!(gap > 5.0 && gap < 15.0, "beside them: {gap}");
    }

    #[test]
    fn dogs_and_cats_leap_up_onto_ledges_now_and_then() {
        let z = zero_brain();
        for who in [Character::Tyler, Character::Corwin] {
            let mut m = staged([who, Character::Classic, Character::Classic, Character::Classic], [60.0, 300.0, 380.0, 440.0]);
            m.f[0].facing = 1.0;
            step_idle(&mut m, &z, true);
            let mut up = false;
            for _ in 0..HOUND_TICKS.min(CAT_TICKS) - 1 {
                step_idle(&mut m, &z, false);
                up |= m.effects.iter().any(|e| matches!(e, Effect::Pet(p) if p.vy == 0.0 && p.pos.y < crate::sim::FLOOR_Y - 10.0));
            }
            assert!(up, "{who:?}'s pets never got up onto a ledge");
        }
    }
}
