//! Characters: every fighter shares the same base body (move, jump, melee, grab, throw, HP);
//! a character adds one special move on a cooldown. Each special is a `Special` strategy, so a
//! new character is a new entry in `Character` plus (optionally) a new `Special` impl.
//!
//! All tuning lives in the constants at the top of this file.

use crate::sim::{Aabb, Arena, Event, Fighter, Match, Source, DT, N_BODIES, N_FIGHTERS, W};
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
// pick things up and throw them like anyone, and have 30% of a fighter's health.
const MINI_COOLDOWN: u32 = 30 * SECOND;
const MINIS_PER_USE: usize = 2;
/// How long a mini lasts (if nothing hits it first; and it goes when Seth does).
const MINI_LIFE: u32 = 15 * SECOND;
pub const MINI_SCALE: f32 = 0.7;
/// A mini's health (a full fighter has MAX_HP, 100).
pub const MINI_HP: f32 = 30.0;

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
/// The highest a pet leaps up after something it's going for (a cat to sushi on a table, a
/// spider to someone on a ledge).
const PET_LEAP_MAX: f32 = 64.0;

// Carrie — Bug Report: she opens her laptop and bugs pour out every which way: spiders that run
// along the floors and horseflies that fly. After a moment's scatter each one hunts down the
// nearest foe, bites them a couple of times, and is done.
const BUGS_COOLDOWN: u32 = 30 * SECOND;
const BUGS: usize = 14;
const BUG_TICKS: u32 = 7 * SECOND;
/// How long they scatter outward before they start hunting.
const BUG_SCATTER: u32 = SECOND * 2 / 5;
/// Both well under a fighter's full run (150): they catch you standing or walking, but you can
/// outrun them.
const SPIDER_SPEED: f32 = 105.0;
/// How hard a spider is flung out of the laptop (up, or down, as the case may be).
const SPIDER_LAUNCH: f32 = 230.0;
const FLY_SPEED: f32 = 85.0;
const FLY_LAUNCH: f32 = 170.0;
/// How sharply a fly can change course (px/s²), and how much it buzzes about on the way.
const FLY_ACCEL: f32 = 420.0;
const FLY_WOBBLE: f32 = 35.0;
const BUG_BITE: f32 = 3.0;
const BUG_BITES: u8 = 2;
const BUG_BITE_EVERY: u32 = SECOND / 2;
/// How long she's seen with the laptop open.
pub const LAPTOP_TICKS: u32 = SECOND;

// Matt — Skate Strike: he drops a skateboard and shoots off along it, fast and far, the way he
// faces, hitting everyone he goes through. He can do it frozen by Michelle or ridden by the Chosen One, and it
// frees him (only him: whoever he hits stays however they were).
const SKATE_COOLDOWN: u32 = 8 * SECOND;
pub const SKATE_SPEED: f32 = 520.0;
/// How long the run lasts (so about 200px).
const SKATE_TICKS: u32 = SECOND * 2 / 5;
const SKATE_DAMAGE: f32 = 14.0;

// Zack — Twin Tornadoes: two tornadoes spin up either side of him, each a column from the floor
// to the ceiling, and wander off outward for a few seconds. Each pulls everyone (Zack too)
// and every loose item toward it like gravity, at any height; get into the column and you're
// carried up it and flung out near the top, the way it spins. Rocks, spears and swords whipped
// round in it hurt whoever they hit (except Zack's side).
const TORNADO_COOLDOWN: u32 = 25 * SECOND;
pub const TORNADO_TICKS: u32 = 5 * SECOND;
/// Where they spin up (either side of him) and how fast they wander outward (px/s).
const TORNADO_SPAWN: f32 = 45.0;
const TORNADO_DRIFT: f32 = 25.0;
/// How far either side of the column the pull reaches, and how hard it drags you at its
/// strongest (px/s, whatever you're doing: a full run gets you away).
pub const TORNADO_R: f32 = 80.0;
const TORNADO_PULL: f32 = 125.0;
/// How close to the column's middle you have to get to be carried up it.
const FUNNEL_HALF: f32 = 8.0;
/// How fast it carries you up, how near the ceiling it flings you out, and how hard.
const TORNADO_LIFT: f32 = 90.0;
const TORNADO_TOP: f32 = 40.0;
const TORNADO_FLING: f32 = 260.0;
/// Loose items within this of the vortex low in the column (see `Tornado::core`) are pulled in
/// (px/s²) and spun round it, up to a top speed.
const ITEM_VORTEX_R: f32 = 100.0;
const ITEM_PULL: f32 = 520.0;
const ITEM_SWIRL: f32 = 620.0;
const ITEM_WHIRL_MAX: f32 = 230.0;
/// What something whipped round by a tornado does, compared with being thrown.
pub const WHIRLED_HIT: f32 = 0.5;

// Kaleb — Golden Arrow: a golden arrow appears over his head for a few seconds. Press again and it
// flies off after wherever he aims, and keeps chasing his aim (the cursor) for as long as it
// lasts, going through every foe it meets. Press again to call it back over his head (and again
// to send it off once more).
const ARROW_COOLDOWN: u32 = 18 * SECOND;
pub const ARROW_TICKS: u32 = 5 * SECOND;
const ARROW_SPEED: f32 = 520.0;
const ARROW_DAMAGE: f32 = 8.0;
/// It hits the same person at most this often (it can be held on someone).
const ARROW_HIT_EVERY: u32 = SECOND * 3 / 4;
/// Where it hovers: this far above his feet.
const ARROW_HOVER: f32 = 22.0;
/// How close it has to pass to hit someone (half-size of its hit box).
const ARROW_REACH: f32 = 3.0;
/// A bot fires it at someone within this.
const ARROW_BOT_RANGE: f32 = 220.0;

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

// Shawn — The Chosen One: he goes inside the opponent nearest the cursor and drives their body
// for a few seconds (they're drawn as falling code), then steps back out.
const CHOSEN_COOLDOWN: u32 = 20 * SECOND;
pub const RIDE_TICKS: u32 = 3 * SECOND;

// Vid — Story Time: he sits down with a book from his dimensional bag of books and reads, eyes
// glowing. For a few seconds a shield stops every attack on him (he can't move or fight while
// he reads), and a few books fly out of the bag to circle him and dart at anyone close by.
const READ_COOLDOWN: u32 = 20 * SECOND;
pub const READ_TICKS: u32 = 4 * SECOND;
const BOOKS: usize = 3;
/// How far from him the books circle, how far off they go after someone, and how fast they fly.
pub const BOOK_ORBIT: f32 = 14.0;
const BOOK_RANGE: f32 = 90.0;
const BOOK_SPEED: f32 = 260.0;
const BOOK_DAMAGE: f32 = 6.0;
/// After a hit, a book goes back round him for this long before it goes after anyone again.
const BOOK_REST: u32 = SECOND * 3 / 5;

// Robert — Forked Lightning: a bolt forks out to every foe in the arena at once (no need to see
// them), hurting most up close and least far away.
const LIGHTNING_COOLDOWN: u32 = 25 * SECOND;
const LIGHTNING_MAX: f32 = 30.0;
const LIGHTNING_MIN: f32 = 5.0;
/// From LIGHTNING_MAX at point blank down to LIGHTNING_MIN at this distance and beyond.
const LIGHTNING_REACH: f32 = 300.0;
/// How long the bolts stay on screen.
pub const LIGHTNING_TICKS: u32 = 20;

// Michelle — Eye of the Storm: everyone else is frozen exactly where they are for a few seconds
// (in mid-air too), a great eye opening over her.
const EYE_COOLDOWN: u32 = 20 * SECOND;
pub const PIN_TICKS: u32 = 3 * SECOND;

// Kyle — Backlog: for a few seconds his swings and throws don't happen; they pile up in an inbox
// over his head (a thrown item leaves his hand, so he can pick up another), and when it ends they
// all go at once: every throw the way it was aimed, every swing in one big punch.
const BACKLOG_COOLDOWN: u32 = 15 * SECOND;
pub const BACKLOG_TICKS: u32 = 3 * SECOND;
/// The punch his piled-up swings make: this much a swing (up to so many swings), reaching this
/// much further than a fist.
const BACKLOG_PUNCH: f32 = 8.0;
const BACKLOG_PUNCHES: u8 = 6;
const BACKLOG_REACH: f32 = 8.0;

// Doug — Judo Throw: lunges forward into a grab, and the first foe he gets hold of is thrown like
// a ragdoll; wherever they land they slam down (taking the damage) and lie there a second,
// getting back up.
const JUDO_COOLDOWN: u32 = 7 * SECOND;
/// Lunging at nobody costs only this much cooldown.
const JUDO_WHIFF_COOLDOWN: u32 = SECOND;
/// How far past the front of his body his hands reach (all through the lunge).
const JUDO_REACH: f32 = 12.0;
/// The lunge: a dash this many ticks long (about 38px), so he can grab someone up to about
/// 50px ahead.
const LUNGE_TICKS: u32 = 6;
/// The throw: up and over, the way he faces (px/s).
const JUDO_THROW: Vec2 = vec2(240.0, -230.0);
const JUDO_DAMAGE: f32 = 12.0;
/// Anyone the thrown body flies into is knocked down too (and gets up just as slowly), taking this
/// as they hit the ground.
const JUDO_BOWL_DAMAGE: f32 = 6.0;
/// Down on the ground after landing, getting up.
pub const JUDO_DOWN: u32 = SECOND;
/// If they somehow never land (pinned in mid-air), they count as landed after this long.
const JUDO_MAX_FLIGHT: u32 = 3 * SECOND;

// Travis — Leg Day: he bulks up in a weighted vest for a few seconds, and every step he takes
// sends a shockwave along the ground that hurts anyone standing near him (landing from a jump,
// a bigger one). Jump to dodge it: it only hits people on the ground.
const BULK_COOLDOWN: u32 = 20 * SECOND;
pub const BULK_TICKS: u32 = 5 * SECOND;
/// A step every this many px of running.
const STRIDE: f32 = 26.0;
/// A step's reach along the ground (either side of him), its damage, and the same for a landing.
pub const QUAKE_R: f32 = 28.0;
const QUAKE_DAMAGE: f32 = 4.0;
pub const QUAKE_BIG_R: f32 = 42.0;
const QUAKE_BIG_DAMAGE: f32 = 8.0;
/// How far above or below his feet someone can stand and still feel it.
const QUAKE_LEVEL: f32 = 8.0;
/// One person is hurt by his steps at most this often.
const QUAKE_EVERY: u32 = SECOND * 2 / 5;
/// How long a shockwave ripples out (for drawing).
pub const QUAKE_TICKS: u32 = 15;

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
        Doug,
        Travis,
        Carrie,
        Matt,
        Zack,
        Kaleb,
        Michelle,
        Shawn,
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
    /// Something they always are, not something they do (only some characters have one; its
    /// name is `passive.<id>` in the text files).
    pub passive: Option<Passive>,
}

/// A character's passive: always on, nothing to press.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Passive {
    /// Michelle's Resting Zen Face: nobody pushes her around. Hits still hurt her, but never
    /// knock her back or stun her, and she can't be pinned or frozen, glued, grabbed and thrown,
    /// knocked down, hooked by a chain, carried off by a tornado or ridden by the Chosen One.
    RestingZenFace,
    /// Ryan's Hobbyist: no special of his own. Each match, as it starts, he picks one up at random
    /// from every special in the game (`ALL_SPECIALS`, Yanking Your Chain the likeliest) and has it
    /// for the whole match (`Fighter::hobby`).
    Hobbyist,
}

impl Passive {
    /// A stable id; players see the text `passive.<id>`.
    pub fn id(self) -> &'static str {
        match self {
            Passive::RestingZenFace => "resting_zen_face",
            Passive::Hobbyist => "hobbyist",
        }
    }
}

/// Every special in the game, whether or not a character has it now: what Ryan's Hobbyist draws
/// from (a new special goes here too; a test checks every character's is).
pub static ALL_SPECIALS: [&dyn Special; 19] = [
    &GlueGun, &PopUp, &Hounds, &MiniMe, &CatHerder, &SushiTable, &YankChain, &IntoThePhone, &ForkedLightning, &EyeOfTheStorm, &Backlog, &StoryTime, &ChosenOne, &JudoThrow, &LegDay,
    &BugReport, &SkateStrike, &TwinTornadoes, &GoldenArrow,
];

/// How likely Hobbyist is to pick each special, relative to the rest: Ryan's old chain the
/// likeliest.
fn hobby_weight(s: &dyn Special) -> u32 {
    if s.id() == "yank_chain" { 3 } else { 1 }
}

impl Match {
    /// At the start of a match, everyone with Hobbyist picks up their special for it.
    pub(crate) fn pick_hobbies(&mut self) {
        let total: u32 = ALL_SPECIALS.iter().map(|s| hobby_weight(*s)).sum();
        for i in 0..N_FIGHTERS {
            if self.f[i].absent || self.f[i].character.passive() != Some(Passive::Hobbyist) || self.f[i].hobby.is_some() {
                continue;
            }
            let mut roll = self.roll(total);
            let k = ALL_SPECIALS.iter().position(|s| {
                let w = hobby_weight(*s);
                if roll < w {
                    true
                } else {
                    roll -= w;
                    false
                }
            });
            self.f[i].hobby = k.map(|k| k as u8);
        }
    }
}

impl Character {
    pub fn def(self) -> &'static CharacterDef {
        match self {
            Character::Classic => &CharacterDef { id: "classic", special: None, passive: None },
            Character::Sam => &CharacterDef { id: "sam", special: Some(&GlueGun), passive: None },
            Character::Austin => &CharacterDef { id: "austin", special: Some(&PopUp), passive: None },
            Character::Tyler => &CharacterDef { id: "tyler", special: Some(&Hounds), passive: None },
            Character::Seth => &CharacterDef { id: "seth", special: Some(&MiniMe), passive: None },
            Character::Corwin => &CharacterDef { id: "corwin", special: Some(&CatHerder), passive: None },
            Character::Justin => &CharacterDef { id: "justin", special: Some(&SushiTable), passive: None },
            Character::Ryan => &CharacterDef { id: "ryan", special: None, passive: Some(Passive::Hobbyist) },
            Character::Jake => &CharacterDef { id: "jake", special: Some(&IntoThePhone), passive: None },
            Character::Robert => &CharacterDef { id: "robert", special: Some(&ForkedLightning), passive: None },
            Character::Kyle => &CharacterDef { id: "kyle", special: Some(&Backlog), passive: None },
            Character::Vid => &CharacterDef { id: "vid", special: Some(&StoryTime), passive: None },
            Character::Doug => &CharacterDef { id: "doug", special: Some(&JudoThrow), passive: None },
            Character::Travis => &CharacterDef { id: "travis", special: Some(&LegDay), passive: None },
            Character::Carrie => &CharacterDef { id: "carrie", special: Some(&BugReport), passive: None },
            Character::Matt => &CharacterDef { id: "matt", special: Some(&SkateStrike), passive: None },
            Character::Zack => &CharacterDef { id: "zack", special: Some(&TwinTornadoes), passive: None },
            Character::Kaleb => &CharacterDef { id: "kaleb", special: Some(&GoldenArrow), passive: None },
            Character::Michelle => &CharacterDef { id: "michelle", special: Some(&EyeOfTheStorm), passive: Some(Passive::RestingZenFace) },
            Character::Shawn => &CharacterDef { id: "shawn", special: Some(&ChosenOne), passive: None },
        }
    }

    pub fn id(self) -> &'static str {
        self.def().id
    }

    pub fn special(self) -> Option<&'static dyn Special> {
        self.def().special
    }

    pub fn passive(self) -> Option<Passive> {
        self.def().passive
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
    /// It can be used while frozen by Michelle, ridden by the Chosen One or carried up a tornado, and frees you
    /// (Matt's).
    fn breaks_free(&self) -> bool {
        false
    }
    /// Pressed again while it's cooling down: for a special that leaves something to steer
    /// (Kaleb's arrow). Most do nothing.
    fn press_again(&self, _m: &mut Match, _who: usize) {}
    /// When a bot presses it again while it's cooling down (see `press_again`).
    fn bot_presses_again(&self, _m: &Match, _who: usize) -> bool {
        false
    }
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
        m.add_effect(Effect::Glue(glob));
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
    arena
        .solids()
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
            mini.hp = MINI_HP;
            mini.scale = MINI_SCALE;
            mini.owner = Some(who);
            mini.life = Some(MINI_LIFE);
            // (Looking like whoever summoned them: Seth, or Ryan with Mini Me as his hobby.)
            mini.character = seth.character;
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
        let (pos, facing) = (m.f[who].pos, m.f[who].facing);
        for (k, &speed) in HOUNDS.iter().enumerate() {
            m.add_effect(Effect::Pet(Pet::new(Breed::Hound, pos + vec2(facing * (2.0 + 4.0 * k as f32), 0.0), facing, speed, who, HOUND_TICKS)));
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
        let (pos, facing) = (m.f[who].pos, m.f[who].facing);
        for k in 0..CATS {
            // Speeds shuffled across the pack (k * 7 mod 20), so neighbours don't run in step.
            let t = ((k * 7) % CATS) as f32 / (CATS - 1) as f32;
            let speed = CAT_SPEEDS.0 + (CAT_SPEEDS.1 - CAT_SPEEDS.0) * t;
            let x = (pos.x + facing * (1.0 + 0.6 * k as f32)).clamp(12.0, W - 12.0);
            m.add_effect(Effect::Pet(Pet::new(Breed::Cat(k as u8), vec2(x, pos.y), facing, speed, who, CAT_TICKS)));
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
        m.retain_effects(|e| !mine(e));
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
        m.add_effect(Effect::Table(t));
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
        m.add_effect(Effect::Chain(Chain { tip, dir, owner: who, out: 0.0, hooked: None, back: false, ticks: CHAIN_TICKS }));
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
        for j in targets {
            let c = m.f[j].center();
            let d = c.distance(from);
            let away = (c - from).normalize_or(vec2(0.0, -1.0));
            // (The bolt is drawn by the client, from this: nothing about it stays in the match.)
            m.events.push(Event::Zap { from, pos: c });
            m.damage(who, j, lightning_damage(d), vec2(away.x * 70.0, -70.0), 8, if away.x < 0.0 { -1.0 } else { 1.0 }, Source::Special, false);
        }
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

struct EyeOfTheStorm;

impl Special for EyeOfTheStorm {
    fn id(&self) -> &'static str {
        "eye_of_the_storm"
    }
    fn cooldown(&self) -> u32 {
        EYE_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // Only her opponents (she and anyone on her side move freely), and nobody who can't be
        // pushed around (another Michelle).
        let caught: Vec<usize> = (0..N_BODIES).filter(|&j| m.foes(who, j) && !m.f[j].unshakeable()).collect();
        for j in caught {
            m.f[j].pinned = PIN_TICKS;
            m.f[j].vel = Vec2::ZERO;
        }
        // (Its shockwave and the eye are drawn by the client, from this.)
        let at = m.f[who].center();
        // Her opponents' dogs, cats and bugs already out are frozen too (not any let out later).
        let team = m.f[who].team(who);
        for e in m.effects.iter_mut() {
            if let Effect::Pet(p) = e
                && m.f[p.owner].team(p.owner) != team
            {
                p.frozen = PIN_TICKS;
            }
        }
        m.events.push(Event::EyeOfTheStorm { who, at });
    }
    /// Someone close enough to hit while they're stuck.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].center();
        foes(m, who).any(|f| f.center().distance(me) < 50.0)
    }
}

struct Backlog;

impl Special for Backlog {
    fn id(&self) -> &'static str {
        "backlog"
    }
    fn cooldown(&self) -> u32 {
        BACKLOG_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let f = &mut m.f[who];
        f.backlog = BACKLOG_TICKS;
        f.inbox = 0;
        f.swings = 0;
        f.stash.clear();
    }
    /// Someone close enough to pile it all onto.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].center();
        foes(m, who).any(|f| f.center().distance(me) < 120.0)
    }
}

struct StoryTime;

impl Special for StoryTime {
    fn id(&self) -> &'static str {
        "story_time"
    }
    fn cooldown(&self) -> u32 {
        READ_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // He puts down whatever he was holding to read.
        if let Some(k) = m.held_item(who) {
            m.items[k].held_by = None;
            m.items[k].vel = Vec2::ZERO;
        }
        let f = &mut m.f[who];
        f.holding = None;
        f.atk_timer = 0;
        f.dash_t = 0;
        f.reading = READ_TICKS;
        let at = f.center();
        m.events.push(Event::Poof { pos: at });
        for k in 0..BOOKS {
            m.add_effect(Effect::Book(Book { pos: at, owner: who, k: k as u8, chasing: None, rest: 0, ticks: READ_TICKS }));
        }
    }
    /// Someone close, or something about to hit him.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].center();
        in_danger(m, who) || foes(m, who).any(|f| f.center().distance(me) < 60.0)
    }
}

/// Where Vid's book `k` circles him at `tick`.
pub fn book_orbit(vid: &Fighter, k: u8, tick: u32) -> Vec2 {
    let a = tick as f32 * 0.08 + k as f32 * std::f32::consts::TAU / BOOKS as f32;
    vid.center() + vec2(libm::cosf(a), libm::sinf(a) * 0.6) * BOOK_ORBIT
}

/// One of Vid's books: circling him while he reads, darting at anyone close by.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book {
    pub pos: Vec2,
    pub owner: usize,
    /// Which of his books (where it circles, and its cover).
    pub k: u8,
    /// Who it's flying at.
    pub chasing: Option<u8>,
    /// Ticks before it goes after anyone again.
    #[serde(skip)]
    pub rest: u32,
    pub ticks: u32,
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
        let target = (0..N_FIGHTERS).filter(|&j| m.foes(who, j) && m.f[j].ridden == 0 && !m.f[j].unshakeable()).min_by(|&a, &b| m.f[a].center().distance(near).total_cmp(&m.f[b].center().distance(near)));
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

/// Where Doug can grab someone: just past the front of his body, about his height.
fn judo_grip(f: &Fighter) -> Aabb {
    let b = f.body();
    let x = if f.facing > 0.0 { b.x + b.w } else { b.x - JUDO_REACH };
    Aabb::new(x, b.y + 1.0, JUDO_REACH, b.h - 2.0)
}

/// The foe Doug would grab: the nearest one in his grip.
fn judo_target(m: &Match, who: usize) -> Option<usize> {
    let me = &m.f[who];
    let grip = judo_grip(me);
    (0..N_BODIES).filter(|&j| m.foes(who, j) && !m.f[j].unshakeable() && grip.overlaps(&m.f[j].body())).min_by(|&a, &b| m.f[a].pos.distance(me.pos).total_cmp(&m.f[b].pos.distance(me.pos)))
}

/// Doug gets hold of `j`: lifted up over his shoulder, then thrown the way he faces (whatever
/// they were holding goes flying).
fn judo_throw(m: &mut Match, who: usize, j: usize) {
    if let Some(k) = m.held_item(j) {
        m.items[k].held_by = None;
        m.items[k].vel = vec2(0.0, -60.0);
    }
    let f = &mut m.f[who];
    // He stops dead (if he was lunging) to throw.
    f.dash_t = 0;
    f.vel.x *= 0.3;
    let (dir, me) = (f.facing, f.pos);
    let v = &mut m.f[j];
    v.holding = None;
    v.atk_timer = 0;
    v.dash_t = 0;
    v.pos = vec2(me.x, (me.y - 6.0).max(crate::sim::WALL + v.height() + 1.0));
    v.vel = vec2(dir * JUDO_THROW.x, JUDO_THROW.y);
    v.grounded = false;
    v.hitstun = 2;
    let at = v.center();
    m.add_effect(Effect::Judo(Judo { at, owner: who, victim: j, dir, flown: 0, down: 0, ticks: JUDO_MAX_FLIGHT + JUDO_DOWN, damage: JUDO_DAMAGE, bowls: true, bowled: 1 << j }));
}

struct GoldenArrow;

impl Special for GoldenArrow {
    fn id(&self) -> &'static str {
        "golden_arrow"
    }
    fn cooldown(&self) -> u32 {
        ARROW_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let pos = arrow_hover(&m.f[who]);
        m.events.push(Event::ArrowCall { pos });
        m.add_effect(Effect::Arrow(Arrow { pos, dir: vec2(m.f[who].facing, 0.0), owner: who, state: ArrowState::Hover, hit_cd: [0; N_BODIES], ticks: ARROW_TICKS }));
    }
    /// Someone in range to fire it at.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].center();
        foes(m, who).any(|f| f.center().distance(me) < ARROW_BOT_RANGE)
    }
    /// Over his head (or on its way back): send it off after his aim. Out chasing it: call it back.
    fn press_again(&self, m: &mut Match, who: usize) {
        let Some(Effect::Arrow(a)) = m.effects.iter_mut().find(|e| matches!(e, Effect::Arrow(a) if a.owner == who)) else { return };
        a.state = if a.state == ArrowState::Out { ArrowState::Back } else { ArrowState::Out };
        let pos = a.pos;
        m.events.push(Event::ArrowCall { pos });
    }
    /// Send it off when it's over his head and someone's in range (it then chases the nearest).
    fn bot_presses_again(&self, m: &Match, who: usize) -> bool {
        m.effects.iter().any(|e| matches!(e, Effect::Arrow(a) if a.owner == who && a.state == ArrowState::Hover)) && self.bot_wants(m, who)
    }
}

/// Where Kaleb's arrow hovers: over his head.
pub fn arrow_hover(f: &Fighter) -> Vec2 {
    vec2(f.pos.x, f.pos.y - ARROW_HOVER * f.scale)
}

/// What Kaleb's arrow is doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ArrowState {
    /// Over his head, ready to send off.
    Hover,
    /// Out, chasing wherever he aims.
    Out,
    /// Called back, on its way to him.
    Back,
}

/// Kaleb's arrow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Arrow {
    /// Its tip.
    pub pos: Vec2,
    /// Which way it points.
    pub dir: Vec2,
    pub owner: usize,
    pub state: ArrowState,
    /// Per body: ticks before it can hit them again.
    #[serde(skip)]
    pub hit_cd: [u32; N_BODIES],
    pub ticks: u32,
}

struct TwinTornadoes;

impl Special for TwinTornadoes {
    fn id(&self) -> &'static str {
        "twin_tornadoes"
    }
    fn cooldown(&self) -> u32 {
        TORNADO_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let x0 = m.f[who].pos.x;
        // One each side, wandering off outward, each spinning so it flings people further out.
        for side in [-1.0, 1.0] {
            let x = (x0 + side * TORNADO_SPAWN).clamp(crate::sim::WALL + 12.0, W - crate::sim::WALL - 12.0);
            let t = Tornado { at: vec2(x, crate::sim::FLOOR_Y), dir: side, spin: side, owner: who, ticks: TORNADO_TICKS };
            m.events.push(Event::Poof { pos: t.core() });
            m.add_effect(Effect::Tornado(t));
        }
    }
    /// Anyone (at any height) within a tornado's reach of where they'd start.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].pos;
        foes(m, who).any(|f| (f.pos.x - me.x).abs() < TORNADO_SPAWN + TORNADO_R * 0.6)
    }
}

/// One of Zack's tornadoes: a column from the floor to the ceiling, wandering slowly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tornado {
    /// The bottom of the column, on the floor (it reaches all the way up to the ceiling).
    pub at: Vec2,
    /// Which way it wanders, and which way it spins (1: flings people to the right).
    pub dir: f32,
    pub spin: f32,
    pub owner: usize,
    pub ticks: u32,
}

impl Tornado {
    /// Low in the column: the vortex loose items swirl round (and where it shows as a threat).
    pub fn core(&self) -> Vec2 {
        self.at - vec2(0.0, 50.0)
    }
}

struct SkateStrike;

impl Special for SkateStrike {
    fn id(&self) -> &'static str {
        "skate_strike"
    }
    fn cooldown(&self) -> u32 {
        SKATE_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // Free of Michelle's freeze, and of the Chosen One (who steps out next tick).
        m.f[who].pinned = 0;
        if m.f[who].ridden > 0 {
            for i in 0..N_FIGHTERS {
                if m.f[i].host == Some(who as u8) && m.f[i].digital > 0 {
                    m.f[i].digital = 1;
                }
            }
        }
        let f = &mut m.f[who];
        f.skate = SKATE_TICKS;
        f.skate_hit = 0;
        // Out of a tornado's grip, too.
        f.hitstun = 0;
        f.caught = 0;
        f.dash_t = 0;
    }
    /// Someone on his level ahead, in range of the run; or he's held and wants out.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        let held = me.pinned > 0 || me.ridden > 0 || me.caught > 0;
        held || foes(m, who).any(|f| {
            let dx = (f.pos.x - me.pos.x) * me.facing;
            (f.center().y - me.center().y).abs() < 8.0 && dx > 0.0 && dx < 180.0
        })
    }
    fn breaks_free(&self) -> bool {
        true
    }
}

struct BugReport;

impl Special for BugReport {
    fn id(&self) -> &'static str {
        "bug_report"
    }
    fn cooldown(&self) -> u32 {
        BUGS_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let f = &m.f[who];
        let at = laptop_at(f);
        // Out of the laptop in a ring, every direction at once: spiders and flies in turn.
        for k in 0..BUGS {
            let a = (k as f32 + 0.5) / BUGS as f32 * std::f32::consts::TAU;
            let (c, s) = (libm::cosf(a), libm::sinf(a));
            let dir = if c >= 0.0 { 1.0 } else { -1.0 };
            let mut bug = if k % 2 == 0 {
                // (Flung straight up or down, a spider barely goes sideways at first.)
                let mut b = Pet::new(Breed::Spider(k as u8), at, dir, SPIDER_SPEED * c.abs().max(0.15), who, BUG_TICKS);
                b.vy = s * SPIDER_LAUNCH;
                b
            } else {
                let mut b = Pet::new(Breed::Fly(k as u8), at, dir, FLY_SPEED, who, BUG_TICKS);
                (b.vx, b.vy) = (c * FLY_LAUNCH, s * FLY_LAUNCH);
                b
            };
            bug.bites = Some(BUG_BITES);
            m.add_effect(Effect::Pet(bug));
        }
        // (The laptop in her hands is drawn by the client, from this.)
        m.events.push(Event::LaptopOpen { who });
    }
    /// Someone close enough for the swarm to reach.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].pos;
        foes(m, who).any(|f| f.pos.distance(me) < 220.0)
    }
}

/// Where Carrie holds her laptop: out in front of her, at chest height (the bottom of it).
pub fn laptop_at(f: &Fighter) -> Vec2 {
    vec2(f.pos.x + f.facing * 5.0 * f.scale, f.pos.y - 6.0 * f.scale)
}

/// Doug lunging into a grab: the first foe in his hands while it lasts is thrown.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lunge {
    /// His middle.
    pub at: Vec2,
    pub owner: usize,
    pub ticks: u32,
}

struct JudoThrow;

impl Special for JudoThrow {
    fn id(&self) -> &'static str {
        "judo_throw"
    }
    fn cooldown(&self) -> u32 {
        JUDO_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        // A player turns to lunge the way they aim (a bot, and a stick, the way he faces).
        if let Some(t) = m.f[who].action.target.filter(|t| t.x.abs() > 1e-3) {
            m.f[who].facing = t.x.signum();
        }
        // Someone already in his hands is thrown at once; otherwise he lunges in after them.
        if let Some(j) = judo_target(m, who).filter(|_| !m.protected()) {
            judo_throw(m, who, j);
            return;
        }
        let f = &mut m.f[who];
        f.dash_t = LUNGE_TICKS;
        let (at, pos) = (f.center(), f.pos);
        m.events.push(Event::Dash { who, pos });
        m.add_effect(Effect::Lunge(Lunge { at, owner: who, ticks: LUNGE_TICKS + 1 }));
    }
    /// Someone within a lunge.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        let mut reach = judo_grip(me);
        let lunge = crate::sim::DASH_SPEED * DT * LUNGE_TICKS as f32;
        reach.w += lunge;
        if me.facing < 0.0 {
            reach.x -= lunge;
        }
        foes(m, who).any(|f| reach.overlaps(&f.body()))
    }
}

/// Someone Doug threw: tumbling through the air, then down on the ground getting up.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Judo {
    /// Their middle.
    pub at: Vec2,
    pub owner: usize,
    pub victim: usize,
    /// Which way they were thrown (1: to the right).
    pub dir: f32,
    /// Ticks in the air so far.
    pub flown: u32,
    /// Landed: ticks left lying there (0 while still in the air).
    pub down: u32,
    pub ticks: u32,
    /// What the landing does to them.
    #[serde(skip)]
    pub damage: f32,
    /// Thrown by Doug (not just bowled over by someone he threw): knocks down whoever it hits.
    #[serde(skip)]
    pub bowls: bool,
    /// Who this throw has already knocked down (a bit per body).
    #[serde(skip)]
    pub bowled: u8,
}

impl Judo {
    pub fn landed(&self) -> bool {
        self.down > 0
    }
}

struct LegDay;

impl Special for LegDay {
    fn id(&self) -> &'static str {
        "leg_day"
    }
    fn cooldown(&self) -> u32 {
        BULK_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let f = &mut m.f[who];
        f.bulked = BULK_TICKS;
        // The first step comes quickly.
        f.stride = STRIDE * 0.6;
    }
    /// Someone on his level, close enough to run at.
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        me.grounded && foes(m, who).any(|f| (f.pos.y - me.pos.y).abs() < 10.0 && (f.pos.x - me.pos.x).abs() < 90.0)
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
    /// One of Carrie's bugs: one that crawls (a spider) or flies (a horsefly). The number varies
    /// how it moves.
    Spider(u8),
    Fly(u8),
}

impl Breed {
    /// Half width and height (px).
    pub fn size(self) -> (f32, f32) {
        match self {
            Breed::Hound => (3.0, 5.0),
            Breed::Cat(_) => (2.0, 4.0),
            Breed::Spider(_) => (2.0, 2.0),
            Breed::Fly(_) => (1.5, 2.0),
        }
    }
}

/// One of Tyler's dogs, Corwin's cats or Carrie's bugs: runs along floors (or flies), and bites
/// (or scratches) anyone not on its owner's side.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pet {
    pub breed: Breed,
    /// Feet centre.
    pub pos: Vec2,
    pub dir: f32,
    pub speed: f32,
    pub vy: f32,
    /// A fly's sideways speed (the rest run at `speed` the way they face).
    pub vx: f32,
    pub owner: usize,
    pub ticks: u32,
    /// A bug: bites left before it's done (None: as many as it likes while it lasts).
    pub bites: Option<u8>,
    /// A cat that's found sushi and is sitting eating it (it's forgotten about fighting).
    pub eating: bool,
    /// Frozen by Michelle's Eye of the Storm (ticks left): stock still, biting nobody.
    pub frozen: u32,
    #[serde(skip)]
    bite_cd: [u32; N_BODIES],
}

impl Pet {
    fn new(breed: Breed, pos: Vec2, dir: f32, speed: f32, owner: usize, ticks: u32) -> Self {
        Self { breed, pos, dir, speed, vy: 0.0, vx: 0.0, owner, ticks, bites: None, eating: false, frozen: 0, bite_cd: [0; N_BODIES] }
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
            let Some(special) = self.f[i].special() else { continue };
            let able = self.f[i].present() && self.f[i].hitstun == 0;
            let f = &mut self.f[i];
            if f.special_cd > 0 {
                f.special_cd -= 1;
                // On cooldown, a press can still steer what the special left in play (Kaleb's
                // arrow: fire it, call it back).
                if wants[i] && able {
                    special.press_again(self, i);
                }
                continue;
            }
            if wants[i] && able {
                let f = &mut self.f[i];
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
    /// The Chosen One inside someone: he goes where their body goes, and when his time's up (or they go
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
                Effect::Rocket(r) => self.step_rocket(r),
                Effect::Fire(f) => self.step_fire(f),
                Effect::Plane(p) => self.step_plane(p),
                Effect::AirBomb(b) => self.step_air_bomb(b),
                Effect::Judo(j) => self.step_judo(j),
                Effect::Lunge(l) => self.step_lunge(l),
                Effect::Tornado(t) => self.step_tornado(t),
                Effect::Arrow(a) => self.step_arrow(a),
                Effect::Book(b) => self.step_book(b),
            }
            self.effects[e] = effect;
        }
        let finished: Vec<Table> = self.effects.iter().filter_map(|e| if let Effect::Table(t) = e { (t.ticks == 0).then(|| t.clone()) } else { None }).collect();
        for t in &finished {
            self.clear_table(t);
        }
        self.retain_effects(|e| !e.done());
    }

    fn step_pet(&mut self, pet: &mut Pet) {
        // Frozen by Michelle: stock still, biting nobody (its time still runs down).
        if pet.frozen > 0 {
            pet.frozen -= 1;
            pet.ticks = pet.ticks.saturating_sub(1);
            return;
        }
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
            Breed::Hound | Breed::Spider(_) | Breed::Fly(_) => None,
        };
        // Carrie's bugs scatter every which way out of the laptop, then each hunts down whoever is
        // nearest: a spider runs after them (leaping up to a ledge they're on), a fly makes a
        // beeline, buzzing about as it goes.
        let bug = matches!(pet.breed, Breed::Spider(_) | Breed::Fly(_));
        let hunting = bug && BUG_TICKS - pet.ticks.min(BUG_TICKS) >= BUG_SCATTER;
        let prey = (0..N_BODIES).filter(|&j| hunting && self.foes(pet.owner, j)).map(|j| self.f[j].pos).min_by(|a, b| a.distance(pet.pos).total_cmp(&b.distance(pet.pos)));
        match pet.breed {
            Breed::Fly(k) => fly_bug(pet, self.arena(), prey.map(|p| p - vec2(0.0, 7.0)), k, self.tick),
            Breed::Spider(_) => {
                if hunting {
                    pet.speed = SPIDER_SPEED;
                }
                run_pet(pet, self.arena(), &props(&self.effects), prey, false);
            }
            _ => {
                let hop = goal.is_none() && pet.vy == 0.0 && self.chance(PET_HOP_CHANCE);
                run_pet(pet, self.arena(), &props(&self.effects), goal, hop);
            }
        }
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
                // A bug's bite is light too, and it only has a few in it: then it's done.
                Breed::Spider(_) | Breed::Fly(_) => {
                    pet.bite_cd[j] = BUG_BITE_EVERY;
                    self.events.push(Event::Scratch { pos });
                    let v = self.f[j].vel;
                    self.damage(owner, j, BUG_BITE, vec2(v.x + dir * 20.0, v.y.min(-30.0)), 0, dir, Source::Special, false);
                    let left = pet.bites.unwrap_or(1).saturating_sub(1);
                    pet.bites = Some(left);
                    if left == 0 {
                        pet.ticks = 0;
                        return;
                    }
                }
            }
        }
        pet.bite_cd.iter_mut().for_each(|t| *t = t.saturating_sub(1));
        pet.ticks = pet.ticks.saturating_sub(1);
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
                let caught = (0..N_BODIES).filter(|&j| self.foes(owner, j) && !self.f[j].unshakeable() && hook.overlaps(&self.f[j].body())).min_by(|&a, &b| self.f[a].center().distance(anchor).total_cmp(&self.f[b].center().distance(anchor)));
                // (Nobody can be hooked during the protected start.)
                if let Some(j) = caught.filter(|_| !self.protected()) {
                    c.hooked = Some(j);
                    c.tip = self.f[j].center();
                    self.events.push(Event::Hook { pos: c.tip });
                    self.damage(owner, j, CHAIN_DAMAGE, Vec2::ZERO, 2, c.dir.x.signum(), Source::Special, false);
                } else if c.out >= CHAIN_RANGE || self.arena().solids().any(|s| s.contains(c.tip)) {
                    c.back = true;
                }
            }
        }
    }

    /// A tornado wanders (turning back at the walls) and pulls at everything in reach, at any
    /// height: everyone (Zack too) is dragged toward it, carried up it if they get into it,
    /// and flung out near the top; loose items are pulled in, lifted and spun round it, and rocks,
    /// spears and swords spun fast enough hurt whoever they hit (but never Zack's side).
    fn step_tornado(&mut self, t: &mut Tornado) {
        t.ticks = t.ticks.saturating_sub(1);
        t.at.x += t.dir * TORNADO_DRIFT * DT;
        let (lo, hi) = (crate::sim::WALL + 12.0, W - crate::sim::WALL - 12.0);
        if t.at.x < lo || t.at.x > hi {
            t.at.x = t.at.x.clamp(lo, hi);
            t.dir = -t.dir;
        }
        let top = crate::sim::WALL + TORNADO_TOP;
        for j in 0..N_BODIES {
            let f = &mut self.f[j];
            let dx = t.at.x - f.pos.x;
            // (Not Matt on his skateboard: he rides straight out of it.)
            if !f.present() || f.pinned > 0 || f.skate > 0 || f.unshakeable() || dx.abs() > TORNADO_R {
                continue;
            }
            if dx.abs() < FUNNEL_HALF {
                if f.pos.y - f.height() > top {
                    // Carried up the column, kept in the middle of it (but hands free: you can
                    // still grab, throw, swing and use your special on the way).
                    f.caught = 2;
                    f.grounded = false;
                    f.dash_t = 0;
                    f.skate = 0;
                    f.vel = vec2(dx * 6.0 + t.spin * 30.0, f.vel.y.min(-TORNADO_LIFT));
                } else {
                    // Out near the top, the way it spins.
                    f.vel = vec2(t.spin * TORNADO_FLING, 0.0);
                    f.hitstun = 18;
                }
            } else {
                f.wind += dx.signum() * TORNADO_PULL * (1.0 - dx.abs() / TORNADO_R);
            }
        }
        // Loose items swirl round a vortex low in the column: pulled toward it, swung round it the
        // way it spins, and held up against gravity, so they orbit it rather than settle.
        let vortex = t.core();
        for it in self.items.iter_mut() {
            let to = vortex - it.pos;
            let d = to.length();
            if it.held_by.is_some() || it.carries.is_some() || d > ITEM_VORTEX_R || d < 1e-3 {
                continue;
            }
            let fall = 1.0 - d / ITEM_VORTEX_R;
            let toward = to / d;
            let round = vec2(-toward.y, toward.x) * t.spin;
            let g = if it.lethal { it.kind.def().flight_gravity } else { 0.8 };
            let push = (toward * ITEM_PULL + round * ITEM_SWIRL) * fall + vec2(0.0, -crate::sim::GRAVITY * g);
            it.vel = (it.vel + push * DT).clamp_length_max(ITEM_WHIRL_MAX);
            if matches!(it.kind, crate::sim::ItemKind::Rock | crate::sim::ItemKind::Spear | crate::sim::ItemKind::Sword) && it.vel.length() > crate::sim::LETHAL_SPEED {
                it.lethal = true;
                it.whirled = true;
                it.owner = Some(t.owner);
            }
        }
    }

    /// Kaleb's arrow: hovers over his head (following him), or chases wherever he aims (a mouse
    /// player's cursor, a pad's aim, a bot's nearest foe), or flies back to him. In flight it
    /// goes through every foe it meets, hitting each at most every so often. It's gone when its
    /// time's up, or he is.
    fn step_arrow(&mut self, a: &mut Arrow) {
        a.ticks = a.ticks.saturating_sub(1);
        a.hit_cd.iter_mut().for_each(|t| *t = t.saturating_sub(1));
        let who = a.owner;
        if !self.f[who].present() {
            a.ticks = 0;
            return;
        }
        let home = arrow_hover(&self.f[who]);
        let goal = match a.state {
            ArrowState::Hover => {
                a.pos = home;
                a.dir = vec2(self.f[who].facing, 0.0);
                return;
            }
            ArrowState::Back => home,
            ArrowState::Out => {
                let f = &self.f[who];
                let goal = match (f.action.cursor, f.action.target) {
                    (Some(c), _) => c,
                    // (A pad's aim: keep going that way.)
                    (None, Some(t)) => a.pos + t.normalize_or(a.dir) * 100.0,
                    (None, None) => foes(self, who).map(|g| g.center()).min_by(|x, y| x.distance(a.pos).total_cmp(&y.distance(a.pos))).unwrap_or(a.pos),
                };
                let (lo, hi) = (crate::sim::WALL + 2.0, vec2(W - crate::sim::WALL - 2.0, crate::sim::FLOOR_Y - 2.0));
                vec2(goal.x.clamp(lo, hi.x), goal.y.clamp(lo, hi.y))
            }
        };
        let to = goal - a.pos;
        let step = ARROW_SPEED * DT;
        if to.length() <= step {
            a.pos = goal;
            if a.state == ArrowState::Back {
                a.state = ArrowState::Hover;
            }
        } else {
            a.dir = to / to.length();
            a.pos += a.dir * step;
        }
        let tip = Aabb::around(a.pos, ARROW_REACH);
        for j in 0..N_BODIES {
            if !self.foes(who, j) || a.hit_cd[j] > 0 || !tip.overlaps(&self.f[j].body()) {
                continue;
            }
            a.hit_cd[j] = ARROW_HIT_EVERY;
            let dir = if a.dir.x < 0.0 { -1.0 } else { 1.0 };
            self.damage(who, j, ARROW_DAMAGE, vec2(a.dir.x * 140.0, -90.0), 8, dir, Source::Special, false);
        }
    }

    /// One of Vid's books: circles him while he reads; when it's rested and
    /// someone's close, it flies at them, hits them once, and comes back round. They're all put
    /// away when he stops reading (or goes down).
    fn step_book(&mut self, b: &mut Book) {
        b.ticks = b.ticks.saturating_sub(1);
        b.rest = b.rest.saturating_sub(1);
        let who = b.owner;
        if !self.f[who].present() || self.f[who].reading == 0 {
            b.ticks = 0;
            return;
        }
        let home = book_orbit(&self.f[who], b.k, self.tick);
        let me = self.f[who].center();
        // Still after someone in reach? Else, rested, the nearest in reach.
        let target = b.chasing.map(|j| j as usize).filter(|&j| self.foes(who, j) && self.f[j].center().distance(me) < BOOK_RANGE * 1.3);
        let target = target.or_else(|| {
            (b.rest == 0).then(|| (0..N_BODIES).filter(|&j| self.foes(who, j) && self.f[j].center().distance(me) < BOOK_RANGE).min_by(|&x, &y| self.f[x].center().distance(me).total_cmp(&self.f[y].center().distance(me)))).flatten()
        });
        b.chasing = target.map(|j| j as u8);
        let goal = target.map_or(home, |j| self.f[j].center());
        let to = goal - b.pos;
        let step = BOOK_SPEED * DT;
        b.pos = if to.length() <= step || target.is_none() && to.length() < 3.0 { goal } else { b.pos + to / to.length() * step };
        if let Some(j) = target
            && Aabb::around(b.pos, 2.0).overlaps(&self.f[j].body())
        {
            let dir = if self.f[j].pos.x < me.x { -1.0 } else { 1.0 };
            self.damage(who, j, BOOK_DAMAGE, vec2(dir * 110.0, -90.0), 8, dir, Source::Special, false);
            b.chasing = None;
            b.rest = BOOK_REST;
        }
    }

    /// Kyle's backlog counts down; when it's up, everything in it goes at once: each throw the way
    /// it was aimed (grenades lit, rockets fired, air strikes called in), and every swing in one
    /// big punch in front of him. (If he goes down first, what he'd thrown just drops.)
    pub(crate) fn step_backlog(&mut self) {
        for i in 0..N_FIGHTERS {
            if self.f[i].backlog == 0 {
                continue;
            }
            if !self.f[i].present() {
                let pos = self.f[i].hand();
                let f = &mut self.f[i];
                (f.backlog, f.inbox, f.swings) = (0, 0, 0);
                for (mut it, _) in std::mem::take(&mut f.stash) {
                    (it.pos, it.vel, it.held_by) = (pos, Vec2::ZERO, None);
                    self.items.push(it);
                }
                continue;
            }
            self.f[i].backlog -= 1;
            if self.f[i].backlog > 0 {
                continue;
            }
            let f = &mut self.f[i];
            let (stash, swings) = (std::mem::take(&mut f.stash), f.swings);
            (f.inbox, f.swings) = (0, 0);
            let pos = f.center();
            self.events.push(Event::BacklogBurst { pos });
            for (mut it, dir) in stash {
                it.held_by = Some(i);
                it.pos = self.f[i].hand();
                self.items.push(it);
                let k = self.items.len() - 1;
                self.launch(i, k, dir);
            }
            if swings > 0 {
                // Every swing he put off, landing at once.
                let f = &self.f[i];
                let mut reach = f.attack_box();
                reach.w += BACKLOG_REACH;
                if f.facing < 0.0 {
                    reach.x -= BACKLOG_REACH;
                }
                let (dir, dmg) = (f.facing, BACKLOG_PUNCH * swings.min(BACKLOG_PUNCHES) as f32);
                let hit: Vec<usize> = (0..N_BODIES).filter(|&j| self.foes(i, j) && reach.overlaps(&self.f[j].body())).collect();
                for j in hit {
                    self.damage(i, j, dmg, vec2(dir * 260.0, -200.0), 20, dir, Source::Fist, false);
                }
            }
        }
    }

    /// Matt on his skateboard hits everyone he goes through, once each per run.
    pub(crate) fn step_skate(&mut self) {
        for i in 0..N_FIGHTERS {
            if self.f[i].skate == 0 || !self.f[i].present() {
                continue;
            }
            let reach = self.f[i].body();
            let dir = self.f[i].facing;
            for j in 0..N_BODIES {
                if !self.foes(i, j) || self.f[i].skate_hit & (1 << j) != 0 || !reach.overlaps(&self.f[j].body()) {
                    continue;
                }
                self.f[i].skate_hit |= 1 << j;
                self.damage(i, j, SKATE_DAMAGE, vec2(dir * 170.0, -190.0), 14, dir, Source::Special, false);
            }
        }
    }

    /// Doug mid-lunge: grabs (and throws) the first foe he reaches. Reaching nobody, or knocked
    /// out of it, the lunge was a miss and he's soon ready to try again.
    fn step_lunge(&mut self, l: &mut Lunge) {
        let who = l.owner;
        l.ticks = l.ticks.saturating_sub(1);
        l.at = self.f[who].center();
        let f = &self.f[who];
        if !f.present() || f.hitstun > 0 {
            l.ticks = 0;
        } else if let Some(j) = judo_target(self, who).filter(|_| !self.protected()) {
            judo_throw(self, who, j);
            l.ticks = 0;
            return;
        }
        if l.ticks == 0 {
            let f = &mut self.f[who];
            f.special_cd = f.special_cd.min(JUDO_WHIFF_COOLDOWN);
        }
    }

    /// Someone Doug threw: helpless in the air until they come down on something; then they slam
    /// down (that's the damage) and lie there, stunned, getting back up.
    /// Someone Doug threw, flying into others on their side: they're knocked flat too, and lie
    /// there getting up just as long. (Pinned people stay put: Michelle's freeze holds.)
    fn bowl_over(&mut self, judo: &mut Judo) {
        let body = self.f[judo.victim].body();
        let down: Vec<usize> = self.effects.iter().filter_map(|e| if let Effect::Judo(o) = e { Some(o.victim) } else { None }).collect();
        for k in 0..N_BODIES {
            let f = &self.f[k];
            if judo.bowled & (1 << k) != 0 || !self.foes(judo.owner, k) || f.pinned > 0 || f.unshakeable() || down.contains(&k) || !body.overlaps(&f.body()) {
                continue;
            }
            judo.bowled |= 1 << k;
            if let Some(i) = self.held_item(k) {
                self.items[i].held_by = None;
                self.items[i].vel = vec2(0.0, -60.0);
            }
            let f = &mut self.f[k];
            f.holding = None;
            f.atk_timer = 0;
            f.dash_t = 0;
            f.skate = 0;
            f.vel = vec2(judo.dir * 110.0, -150.0);
            f.grounded = false;
            f.hitstun = 2;
            let at = f.center();
            self.events.push(Event::Slam { pos: at });
            self.add_effect(Effect::Judo(Judo { at, owner: judo.owner, victim: k, dir: judo.dir, flown: 0, down: 0, ticks: JUDO_MAX_FLIGHT + JUDO_DOWN, damage: JUDO_BOWL_DAMAGE, bowls: false, bowled: 0 }));
        }
    }

    fn step_judo(&mut self, judo: &mut Judo) {
        judo.ticks = judo.ticks.saturating_sub(1);
        let j = judo.victim;
        if !self.f[j].present() {
            judo.ticks = 0;
            return;
        }
        if !judo.landed() {
            judo.flown += 1;
            let f = &mut self.f[j];
            f.hitstun = f.hitstun.max(2);
            if (f.grounded && judo.flown > 2) || judo.flown >= JUDO_MAX_FLIGHT {
                judo.down = JUDO_DOWN;
                judo.ticks = JUDO_DOWN;
                let pos = f.center();
                self.events.push(Event::Slam { pos });
                self.damage(judo.owner, j, judo.damage, Vec2::ZERO, JUDO_DOWN, judo.dir, Source::Special, false);
            } else if judo.bowls {
                self.bowl_over(judo);
            }
        } else {
            judo.down -= 1;
            let f = &mut self.f[j];
            // Stuck where they landed, whatever they press (a hit can still knock them about).
            f.hitstun = f.hitstun.max(judo.down);
            if f.grounded {
                f.vel.x = 0.0;
            }
            if judo.down == 0 {
                judo.ticks = 0;
            }
        }
        judo.at = self.f[j].center();
    }

    /// Travis bulked up: counts down, and each step he takes (every so far run along the ground,
    /// and every landing) sends out a shockwave. Everyone's shaken cooldown counts down too.
    pub(crate) fn step_bulk(&mut self, landed: [bool; N_BODIES]) {
        for f in self.f.iter_mut() {
            f.shaken = f.shaken.saturating_sub(1);
            f.caught = f.caught.saturating_sub(1);
        }
        for i in 0..N_FIGHTERS {
            let f = &mut self.f[i];
            if f.bulked == 0 {
                continue;
            }
            f.bulked = if f.present() { f.bulked - 1 } else { 0 };
            if !f.present() {
                continue;
            }
            let big = landed[i];
            let mut step = big;
            if f.grounded && !big {
                f.stride += f.vel.x.abs() * DT;
                if f.stride >= STRIDE {
                    f.stride -= STRIDE;
                    step = true;
                }
            }
            if big {
                f.stride = 0.0;
            }
            if step {
                self.quake(i, big);
            }
        }
    }

    /// One of Travis's steps: everyone on the ground near his feet, on his level, is hurt and
    /// bounced up a little (not more than once in a short while).
    fn quake(&mut self, who: usize, big: bool) {
        let (radius, dmg) = if big { (QUAKE_BIG_R, QUAKE_BIG_DAMAGE) } else { (QUAKE_R, QUAKE_DAMAGE) };
        let at = self.f[who].pos;
        self.events.push(Event::Quake { pos: at, big });
        // (The ring it sends out is drawn by the client, from that event.)
        for j in 0..N_BODIES {
            let f = &self.f[j];
            let d = f.pos - at;
            if !self.foes(who, j) || !f.grounded || f.shaken > 0 || d.x.abs() > radius || d.y.abs() > QUAKE_LEVEL {
                continue;
            }
            self.f[j].shaken = QUAKE_EVERY;
            let dir = if d.x < 0.0 { -1.0 } else { 1.0 };
            let pop = if big { vec2(dir * 110.0, -170.0) } else { vec2(dir * 60.0, -110.0) };
            self.damage(who, j, dmg, pop, if big { 10 } else { 6 }, dir, Source::Special, false);
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
            self.add_effect(Effect::AirBomb(AirBomb { pos, vel: vec2(p.dir * PLANE_SPEED * 0.3, 0.0), owner: p.owner, ticks: 10 * SECOND }));
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
        let solid = self.arena().solids().chain(props.iter()).any(|s| s.contains(b.pos));
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
        let walled = self.arena().solids().chain(props(&self.effects).iter()).any(|s| s.contains(r.pos));
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
        let victim = (0..N_BODIES).find(|&j| self.foes(g.owner, j) && !self.f[j].unshakeable() && hit.overlaps(&self.f[j].body()));
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
        let in_solid = self.arena().solids().chain(props.iter()).any(|s| s.contains(g.pos));
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
            if g.held.contains(&(j as u8)) || !self.foes(g.owner, j) || self.f[j].unshakeable() {
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

/// Something a special leaves in the arena that matters to play. Each kind is a variant: its step
/// (in `step_effects`), how the bots perceive it (`threat_pos`), whether clients are sent it
/// (`on_wire`) and how it's drawn (the render crate) are all exhaustive matches, so the compiler
/// lists what a new one needs.
///
/// Something that's only for looks (a flash, a ring, a bolt) isn't an effect: the special pushes
/// an `Event` and the client draws it from that (see render's `Visual`), so the server neither
/// keeps nor sends it.
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
    /// A homing rocket (from a rocket launcher, not a special).
    Rocket(Rocket),
    /// Burning ground, from a Molotov.
    Fire(Fire),
    /// An air strike's jet, and a bomb it dropped.
    Plane(Plane),
    AirBomb(AirBomb),
    /// Someone Doug threw, in the air or down on the ground.
    Judo(Judo),
    /// Doug lunging into a grab.
    Lunge(Lunge),
    /// One of Zack's tornadoes.
    Tornado(Tornado),
    /// Kaleb's arrow.
    Arrow(Arrow),
    /// One of Vid's books.
    Book(Book),
}

impl Effect {
    /// Who made it (it never harms them, and they don't see it as a threat).
    pub fn owner(&self) -> usize {
        match self {
            Effect::Glue(g) => g.owner,
            Effect::Pet(p) => p.owner,
            Effect::Table(t) => t.owner,
            Effect::Chain(c) => c.owner,
            Effect::Rocket(r) => r.owner,
            Effect::Fire(f) => f.owner,
            Effect::Plane(p) => p.owner,
            Effect::AirBomb(b) => b.owner,
            Effect::Judo(j) => j.owner,
            Effect::Lunge(l) => l.owner,
            Effect::Tornado(t) => t.owner,
            Effect::Arrow(a) => a.owner,
            Effect::Book(b) => b.owner,
        }
    }

    /// Where the bots see it as a threat (None: harmless, like a table).
    pub fn threat_pos(&self) -> Option<Vec2> {
        match self {
            Effect::Glue(g) => Some(g.pos),
            Effect::Pet(p) => Some(p.center()),
            Effect::Table(_) => None,
            Effect::Chain(c) => Some(c.tip),
            Effect::Rocket(r) => Some(r.pos),
            Effect::Fire(f) => Some(f.at),
            Effect::Plane(_) => None,
            Effect::AirBomb(b) => Some(b.pos),
            Effect::Judo(_) => None,
            Effect::Lunge(_) => None,
            Effect::Tornado(t) => Some(t.core()),
            Effect::Arrow(a) => (a.state != ArrowState::Hover).then_some(a.pos),
            Effect::Book(b) => b.chasing.map(|_| b.pos),
        }
    }

    /// Its position (a pet's feet, a table's bottom centre).
    pub fn pos(&self) -> Vec2 {
        match self {
            Effect::Glue(g) => g.pos,
            Effect::Pet(p) => p.pos,
            Effect::Table(t) => t.pos,
            Effect::Chain(c) => c.tip,
            Effect::Rocket(r) => r.pos,
            Effect::Fire(f) => f.at,
            Effect::Plane(p) => p.pos,
            Effect::AirBomb(b) => b.pos,
            Effect::Judo(j) => j.at,
            Effect::Lunge(l) => l.at,
            Effect::Tornado(t) => t.at,
            Effect::Arrow(a) => a.pos,
            Effect::Book(b) => b.pos,
        }
    }

    /// Its position, for smoothing it between snapshots on the client.
    pub fn pos_mut(&mut self) -> &mut Vec2 {
        match self {
            Effect::Glue(g) => &mut g.pos,
            Effect::Pet(p) => &mut p.pos,
            Effect::Table(t) => &mut t.pos,
            Effect::Chain(c) => &mut c.tip,
            Effect::Rocket(r) => &mut r.pos,
            Effect::Fire(f) => &mut f.at,
            Effect::Plane(p) => &mut p.pos,
            Effect::AirBomb(b) => &mut b.pos,
            Effect::Judo(j) => &mut j.at,
            Effect::Lunge(l) => &mut l.at,
            Effect::Tornado(t) => &mut t.at,
            Effect::Arrow(a) => &mut a.pos,
            Effect::Book(b) => &mut b.pos,
        }
    }

    fn done(&self) -> bool {
        match self {
            Effect::Glue(g) => g.ticks == 0,
            Effect::Pet(p) => p.ticks == 0,
            Effect::Table(t) => t.ticks == 0,
            Effect::Chain(c) => c.ticks == 0,
            Effect::Rocket(r) => r.ticks == 0,
            Effect::Fire(f) => f.ticks == 0,
            Effect::Plane(p) => p.ticks == 0,
            Effect::AirBomb(b) => b.ticks == 0,
            Effect::Judo(j) => j.ticks == 0,
            Effect::Lunge(l) => l.ticks == 0,
            Effect::Tornado(t) => t.ticks == 0,
            Effect::Arrow(a) => a.ticks == 0,
            Effect::Book(b) => b.ticks == 0,
        }
    }

    /// Whether clients are sent it. Only what they draw, or predict with: something the server
    /// needs but nobody sees (Doug's lunge) stays off the wire.
    pub fn on_wire(&self) -> bool {
        match self {
            Effect::Glue(_) | Effect::Pet(_) | Effect::Table(_) | Effect::Chain(_) | Effect::Rocket(_) | Effect::Fire(_) => true,
            Effect::Plane(_) | Effect::AirBomb(_) | Effect::Judo(_) | Effect::Tornado(_) | Effect::Arrow(_) | Effect::Book(_) => true,
            Effect::Lunge(_) => false,
        }
    }
}

/// One tick of a fly: steering (not too sharply) at `prey`, buzzing about on the way, and
/// bouncing off walls, floor and ceiling (it flies straight over ledges). With nobody to chase
/// it drifts, slowing.
fn fly_bug(fly: &mut Pet, arena: &Arena, prey: Option<Vec2>, k: u8, tick: u32) {
    let v = vec2(fly.vx, fly.vy);
    let v = match prey {
        Some(p) => {
            // (libm, not std: matches must play out identically everywhere.)
            let t = tick as f32 * DT + k as f32 * 1.7;
            let buzz = vec2(libm::sinf(t * 7.0), libm::cosf(t * 5.3)) * FLY_WOBBLE;
            let want = (p - fly.pos).normalize_or_zero() * fly.speed + buzz;
            v + (want - v).clamp_length_max(FLY_ACCEL * DT)
        }
        None => v * 0.985,
    };
    (fly.vx, fly.vy) = (v.x, v.y);
    let was = fly.pos;
    fly.pos.x += fly.vx * DT;
    if arena.solids().any(|s| fly.body().overlaps(s)) {
        fly.pos.x = was.x;
        fly.vx = -fly.vx * 0.5;
    }
    fly.pos.y += fly.vy * DT;
    if arena.solids().any(|s| fly.body().overlaps(s)) {
        fly.pos.y = was.y;
        fly.vy = -fly.vy * 0.5;
    }
    if fly.vx.abs() > 1.0 {
        fly.dir = fly.vx.signum();
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
                dog.vy = -libm::sqrtf(2.0 * 640.0 * (dog.pos.y - g.y + 4.0).min(PET_LEAP_MAX));
            }
            dog.eating = dx.abs() <= 1.0 && (g.y - dog.pos.y).abs() < 6.0;
            dx.clamp(-dog.speed * DT, dog.speed * DT)
        }
    };
    dog.pos.x += step;
    if let Some(s) = arena.solids().find(|s| dog.body().overlaps(s)) {
        dog.pos.x = if dog.dir > 0.0 { s.x - half_w } else { s.x + s.w + half_w };
        dog.dir = -dog.dir;
    }
    let prev = dog.pos.y;
    dog.vy = (dog.vy + 640.0 * DT).min(320.0);
    dog.pos.y += dog.vy * DT;
    if let Some(s) = arena.solids().find(|s| dog.body().overlaps(s)) {
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
        let mut m = Match::new(7, &MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, unstick: false, ..MatchConfig::default() });
        m.items.clear();
        for i in 0..N_FIGHTERS {
            m.f[i].character = cast[i];
            m.f[i].pos = vec2(xs[i], crate::sim::FLOOR_Y);
            // (Classic bots keep their special to themselves: these tests want them standing still.)
            if cast[i] == Character::Classic {
                m.f[i].special_cd = u32::MAX / 2;
            }
        }
        m
    }

    /// One tick: seat 0 is a "human" pressing only special (or nothing); the rest run an
    /// all-zero brain, which stands still.
    fn step_idle(m: &mut Match, brain: &Brain, special: bool) {
        let a = Action { special, ..Action::default() };
        m.step_with([brain; N_FIGHTERS], Some((0, a)));
    }

    /// Yanking Your Chain's place in `ALL_SPECIALS` (what Ryan's tests give him as his hobby).
    fn chain_hobby() -> u8 {
        ALL_SPECIALS.iter().position(|s| s.id() == "yank_chain").unwrap() as u8
    }

    #[test]
    fn every_special_is_in_all_specials_once() {
        for c in Character::ALL {
            if let Some(s) = c.special() {
                assert!(ALL_SPECIALS.iter().any(|t| t.id() == s.id()), "{}'s special isn't in ALL_SPECIALS (Ryan can't draw it)", c.id());
            }
        }
        let mut ids: Vec<&str> = ALL_SPECIALS.iter().map(|s| s.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ALL_SPECIALS.len(), "a special is in ALL_SPECIALS twice");
    }

    #[test]
    fn ryan_picks_up_a_special_for_each_match_and_it_s_most_often_the_chain() {
        let z = zero_brain();
        let mut picked = [0u32; ALL_SPECIALS.len()];
        for seed in 0..400u64 {
            let mut m = Match::new(seed, &MatchConfig { heals: 0..=0, unstick: false, ..MatchConfig::default() });
            m.f[0].character = Character::Ryan;
            assert!(m.f[0].special().is_none(), "nothing before the match starts");
            m.step([&z; N_FIGHTERS]);
            let k = m.f[0].hobby.expect("picked one as it started") as usize;
            assert_eq!(m.f[0].special().map(|s| s.id()), Some(ALL_SPECIALS[k].id()));
            picked[k] += 1;
            // It stays the same all match.
            for _ in 0..60 {
                m.step([&z; N_FIGHTERS]);
            }
            assert_eq!(m.f[0].hobby, Some(k as u8));
            assert!(m.f[1].hobby.is_none(), "only Ryan");
        }
        let chain = picked[chain_hobby() as usize];
        assert!(picked.iter().all(|&n| n > 0), "every special comes up: {picked:?}");
        assert!(picked.iter().enumerate().all(|(k, &n)| k == chain_hobby() as usize || n < chain), "the chain's the likeliest: {picked:?}");
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
            // (Ryan has no special of his own, but always picks one up for the match.)
            match (c.special(), c.passive()) {
                (None, Some(Passive::Hobbyist)) | (Some(_), _) => assert!(n > 0, "bots never used {c:?}'s special: {fired:?}"),
                (None, _) => assert_eq!(n, 0, "{c:?} has no special"),
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
    fn seths_minis_fight_for_him_and_take_a_few_hits() {
        let z = zero_brain();
        let mut m = staged([Character::Seth, Character::Classic, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        let minis: Vec<usize> = (N_FIGHTERS..N_BODIES).filter(|&j| !m.f[j].absent).collect();
        assert_eq!(minis.len(), MINIS_PER_USE);
        let a = minis[0];
        assert_eq!((m.f[a].scale, m.f[a].owner, m.f[a].hp), (MINI_SCALE, Some(0), MINI_HP));
        assert!(m.f[a].body().h < m.f[0].body().h * 0.8, "smaller than a full fighter");
        // On Seth's side: not each other's foes, everyone else's.
        assert!(!m.foes(a, 0) && !m.foes(0, a) && !m.foes(a, minis[1]));
        assert!(m.foes(a, 1) && m.foes(1, a));
        // What a mini does counts for Seth.
        m.damage(a, 1, 10.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].dmg_dealt, 10.0);
        // It takes a few hits now, then it's gone (in a puff).
        m.damage(1, a, 8.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        step_idle(&mut m, &z, false);
        assert!(!m.f[a].absent && m.f[a].hp == MINI_HP - 8.0, "one punch doesn't finish it");
        m.damage(1, a, MINI_HP, Vec2::ZERO, 0, 1.0, Source::Fist, false);
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
        m.f[0].hobby = Some(chain_hobby());
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
        m.f[0].hobby = Some(chain_hobby());
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
        assert_eq!(m.events.iter().filter(|e| matches!(e, Event::Zap { .. })).count(), 3, "a bolt to each");
        assert!(m.effects.is_empty(), "nothing left in the arena: the bolts are drawn from those events");
    }

    #[test]
    fn michelles_eye_of_the_storm_freezes_everyone_else_for_three_seconds() {
        let z = zero_brain();
        let mut m = staged([Character::Michelle, Character::Classic, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
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
        assert!(m.f[0].pos.x > 150.0, "Michelle moves freely");
        assert_eq!(m.f[0].pinned, 0, "nor is she frozen by her own");
        for _ in 0..30 {
            let mut humans = [None; N_FIGHTERS];
            humans[1] = Some(run);
            m.step_humans(brains, humans);
        }
        assert!(m.f[1].pos.x > x1 + 5.0, "free again after 3s");
        assert!(m.f[2].pos.y > y2, "and falling");
    }

    #[test]
    fn the_eye_of_the_storm_freezes_dogs_and_minis_already_out_but_not_ones_let_out_after() {
        let z = zero_brain();
        let pets = |m: &Match| m.effects.iter().filter_map(|e| if let Effect::Pet(p) = e { Some(p.clone()) } else { None }).collect::<Vec<_>>();
        // Tyler's dogs and Seth's minis out first.
        let mut m = staged([Character::Michelle, Character::Tyler, Character::Seth, Character::Classic], [440.0, 60.0, 200.0, 300.0]);
        m.f[1].facing = 1.0;
        let press = |m: &mut Match, seats: &[usize]| {
            let mut h = [None; N_FIGHTERS];
            for &s in seats {
                h[s] = Some(Action { special: true, ..Action::default() });
            }
            m.step_humans([&z; N_FIGHTERS], h);
        };
        press(&mut m, &[1, 2]);
        for _ in 0..20 {
            press(&mut m, &[]);
        }
        let minis: Vec<usize> = (N_FIGHTERS..N_BODIES).filter(|&j| !m.f[j].absent).collect();
        assert!(!minis.is_empty());
        // Michelle freezes them.
        press(&mut m, &[0]);
        let before: Vec<Vec2> = pets(&m).iter().map(|p| p.pos).collect();
        let mini_before: Vec<Vec2> = minis.iter().map(|&j| m.f[j].pos).collect();
        assert!(pets(&m).iter().all(|p| p.frozen > 0));
        assert!(minis.iter().all(|&j| m.f[j].pinned > 0), "the minis too");
        for _ in 0..SECOND {
            press(&mut m, &[]);
        }
        let after: Vec<Vec2> = pets(&m).iter().take(before.len()).map(|p| p.pos).collect();
        assert_eq!(before, after, "the dogs stood still");
        assert_eq!(mini_before, minis.iter().map(|&j| m.f[j].pos).collect::<Vec<_>>(), "and the minis");
        // Cats let out now, while Corwin's frozen, run as normal.
        let mut m = staged([Character::Michelle, Character::Corwin, Character::Classic, Character::Classic], [440.0, 60.0, 300.0, 380.0]);
        m.f[1].facing = 1.0;
        press(&mut m, &[0]);
        assert!(m.f[1].pinned > 0);
        press(&mut m, &[1]);
        let start: Vec<Vec2> = pets(&m).iter().map(|p| p.pos).collect();
        assert!(!start.is_empty() && pets(&m).iter().all(|p| p.frozen == 0), "let out after: not frozen");
        for _ in 0..30 {
            press(&mut m, &[]);
        }
        assert!(pets(&m).iter().zip(&start).any(|(p, s)| p.pos.distance(*s) > 10.0), "and they run");
    }

    #[test]
    fn nothing_pushes_michelle_around() {
        let z = zero_brain();
        // A hit hurts but doesn't move her or stun her.
        let mut m = staged([Character::Classic, Character::Michelle, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
        m.damage(0, 1, 20.0, vec2(300.0, -300.0), 30, 1.0, Source::Fist, false);
        assert_eq!(m.f[1].hp, MAX_HP - 20.0);
        assert!(m.f[1].vel == Vec2::ZERO && m.f[1].hitstun == 0 && m.f[1].grounded, "unmoved, unstunned");
        // Nobody else's Eye of the Storm freezes her.
        let mut m = staged([Character::Michelle, Character::Michelle, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
        step_idle(&mut m, &z, true);
        assert!(m.f[2].pinned > 0 && m.f[1].pinned == 0, "the Classic's frozen, the other Michelle isn't");
        // Doug can't get hold of her.
        let mut m = staged([Character::Doug, Character::Michelle, Character::Classic, Character::Classic], [100.0, 110.0, 300.0, 400.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        for _ in 0..LUNGE_TICKS + 2 {
            step_idle(&mut m, &z, false);
        }
        assert!(m.effects.iter().all(|e| !matches!(e, Effect::Judo(_))), "not thrown");
        // Ryan's chain goes straight past her.
        let mut m = staged([Character::Ryan, Character::Michelle, Character::Classic, Character::Classic], [100.0, 160.0, 300.0, 440.0]);
        m.f[0].hobby = Some(chain_hobby());
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        for _ in 0..CHAIN_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert_eq!(m.f[1].pos.x, 160.0, "not hauled in");
        // A tornado doesn't pull at her.
        let mut m = staged([Character::Zack, Character::Michelle, Character::Classic, Character::Classic], [200.0, 300.0, 30.0, 470.0]);
        step_idle(&mut m, &z, true);
        for _ in 0..SECOND {
            step_idle(&mut m, &z, false);
        }
        assert_eq!(m.f[1].pos.x, 300.0, "not pulled");
        // And the Chosen One can't ride her.
        let mut m = staged([Character::Shawn, Character::Michelle, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
        m.f[0].special_cd = 0;
        m.step_with([&z; N_FIGHTERS], Some((0, Action { special: true, cursor: Some(m.f[1].center()), ..Action::default() })));
        assert_eq!(m.f[1].ridden, 0, "not ridden");
    }

    #[test]
    fn kyles_backlog_piles_up_swings_and_throws_then_lets_them_all_go() {
        use crate::sim::{Item, ItemKind};
        let z = zero_brain();
        let mut m = staged([Character::Kyle, Character::Classic, Character::Classic, Character::Classic], [100.0, 108.0, 250.0, 440.0]);
        m.f[0].facing = 1.0;
        let kyle = |m: &mut Match, a: Action| m.step_with([&z; N_FIGHTERS], Some((0, a)));
        kyle(&mut m, Action { special: true, ..Action::default() });
        assert!(m.f[0].backlog > 0);
        // Swinging at the fighter right in front: it swings, but nothing lands.
        for _ in 0..3 {
            kyle(&mut m, Action { attack: true, ..Action::default() });
            for _ in 0..40 {
                kyle(&mut m, Action::default());
            }
        }
        assert_eq!(m.f[1].hp, MAX_HP, "nothing lands yet");
        // Two rocks thrown at the one further off: each leaves his hand, but goes nowhere.
        for _ in 0..2 {
            let mut rock = Item::new(ItemKind::Rock, m.f[0].hand());
            rock.held_by = Some(0);
            m.items.push(rock);
            m.f[0].holding = Some(ItemKind::Rock);
            kyle(&mut m, Action { throw: true, target: Some(vec2(1.0, -0.15)), ..Action::default() });
            assert!(m.f[0].holding.is_none(), "his hand's free again");
            for _ in 0..25 {
                kyle(&mut m, Action::default());
            }
        }
        assert!(m.items.is_empty(), "the rocks are in his backlog, not the arena");
        assert_eq!(m.f[0].inbox, 5, "three swings and two throws piled up");
        // When it's up, it all goes at once.
        let mut burst = false;
        for _ in 0..BACKLOG_TICKS {
            kyle(&mut m, Action::default());
            burst |= m.events.iter().any(|e| matches!(e, Event::BacklogBurst { .. }));
            if burst {
                break;
            }
        }
        assert!(burst, "the backlog went off");
        assert_eq!(m.f[0].inbox, 0);
        assert_eq!(m.f[1].hp, MAX_HP - 3.0 * BACKLOG_PUNCH, "three swings' worth in one punch");
        assert_eq!(m.items.iter().filter(|it| it.kind == ItemKind::Rock && it.lethal).count(), 2, "both rocks thrown at once");
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
        m.add_effect(Effect::Glue(Glue { pos: vec2(200.0, crate::sim::FLOOR_Y), vel: Vec2::ZERO, owner: 0, stuck: true, held: Vec::new(), ticks: GLUE_HOLD }));
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
    fn the_chosen_one_rides_the_opponent_nearest_the_cursor_then_steps_out() {
        let z = zero_brain();
        let mut m = staged([Character::Shawn, Character::Classic, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
        m.f[0].special_cd = 0;
        // The cursor is nearest the fighter at 300.
        let a = Action { special: true, cursor: Some(vec2(290.0, 240.0)), ..Action::default() };
        m.step_with([&z; N_FIGHTERS], Some((0, a)));
        assert_eq!(m.f[0].host, Some(2));
        assert!(m.f[2].ridden > 0 && !m.f[0].present());
        // Inside, the Chosen One can't be hurt; his input drives their body.
        m.damage(1, 0, 50.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP);
        let x2 = m.f[2].pos.x;
        let left = Action { mv: -1.0, run: true, ..Action::default() };
        for _ in 0..SECOND {
            m.step_with([&z; N_FIGHTERS], Some((0, left)));
        }
        assert!(m.f[2].pos.x < x2 - 40.0, "he ran their body left: {}", m.f[2].pos.x);
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

    #[test]
    fn doug_throws_whoever_is_in_front_and_they_lie_stunned_where_they_land() {
        let z = zero_brain();
        let mut m = staged([Character::Doug, Character::Classic, Character::Classic, Character::Classic], [100.0, 110.0, 300.0, 400.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        assert_eq!(m.f[0].special_cd, JUDO_COOLDOWN, "a real throw: full cooldown");
        assert!(m.f[1].vel.y < 0.0 && m.f[1].vel.x > 0.0, "thrown up and forward: {:?}", m.f[1].vel);
        // Helpless in the air; once down they're stuck where they landed, whatever they press.
        let run = Action { mv: 1.0, run: true, jump: true, ..Action::default() };
        let mut slam_at = None;
        for t in 0..JUDO_MAX_FLIGHT {
            m.step_with([&z; N_FIGHTERS], Some((1, run)));
            if m.events.iter().any(|e| matches!(e, Event::Slam { .. })) {
                slam_at = Some(t);
                break;
            }
        }
        assert!(slam_at.is_some(), "came down");
        let landed = m.f[1].pos.x;
        assert!(landed > 160.0, "thrown a good way: landed at x {landed}");
        assert_eq!(m.f[1].hp, MAX_HP - JUDO_DAMAGE, "the slam hurts");
        for _ in 0..JUDO_DOWN - 2 {
            m.step_with([&z; N_FIGHTERS], Some((1, run)));
        }
        assert!((m.f[1].pos.x - landed).abs() < 1.0, "still down where they landed: {}", m.f[1].pos.x);
        for _ in 0..30 {
            m.step_with([&z; N_FIGHTERS], Some((1, run)));
        }
        assert!(m.f[1].pos.x > landed + 10.0, "up and running again");
        assert!(m.effects.iter().all(|e| !matches!(e, Effect::Judo(_))));
        assert_eq!(m.f[0].hp, MAX_HP);
    }

    #[test]
    fn dougs_throw_bowls_over_whoever_it_flies_into() {
        let z = zero_brain();
        let mut m = staged([Character::Doug, Character::Classic, Character::Classic, Character::Classic], [100.0, 110.0, 175.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        let mut knocked = false;
        for _ in 0..SECOND {
            step_idle(&mut m, &z, false);
            knocked |= m.effects.iter().any(|e| matches!(e, Effect::Judo(j) if j.victim == 2));
            if knocked {
                break;
            }
        }
        assert!(knocked, "the one in the way was knocked down");
        // Down a second, like the one thrown; hurt a little; then up.
        let run = Action { mv: 1.0, run: true, ..Action::default() };
        let mut slammed = false;
        for _ in 0..SECOND / 2 {
            m.step_with([&z; N_FIGHTERS], Some((2, run)));
            slammed |= m.f[2].hp < MAX_HP;
        }
        assert!(slammed, "and hurt when they hit the ground");
        let x = m.f[2].pos.x;
        for _ in 0..SECOND / 3 {
            m.step_with([&z; N_FIGHTERS], Some((2, run)));
        }
        assert!((m.f[2].pos.x - x).abs() < 1.0, "still down, can't run");
        assert_eq!(m.f[2].hp, MAX_HP - JUDO_BOWL_DAMAGE);
        for _ in 0..SECOND {
            m.step_with([&z; N_FIGHTERS], Some((2, run)));
        }
        assert!(m.f[2].pos.x > x + 20.0, "up and running again");
        assert_eq!(m.f[3].hp, MAX_HP, "nobody else");
    }

    #[test]
    fn doug_lunging_at_nobody_is_soon_ready_again() {
        let z = zero_brain();
        let mut m = staged([Character::Doug, Character::Classic, Character::Classic, Character::Classic], [100.0, 200.0, 300.0, 400.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        for _ in 0..LUNGE_TICKS + 2 {
            step_idle(&mut m, &z, false);
        }
        assert!(m.f[0].pos.x > 130.0, "lunged forward: {}", m.f[0].pos.x);
        assert!(m.f[0].special_cd <= JUDO_WHIFF_COOLDOWN && m.f[0].special_cd > 0);
        assert!(m.effects.is_empty() && m.f[1].vel == Vec2::ZERO);
    }

    #[test]
    fn doug_lunges_into_a_grab_on_someone_a_little_way_off() {
        let z = zero_brain();
        let mut m = staged([Character::Doug, Character::Classic, Character::Classic, Character::Classic], [100.0, 145.0, 300.0, 400.0]);
        m.f[0].facing = 1.0;
        let wants = JudoThrow.bot_wants(&m, 0);
        assert!(wants, "a bot sees them in lunging range");
        step_idle(&mut m, &z, true);
        let mut thrown = false;
        for _ in 0..LUNGE_TICKS + 2 {
            step_idle(&mut m, &z, false);
            thrown |= m.effects.iter().any(|e| matches!(e, Effect::Judo(j) if j.victim == 1));
        }
        assert!(thrown, "grabbed and thrown");
        assert_eq!(m.f[0].special_cd > JUDO_WHIFF_COOLDOWN, true, "a real throw: full cooldown");
        assert!(m.f[0].pos.x < 140.0, "stopped to throw: {}", m.f[0].pos.x);
    }

    #[test]
    fn travis_steps_shake_those_on_the_ground_near_him() {
        let z = zero_brain();
        let mut m = staged([Character::Travis, Character::Classic, Character::Classic, Character::Classic], [100.0, 140.0, 300.0, 440.0]);
        m.f[2].pos.y = 150.0;
        m.f[2].grounded = false;
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        assert_eq!(m.f[0].bulked, BULK_TICKS);
        // Standing still: no steps, nothing shakes.
        for _ in 0..20 {
            step_idle(&mut m, &z, false);
        }
        assert_eq!(m.f[1].hp, MAX_HP);
        // Running at the fighter at 140: every step he's near enough hurts them.
        let run = Action { mv: 1.0, run: true, ..Action::default() };
        let mut quakes = 0;
        for _ in 0..SECOND {
            m.step_with([&z; N_FIGHTERS], Some((0, run)));
            quakes += m.events.iter().filter(|e| matches!(e, Event::Quake { .. })).count();
        }
        assert!(quakes >= 3, "a shockwave each step: {quakes}");
        assert!(m.f[1].hp < MAX_HP, "the one he ran at was shaken");
        assert!(m.f[1].hp >= MAX_HP - QUAKE_DAMAGE * 3.0, "but not too often: {}", m.f[1].hp);
        assert_eq!(m.f[3].hp, MAX_HP, "nobody far away");
        assert_eq!(m.f[0].hp, MAX_HP);
        // It wears off.
        for _ in 0..BULK_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert_eq!(m.f[0].bulked, 0);
        let hp = m.f[1].hp;
        m.f[0].pos.x = m.f[1].pos.x - 30.0;
        for _ in 0..30 {
            m.step_with([&z; N_FIGHTERS], Some((0, run)));
            assert!(!m.events.iter().any(|e| matches!(e, Event::Quake { .. })));
        }
        assert_eq!(m.f[1].hp, hp - 0.0, "no more quakes");
    }

    #[test]
    fn travis_landing_makes_a_big_quake_that_a_jump_dodges() {
        let z = zero_brain();
        let mut m = staged([Character::Travis, Character::Classic, Character::Classic, Character::Classic], [200.0, 230.0, 170.0, 440.0]);
        step_idle(&mut m, &z, true);
        // Travis hops; the fighter at 170 is up in the air when he comes down.
        m.step_with([&z; N_FIGHTERS], Some((0, Action { jump: true, ..Action::default() })));
        let mut big = false;
        let mut lifted = false;
        for _ in 0..SECOND {
            if m.f[0].vel.y > 0.0 && !lifted {
                lifted = true;
                m.f[2].pos.y -= 40.0;
                m.f[2].grounded = false;
            }
            step_idle(&mut m, &z, false);
            big |= m.events.iter().any(|e| matches!(e, Event::Quake { big: true, .. }));
            if big {
                break;
            }
        }
        assert!(!m.f[2].grounded);
        assert!(big, "landing shook the ground");
        assert_eq!(m.f[1].hp, MAX_HP - QUAKE_BIG_DAMAGE, "the one standing next to him was hurt");
        assert_eq!(m.f[2].hp, MAX_HP, "the one in the air wasn't");
    }

    #[test]
    fn matt_skates_far_and_fast_through_everyone_in_his_way() {
        let z = zero_brain();
        let mut m = staged([Character::Matt, Character::Classic, Character::Classic, Character::Classic], [60.0, 120.0, 200.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        // Pushing the other way doesn't turn him round.
        let back = Action { mv: -1.0, run: true, ..Action::default() };
        for _ in 0..SKATE_TICKS + 1 {
            m.step_with([&z; N_FIGHTERS], Some((0, back)));
        }
        assert!(m.f[0].pos.x > 60.0 + 180.0, "a long way: to x {}", m.f[0].pos.x);
        assert_eq!(m.f[1].hp, MAX_HP - SKATE_DAMAGE, "hit once on the way through");
        assert_eq!(m.f[2].hp, MAX_HP - SKATE_DAMAGE, "and the next one too");
        assert_eq!(m.f[3].hp, MAX_HP, "not the one out of reach");
        assert_eq!(m.f[0].skate, 0);
    }

    #[test]
    fn matts_skate_frees_him_of_the_eye_of_the_storm_and_the_chosen_one_but_nobody_else() {
        let z = zero_brain();
        // Frozen by Michelle (with a frozen Classic in his path).
        let mut m = staged([Character::Matt, Character::Michelle, Character::Classic, Character::Classic], [100.0, 400.0, 160.0, 440.0]);
        m.f[0].facing = 1.0;
        let mut h = [None; N_FIGHTERS];
        h[0] = Some(Action::default());
        h[1] = Some(Action { special: true, ..Action::default() });
        m.step_humans([&z; N_FIGHTERS], h);
        assert!(m.f[0].pinned > 0 && m.f[2].pinned > 0);
        step_idle(&mut m, &z, true);
        for _ in 0..SKATE_TICKS + 1 {
            step_idle(&mut m, &z, false);
        }
        assert!(m.f[0].pos.x > 200.0, "skated off out of the pin: {}", m.f[0].pos.x);
        assert!(m.f[2].pinned > 0 && m.f[2].hp < MAX_HP, "the one he went through is still pinned");
        // Ridden by the Chosen One: he takes his body back, and the rider's put out.
        let mut m = staged([Character::Matt, Character::Shawn, Character::Classic, Character::Classic], [100.0, 300.0, 380.0, 440.0]);
        m.f[1].special_cd = 0;
        let mut h = [None; N_FIGHTERS];
        h[1] = Some(Action { special: true, cursor: Some(vec2(100.0, 240.0)), ..Action::default() });
        m.step_humans([&z; N_FIGHTERS], h);
        assert!(m.f[0].ridden > 0);
        let mut h = [None; N_FIGHTERS];
        h[0] = Some(Action { special: true, ..Action::default() });
        h[1] = Some(Action::default());
        m.step_humans([&z; N_FIGHTERS], h);
        step_idle(&mut m, &z, false);
        assert!(m.f[0].ridden == 0 && m.f[1].host.is_none() && m.f[1].present(), "the rider's out");
        assert!(m.f[0].skate > 0 || m.f[0].pos.x > 110.0, "and Matt's off");
    }

    #[test]
    fn the_chosen_one_inside_someone_fires_their_special_if_its_ready() {
        let z = zero_brain();
        let mut m = staged([Character::Shawn, Character::Tyler, Character::Classic, Character::Classic], [100.0, 200.0, 380.0, 440.0]);
        m.f[0].special_cd = 0;
        let ride = Action { special: true, cursor: Some(vec2(200.0, 240.0)), ..Action::default() };
        let mut h = [None; N_FIGHTERS];
        h[0] = Some(ride);
        h[1] = Some(Action::default());
        m.step_humans([&z; N_FIGHTERS], h);
        assert_eq!(m.f[0].host, Some(1));
        // The Chosen One presses special again inside Tyler: Tyler's hounds come out, on Tyler's cooldown.
        let mut h = [None; N_FIGHTERS];
        h[0] = Some(Action { special: true, ..Action::default() });
        h[1] = Some(Action::default());
        m.step_humans([&z; N_FIGHTERS], h);
        assert_eq!(m.f[1].special_cd, HOUNDS_COOLDOWN, "Tyler's special was used");
        assert_eq!(m.effects.iter().filter(|e| matches!(e, Effect::Pet(_))).count(), 3);
        // Spent: pressing again does nothing more.
        let mut h = [None; N_FIGHTERS];
        h[0] = Some(Action { special: true, ..Action::default() });
        h[1] = Some(Action::default());
        m.step_humans([&z; N_FIGHTERS], h);
        assert_eq!(m.effects.iter().filter(|e| matches!(e, Effect::Pet(_))).count(), 3);
    }

    #[test]
    fn zacks_tornadoes_drag_people_in_and_fling_them_out() {
        let z = zero_brain();
        // Someone standing still 60px past where the right-hand tornado spins up.
        let mut m = staged([Character::Zack, Character::Classic, Character::Classic, Character::Classic], [200.0, 305.0, 30.0, 470.0]);
        step_idle(&mut m, &z, true);
        assert_eq!(m.effects.iter().filter(|e| matches!(e, Effect::Tornado(_))).count(), 2);
        let (mut swept, mut flung) = (false, false);
        for _ in 0..TORNADO_TICKS {
            step_idle(&mut m, &z, false);
            swept |= m.f[1].pos.y < crate::sim::FLOOR_Y - 20.0;
            flung |= m.f[1].vel.x.abs() > 200.0;
        }
        assert!(swept, "swept up the funnel");
        assert!(flung, "and flung out of it");
        assert!(m.effects.iter().all(|e| !matches!(e, Effect::Tornado(_))), "gone after 5s");
        // A fighter running away from the edge of its reach gets away.
        let mut m = staged([Character::Zack, Character::Classic, Character::Classic, Character::Classic], [200.0, 320.0, 30.0, 470.0]);
        step_idle(&mut m, &z, true);
        let run = Action { mv: 1.0, run: true, ..Action::default() };
        for _ in 0..SECOND {
            m.step_with([&z; N_FIGHTERS], Some((1, run)));
        }
        assert!(m.f[1].pos.x > 360.0, "outran it: {}", m.f[1].pos.x);
    }

    #[test]
    fn zacks_own_tornadoes_pull_at_him_too() {
        let z = zero_brain();
        // Near the wall: the left-hand tornado bounces off it and comes back over him.
        let mut m = staged([Character::Zack, Character::Classic, Character::Classic, Character::Classic], [60.0, 300.0, 380.0, 470.0]);
        step_idle(&mut m, &z, true);
        let mut swept = false;
        for _ in 0..3 * SECOND {
            step_idle(&mut m, &z, false);
            swept |= m.f[0].pos.y < crate::sim::FLOOR_Y - 20.0;
        }
        assert!(swept, "Zack was carried up his own tornado");
    }

    #[test]
    fn in_a_tornado_you_can_still_swing_throw_and_use_your_special_but_not_steer() {
        use crate::sim::{Item, ItemKind};
        let z = zero_brain();
        // A Tyler holding a rock, right where the right-hand tornado spins up.
        let mut m = staged([Character::Zack, Character::Tyler, Character::Classic, Character::Classic], [200.0, 245.0, 30.0, 470.0]);
        let mut rock = Item::new(ItemKind::Rock, m.f[1].hand());
        rock.held_by = Some(1);
        m.items.push(rock);
        m.f[1].holding = Some(ItemKind::Rock);
        let mut h = [None; N_FIGHTERS];
        h[0] = Some(Action { special: true, ..Action::default() });
        h[1] = Some(Action::default());
        m.step_humans([&z; N_FIGHTERS], h);
        let tyler = |m: &mut Match, a: Action| {
            let mut h = [None; N_FIGHTERS];
            h[1] = Some(a);
            m.step_humans([&z; N_FIGHTERS], h);
        };
        for _ in 0..20 {
            tyler(&mut m, Action::default());
        }
        assert!(m.f[1].caught > 0 && m.f[1].hitstun == 0, "carried, but not stunned");
        // Pushing left (and dashing) doesn't get him out.
        let x = m.f[1].pos.x;
        for _ in 0..10 {
            tyler(&mut m, Action { mv: -1.0, run: true, dash: Some(-1.0), ..Action::default() });
        }
        assert!((m.f[1].pos.x - x).abs() < 8.0 && m.f[1].caught > 0, "still in it");
        tyler(&mut m, Action { attack: true, ..Action::default() });
        assert!(m.events.iter().any(|e| matches!(e, Event::Swing { who: 1 })), "swung");
        for _ in 0..30 {
            tyler(&mut m, Action::default());
        }
        tyler(&mut m, Action { throw: true, target: Some(vec2(-1.0, 0.0)), ..Action::default() });
        assert!(m.events.iter().any(|e| matches!(e, Event::Throw { who: 1 })), "threw");
        tyler(&mut m, Action { special: true, ..Action::default() });
        assert!(m.events.iter().any(|e| matches!(e, Event::Special { who: 1 })), "released the hounds");
    }

    #[test]
    fn matt_skates_out_of_a_tornado() {
        let z = zero_brain();
        // Matt right where the right-hand tornado spins up.
        let mut m = staged([Character::Zack, Character::Matt, Character::Classic, Character::Classic], [200.0, 245.0, 30.0, 470.0]);
        m.f[1].facing = 1.0;
        let mut h = [None; N_FIGHTERS];
        h[0] = Some(Action { special: true, ..Action::default() });
        h[1] = Some(Action::default());
        m.step_humans([&z; N_FIGHTERS], h);
        // (Matt as a player standing still: a bot Matt would skate straight out by himself.)
        for _ in 0..30 {
            let mut h = [None; N_FIGHTERS];
            h[1] = Some(Action::default());
            m.step_humans([&z; N_FIGHTERS], h);
        }
        assert!(m.f[1].caught > 0 && m.f[1].pos.y < crate::sim::FLOOR_Y - 5.0, "carried up it");
        let y = m.f[1].pos.y;
        let mut h = [None; N_FIGHTERS];
        h[1] = Some(Action { special: true, ..Action::default() });
        m.step_humans([&z; N_FIGHTERS], h);
        assert!(m.f[1].skate > 0, "skated");
        for _ in 0..10 {
            step_idle(&mut m, &z, false);
        }
        assert!(m.f[1].pos.x > 245.0 + 60.0, "rode straight out of it: {}", m.f[1].pos.x);
        assert!((m.f[1].pos.y - y).abs() < 3.0, "level, not lifted");
    }

    #[test]
    fn rocks_whipped_round_by_a_tornado_hurt_people_but_not_zack() {
        use crate::sim::{Item, ItemKind};
        let z = zero_brain();
        let mut m = staged([Character::Zack, Character::Classic, Character::Classic, Character::Classic], [200.0, 260.0, 30.0, 470.0]);
        for x in [230.0, 250.0, 270.0, 210.0] {
            m.items.push(Item::new(ItemKind::Rock, vec2(x, crate::sim::FLOOR_Y - crate::sim::ITEM_R)));
        }
        // (Pinned where they stand, so the tornado can't carry them off before a rock comes round.)
        m.f[1].pinned = TORNADO_TICKS;
        step_idle(&mut m, &z, true);
        let mut whirled = false;
        for _ in 0..TORNADO_TICKS {
            step_idle(&mut m, &z, false);
            whirled |= m.items.iter().any(|it| it.whirled && it.lethal && it.owner == Some(0));
        }
        assert!(whirled, "the rocks were whipped up");
        assert!(m.f[1].hp < MAX_HP, "and someone took a hit");
        assert_eq!(m.f[0].hp, MAX_HP, "never Zack");
    }

    #[test]
    fn vid_reads_shielded_while_his_books_go_after_people_close_by() {
        let z = zero_brain();
        let mut m = staged([Character::Vid, Character::Classic, Character::Classic, Character::Classic], [100.0, 140.0, 380.0, 440.0]);
        step_idle(&mut m, &z, true);
        assert!(m.f[0].reading > 0, "reading");
        assert_eq!(m.effects.iter().filter(|e| matches!(e, Effect::Book(_))).count(), BOOKS);
        // Shielded: hits stop at him.
        m.damage(1, 0, 30.0, vec2(200.0, -100.0), 10, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP, "the shield stopped it");
        assert!(m.events.iter().any(|e| matches!(e, Event::Blocked { .. })));
        // He can't go anywhere, whatever he presses.
        let x = m.f[0].pos.x;
        for _ in 0..2 * SECOND {
            m.step_with([&z; N_FIGHTERS], Some((0, Action { mv: 1.0, run: true, jump: true, attack: true, ..Action::default() })));
        }
        assert!((m.f[0].pos.x - x).abs() < 1.0, "sat still reading");
        // The books went after the one close by (not the ones far off).
        assert!(m.f[1].hp < MAX_HP, "the close one got hit");
        assert_eq!((m.f[2].hp, m.f[3].hp), (MAX_HP, MAX_HP));
        // When he's done reading, the books are put away and he can be hurt again.
        for _ in 0..READ_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert_eq!(m.f[0].reading, 0);
        assert!(m.effects.iter().all(|e| !matches!(e, Effect::Book(_))));
        m.damage(1, 0, 10.0, Vec2::ZERO, 0, 1.0, Source::Fist, false);
        assert_eq!(m.f[0].hp, MAX_HP - 10.0);
    }

    fn arrow_of(m: &Match) -> Option<Arrow> {
        m.effects.iter().find_map(|e| if let Effect::Arrow(a) = e { Some(a.clone()) } else { None })
    }

    #[test]
    fn kalebs_arrow_chases_his_cursor_through_foes() {
        let z = zero_brain();
        let mut m = staged([Character::Kaleb, Character::Classic, Character::Classic, Character::Classic], [100.0, 200.0, 380.0, 440.0]);
        let kaleb = |m: &mut Match, a: Action| m.step_with([&z; N_FIGHTERS], Some((0, a)));
        let aim = |at: Vec2, special: bool| Action { special, cursor: Some(at), ..Action::default() };
        kaleb(&mut m, aim(vec2(0.0, 0.0), true));
        let a = arrow_of(&m).expect("an arrow");
        assert_eq!(a.state, ArrowState::Hover);
        assert!(a.pos.y < m.f[0].pos.y - 15.0, "over his head");
        // Sent off at a spot past the fighter at 200: it goes through them and waits at the cursor.
        let spot = vec2(260.0, m.f[1].center().y);
        kaleb(&mut m, aim(spot, true));
        for _ in 0..SECOND / 2 {
            kaleb(&mut m, aim(spot, false));
        }
        let a = arrow_of(&m).unwrap();
        assert_eq!(a.state, ArrowState::Out);
        assert!(a.pos.distance(spot) < 0.5, "at the cursor");
        assert_eq!(m.f[1].hp, MAX_HP - ARROW_DAMAGE, "went through them");
        // Moving the cursor, it follows: up and over to the left.
        let up = vec2(150.0, 60.0);
        for _ in 0..SECOND / 2 {
            kaleb(&mut m, aim(up, false));
        }
        assert!(arrow_of(&m).unwrap().pos.distance(up) < 0.5, "followed the cursor");
        // Held right on someone, it hits them only every so often.
        let on = m.f[1].center();
        let before = m.f[1].hp;
        for _ in 0..SECOND {
            let at = m.f[1].center();
            kaleb(&mut m, aim(at, false));
        }
        let hits = ((before - m.f[1].hp) / ARROW_DAMAGE).round() as u32;
        assert!((1..=2).contains(&hits), "{hits} hits in a second held on them (from {on})");
        // Pressing again calls it back; again sends it off.
        kaleb(&mut m, aim(spot, true));
        assert_eq!(arrow_of(&m).unwrap().state, ArrowState::Back);
        for _ in 0..SECOND / 2 {
            kaleb(&mut m, aim(spot, false));
        }
        assert_eq!(arrow_of(&m).unwrap().state, ArrowState::Hover, "back over his head");
        kaleb(&mut m, aim(spot, true));
        assert_eq!(arrow_of(&m).unwrap().state, ArrowState::Out);
        for _ in 0..ARROW_TICKS {
            kaleb(&mut m, aim(spot, false));
        }
        assert!(arrow_of(&m).is_none(), "gone after 5s");
        assert_eq!(m.f[0].hp, MAX_HP);
    }

    fn bugs(m: &Match) -> Vec<Pet> {
        m.effects.iter().filter_map(|e| if let Effect::Pet(p) = e { Some(p.clone()) } else { None }).collect()
    }

    #[test]
    fn carries_bugs_scatter_every_way_then_hunt_people_down() {
        let z = zero_brain();
        // Two behind her (she faces right), one ahead.
        let mut m = staged([Character::Carrie, Character::Classic, Character::Classic, Character::Classic], [300.0, 120.0, 60.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        let out = bugs(&m);
        assert_eq!(out.len(), BUGS);
        assert_eq!(out.iter().filter(|b| matches!(b.breed, Breed::Spider(_))).count(), BUGS / 2);
        assert!(m.events.iter().any(|e| matches!(e, Event::LaptopOpen { who: 0 })), "the laptop came out");
        for _ in 0..10 {
            step_idle(&mut m, &z, false);
        }
        let at = laptop_at(&m.f[0]);
        let b = bugs(&m);
        assert!(b.iter().any(|b| b.pos.x < at.x - 10.0) && b.iter().any(|b| b.pos.x > at.x + 10.0), "left and right");
        assert!(b.iter().any(|b| b.pos.y < at.y - 10.0), "and up");
        let mut bitten = [0u32; N_FIGHTERS];
        for _ in 0..BUG_TICKS {
            step_idle(&mut m, &z, false);
            for e in &m.events {
                if let Event::Scratch { pos } = e {
                    let j = (1..N_FIGHTERS).min_by(|&a, &b| m.f[a].center().distance(*pos).total_cmp(&m.f[b].center().distance(*pos))).unwrap();
                    bitten[j] += 1;
                }
            }
        }
        assert!(bugs(&m).is_empty(), "all gone by the end");
        assert!(bitten[1] > 0, "they turned round and chased down the nearest one behind her: {bitten:?}");
        assert!(bitten[3] > 0, "and the one ahead: {bitten:?}");
        let total: f32 = (1..N_FIGHTERS).map(|j| MAX_HP - m.f[j].hp).sum();
        assert!(total <= BUGS as f32 * BUG_BITES as f32 * BUG_BITE, "each bug only bites a couple of times: {total}");
        assert_eq!(m.f[0].hp, MAX_HP, "never Carrie");
    }

    #[test]
    fn seths_minis_use_what_they_pick_up() {
        use crate::sim::{Item, ItemKind};
        let z = zero_brain();
        let mut m = staged([Character::Seth, Character::Classic, Character::Classic, Character::Classic], [100.0, 220.0, 400.0, 440.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        let mini = (N_FIGHTERS..N_BODIES).find(|&j| !m.f[j].absent).unwrap();
        for _ in 0..30 {
            step_idle(&mut m, &z, false);
        }
        // Hand it a rock: with a foe in range, it throws it (the brain alone just stands there).
        let mut rock = Item::new(ItemKind::Rock, m.f[mini].hand());
        rock.held_by = Some(mini);
        m.items.push(rock);
        m.f[mini].holding = Some(ItemKind::Rock);
        let mut thrown = false;
        for _ in 0..10 {
            step_idle(&mut m, &z, false);
            thrown |= m.events.iter().any(|e| matches!(e, Event::Throw { who } if *who == mini));
        }
        assert!(thrown, "the mini threw its rock");
    }
}
