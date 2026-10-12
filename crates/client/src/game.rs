//! The match as this client sees it.
//!
//! Everyone else is drawn a little in the past (`INTERP_TICKS` behind the newest snapshot),
//! smoothly interpolated between snapshots, so network jitter doesn't make them stutter. Your
//! own fighter is drawn in the present: every input is applied locally right away (`predict`)
//! and, when a snapshot arrives, the server's version of you is taken and the inputs it hasn't
//! applied yet are replayed on top. Small corrections are smoothed out rather than snapped.

use go_box_protocol::{MatchStart, Snapshot};
use go_box_sim::character::{Effect, props};
use go_box_sim::sim::{Action, Event, Fighter, Match, MatchView, N_BODIES, TICK_RATE, predict};
use macroquad::math::Vec2;
use std::collections::VecDeque;

/// How far behind the newest snapshot others are drawn: 6 ticks = 100ms = three snapshots,
/// enough to ride out a late one.
const INTERP_TICKS: f32 = 6.0;
/// Corrections bigger than this (a teleport, a big knockback) are shown at once, not smoothed.
const SNAP_DISTANCE: f32 = 24.0;

pub struct Game {
    pub start: MatchStart,
    snaps: VecDeque<Snapshot>,
    /// The (fractional) server tick being drawn for everyone but you.
    render_tick: f32,
    /// Events of snapshots up to this tick have been played.
    events_through: Option<u32>,
    /// Inputs sent but not yet acknowledged, oldest first.
    pending: VecDeque<(u32, Action)>,
    /// Your fighter, predicted.
    me: Option<Fighter>,
    /// Where you were drawn minus where prediction now says you are; fades to zero.
    offset: Vec2,
}

impl Game {
    pub fn new(start: MatchStart) -> Self {
        Self { start, snaps: VecDeque::new(), render_tick: 0.0, events_through: None, pending: VecDeque::new(), me: None, offset: Vec2::ZERO }
    }

    pub fn seat(&self) -> Option<usize> {
        self.start.your_seat
    }

    pub fn latest(&self) -> Option<&MatchView> {
        self.snaps.back().map(|s| &s.view)
    }

    pub fn over(&self) -> bool {
        self.latest().is_some_and(|v| v.over)
    }

    /// The match hasn't started yet (going by the newest snapshot).
    pub fn counting_down(&self) -> bool {
        self.latest().is_none_or(|v| v.tick < self.start.config.countdown)
    }

    /// Your fighter as you see it (predicted), if you have a seat.
    pub fn me(&self) -> Option<&Fighter> {
        self.me.as_ref()
    }

    pub fn on_snapshot(&mut self, s: Snapshot) {
        if self.snaps.back().is_some_and(|last| s.view.tick <= last.view.tick) {
            return;
        }
        if let Some(seat) = self.seat() {
            self.pending.retain(|&(seq, _)| seq > s.ack);
            let mut f = s.view.f[seat].clone();
            let props = props(&s.view.effects);
            for &(_, a) in &self.pending {
                predict(self.start.config.arena.def(), &props, &mut f, seat, a, &mut Vec::new());
            }
            if let Some(old) = &self.me {
                self.offset += old.pos - f.pos;
                if self.offset.length() > SNAP_DISTANCE {
                    self.offset = Vec2::ZERO;
                }
            }
            self.me = Some(f);
        }
        if self.snaps.is_empty() {
            self.render_tick = s.view.tick as f32 - INTERP_TICKS;
        }
        self.snaps.push_back(s);
        while self.snaps.len() > 40 {
            self.snaps.pop_front();
        }
    }

    /// One local tick of your input: shown immediately, and remembered until acknowledged.
    /// Your jumps and swings this tick go in `events` (so their sounds needn't wait for the
    /// server).
    pub fn local_tick(&mut self, seq: u32, a: Action, events: &mut Vec<Event>) {
        let (Some(seat), Some(me)) = (self.start.your_seat, &mut self.me) else { return };
        if self.snaps.back().is_some_and(|s| s.view.over) {
            return;
        }
        // (Justin's tables, as of the newest snapshot.)
        let props = self.snaps.back().map(|s| props(&s.view.effects)).unwrap_or_default();
        predict(self.start.config.arena.def(), &props, me, seat, a, events);
        self.pending.push_back((seq, a));
        while self.pending.len() > 120 {
            self.pending.pop_front();
        }
    }

    /// Advances the drawing clock by `dt` and returns the match to draw this frame, with the
    /// events that became visible (for effects).
    pub fn frame(&mut self, dt: f32) -> Option<Match> {
        let newest = self.snaps.back()?.view.tick as f32;
        // Drift gently toward the target delay; jump if far off (a long stall, a fresh start).
        let target = if self.over() { newest } else { newest - INTERP_TICKS };
        self.render_tick += dt * TICK_RATE as f32;
        let err = target - self.render_tick;
        if err.abs() > 30.0 {
            self.render_tick = target;
        } else {
            self.render_tick += err * (dt * 2.0).min(1.0);
        }
        self.render_tick = self.render_tick.min(newest);
        self.offset *= 1.0 - (dt * 12.0).min(1.0);

        // The snapshots either side of the drawing time.
        let rt = self.render_tick;
        let b = self.snaps.iter().position(|s| s.view.tick as f32 > rt);
        let (a, b) = match b {
            Some(0) => (0, None),
            Some(k) => (k - 1, Some(k)),
            None => (self.snaps.len() - 1, None),
        };
        let mut events = Vec::new();
        for s in self.snaps.iter().take(a + 1) {
            if self.events_through.is_none_or(|t| s.view.tick > t) {
                events.extend_from_slice(&s.events);
                self.events_through = Some(s.view.tick);
            }
        }

        let sa = &self.snaps[a].view;
        let mut view = sa.clone();
        if let Some(b) = b {
            let sb = &self.snaps[b].view;
            let t = ((rt - sa.tick as f32) / (sb.tick - sa.tick) as f32).clamp(0.0, 1.0);
            for i in 0..N_BODIES {
                // (A mini slot only lerps while it holds the same mini in both.)
                if sa.f[i].absent || sb.f[i].absent || sa.f[i].owner != sb.f[i].owner {
                    continue;
                }
                view.f[i].pos = sa.f[i].pos.lerp(sb.f[i].pos, t);
                view.f[i].vel = sa.f[i].vel.lerp(sb.f[i].vel, t);
            }
            // Items and effects only line up one-to-one if nothing appeared or vanished in between.
            if sa.items.len() == sb.items.len() && sa.items.iter().zip(&sb.items).all(|(x, y)| x.kind == y.kind) {
                for (k, it) in view.items.iter_mut().enumerate() {
                    it.pos = sa.items[k].pos.lerp(sb.items[k].pos, t);
                }
            }
            let same = |x: &Effect, y: &Effect| std::mem::discriminant(x) == std::mem::discriminant(y);
            if sa.effects.len() == sb.effects.len() && sa.effects.iter().zip(&sb.effects).all(|(x, y)| same(x, y)) {
                for (k, e) in view.effects.iter_mut().enumerate() {
                    *e.pos_mut() = sa.effects[k].pos().lerp(sb.effects[k].pos(), t);
                }
            }
        }
        if let (Some(seat), Some(me)) = (self.seat(), &self.me) {
            let mut me = me.clone();
            me.pos += self.offset;
            for it in view.items.iter_mut().filter(|it| it.held_by == Some(seat)) {
                it.pos = me.hand();
            }
            view.f[seat] = me;
        }
        Some(Match::from_view(view, &self.start.config, events))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use go_box_sim::sim::{MatchConfig, N_FIGHTERS};
    use go_box_protocol::Seat;
    use go_box_sim::brain::Brain;
    use go_box_sim::character::Character;

    /// You're in seat 0; the others are bots that stand still (all-zero brains).
    fn game() -> Game {
        let seats = std::array::from_fn(|i| Seat { name: String::new(), character: Character::Classic, player: (i == 0).then_some(1) });
        Game::new(MatchStart { seats, your_seat: Some(0), config: MatchConfig::default() })
    }

    /// No health pickups.
    fn quiet() -> MatchConfig {
        MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, ..MatchConfig::default() }
    }

    /// The server's match: nothing lying around, nobody else moving.
    fn server() -> (Match, Brain) {
        let mut m = Match::new(3, &quiet());
        m.items.clear();
        (m, Brain { w: vec![0.0; Brain::N_WEIGHTS] })
    }

    fn snapshot(m: &Match, ack: u32, events: Vec<Event>) -> Snapshot {
        Snapshot { ack, view: m.view(), events }
    }

    const RIGHT: Action = Action { mv: 1.0, run: true, jump: false, attack: false, throw: false, aim: 0.3, grab: false, special: false, target: None };

    #[test]
    fn a_snapshot_behind_your_inputs_is_caught_up_by_replaying_them() {
        let (mut m, z) = server();
        let mut g = game();
        g.on_snapshot(snapshot(&m, 0, Vec::new()));
        let mut e = Vec::new();
        for seq in 1..=10 {
            g.local_tick(seq, RIGHT, &mut e);
        }
        // The server has only applied the first 6 when it sends its next snapshot.
        for _ in 0..6 {
            m.step_with([&z; N_FIGHTERS], Some((0, RIGHT)));
        }
        g.on_snapshot(snapshot(&m, 6, Vec::new()));
        // Where you'll be once the server applies the other 4 too.
        let mut ahead = m.f[0].clone();
        for _ in 0..4 {
            predict(m.arena(), &[], &mut ahead, 0, RIGHT, &mut Vec::new());
        }
        assert!((g.me().unwrap().pos - ahead.pos).length() < 1e-4, "predicted {} vs {}", g.me().unwrap().pos, ahead.pos);
        assert!(ahead.pos.x > m.f[0].pos.x, "and that's ahead of the snapshot");
    }

    #[test]
    fn small_corrections_are_smoothed_but_big_ones_snap() {
        let (mut m, _) = server();
        let mut g = game();
        g.on_snapshot(snapshot(&m, 0, Vec::new()));
        // A nudge: drawn where you were, easing toward the corrected spot.
        m.tick += 1;
        m.f[0].pos.x += 3.0;
        g.on_snapshot(snapshot(&m, 0, Vec::new()));
        let drawn = g.frame(0.0).unwrap().f[0].pos;
        assert!((drawn.x - (m.f[0].pos.x - 3.0)).abs() < 1e-4, "drawn at {}", drawn.x);
        // A teleport: shown at once.
        m.tick += 1;
        m.f[0].pos.x += 100.0;
        g.on_snapshot(snapshot(&m, 0, Vec::new()));
        assert_eq!(g.frame(0.0).unwrap().f[0].pos, m.f[0].pos);
    }

    #[test]
    fn each_event_is_played_once() {
        let (mut m, z) = server();
        let mut g = game();
        for k in 0..5 {
            m.step([&z; N_FIGHTERS]);
            g.on_snapshot(snapshot(&m, 0, vec![Event::Spawn { pos: Vec2::new(k as f32, 0.0) }]));
        }
        let mut seen = Vec::new();
        for _ in 0..60 {
            seen.extend(g.frame(1.0 / 60.0).unwrap().events);
        }
        let xs: Vec<f32> = seen.iter().map(|e| if let Event::Spawn { pos } = e { pos.x } else { -1.0 }).collect();
        assert_eq!(xs, [0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn interpolation_copes_with_an_item_appearing() {
        // A match with items lying around; the second snapshot has one more than the first.
        let mut m = Match::new(3, &quiet());
        let z = Brain { w: vec![0.0; Brain::N_WEIGHTS] };
        let all = m.items.clone();
        let mut g = game();
        m.items = all[..2].to_vec();
        m.step([&z; N_FIGHTERS]);
        g.on_snapshot(snapshot(&m, 0, Vec::new()));
        m.items = all[..3].to_vec();
        m.step([&z; N_FIGHTERS]);
        g.on_snapshot(snapshot(&m, 0, Vec::new()));
        // In between, items are drawn as in the earlier snapshot rather than paired up wrongly.
        let mut drawn = None;
        for _ in 0..12 {
            let f = g.frame(1.0 / 60.0).unwrap();
            if f.tick == m.tick - 1 {
                drawn = Some(f.items.len());
            }
        }
        assert_eq!(drawn, Some(2));
    }
}
