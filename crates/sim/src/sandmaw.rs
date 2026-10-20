//! The Sandmaw (`BossKind::Sandmaw`, on the Dune map): a pit monster whose mouth is the middle of
//! the floor. It never moves; its body is the ring of its mouth (lips and teeth), so swings from the
//! edge, throws, blasts and a Molotov's fire all hurt it, and grenades dropped into it go off right
//! on it. Its throat swallows whoever touches it. Its attacks (each picked at someone, after a
//! wind-up the client draws):
//! - Tentacles: burst out of the sand under people (the sand bubbles first) and fling whoever they
//!   catch toward the mouth.
//! - Inhale: sucks everyone (and everything lying about) toward the mouth for a few seconds; a full
//!   run gets you away.
//! - Chomp: snaps shut, biting anyone over or at its mouth, and shockwaves run out along the floor.
//! - Acid: globs spat in arcs at people (they splatter on ledges, too). And between attacks it
//!   spits one now and then.

use crate::boss::{lob, BossMove, BossShot, BossState, ShotKind, Shockwave, RAGE_REST, REST, WAVE_W};
use crate::character::Effect;
use crate::sim::{secs, Aabb, Action, Event, Match, Source, BOSS, DT, FLOOR_Y, GRAVITY, ITEM_R, W, WALL};
use glam::{vec2, Vec2};
use serde::{Deserialize, Serialize};

/// Its body (the ring of its mouth) either side of the middle, and how far above the floor it rises.
pub const BODY_HALF_W: f32 = 60.0;
pub const BODY_H: f32 = 22.0;
/// Its throat: the stretch of floor that swallows whoever touches it.
pub const THROAT: (f32, f32) = (W / 2.0 - 48.0, W / 2.0 + 48.0);

/// How fast its slick slopes slide you toward its mouth (px/s; a full run is 150).
const SLIDE: f32 = 70.0;

// Tentacles.
const TENTACLE_WINDUP: u32 = 16;
const TENTACLE_TOTAL: u32 = 70;
const TENTACLES: u32 = 2;
const RAGE_TENTACLES: u32 = 3;
/// The sand bubbles this long before it bursts, then it rises, holds, and sinks back.
pub const T_WARN: u32 = 40;
pub const T_RISE: u32 = 7;
pub const T_HOLD: u32 = 22;
pub const T_SINK: u32 = 10;
/// How high it reaches, and how wide it is.
pub const T_REACH: f32 = 84.0;
pub const T_W: f32 = 10.0;
const TENTACLE_DAMAGE: f32 = 16.0;

// Inhale.
const INHALE_WINDUP: u32 = 20;
const INHALE_TICKS: u32 = 150;
const RAGE_INHALE_TICKS: u32 = 200;
/// How hard it pulls (px/s, on top of however you're moving: a full run is 150).
// (On top of its slopes' slide, so together they're just short of a run: on a slope, you'll want
// to jump too.)
const PULL: f32 = 50.0;
const RAGE_PULL: f32 = 65.0;

// Chomp.
const CHOMP_WINDUP: u32 = 30;
const CHOMP_TOTAL: u32 = 52;
/// How far above its mouth (and past its lips) the bite reaches.
const CHOMP_ABOVE: f32 = 60.0;
const CHOMP_DAMAGE: f32 = 24.0;
const WAVE_SPEED: f32 = 180.0;
const RAGE_WAVE_SPEED: f32 = 240.0;

// Acid.
const ACID_WINDUP: u32 = 20;
const ACID_EVERY: u32 = 8;
const GLOBS: u32 = 4;
const RAGE_GLOBS: u32 = 6;
const GLOB_FLIGHT: f32 = 0.95;
const GLOB_GRAVITY: f32 = GRAVITY * 0.6;
const GLOB_DAMAGE: f32 = 12.0;
const GLOB_TICKS: u32 = secs(4);
pub const GLOB_R: f32 = 3.0;
/// And between attacks, one at someone every so often.
const SPIT_EVERY: u32 = 60;
const RAGE_SPIT_EVERY: u32 = 40;

/// (ticks of wind-up before it lands, ticks all told), for its moves.
pub fn timing(mv: BossMove) -> (u32, u32) {
    match mv {
        BossMove::Tentacles => (TENTACLE_WINDUP, TENTACLE_TOTAL),
        BossMove::Inhale => (INHALE_WINDUP, INHALE_WINDUP + RAGE_INHALE_TICKS),
        BossMove::Chomp => (CHOMP_WINDUP, CHOMP_TOTAL),
        BossMove::Acid => (ACID_WINDUP, ACID_WINDUP + ACID_EVERY * RAGE_GLOBS + 10),
        _ => (0, 0),
    }
}

/// A tentacle bursting out of the sand (after the sand bubbles a while).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tentacle {
    /// Where it comes up, on the floor.
    pub pos: Vec2,
    /// Ticks since the sand started bubbling.
    pub t: u32,
    pub ticks: u32,
    /// Who it's already hit (a bit per body).
    #[serde(skip)]
    pub hit: u32,
}

impl Tentacle {
    pub fn new(x: f32) -> Self {
        Self { pos: vec2(x, FLOOR_Y), t: 0, ticks: T_WARN + T_RISE + T_HOLD + T_SINK, hit: 0 }
    }

    /// How high it reaches now (0 while the sand is still bubbling).
    pub fn reach(&self) -> f32 {
        let t = self.t;
        if t < T_WARN {
            0.0
        } else if t < T_WARN + T_RISE {
            T_REACH * (t - T_WARN) as f32 / T_RISE as f32
        } else if t < T_WARN + T_RISE + T_HOLD {
            T_REACH
        } else {
            T_REACH * (1.0 - (t - T_WARN - T_RISE - T_HOLD) as f32 / T_SINK as f32).max(0.0)
        }
    }

    pub fn area(&self) -> Aabb {
        let r = self.reach();
        Aabb::new(self.pos.x - T_W / 2.0, self.pos.y - r, T_W, r)
    }
}

impl Match {
    /// The Sandmaw's tick (after `step_boss` has seen to its bursting up and its rage).
    pub(crate) fn step_sandmaw(&mut self, b: &mut BossState) {
        self.swallow();
        // Between attacks (and through all but its own volleys), a glob at someone now and then.
        if b.mv != BossMove::Acid {
            b.spit = b.spit.saturating_sub(1);
            if b.spit == 0 {
                b.spit = if b.enraged { RAGE_SPIT_EVERY } else { SPIT_EVERY };
                if let Some(j) = self.boss_pick() {
                    self.maw_glob(j, 0.0);
                }
            }
        }
        b.t += 1;
        match b.mv {
            BossMove::Rest => {
                if b.t >= b.rest {
                    self.maw_choose(b);
                }
            }
            mv => {
                self.maw_attack(b);
                if b.t >= timing(mv).1 {
                    *b = BossState { mv: BossMove::Rest, t: 0, rest: if b.enraged { RAGE_REST } else { REST }, ..*b };
                }
            }
        }
    }

    /// Whoever's standing in its throat is swallowed whole; and whatever's lying there (but a lit
    /// grenade, which goes off in its mouth) is swallowed too.
    fn swallow(&mut self) {
        let (t0, t1) = THROAT;
        let in_throat = |p: Vec2| p.x > t0 && p.x < t1 && p.y >= FLOOR_Y - 1.0;
        for j in self.boss_targets() {
            let f = &self.f[j];
            if !(f.grounded && in_throat(f.pos)) {
                continue;
            }
            let pos = f.pos;
            if self.protected() {
                // (Not at the start: thrown back out.)
                let out = if pos.x < W / 2.0 { -1.0 } else { 1.0 };
                self.f[j].vel = vec2(out * 220.0, -260.0);
                self.f[j].grounded = false;
                continue;
            }
            let hp = self.f[j].hp;
            self.damage(BOSS, j, hp + 1.0, Vec2::ZERO, 0, 0.0, Source::Boss, false);
            if !self.f[j].alive() {
                self.events.push(Event::Swallowed { pos });
            }
        }
        // Dogs, cats and spiders running into it are swallowed too (not flies: they fly over).
        let mut gulps = Vec::new();
        for e in self.effects.iter_mut() {
            if let Effect::Pet(p) = e
                && !matches!(p.breed, crate::character::Breed::Fly(_))
                && p.ticks > 0
                && in_throat(p.pos)
            {
                p.ticks = 0;
                gulps.push(p.pos);
            }
        }
        self.events.extend(gulps.into_iter().map(|pos| Event::Swallowed { pos }));
        let eaten = |it: &crate::sim::Item| it.held_by.is_none() && it.fuse.is_none() && it.carries.is_none() && !it.lethal && in_throat(it.pos + vec2(0.0, ITEM_R + 1.0));
        if self.items.iter().any(eaten) {
            self.items.retain(|it| !eaten(it));
        }
    }

    /// Picks its next attack: a chomp at anyone over or at its mouth, else something at random.
    fn maw_choose(&mut self, b: &mut BossState) {
        let me = &self.f[BOSS];
        let over = self.boss_targets().into_iter().any(|j| {
            let f = &self.f[j];
            (f.pos.x - me.pos.x).abs() < BODY_HALF_W + 16.0 && f.pos.y > FLOOR_Y - CHOMP_ABOVE
        });
        let mv = if over && self.chance(0.6) {
            BossMove::Chomp
        } else {
            let options = [(BossMove::Tentacles, 4), (BossMove::Acid, 3), (BossMove::Inhale, 2), (BossMove::Chomp, 2)];
            let total: u32 = options.iter().map(|o| o.1).sum();
            let mut roll = self.roll(total);
            options.iter().find(|o| if roll < o.1 { true } else { roll -= o.1; false }).map_or(BossMove::Tentacles, |o| o.0)
        };
        *b = BossState { mv, t: 0, hit: 0, ..*b };
    }

    fn maw_attack(&mut self, b: &mut BossState) {
        let (windup, _) = timing(b.mv);
        let me = self.f[BOSS].clone();
        match b.mv {
            BossMove::Tentacles if b.t == windup => {
                // Under people (on the floor, or under a ledge they're on), clear of its throat.
                let n = if b.enraged { RAGE_TENTACLES } else { TENTACLES };
                for k in 0..n {
                    let Some(j) = self.boss_pick() else { break };
                    let lead = self.f[j].vel.x * 0.3 + (k as f32 - 1.0) * 6.0;
                    let mut x = (self.f[j].pos.x + lead).clamp(WALL + T_W, W - WALL - T_W);
                    let (t0, t1) = THROAT;
                    if x > t0 - T_W && x < t1 + T_W {
                        x = if x < W / 2.0 { t0 - T_W } else { t1 + T_W };
                    }
                    let mut t = Tentacle::new(x);
                    t.pos.y = self.arena().ground_at(x, 0.0);
                    self.add_effect(Effect::Tentacle(t));
                }
            }
            BossMove::Inhale if b.t >= windup && b.t < windup + if b.enraged { RAGE_INHALE_TICKS } else { INHALE_TICKS } => {
                let pull = if b.enraged { RAGE_PULL } else { PULL };
                for j in self.boss_targets() {
                    let f = &mut self.f[j];
                    if f.unshakeable() || f.pinned > 0 {
                        continue;
                    }
                    let toward = if f.pos.x < me.pos.x { 1.0 } else { -1.0 };
                    f.wind += toward * pull;
                }
                for it in self.items.iter_mut().filter(|it| it.held_by.is_none() && it.vel.y.abs() < 1.0) {
                    let toward = if it.pos.x < me.pos.x { 1.0 } else { -1.0 };
                    it.vel.x = (it.vel.x + toward * 300.0 * DT).clamp(-60.0, 60.0);
                }
            }
            BossMove::Chomp if b.t == windup => {
                self.events.push(Event::BossSlam { pos: vec2(me.pos.x, FLOOR_Y) });
                let bite = Aabb::new(me.pos.x - BODY_HALF_W - 10.0, FLOOR_Y - CHOMP_ABOVE, (BODY_HALF_W + 10.0) * 2.0, CHOMP_ABOVE);
                for j in self.boss_targets() {
                    if bite.overlaps(&self.f[j].body()) {
                        let out = if self.f[j].pos.x < me.pos.x { -1.0 } else { 1.0 };
                        self.damage(BOSS, j, CHOMP_DAMAGE, vec2(out * 220.0, -230.0), 16, out, Source::Boss, false);
                    }
                }
                let speed = if b.enraged { RAGE_WAVE_SPEED } else { WAVE_SPEED };
                for s in [-1.0, 1.0] {
                    let x = me.pos.x + s * (BODY_HALF_W + WAVE_W);
                    let ticks = ((W / 2.0) / speed / DT) as u32 + 10;
                    self.add_effect(Effect::Shockwave(Shockwave { pos: vec2(x, FLOOR_Y), start: x, dir: s, speed, ticks, hit: 0 }));
                }
            }
            BossMove::Acid if b.t >= windup && (b.t - windup) % ACID_EVERY == 0 => {
                let k = (b.t - windup) / ACID_EVERY;
                if k < if b.enraged { RAGE_GLOBS } else { GLOBS }
                    && let Some(j) = self.boss_pick()
                {
                    self.maw_glob(j, k as f32 * 0.04);
                }
            }
            _ => {}
        }
    }

    /// A glob of acid lobbed at `j` (where they're heading), up out of its mouth.
    fn maw_glob(&mut self, j: usize, later: f32) {
        let from = crate::boss::mouth(&self.f[BOSS]) + vec2(0.0, -6.0);
        let to = self.f[j].center() + vec2(self.f[j].vel.x * 0.4, 0.0);
        let vel = lob(from, to, GLOB_FLIGHT + later, GLOB_GRAVITY);
        self.add_effect(Effect::BossShot(BossShot { pos: from, vel, kind: ShotKind::Glob, ticks: GLOB_TICKS, target: None }));
        self.events.push(Event::Throw { who: BOSS });
    }

    /// A glob arcs along and burns whoever it hits; it splatters on whatever it lands on (a ledge
    /// too, coming down onto it).
    pub(crate) fn step_glob(&mut self, s: &mut BossShot) {
        let prev = s.pos;
        s.vel.y += GLOB_GRAVITY * DT;
        s.pos += s.vel * DT;
        let ledge = s.vel.y > 0.0 && self.arena().platforms.iter().any(|p| s.pos.x > p.x && s.pos.x < p.x + p.w && prev.y <= p.y && s.pos.y >= p.y);
        if ledge || self.arena().solids().any(|w| w.contains(s.pos)) {
            s.ticks = 0;
            self.events.push(Event::BossSplat { pos: s.pos, sand: false });
            return;
        }
        let hit = Aabb::around(s.pos, GLOB_R);
        if let Some(j) = self.boss_targets().into_iter().find(|&j| hit.overlaps(&self.f[j].body())) {
            s.ticks = 0;
            let dir = if s.vel.x < 0.0 { -1.0 } else { 1.0 };
            self.damage(BOSS, j, GLOB_DAMAGE, vec2(dir * 90.0, -110.0), 8, dir, Source::Ember, false);
            self.events.push(Event::BossSplat { pos: s.pos, sand: false });
        }
    }

    /// The sand bubbles, then the tentacle bursts up, catching whoever it reaches (once each) and
    /// flinging them toward the mouth; then it sinks back.
    pub(crate) fn step_tentacle(&mut self, t: &mut Tentacle) {
        t.t += 1;
        t.ticks = t.ticks.saturating_sub(1);
        if t.t == T_WARN {
            self.events.push(Event::BossSplat { pos: t.pos, sand: true });
        }
        if t.reach() < 6.0 {
            return;
        }
        let area = t.area();
        for j in self.boss_targets() {
            if t.hit & (1 << j) != 0 || !area.overlaps(&self.f[j].body()) {
                continue;
            }
            t.hit |= 1 << j;
            let toward = if self.f[j].pos.x < W / 2.0 { 1.0 } else { -1.0 };
            self.damage(BOSS, j, TENTACLE_DAMAGE, vec2(toward * 170.0, -250.0), 16, toward, Source::Boss, false);
        }
    }

    /// The sand (and the skiffs' decks) are slick: whoever's standing on a slope slides down it,
    /// toward the mouth, unless they run or jump the other way.
    pub(crate) fn slide_downhill(&mut self) {
        let arena = &self.arena();
        for f in self.f.iter_mut().filter(|f| f.present() && f.grounded && f.boss.is_none()) {
            // (Standing on top of a step of it, anywhere under their feet.)
            let (l, r) = (f.pos.x - f.half_w(), f.pos.x + f.half_w());
            if arena.hills.iter().chain(arena.platforms.iter()).any(|s| s.x < r && s.x + s.w > l && (s.y - f.pos.y).abs() < 0.6) {
                f.wind += if f.pos.x < W / 2.0 { SLIDE } else { -SLIDE };
            }
        }
    }

    /// A bot on the floor doesn't walk (or dash) into the pit's throat, nor jump in from its edge;
    /// one coming down over it steers for the nearer edge.
    pub(crate) fn keep_off_pit(&self, i: usize, (t0, t1): (f32, f32), a: &mut Action) {
        let f = &self.f[i];
        let nearer_edge = if f.pos.x - t0 < t1 - f.pos.x { -1.0 } else { 1.0 };
        if !f.grounded {
            // Where it'll come down, going as it is: if that's its throat (or close), steer for the
            // nearer edge of it.
            let dy = (FLOOR_Y - f.pos.y).max(0.0);
            let fall = (-f.vel.y + (f.vel.y * f.vel.y + 2.0 * GRAVITY * dy).sqrt()) / GRAVITY;
            let land = f.pos.x + f.vel.x * fall;
            let over = |x: f32| x > t0 - 24.0 && x < t1 + 24.0;
            if over(land) || over(f.pos.x) && f.pos.y > FLOOR_Y - 120.0 {
                // (Back toward the side it's on, while it's still outside the throat: braking short is
                // surer than trying to clear it.)
                let outside = f.pos.x < t0 || f.pos.x > t1;
                a.mv = if outside || !over(land) { nearer_edge } else if land - t0 < t1 - land { -1.0 } else { 1.0 };
                a.run = true;
                a.down = false;
            }
            return;
        }
        let away = if f.pos.x < W / 2.0 { -1.0 } else { 1.0 };
        // Near it, no jumping (or dashing) while still moving its way: it'd sail in.
        if f.pos.x > t0 - 100.0 && f.pos.x < t1 + 100.0 && f.vel.x * away < 0.0 {
            a.jump = false;
            a.dash = None;
        }
        if f.pos.y < FLOOR_Y - 1.0 {
            // On the slope or a ledge near the mouth: no dropping through toward it, and (the
            // slope sliding it in) no standing about or heading for it: back up the slope.
            if f.pos.x > t0 - 40.0 && f.pos.x < t1 + 40.0 {
                a.down = false;
                if a.mv * away <= 0.0 {
                    a.mv = away;
                    a.run = true;
                }
            }
            return;
        }
        let ahead = f.pos.x + a.mv.signum() * if a.mv.abs() < 0.2 { 0.0 } else { 18.0 };
        if ahead > t0 - 6.0 && ahead < t1 + 6.0 {
            // Back away from it instead.
            a.mv = if f.pos.x < W / 2.0 { -1.0 } else { 1.0 };
            a.jump = false;
            a.dash = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boss::BossKind;
    use crate::brain::Brain;
    use crate::sim::{ArenaId, MatchConfig, Mode, MAX_HP, N_FIGHTERS};

    fn maw_match() -> (Match, Brain) {
        let config = MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, unstick: false, ..MatchConfig::for_arena(ArenaId::Dune) };
        let mut m = Match::new(5, &config);
        m.items.clear();
        (m, Brain { w: vec![0.0; Brain::N_WEIGHTS] })
    }

    /// The boss holding off attacking (for tests of one thing at a time).
    fn hush(m: &mut Match) {
        if let Some(b) = &mut m.f[BOSS].boss {
            (b.rest, b.spit) = (u32::MAX / 2, u32::MAX / 2);
        }
    }

    #[test]
    fn the_dune_is_a_sandmaw_fight() {
        let (m, _) = maw_match();
        assert_eq!(m.config.mode, Mode::Boss);
        assert_eq!(m.f[BOSS].boss.unwrap().kind, BossKind::Sandmaw);
        let b = m.f[BOSS].body();
        assert_eq!((b.x, b.x + b.w, b.y + b.h), (W / 2.0 - BODY_HALF_W, W / 2.0 + BODY_HALF_W, FLOOR_Y));
        // No headshots on a mouth.
        assert!(!m.f[BOSS].is_head(b.y));
    }

    #[test]
    fn walking_into_its_throat_is_the_end_but_its_lips_are_safe() {
        let (mut m, z) = maw_match();
        hush(&mut m);
        // (On the flat at the foot of the slope, and on its lip.)
        m.f[0].pos = vec2(THROAT.0 - 14.0, FLOOR_Y);
        m.f[1].pos = vec2(W / 2.0 - BODY_HALF_W + 4.0, FLOOR_Y);
        let right = Action { mv: 1.0, ..Action::default() };
        for _ in 0..secs(2) {
            m.step_with([&z; N_FIGHTERS], Some((0, right)));
        }
        assert!(!m.f[0].alive(), "swallowed");
        assert_eq!(m.f[1].hp, MAX_HP, "standing on its lip is fine");
    }

    #[test]
    fn you_run_up_and_down_its_slopes_and_stand_still_to_slide_toward_its_mouth() {
        use crate::arena::{dune_sand_y, DUNE_FOOT};
        let (mut m, z) = maw_match();
        hush(&mut m);
        let x0 = 120.0;
        m.f[0].pos = vec2(x0, m.arena().ground_at(x0, crate::sim::HALF_W));
        // Up toward the wall: higher and higher, never in the sand, always on it.
        let up = Action { mv: -1.0, run: true, ..Action::default() };
        for _ in 0..secs(2) {
            m.step_with([&z; N_FIGHTERS], Some((0, up)));
            let f = &m.f[0];
            assert!(!m.arena().solids().any(|s| f.body().overlaps(s)), "in the sand at {}", f.pos);
        }
        let f = &m.f[0];
        assert!(f.pos.x < 40.0 && f.grounded, "ran up to the wall: {}", f.pos);
        assert!((f.pos.y - dune_sand_y(f.pos.x).unwrap()).abs() < 3.0, "on the sand: {}", f.pos);
        // Standing still: it slides you down toward the mouth.
        let x1 = f.pos.x;
        for _ in 0..secs(1) {
            m.step_with([&z; N_FIGHTERS], Some((0, Action::default())));
        }
        assert!(m.f[0].pos.x > x1 + 30.0 && m.f[0].grounded, "slid down: {} -> {}", x1, m.f[0].pos);
        // Down it at a run: it stays on its feet the whole way to the flat.
        let down = Action { mv: 1.0, run: true, ..Action::default() };
        let mut airborne = 0;
        while m.f[0].pos.x < DUNE_FOOT.0 - 6.0 {
            m.step_with([&z; N_FIGHTERS], Some((0, down)));
            airborne += !m.f[0].grounded as u32;
        }
        assert_eq!(airborne, 0, "kept its footing running down");
    }

    #[test]
    fn its_skiffs_slope_and_slide_you_toward_its_mouth_too() {
        use crate::arena::{skiff_deck, DUNE_SKIFFS};
        let (mut m, z) = maw_match();
        hush(&mut m);
        let skiff = DUNE_SKIFFS[0];
        let x0 = skiff.0 + 6.0;
        m.f[0].pos = vec2(x0, m.arena().platforms.iter().filter(|p| p.x < x0 + 4.0 && p.x + p.w > x0 - 4.0 && (p.y - skiff_deck(skiff, x0)).abs() < 4.0).map(|p| p.y).fold(f32::MAX, f32::min));
        m.f[0].grounded = true;
        for _ in 0..secs(1) / 2 {
            m.step_with([&z; N_FIGHTERS], Some((0, Action::default())));
        }
        let f = &m.f[0];
        assert!(f.pos.x > x0 + 15.0 && f.grounded, "slid along it: {}", f.pos);
        assert!((f.pos.y - skiff_deck(skiff, f.pos.x)).abs() < 3.0, "on its deck: {}", f.pos);
        // And you can walk back up it.
        let x1 = f.pos.x;
        for _ in 0..secs(1) / 2 {
            m.step_with([&z; N_FIGHTERS], Some((0, Action { mv: -1.0, run: true, ..Action::default() })));
        }
        assert!(m.f[0].pos.x < x1 - 15.0 && m.f[0].grounded, "walked back up: {}", m.f[0].pos);
    }

    #[test]
    fn a_dog_running_into_its_mouth_is_swallowed() {
        let (mut m, z) = maw_match();
        hush(&mut m);
        m.step([&z; N_FIGHTERS]);
        m.f[0].character = crate::character::Character::Tyler;
        m.f[0].pos = vec2(THROAT.0 - 14.0, FLOOR_Y);
        m.f[0].facing = 1.0;
        let dogs = |m: &Match| m.effects.iter().filter(|e| matches!(e, Effect::Pet(_))).count();
        m.step_with([&z; N_FIGHTERS], Some((0, Action { special: true, ..Action::default() })));
        assert!(dogs(&m) > 0, "the dogs are out");
        for _ in 0..secs(4) {
            m.step_with([&z; N_FIGHTERS], Some((0, Action { mv: -1.0, ..Action::default() })));
        }
        assert_eq!(dogs(&m), 0, "every dog that ran across it was swallowed");
    }

    #[test]
    fn nothing_spawns_in_its_mouth() {
        let (mut m, _) = maw_match();
        for _ in 0..2000 {
            let p = m.random_surface_spot_between(0.0, W).unwrap();
            assert!(!(p.y == FLOOR_Y && p.x > THROAT.0 - 10.0 && p.x < THROAT.1 + 10.0), "a spot in its mouth at {p}");
        }
    }

    #[test]
    fn a_grenade_dropped_in_its_mouth_hurts_it() {
        let (mut m, z) = maw_match();
        hush(&mut m);
        m.step([&z; N_FIGHTERS]);
        let hp = m.f[BOSS].hp;
        let mut g = crate::sim::Item::new(crate::sim::ItemKind::Grenade, vec2(W / 2.0, 120.0));
        g.owner = Some(0);
        g.fuse = crate::sim::ItemKind::Grenade.def().fuse;
        m.items.push(g);
        for _ in 0..secs(3) {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[BOSS].hp < hp - 30.0, "a big hit: {} of {hp}", m.f[BOSS].hp);
        assert!(m.items.is_empty(), "gone off");
    }

    #[test]
    fn a_tentacle_bubbles_then_flings_you_toward_the_mouth() {
        let (mut m, z) = maw_match();
        hush(&mut m);
        // (On the flat at the foot of the slope, so it doesn't slide away while the sand bubbles.)
        m.f[0].pos = vec2(THROAT.0 - 14.0, FLOOR_Y);
        m.add_effect(Effect::Tentacle(Tentacle::new(THROAT.0 - 14.0)));
        for _ in 0..T_WARN - 2 {
            m.step([&z; N_FIGHTERS]);
        }
        assert_eq!(m.f[0].hp, MAX_HP, "a warning first");
        for _ in 0..10 {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[0].hp < MAX_HP && m.f[0].vel.x > 0.0, "hit and flung toward the middle: {}", m.f[0].vel);
    }

    #[test]
    fn its_inhale_pulls_but_running_gets_you_away() {
        let (mut m, z) = maw_match();
        let on_sand = |x: f32| vec2(x, ArenaId::Dune.def().ground_at(x, crate::sim::HALF_W));
        m.f[0].pos = on_sand(60.0);
        m.f[1].pos = on_sand(110.0);
        m.f[2].pos = on_sand(420.0);
        m.f[3].pos = on_sand(440.0);
        m.f[BOSS].boss = Some(BossState { mv: BossMove::Inhale, t: INHALE_WINDUP, rest: u32::MAX / 2, spit: u32::MAX / 2, ..m.f[BOSS].boss.unwrap() });
        let away = Action { mv: -1.0, run: true, ..Action::default() };
        for _ in 0..secs(2) {
            m.step_with([&z; N_FIGHTERS], Some((0, away)));
        }
        assert!(m.f[1].pos.x > 125.0, "standing still: pulled in ({})", m.f[1].pos.x);
        assert!(m.f[0].pos.x < 60.0, "running: got away ({})", m.f[0].pos.x);
    }

    #[test]
    fn bots_dont_walk_into_its_mouth() {
        let brains = Brain::committed();
        let lineup: [&Brain; N_FIGHTERS] = std::array::from_fn(|i| &brains[i % brains.len()]);
        let mut swallowed = 0;
        let mut dealt = 0.0;
        for seed in 0..4 {
            let mut m = Match::new(seed, &MatchConfig::for_arena(ArenaId::Dune));
            hush(&mut m);
            for _ in 0..secs(30) {
                m.step(lineup);
                swallowed += m.events.iter().filter(|e| matches!(e, Event::Swallowed { .. })).count();
            }
            dealt += m.f[..N_FIGHTERS].iter().map(|f| f.dmg_dealt).sum::<f32>();
        }
        assert_eq!(swallowed, 0, "nobody walked in");
        assert!(dealt > 50.0, "and they still fought it: {dealt}");
    }
}
