//! The Architect's room (a map with `Arena::shift`): every `SHIFT_EVERY` he reshapes it into
//! another map (for now, the same room upside down: `arena::flipped`), and everyone and
//! everything in it goes with it, mirrored top to bottom. Whoever was on the floor ends up at the
//! top and falls; whoever was high up comes down near the floor. Clients draw it folding flat and
//! unfolding (`Match::shift_fold`), and the swap happens with the room folded edge-on.
//!
//! No boss yet: the room plays a free-for-all while the mechanic finds its feet.

use crate::arena::FLIP_AXIS;
use crate::sim::{secs, Event, Match};

/// How often he reshapes the room, and how long its fold takes either side of the swap.
pub const SHIFT_EVERY: u32 = secs(10);
pub const FOLD: u32 = 24;
/// How long before a shift its warning goes out (the code glitching).
pub const WARN: u32 = secs(2);
/// A match in his room lasts this long (time for a few shifts).
pub const ARCHITECT_MATCH: u32 = secs(90);

impl Match {
    /// Ticks until his next shift (None if this room doesn't shift, or the match hasn't started).
    pub fn ticks_to_shift(&self) -> Option<u32> {
        self.arena().shift?;
        if self.counting_down() {
            return None;
        }
        let k = self.play_ticks() % SHIFT_EVERY;
        Some(SHIFT_EVERY - k)
    }

    /// How far the room is folded, as drawn: 1 flat open, shrinking to 0 (edge-on) at a shift and
    /// opening back up after it.
    pub fn shift_fold(&self) -> f32 {
        let Some(to) = self.ticks_to_shift() else { return 1.0 };
        let since = SHIFT_EVERY - to;
        let edge = to.min(since).min(FOLD);
        // (Not before the first one's had a chance to fold in.)
        if self.play_ticks() < FOLD {
            return 1.0;
        }
        edge as f32 / FOLD as f32
    }

    /// His tick: the warning, then the shift itself.
    pub(crate) fn step_shift(&mut self) {
        let Some(next) = self.arena().shift else { return };
        if self.counting_down() || self.play_ticks() == 0 {
            return;
        }
        let k = self.play_ticks() % SHIFT_EVERY;
        if k == SHIFT_EVERY - WARN {
            self.events.push(Event::ShiftWarn);
        }
        if k == 0 {
            self.shift_to(next);
        }
    }

    /// The room becomes `next`, and everyone and everything in it is turned upside down with it.
    pub(crate) fn shift_to(&mut self, next: crate::sim::ArenaId) {
        self.config.arena = next;
        for f in self.f.iter_mut().filter(|f| !f.absent) {
            // (Their body flipped: its top is where their feet now go.)
            f.pos.y = FLIP_AXIS - f.pos.y + f.height();
            f.vel.y = -f.vel.y;
            f.grounded = false;
        }
        for it in self.items.iter_mut() {
            it.pos.y = FLIP_AXIS - it.pos.y;
            it.vel.y = -it.vel.y;
        }
        for h in self.heals.iter_mut() {
            h.y = FLIP_AXIS - h.y;
        }
        for e in self.effects.iter_mut() {
            let p = e.pos_mut();
            p.y = FLIP_AXIS - p.y;
        }
        self.events.push(Event::Shift);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::Brain;
    use crate::sim::{ArenaId, MatchConfig, FLOOR_Y, N_FIGHTERS};
    use glam::vec2;

    #[test]
    fn every_so_often_the_room_flips_and_everyone_with_it() {
        let config = MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, unstick: false, ..MatchConfig::for_arena(ArenaId::Architect) };
        let mut m = Match::new(1, &config);
        let z = Brain { w: vec![0.0; Brain::N_WEIGHTS] };
        m.f[0].pos = vec2(240.0, FLOOR_Y);
        let mut warned = false;
        for _ in 0..SHIFT_EVERY {
            m.step([&z; N_FIGHTERS]);
            warned |= m.events.iter().any(|e| matches!(e, Event::ShiftWarn));
            m.f[0].pos.x = 240.0;
        }
        assert!(warned, "a warning first");
        assert_eq!(m.config.arena, ArenaId::Architect);
        assert_eq!(m.shift_fold(), 0.0, "edge-on, about to flip");
        m.step([&z; N_FIGHTERS]);
        assert_eq!(m.config.arena, ArenaId::ArchitectFlipped, "flipped");
        assert!(m.f[0].pos.y < 40.0, "the one on the floor is up at the top now: {}", m.f[0].pos);
        // ...and falls back down to it.
        for _ in 0..secs(2) {
            m.step([&z; N_FIGHTERS]);
        }
        assert!(m.f[0].grounded, "landed: {}", m.f[0].pos);
        // And back again next time.
        for _ in 0..SHIFT_EVERY {
            m.step([&z; N_FIGHTERS]);
        }
        assert_eq!(m.config.arena, ArenaId::Architect);
    }

    #[test]
    fn upside_down_the_ledges_are_the_same_ledges_flipped() {
        let (a, b) = (ArenaId::Architect.def(), ArenaId::ArchitectFlipped.def());
        for (p, q) in a.platforms.iter().zip(b.platforms) {
            assert_eq!((p.x, p.w), (q.x, q.w));
            assert_eq!(q.y + q.h, FLIP_AXIS - p.y, "{p:?} flips to {q:?}");
        }
        assert!(b.hidden && !a.hidden);
    }
}
