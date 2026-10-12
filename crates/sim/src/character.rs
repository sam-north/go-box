//! Characters: every fighter shares the same base body (move, jump, melee, grab, throw, HP);
//! a character adds one special move on a cooldown. Each special is a `Special` strategy, so a
//! new character is a new entry in `Character` plus (optionally) a new `Special` impl.
//!
//! All tuning lives in the constants at the top of this file.

use crate::sim::{Aabb, Event, Fighter, Match, Source, BODY_H, DT, N_FIGHTERS, PLATFORMS, SOLIDS};
use glam::{vec2, Vec2};
use serde::{Deserialize, Serialize};

const SECOND: u32 = 60;

// Sam — Gravity Well.
const WELL_COOLDOWN: u32 = 30 * SECOND;
const WELL_TICKS: u32 = SECOND;
/// Fighters farther than this from the well feel nothing.
pub const WELL_RADIUS: f32 = 90.0;
/// Pull speed (px/s) for someone right next to Sam when he fires, falling off linearly to 0
/// at WELL_RADIUS. Each fighter's pull is fixed by where they stood at that moment, so the
/// far edge gets a ~25px nudge over the second and the near ones a strong tug.
/// Walking is 45 and running 85, so you can run out of the outer part but not the middle.
const WELL_PULL: f32 = 150.0;
/// The pull stops this far from the well: it tugs people in, it never stacks them on Sam.
const WELL_STOP: f32 = 8.0;
pub const WELL_POP_RADIUS: f32 = 16.0;
const WELL_POP_DAMAGE: f32 = 12.0;

// Austin — Pop-Up.
const POPUP_COOLDOWN: u32 = 15 * SECOND;
/// Austin never lands closer than this to a living opponent (if anywhere allows it).
const POPUP_MIN_GAP: f32 = 40.0;

// Tyler — Release the Hounds.
const HOUNDS_COOLDOWN: u32 = 30 * SECOND;
const HOUNDS: [f32; 3] = [110.0, 125.0, 140.0];
const HOUND_TICKS: u32 = 5 * SECOND;
const BITE_DAMAGE: f32 = 6.0;
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

    pub fn name(self) -> &'static str {
        match self {
            Character::Classic => "Classic",
            Character::Sam => "Sam",
            Character::Austin => "Austin",
            Character::Tyler => "Tyler",
        }
    }

    pub fn special(self) -> Option<&'static dyn Special> {
        match self {
            Character::Classic => None,
            Character::Sam => Some(&GravityWell),
            Character::Austin => Some(&PopUp),
            Character::Tyler => Some(&Hounds),
        }
    }
}

/// One character's special move.
pub trait Special: Sync {
    fn name(&self) -> &'static str;
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

struct GravityWell;

impl Special for GravityWell {
    fn name(&self) -> &'static str {
        "Gravity Well"
    }
    fn cooldown(&self) -> u32 {
        WELL_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let pos = m.f[who].pos;
        let pull = std::array::from_fn(|j| {
            let d = m.f[j].pos.distance(pos);
            if j == who || !m.f[j].alive() || d >= WELL_RADIUS { 0.0 } else { WELL_PULL * (1.0 - d / WELL_RADIUS) }
        });
        m.wells.push(Well { pos, owner: who, ticks: WELL_TICKS, pull });
    }
    fn bot_wants(&self, m: &Match, who: usize) -> bool {
        let me = m.f[who].center();
        let near = foes(m, who).map(|f| f.center().distance(me)).filter(|&d| d < WELL_RADIUS * 0.8).count();
        near >= 2 || foes(m, who).any(|f| f.center().distance(me) < 30.0)
    }
}

struct PopUp;

impl Special for PopUp {
    fn name(&self) -> &'static str {
        "Pop-Up"
    }
    fn cooldown(&self) -> u32 {
        POPUP_COOLDOWN
    }
    fn activate(&self, m: &mut Match, who: usize) {
        let from = m.f[who].center();
        // The best of a handful of random standing spots: the first far enough from everyone,
        // else whichever is farthest from its nearest opponent.
        let mut best = (f32::MIN, m.f[who].pos);
        for _ in 0..24 {
            let spot = m.random_surface_spot();
            let gap = foes(m, who).map(|f| f.pos.distance(spot)).fold(f32::MAX, f32::min);
            if gap > best.0 {
                best = (gap, spot);
            }
            if gap >= POPUP_MIN_GAP {
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
    fn name(&self) -> &'static str {
        "Release the Hounds"
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

/// Sam's well: pulls nearby fighters in for a second, then pops.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Well {
    pub pos: Vec2,
    pub owner: usize,
    pub ticks: u32,
    /// Pull speed on each fighter, fixed when the well opened.
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

    /// Advances wells and dogs. Nothing happens (and nothing is drawn from the RNG) when there
    /// are none, so classic matches are unchanged.
    pub(crate) fn step_effects(&mut self) {
        for w in 0..self.wells.len() {
            let Well { pos, owner, pull: strength, .. } = self.wells[w];
            for j in 0..N_FIGHTERS {
                if strength[j] > 0.0 && self.f[j].alive() {
                    pull(&mut self.f[j], pos, strength[j]);
                }
            }
            self.wells[w].ticks -= 1;
            if self.wells[w].ticks == 0 {
                self.events.push(Event::WellPop { pos });
                for j in 0..N_FIGHTERS {
                    if j == owner || !self.f[j].alive() {
                        continue;
                    }
                    let to = self.f[j].center() - pos;
                    if to.length() < WELL_POP_RADIUS {
                        let dir = if to.x == 0.0 { 1.0 } else { to.x.signum() };
                        self.damage(owner, j, WELL_POP_DAMAGE, vec2(dir * 90.0, -120.0), 10, dir, Source::Special, false);
                    }
                }
            }
        }
        self.wells.retain(|w| w.ticks > 0);

        for d in 0..self.dogs.len() {
            run_dog(&mut self.dogs[d]);
            let dog = &self.dogs[d];
            let (body, owner, dir) = (dog.body(), dog.owner, dog.dir);
            for j in 0..N_FIGHTERS {
                if j == owner || !self.f[j].alive() || self.dogs[d].bite_cd[j] > 0 || !body.overlaps(&self.f[j].body()) {
                    continue;
                }
                self.dogs[d].bite_cd[j] = BITE_EVERY;
                self.damage(owner, j, BITE_DAMAGE, vec2(dir * 120.0, -90.0), 8, dir, Source::Special, false);
            }
            let dog = &mut self.dogs[d];
            dog.bite_cd.iter_mut().for_each(|t| *t = t.saturating_sub(1));
            dog.ticks -= 1;
        }
        self.dogs.retain(|d| d.ticks > 0);
    }
}

/// Drags a fighter toward the well at `strength` px/s. Horizontal pull moves them
/// directly (so it works against their footing), stopping short of the well; in the air it
/// also bends their fall toward it.
fn pull(f: &mut Fighter, well: Vec2, strength: f32) {
    let to = well - f.pos;
    let d = to.length();
    if d <= WELL_STOP {
        return;
    }
    let dir = to / d;
    let room = (to.x.abs() - WELL_STOP).max(0.0);
    let dx = (dir.x * strength * DT).clamp(-room, room);
    let mut moved = f.pos.x + dx;
    let body = |x: f32| Aabb::new(x - crate::sim::HALF_W, f.pos.y - BODY_H, crate::sim::HALF_W * 2.0, BODY_H);
    if let Some(s) = SOLIDS.iter().find(|s| body(moved).overlaps(s)) {
        moved = if dx > 0.0 { s.x - crate::sim::HALF_W } else { s.x + s.w + crate::sim::HALF_W };
    }
    f.pos.x = moved;
    if !f.grounded {
        f.vel.y += dir.y * strength * 4.0 * DT;
    }
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
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn brains() -> Vec<Brain> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../brains");
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        paths.sort();
        paths.iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
    }

    /// A match with the given cast, everyone standing still on the floor at chosen x positions.
    fn staged(cast: [Character; N_FIGHTERS], xs: [f32; N_FIGHTERS]) -> Match {
        let mut m = Match::new(&mut SmallRng::seed_from_u64(7), 0..=0);
        m.items.clear();
        for i in 0..N_FIGHTERS {
            m.f[i].character = cast[i];
            m.f[i].pos = vec2(xs[i], 164.0);
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
        let brains = brains();
        let mut fired = [0u32; 4];
        for seed in 0..12u64 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let mut m = Match::new(&mut rng, HEALS_PER_ROUND);
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
    fn austin_pops_up_away_from_everyone() {
        let z = zero_brain();
        for seed in 0..20 {
            let mut m = staged([Character::Austin, Character::Classic, Character::Classic, Character::Classic], [150.0, 140.0, 160.0, 170.0]);
            m.rng_reseed(seed);
            step_idle(&mut m, &z, true);
            let a = &m.f[0];
            let gap = m.f[1..].iter().map(|f| f.pos.distance(a.pos)).fold(f32::MAX, f32::min);
            assert!(gap >= POPUP_MIN_GAP, "landed {gap} from a foe");
            assert!(a.grounded && a.vel == Vec2::ZERO);
            assert_eq!(m.events.iter().filter(|e| matches!(e, Event::Poof { .. })).count(), 2);
        }
    }

    #[test]
    fn sam_pulls_nearby_harder_than_far_and_pops() {
        let z = zero_brain();
        let mut m = staged([Character::Sam, Character::Classic, Character::Classic, Character::Classic], [160.0, 130.0, 85.0, 300.0]);
        let start: Vec<f32> = m.f.iter().map(|f| f.pos.x).collect();
        step_idle(&mut m, &z, true);
        for _ in 0..WELL_TICKS - 2 {
            step_idle(&mut m, &z, false);
        }
        let near_moved = m.f[1].pos.x - start[1];
        let far_moved = m.f[2].pos.x - start[2];
        // Near (30px out) is hauled in to the stop distance; far (75px out) only nudged.
        assert!((160.0 - m.f[1].pos.x - WELL_STOP).abs() < 0.5, "near ended {} from Sam", 160.0 - m.f[1].pos.x);
        assert!(near_moved > 15.0, "near {near_moved}");
        assert!(far_moved > 10.0 && far_moved < 30.0, "far should be nudged, moved {far_moved}");
        assert_eq!(m.f[3].pos.x, start[3], "out of range: untouched");
        // Let it pop; the near fighter is in the blast.
        for _ in 0..4 {
            step_idle(&mut m, &z, false);
        }
        assert!(m.wells.is_empty());
        assert!(m.f[1].hp < MAX_HP && m.f[0].dmg_special > 0.0);
        assert_eq!(m.f[0].hp, MAX_HP, "Sam is unaffected");
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
        assert!(m.f[0].dmg_special > 0.0);
    }
}
