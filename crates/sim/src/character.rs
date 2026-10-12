//! Characters: every fighter shares the same base body (move, jump, melee, grab, throw, HP);
//! a character adds one special move on a cooldown. Each special is a `Special` strategy, so a
//! new character is a new entry in `Character` plus (optionally) a new `Special` impl.
//!
//! All tuning lives in the constants at the top of this file.

use crate::sim::{Aabb, Event, Fighter, Match, Source, DT, N_FIGHTERS, PLATFORMS, SOLIDS, W};
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

// Tyler — Release the Hounds.
const HOUNDS_COOLDOWN: u32 = 30 * SECOND;
const HOUNDS: [f32; 3] = [110.0, 125.0, 140.0];
const HOUND_TICKS: u32 = 5 * SECOND;
const BITE_DAMAGE: f32 = 15.0;
const BITE_EVERY: u32 = SECOND / 2;
pub const DOG_HALF_W: f32 = 3.0;
pub const DOG_H: f32 = 5.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Character {
    /// The base fighter the brains were trained as: no special.
    #[default]
    Classic,
    Sam,
    Austin,
    Tyler,
}

impl Character {
    pub const ALL: [Character; 4] = [Character::Classic, Character::Sam, Character::Austin, Character::Tyler];

    /// A stable id (lowercase, no spaces): names it in text files, settings and the command
    /// line. What players see comes from the client's text files (`character.<id>`).
    pub fn id(self) -> &'static str {
        match self {
            Character::Classic => "classic",
            Character::Sam => "sam",
            Character::Austin => "austin",
            Character::Tyler => "tyler",
        }
    }

    pub fn special(self) -> Option<&'static dyn Special> {
        match self {
            Character::Classic => None,
            Character::Sam => Some(&GravityBomb),
            Character::Austin => Some(&PopUp),
            Character::Tyler => Some(&Hounds),
        }
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

/// Living opponents of `who`, nearest first is not guaranteed.
fn foes(m: &Match, who: usize) -> impl Iterator<Item = &Fighter> {
    m.f.iter().enumerate().filter(move |&(j, f)| j != who && f.alive()).map(|(_, f)| f)
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
        m.bombs.push(new_bomb(&m.f[who], who));
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
        let me = &m.f[who];
        let c = me.center();
        let crowded = foes(m, who).any(|f| f.center().distance(c) < 24.0);
        // A projectile about to hit: lethal, not mine, close and closing.
        let incoming = m.items.iter().any(|it| it.lethal && it.owner != Some(who) && it.pos.distance(c) < 36.0 && (c - it.pos).dot(it.vel) > 0.0);
        let grenade = m.items.iter().any(|it| it.fuse.is_some_and(|t| t < 30) && it.pos.distance(c) < 30.0);
        incoming || grenade || (crowded && me.hp < 40.0)
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
            m.dogs.push(Dog {
                pos: f.pos + vec2(f.facing * (2.0 + 4.0 * k as f32), 0.0),
                dir: f.facing,
                speed,
                vy: 0.0,
                owner: who,
                ticks: HOUND_TICKS,
                bite_cd: [0; N_FIGHTERS],
            });
        }
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = &m.f[who];
        // Someone on my level, ahead of me, within a run's reach.
        foes(m, who).any(|f| (f.pos.y - me.pos.y).abs() < 10.0 && (f.pos.x - me.pos.x) * me.facing > 0.0 && (f.pos.x - me.pos.x).abs() < 160.0)
    }
}

/// Sam's gravity bomb: flying, then (once it goes off) crunching everyone nearby together.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub pull: [f32; N_FIGHTERS],
}

/// One of Tyler's dogs: runs along floors, turns at walls, bites anyone but Tyler.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dog {
    /// Feet centre.
    pub pos: Vec2,
    pub dir: f32,
    pub speed: f32,
    pub vy: f32,
    pub owner: usize,
    pub ticks: u32,
    #[serde(skip)]
    bite_cd: [u32; N_FIGHTERS],
}

impl Dog {
    pub fn body(&self) -> Aabb {
        Aabb::new(self.pos.x - DOG_HALF_W, self.pos.y - DOG_H, DOG_HALF_W * 2.0, DOG_H)
    }
    pub fn center(&self) -> Vec2 {
        vec2(self.pos.x, self.pos.y - DOG_H * 0.5)
    }
}

impl Match {
    /// Fires specials that were asked for and are ready, and counts down cooldowns.
    pub(crate) fn step_specials(&mut self, wants: [bool; N_FIGHTERS]) {
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

    /// Advances bombs and dogs. Nothing happens (and nothing is drawn from the RNG) when there
    /// are none, so classic matches are unchanged.
    pub(crate) fn step_effects(&mut self) {
        for b in 0..self.bombs.len() {
            match self.bombs[b].crunch {
                None => self.fly_bomb(b),
                Some(t) => self.crunch_bomb(b, t),
            }
        }
        self.bombs.retain(|b| b.crunch != Some(0));

        for d in 0..self.dogs.len() {
            run_dog(&mut self.dogs[d]);
            let dog = &self.dogs[d];
            let (body, owner, dir) = (dog.body(), dog.owner, dog.dir);
            for j in 0..N_FIGHTERS {
                if j == owner || !self.f[j].alive() || self.dogs[d].bite_cd[j] > 0 || !body.overlaps(&self.f[j].body()) {
                    continue;
                }
                self.dogs[d].bite_cd[j] = BITE_EVERY;
                self.events.push(Event::Bite { pos: self.f[j].center() });
                self.damage(owner, j, BITE_DAMAGE, vec2(dir * 120.0, -90.0), 8, dir, Source::Special, false);
            }
            let dog = &mut self.dogs[d];
            dog.bite_cd.iter_mut().for_each(|t| *t = t.saturating_sub(1));
            dog.ticks -= 1;
        }
        self.dogs.retain(|d| d.ticks > 0);
    }
}

impl Match {
    /// A bomb in flight: arcs under gravity and goes off on terrain, on a foe, or when its
    /// fuse runs out.
    fn fly_bomb(&mut self, b: usize) {
        if !fly(&mut self.bombs[b], &self.f) {
            return;
        }
        let (pos, owner) = (self.bombs[b].pos, self.bombs[b].owner);
        let pull = std::array::from_fn(|j| if j == owner || !self.f[j].alive() { 0.0 } else { pull_strength(self.f[j].center().distance(pos)) });
        let bomb = &mut self.bombs[b];
        bomb.vel = Vec2::ZERO;
        bomb.crunch = Some(BOMB_CRUNCH);
        bomb.pull = pull;
        self.events.push(Event::BombCrunch { pos });
    }

    /// A bomb that's gone off: yanks everyone toward the middle (they can't fight it, and the
    /// far side of the map feels it too, just less),
    /// then explodes.
    fn crunch_bomb(&mut self, b: usize, left: u32) {
        let Bomb { pos, owner, pull, .. } = self.bombs[b];
        let left = left - 1;
        self.bombs[b].crunch = Some(left);
        for j in 0..N_FIGHTERS {
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
        for j in 0..N_FIGHTERS {
            let d = self.f[j].center().distance(pos);
            if j == owner || !self.f[j].alive() || d >= BOMB_BLAST_RADIUS {
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
fn fly(bomb: &mut Bomb, fighters: &[Fighter; N_FIGHTERS]) -> bool {
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
        && let Some(p) = PLATFORMS.iter().find(|p| bomb.pos.x > p.x && bomb.pos.x < p.x + p.w && prev.y <= p.y && bomb.pos.y >= p.y)
    {
        bomb.pos.y = p.y - 0.5;
        hit = true;
    }
    let pos = bomb.pos;
    hit || (0..N_FIGHTERS).any(|j| j != bomb.owner && fighters[j].alive() && fighters[j].body().overlaps(&Aabb::around(pos, 2.0)))
}

/// Where a bomb Sam threw right now would go off: flown through the same physics, everyone
/// else standing still.
fn bomb_landing(m: &Match, who: usize) -> Vec2 {
    let mut b = new_bomb(&m.f[who], who);
    while !fly(&mut b, &m.f) {}
    b.pos
}

fn new_bomb(f: &Fighter, who: usize) -> Bomb {
    let vel = vec2(BOMB_THROW.x * f.facing, BOMB_THROW.y) + f.vel * 0.5;
    Bomb { pos: f.hand(), vel, owner: who, fuse: BOMB_FUSE, crunch: None, pull: [0.0; N_FIGHTERS] }
}

/// How hard a gravity bomb yanks someone `d` px away: fully within BOMB_PULL_RADIUS, then
/// weaker with distance (but never nothing).
fn pull_strength(d: f32) -> f32 {
    if d <= BOMB_PULL_RADIUS { 1.0 } else { libm::powf(BOMB_PULL_RADIUS / d, BOMB_PULL_FALLOFF) }
}

fn run_dog(dog: &mut Dog) {
    dog.pos.x += dog.dir * dog.speed * DT;
    if let Some(s) = SOLIDS.iter().find(|s| dog.body().overlaps(s)) {
        dog.pos.x = if dog.dir > 0.0 { s.x - DOG_HALF_W } else { s.x + s.w + DOG_HALF_W };
        dog.dir = -dog.dir;
    }
    let prev = dog.pos.y;
    dog.vy = (dog.vy + 640.0 * DT).min(320.0);
    dog.pos.y += dog.vy * DT;
    if let Some(s) = SOLIDS.iter().find(|s| dog.body().overlaps(s)) {
        dog.pos.y = if dog.vy > 0.0 { s.y } else { s.y + s.h + DOG_H };
        dog.vy = 0.0;
    }
    if dog.vy >= 0.0 {
        for p in PLATFORMS.iter() {
            let over = dog.pos.x + DOG_HALF_W > p.x && dog.pos.x - DOG_HALF_W < p.x + p.w;
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
    use crate::sim::{Action, HEALS_PER_ROUND, MAX_HP};

    /// A match with the given cast, everyone standing still on the floor at chosen x positions.
    fn staged(cast: [Character; N_FIGHTERS], xs: [f32; N_FIGHTERS]) -> Match {
        let mut m = Match::new(7, 0..=0);
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
    fn bots_use_every_special_and_matches_finish() {
        let brains = Brain::committed();
        let mut fired = [0u32; 4];
        for seed in 0..12u64 {
            let mut m = Match::new(seed, HEALS_PER_ROUND);
            for i in 0..N_FIGHTERS {
                m.f[i].character = Character::ALL[(seed as usize + i) % 4];
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
        assert_eq!(fired[0], 0, "Classic has no special");
        assert!(fired[1..].iter().all(|&n| n > 0), "every special should get used by bots: {fired:?}");
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
            let gap = m.f[1..].iter().map(|f| f.pos.distance(a.pos)).fold(f32::MAX, f32::min);
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
        assert_eq!(m.bombs.len(), 1);
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
        assert!(m.bombs.is_empty(), "it exploded");
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
    fn tylers_hounds_bite_others_then_vanish() {
        let z = zero_brain();
        let mut m = staged([Character::Tyler, Character::Classic, Character::Classic, Character::Classic], [40.0, 120.0, 200.0, 280.0]);
        m.f[0].facing = 1.0;
        step_idle(&mut m, &z, true);
        assert_eq!(m.dogs.len(), 3);
        for _ in 0..HOUND_TICKS {
            step_idle(&mut m, &z, false);
        }
        assert!(m.dogs.is_empty(), "gone after 5s");
        assert_eq!(m.f[0].hp, MAX_HP, "dogs never bite Tyler");
        assert!(m.f[1].hp < MAX_HP, "the fighter in their path got bitten");
        assert!(m.f[0].dmg_dealt > 0.0);
    }
}
