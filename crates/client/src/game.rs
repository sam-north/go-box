//! The match as this client sees it.
//!
//! Everyone else is drawn a little in the past (`INTERP_TICKS` behind the newest snapshot),
//! smoothly interpolated between snapshots, so network jitter doesn't make them stutter. Your
//! own fighter is drawn in the present: every input is applied locally right away (`predict`)
//! and, when a snapshot arrives, the server's version of you is taken and the inputs it hasn't
//! applied yet are replayed on top. Small corrections are smoothed out rather than snapped.

use go_box_protocol::{MatchStart, Snapshot};
use go_box_sim::character::Effect;
use go_box_sim::sim::{Action, Event, Fighter, Match, MatchView, N_FIGHTERS, TICK_RATE, predict};
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
            for &(_, a) in &self.pending {
                predict(&mut f, seat, a, &mut Vec::new());
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
        predict(me, seat, a, events);
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
            for i in 0..N_FIGHTERS {
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
        Some(Match::from_view(view, events))
    }
}
