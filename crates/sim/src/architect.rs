//! The Simulation (`Arena::architect`): the Architect's room. Its ledges are the match's own
//! (`Room`), not the map's, because every `SHIFT_EVERY` he reshapes it, one of four ways, chosen at
//! random a shift ahead (so clients can show which is coming):
//! - Flip: the room turned upside down, everyone and everything in it with it. Whoever was on the
//!   floor ends up at the top and falls.
//! - Gravity: the same flip, but the screen turns over with it, so the room looks just as it was
//!   and everyone falls *up* onto the undersides of things (`Room::upside_down`).
//! - Mirror: left and right swapped, everyone and everything with them.
//! - Rebuild: every ledge gone, and a new, lopsided set of them in their place.
//! Some ledges carry a live cable (digital thorns: touch it and it shocks you), and some are loose:
//! when the room's turned over they fall until they land on a ledge or the floor, and stay there.
//! Ledges are one-way and there are no walls but the box's, so nobody can be shut in: you can jump up
//! through, drop down through, and wall-jump the sides.
//!
//! No boss yet: the room plays a free-for-all while the mechanic finds its feet.

use crate::arena::{Arena, FLIP_AXIS};
use crate::sim::{secs, Aabb, Event, Match, Source, DT, FLOOR_Y, GRAVITY, N_BODIES, W, WALL};
use glam::vec2;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::Arc;

/// How often he reshapes the room, how long its fold takes either side of the swap, and how long
/// before a shift its warning goes out.
pub const SHIFT_EVERY: u32 = secs(10);
pub const FOLD: u32 = 24;
pub const WARN: u32 = secs(2);
/// A match in his room lasts this long (time for a good few shifts).
pub const ARCHITECT_MATCH: u32 = secs(120);

/// A live cable's height, and its shock: how much, how often (for each person touching it).
pub const CABLE_H: f32 = 6.0;
const SHOCK: f32 = 8.0;
const SHOCK_EVERY: u32 = 30;

/// A rebuilt room: how many ledges, how wide, how far apart they keep (overlapping ones at least
/// `GAP` above or below each other), how many with cables and how many loose.
const REBUILD_LEDGES: (u32, u32) = (8, 13);
const LEDGE_W: (f32, f32) = (36.0, 96.0);
pub const GAP: f32 = 26.0;
const CABLES: (u32, u32) = (2, 4);
const LOOSE: (u32, u32) = (1, 2);
/// Gravity turned round stays so for this many shifts (30 seconds), then he turns it back.
const UPSIDE_DOWN_FOR: u32 = 3;
/// How thick a loose ledge is (a slab, heavier-looking than a ledge).
pub const LOOSE_H: f32 = 8.0;

/// How he reshapes the room (see the module notes).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ShiftKind {
    Flip,
    Gravity,
    Mirror,
    Rebuild,
}

/// One of the room's ledges: where it is, and what's on it.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Ledge {
    pub rect: Aabb,
    /// A live cable along part of it: (from its left end, how long).
    pub cable: Option<(f32, f32)>,
    /// Whether its cable lies on top of it (or hangs under it, the room having been turned over).
    pub cable_up: bool,
    /// Falls when the room's turned over, until it lands.
    pub loose: bool,
    /// Falling now: how fast (0 at rest).
    pub vy: f32,
}

impl Ledge {
    const fn new(x: f32, y: f32, w: f32) -> Self {
        Self { rect: Aabb::new(x, y, w, 5.0), cable: None, cable_up: true, loose: false, vy: 0.0 }
    }

    const fn cabled(mut self, from: f32, len: f32) -> Self {
        self.cable = Some((from, len));
        self
    }

    /// Loose: a heavier slab (thicker) that always falls till it's resting on something.
    const fn loose(mut self) -> Self {
        self.loose = true;
        self.rect.h = LOOSE_H;
        self
    }

    /// Its cable's live area, if it has one.
    pub fn cable_area(&self) -> Option<Aabb> {
        let (from, len) = self.cable?;
        let r = self.rect;
        let y = if self.cable_up { r.y - CABLE_H } else { r.y + r.h };
        Some(Aabb::new(r.x + from, y, len, CABLE_H))
    }
}

/// The room as it stands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Room {
    pub ledges: Vec<Ledge>,
    /// The next shift, and the last.
    pub next: ShiftKind,
    pub last: ShiftKind,
    /// Turned over an odd number of times (the code rains the other way).
    pub inverted: bool,
    /// Gravity's turned round: the screen is shown upside down (`ShiftKind::Gravity`).
    pub upside_down: bool,
    /// Gravity turned round: shifts left before he's made to turn it back (`UPSIDE_DOWN_FOR`).
    #[serde(skip)]
    upside_left: u32,
    /// Ticks before each body can be shocked again.
    #[serde(skip)]
    shock_cd: Vec<u32>,
}

/// The room as a match starts: its ledges only halfway up, more of them on the left, a few
/// carrying cables, and two loose slabs, one on the floor and one resting on a ledge. (Turned over,
/// they're all up top: wall-jump to reach them.)
const START: [Ledge; 10] = [
    Ledge::new(20.0, 218.0, 64.0),
    Ledge::new(100.0, FLOOR_Y - LOOSE_H, 56.0).loose(),
    Ledge::new(24.0, 160.0, 56.0).cabled(16.0, 24.0),
    Ledge::new(120.0, 150.0, 60.0),
    Ledge::new(186.0, 214.0, 84.0).cabled(30.0, 26.0),
    Ledge::new(200.0, 214.0 - LOOSE_H, 56.0).loose(),
    Ledge::new(196.0, 140.0, 40.0),
    Ledge::new(300.0, 202.0, 70.0),
    Ledge::new(388.0, 222.0, 72.0).cabled(8.0, 30.0),
    Ledge::new(380.0, 172.0, 56.0),
];

impl Room {
    pub fn new() -> Self {
        Self { ledges: START.to_vec(), next: ShiftKind::Flip, last: ShiftKind::Flip, inverted: false, upside_down: false, upside_left: 0, shock_cd: vec![0; N_BODIES] }
    }

    /// The live map: the box, with these ledges.
    pub fn arena(&self, base: &Arena) -> Arena {
        Arena { platforms: Cow::Owned(self.ledges.iter().map(|l| l.rect).collect()), ..base.clone() }
    }
}

impl Default for Room {
    fn default() -> Self {
        Self::new()
    }
}

/// A fresh, lopsided set of ledges from the match's dice (`roll(n)`: a number below `n`): anywhere
/// in the box, none overlapping another within `GAP` above or below it.
pub fn rebuild(mut roll: impl FnMut(u32) -> u32) -> Vec<Ledge> {
    let mut range = |lo: f32, hi: f32| lo + (hi - lo) * roll(1000) as f32 / 1000.0;
    let span = REBUILD_LEDGES.1 - REBUILD_LEDGES.0;
    let n = REBUILD_LEDGES.0 + (range(0.0, (span + 1) as f32) as u32).min(span);
    // (Lopsided: one side gets more of them.)
    let lean = range(-0.6, 0.6);
    let mut ledges: Vec<Ledge> = Vec::new();
    let mut tries = 0;
    while (ledges.len() as u32) < n && tries < 400 {
        tries += 1;
        let w = range(LEDGE_W.0, LEDGE_W.1).round();
        let u = range(0.0, 1.0);
        let u = (u + lean * u * (1.0 - u) * 2.0).clamp(0.0, 1.0);
        let x = (WALL + 4.0 + u * (W - 2.0 * WALL - 8.0 - w)).round();
        let y = range(40.0, FLOOR_Y - 30.0).round();
        let clear = ledges.iter().all(|l| l.rect.x > x + w + 4.0 || l.rect.x + l.rect.w + 4.0 < x || (l.rect.y - y).abs() >= GAP);
        if clear {
            ledges.push(Ledge::new(x, y, w));
        }
    }
    let cables = (CABLES.0 + range(0.0, (CABLES.1 - CABLES.0 + 1) as f32) as u32).min(CABLES.1);
    for l in ledges.iter_mut().take(cables as usize) {
        let len = (l.rect.w * range(0.3, 0.55)).round();
        let from = range(0.0, l.rect.w - len).round();
        l.cable = Some((from, len));
    }
    let loose = (LOOSE.0 + range(0.0, (LOOSE.1 - LOOSE.0 + 1) as f32) as u32).min(LOOSE.1);
    for l in ledges.iter_mut().rev().take(loose as usize) {
        *l = l.loose();
    }
    ledges
}

impl Match {
    /// Ticks until his next shift (None if this isn't his room, or the match hasn't started).
    pub fn ticks_to_shift(&self) -> Option<u32> {
        self.room.as_ref()?;
        if self.counting_down() {
            return None;
        }
        Some(SHIFT_EVERY - self.play_ticks() % SHIFT_EVERY)
    }

    /// How the room is being drawn mid-shift: which shift (the coming one as it folds, the last as
    /// it opens back up), and how far folded (1 open, 0 edge-on, at the swap).
    pub fn shift_anim(&self) -> Option<(ShiftKind, f32)> {
        let room = self.room.as_ref()?;
        let to = self.ticks_to_shift()?;
        let since = SHIFT_EVERY - to;
        if self.play_ticks() < FOLD {
            return None;
        }
        let kind = if to <= since { room.next } else { room.last };
        Some((kind, to.min(since).min(FOLD) as f32 / FOLD as f32))
    }

    /// His tick: cables shock, loose ledges fall, the warning goes out, then the shift itself.
    pub(crate) fn step_architect(&mut self) {
        let Some(mut room) = self.room.take() else { return };
        self.shock(&mut room);
        if self.fall(&mut room) {
            self.live = Arc::new(room.arena(self.config.arena.def()));
        }
        if !self.counting_down() && self.play_ticks() > 0 {
            let k = self.play_ticks() % SHIFT_EVERY;
            if k == SHIFT_EVERY - WARN {
                self.events.push(Event::ShiftWarn { kind: room.next });
            }
            if k == 0 {
                self.shift(&mut room);
            }
        }
        self.room = Some(room);
    }

    /// Whoever's touching a live cable gets a shock (each at most every `SHOCK_EVERY`).
    fn shock(&mut self, room: &mut Room) {
        room.shock_cd.resize(N_BODIES, 0);
        for cd in room.shock_cd.iter_mut() {
            *cd = cd.saturating_sub(1);
        }
        if self.protected() {
            return;
        }
        let cables: Vec<Aabb> = room.ledges.iter().filter_map(|l| l.cable_area()).collect();
        for j in 0..N_BODIES {
            let body = self.f[j].body();
            if !self.f[j].present() || room.shock_cd[j] > 0 {
                continue;
            }
            if let Some(c) = cables.iter().find(|c| c.overlaps(&body)) {
                room.shock_cd[j] = SHOCK_EVERY;
                let away = if self.f[j].pos.x < c.x + c.w / 2.0 { -1.0 } else { 1.0 };
                // (Thrown off it: up off a cable lying on a ledge, down off one hanging under it.)
                let up = if c.y + c.h / 2.0 > body.y + body.h / 2.0 { -170.0 } else { 120.0 };
                let pos = self.f[j].center();
                self.damage(j, j, SHOCK, vec2(away * 90.0, up), 10, away, Source::Shock, false);
                self.events.push(Event::Shock { pos });
            }
        }
    }

    /// Loose ledges fall whenever nothing's under them, until they land on another ledge or the floor
    /// (carrying whoever's standing on them). Returns whether any moved.
    fn fall(&mut self, room: &mut Room) -> bool {
        let mut moved = false;
        for k in 0..room.ledges.len() {
            let l = room.ledges[k];
            if !l.loose {
                continue;
            }
            if l.vy == 0.0 {
                let r = l.rect;
                let resting = r.y + r.h >= FLOOR_Y - 0.01 || room.ledges.iter().enumerate().any(|(i, o)| i != k && (o.rect.y - (r.y + r.h)).abs() < 0.01 && o.rect.x < r.x + r.w && o.rect.x + o.rect.w > r.x);
                if resting {
                    continue;
                }
                room.ledges[k].vy = 1.0;
            }
            moved = true;
            let l = room.ledges[k];
            let vy = (l.vy + GRAVITY * DT).min(320.0);
            let r = l.rect;
            // Whatever's under it: another ledge's top, or the floor.
            let under = room.ledges.iter().enumerate().filter(|&(i, o)| i != k && o.rect.x < r.x + r.w && o.rect.x + o.rect.w > r.x && o.rect.y >= r.y + r.h - 0.01).map(|(_, o)| o.rect.y).fold(FLOOR_Y, f32::min);
            let (ny, landed) = if r.y + r.h + vy * DT >= under { (under - r.h, true) } else { (r.y + vy * DT, false) };
            let step = ny - r.y;
            // Whoever's standing on it rides down with it.
            for f in self.f.iter_mut().filter(|f| f.present() && f.grounded && (f.pos.y - r.y).abs() < 0.6 && f.pos.x + f.half_w() > r.x && f.pos.x - f.half_w() < r.x + r.w) {
                f.pos.y += step;
            }
            let l = &mut room.ledges[k];
            l.rect.y = ny;
            l.vy = if landed { 0.0 } else { vy };
            if landed {
                self.events.push(Event::Land { pos: vec2(r.x + r.w / 2.0, ny) });
            }
        }
        moved
    }

    /// The shift: the room reshaped as `room.next` says, and the one after that picked.
    fn shift(&mut self, room: &mut Room) {
        let kind = room.next;
        match kind {
            ShiftKind::Flip | ShiftKind::Gravity => {
                self.turn_over(room);
                if kind == ShiftKind::Gravity {
                    room.upside_down = !room.upside_down;
                }
            }
            ShiftKind::Mirror => self.mirror(room),
            ShiftKind::Rebuild => room.ledges = rebuild(|n| self.roll(n)),
        }
        room.last = kind;
        // Gravity turned round lasts `UPSIDE_DOWN_FOR` shifts, then he turns it back (and meanwhile
        // doesn't turn it again).
        if kind == ShiftKind::Gravity {
            room.upside_left = if room.upside_down { UPSIDE_DOWN_FOR } else { 0 };
        } else if room.upside_down {
            room.upside_left = room.upside_left.saturating_sub(1);
        }
        room.next = if room.upside_down && room.upside_left <= 1 {
            ShiftKind::Gravity
        } else {
            match self.roll(if room.upside_down { 7 } else { 9 }) {
                0..=2 => ShiftKind::Flip,
                3..=4 => ShiftKind::Mirror,
                5..=6 => ShiftKind::Rebuild,
                _ => ShiftKind::Gravity,
            }
        };
        self.live = Arc::new(room.arena(self.config.arena.def()));
        self.events.push(Event::Shift { kind });
    }

    /// Everything turned upside down: the ledges (their cables now hanging under them; loose ones
    /// set falling), everyone and everything in the room.
    fn turn_over(&mut self, room: &mut Room) {
        for l in room.ledges.iter_mut() {
            l.rect.y = FLIP_AXIS - l.rect.y - l.rect.h;
            l.cable_up = !l.cable_up;
            if l.loose {
                l.vy = 1.0;
            }
        }
        room.inverted = !room.inverted;
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
    }

    /// Left and right swapped: the ledges (and where their cables lie), everyone and everything.
    fn mirror(&mut self, room: &mut Room) {
        for l in room.ledges.iter_mut() {
            l.rect.x = W - l.rect.x - l.rect.w;
            if let Some((from, len)) = l.cable {
                l.cable = Some((l.rect.w - from - len, len));
            }
        }
        for f in self.f.iter_mut().filter(|f| !f.absent) {
            f.pos.x = W - f.pos.x;
            f.vel.x = -f.vel.x;
            f.facing = -f.facing;
        }
        for it in self.items.iter_mut() {
            it.pos.x = W - it.pos.x;
            it.vel.x = -it.vel.x;
        }
        for h in self.heals.iter_mut() {
            h.x = W - h.x;
        }
        for e in self.effects.iter_mut() {
            let p = e.pos_mut();
            p.x = W - p.x;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::Brain;
    use crate::sim::{ArenaId, MatchConfig, H, MAX_HP, N_FIGHTERS};

    fn room_match() -> (Match, Brain) {
        let config = MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, unstick: false, ..MatchConfig::for_arena(ArenaId::Architect) };
        let mut m = Match::new(1, &config);
        m.items.clear();
        (m, Brain { w: vec![0.0; Brain::N_WEIGHTS] })
    }

    /// Runs to just after the next shift, made `kind`.
    fn shift_as(m: &mut Match, z: &Brain, kind: ShiftKind) {
        m.room.as_mut().unwrap().next = kind;
        let to = m.ticks_to_shift().unwrap();
        for _ in 0..to + 1 {
            m.step([z; N_FIGHTERS]);
            if m.events.iter().any(|e| matches!(e, Event::Shift { .. })) {
                return;
            }
        }
        panic!("no shift");
    }

    #[test]
    fn the_room_starts_only_halfway_up() {
        let (m, _) = room_match();
        assert!(!m.arena().platforms.is_empty());
        assert!(m.arena().platforms.iter().all(|p| p.y > H / 2.0));
    }

    #[test]
    fn a_flip_turns_everyone_over_and_loose_ledges_fall_till_they_land() {
        let (mut m, z) = room_match();
        m.f[0].pos = vec2(240.0, FLOOR_Y);
        let before: Vec<Aabb> = m.arena().platforms.to_vec();
        shift_as(&mut m, &z, ShiftKind::Flip);
        assert!(m.f[0].pos.y < 60.0, "the one on the floor's up top: {}", m.f[0].pos);
        let room = m.room.as_ref().unwrap();
        for (l, b) in room.ledges.iter().zip(&before) {
            if !l.loose {
                assert_eq!(l.rect.y, FLIP_AXIS - b.y - b.h, "flipped");
            }
        }
        // The loose ones fall, and come to rest on something.
        for _ in 0..secs(3) {
            m.step([&z; N_FIGHTERS]);
        }
        let room = m.room.as_ref().unwrap();
        for l in room.ledges.iter().filter(|l| l.loose) {
            assert_eq!(l.vy, 0.0, "at rest");
            let r = l.rect;
            let on = r.y + r.h == FLOOR_Y || room.ledges.iter().any(|o| o.rect != r && o.rect.y == r.y + r.h && o.rect.x < r.x + r.w && o.rect.x + o.rect.w > r.x);
            assert!(on, "landed on something: {r:?}");
        }
        let live: Vec<Aabb> = m.arena().platforms.to_vec();
        assert_eq!(live, room.ledges.iter().map(|l| l.rect).collect::<Vec<_>>(), "the live map follows");
    }

    #[test]
    fn loose_slabs_are_always_resting_on_something() {
        let resting = |m: &Match| {
            let room = m.room.as_ref().unwrap();
            room.ledges.iter().filter(|l| l.loose).all(|l| {
                let r = l.rect;
                l.vy == 0.0 && (r.y + r.h == FLOOR_Y || room.ledges.iter().any(|o| o.rect != r && o.rect.y == r.y + r.h && o.rect.x < r.x + r.w && o.rect.x + o.rect.w > r.x))
            })
        };
        let (mut m, z) = room_match();
        assert!(resting(&m), "as it starts");
        for kind in [ShiftKind::Rebuild, ShiftKind::Mirror, ShiftKind::Rebuild] {
            shift_as(&mut m, &z, kind);
            for _ in 0..secs(3) {
                m.step([&z; N_FIGHTERS]);
            }
            assert!(resting(&m), "after a {kind:?}");
        }
    }

    #[test]
    fn a_gravity_flip_turns_the_room_over_and_the_screen_with_it() {
        let (mut m, z) = room_match();
        shift_as(&mut m, &z, ShiftKind::Gravity);
        let room = m.room.as_ref().unwrap();
        assert!(room.upside_down && room.inverted);
    }

    #[test]
    fn gravity_turns_back_after_thirty_seconds() {
        let (mut m, z) = room_match();
        shift_as(&mut m, &z, ShiftKind::Gravity);
        assert!(m.room.as_ref().unwrap().upside_down);
        let mut shifts = 0;
        while m.room.as_ref().unwrap().upside_down {
            m.step([&z; N_FIGHTERS]);
            shifts += m.events.iter().filter(|e| matches!(e, Event::Shift { .. })).count();
            assert!(shifts <= UPSIDE_DOWN_FOR as usize, "still upside down after {shifts} shifts");
        }
        assert_eq!(shifts, UPSIDE_DOWN_FOR as usize, "back the right way up, 30 seconds on");
    }

    #[test]
    fn a_mirror_swaps_left_and_right() {
        let (mut m, z) = room_match();
        m.f[0].pos = vec2(60.0, FLOOR_Y);
        let before: Vec<Aabb> = m.arena().platforms.to_vec();
        shift_as(&mut m, &z, ShiftKind::Mirror);
        assert!((m.f[0].pos.x - (W - 60.0)).abs() < 3.0, "over on the other side: {}", m.f[0].pos);
        for (p, b) in m.arena().platforms.iter().zip(&before) {
            assert_eq!(p.x, W - b.x - b.w);
        }
    }

    #[test]
    fn a_cable_shocks_whoever_touches_it() {
        let (mut m, z) = room_match();
        let cable = m.room.as_ref().unwrap().ledges.iter().find_map(|l| l.cable_area()).unwrap();
        m.f[0].pos = vec2(cable.x + cable.w / 2.0, cable.y + cable.h);
        m.step([&z; N_FIGHTERS]);
        assert!(m.f[0].hp < MAX_HP, "shocked");
    }

    #[test]
    fn rebuilt_rooms_stay_in_the_box_and_never_crowd_a_ledge() {
        let mut seed = 12345u32;
        for _ in 0..300 {
            let ledges = rebuild(|n| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed % n
            });
            assert!(ledges.len() >= REBUILD_LEDGES.0 as usize - 2, "{} ledges", ledges.len());
            for (i, l) in ledges.iter().enumerate() {
                let r = l.rect;
                assert!(r.x >= WALL && r.x + r.w <= W - WALL && r.y > WALL + 20.0 && r.y + r.h < FLOOR_Y, "in the box: {r:?}");
                if let Some(c) = l.cable_area() {
                    assert!(c.x >= r.x && c.x + c.w <= r.x + r.w, "its cable on it");
                }
                for o in &ledges[i + 1..] {
                    let over = o.rect.x < r.x + r.w + 4.0 && o.rect.x + o.rect.w + 4.0 > r.x;
                    assert!(!over || (o.rect.y - r.y).abs() >= GAP, "crowded: {r:?} and {:?}", o.rect);
                }
            }
        }
    }

    #[test]
    fn every_kind_of_shift_comes_up() {
        let (mut m, z) = room_match();
        let mut seen = std::collections::HashSet::new();
        let mut seed = 1;
        for _ in 0..secs(600) {
            if m.over {
                seed += 1;
                m = Match::new(seed, &m.config.clone());
            }
            m.step([&z; N_FIGHTERS]);
            for e in &m.events {
                if let Event::Shift { kind } = e {
                    seen.insert(format!("{kind:?}"));
                }
            }
        }
        assert_eq!(seen.len(), 4, "{seen:?}");
    }
}
