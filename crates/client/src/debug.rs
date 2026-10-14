//! Debug mode, for chasing lag: an overlay (top right) of frame rate and frame timing, how long
//! a jump takes to show, snapshot timing and prediction, with a summary logged to the console
//! every second and a line for each hitch. Off unless turned on locally: `?debug=1` in the page's
//! URL (remembered in this browser until `?debug=0`), or `--debug` (or GOBOX_DEBUG) for the
//! native client. Off, it costs a bool check a frame.
//!
//! The numbers:
//! - fps, and each frame's gap from the last (avg/max): what the browser gives us.
//! - work: how long our own code takes each frame (avg/max). If it's near the gap, we're the
//!   bottleneck; if it's small and the gap is still long, the browser or GPU is.
//! - ticks/frame (max): sim ticks run in one frame (more than 1 means frames are late).
//! - jump: from the key's own browser timestamp to the frame that read it ("event"), and from
//!   that frame to the one that drew you jumping ("draw"). The rest of the lag, from that frame
//!   to the screen, is the browser's and the display's.
//! - snaps: gap between server snapshots arriving (avg/max), inputs not yet acknowledged, and
//!   how far you're drawn from your predicted position (a correction being smoothed out).

use crate::platform;
use crate::ui::Ui;
use go_box_sim::sim::Fighter;
use macroquad::prelude::*;

/// A frame this much longer than a 60Hz one is logged as a hitch.
const HITCH_MS: f64 = 30.0;

#[derive(Default)]
pub struct Debug {
    pub on: bool,
    frame_start: f64,
    last_frame: f64,
    // This second so far.
    window_start: f64,
    frames: u32,
    gap_max: f64,
    work_sum: f64,
    work_max: f64,
    ticks_max: u32,
    snaps: u32,
    snap_gap_sum: f64,
    snap_gap_max: f64,
    last_snap: Option<(u32, f64)>,
    /// The last whole second, as shown.
    shown: Vec<String>,
    /// A jump pressed and not yet drawn: (when its frame started, the key event's timestamp).
    jump: Option<(f64, Option<f64>)>,
    /// The last jump measured: (event to frame, frame to drawn), ms.
    last_jump: Option<(Option<f64>, f64)>,
}

impl Debug {
    pub fn new() -> Self {
        Self { on: platform::debug_mode(), ..Default::default() }
    }

    /// At the top of the frame.
    pub fn begin(&mut self) {
        if !self.on {
            return;
        }
        let now = platform::now_ms();
        if self.last_frame > 0.0 {
            let gap = now - self.last_frame;
            self.gap_max = self.gap_max.max(gap);
            if gap > HITCH_MS {
                macroquad::miniquad::info!("[debug] hitch: {gap:.1}ms between frames (last frame's own work {:.1}ms)", self.work_max);
            }
        }
        self.last_frame = now;
        self.frame_start = now;
        if self.window_start == 0.0 {
            self.window_start = now;
        }
    }

    /// The sim ticks run this frame.
    pub fn ticks(&mut self, n: u32) {
        self.ticks_max = self.ticks_max.max(n);
    }

    /// Jump was freshly pressed this frame.
    pub fn jump_pressed(&mut self) {
        if self.on && self.jump.is_none() {
            self.jump = Some((self.frame_start, platform::last_key_ms()));
        }
    }

    /// What's about to be drawn: you (predicted), and the newest snapshot's tick.
    pub fn observe(&mut self, me: Option<&Fighter>, newest: Option<u32>) {
        if !self.on {
            return;
        }
        if let Some((pressed, key)) = self.jump {
            let now = self.frame_start;
            if me.is_some_and(|f| f.vel.y < -100.0) {
                let event = key.map(|k| pressed - k).filter(|&ms| (0.0..1000.0).contains(&ms));
                self.last_jump = Some((event, now - pressed));
                macroquad::miniquad::info!("[debug] jump: key event to frame {}, frame to drawn {:.1}ms", event.map_or("?".into(), |ms| format!("{ms:.1}ms")), now - pressed);
                self.jump = None;
            } else if now - pressed > 500.0 {
                // (In the air already, or stunned: no jump to see.)
                self.jump = None;
            }
        }
        if let Some(tick) = newest
            && self.last_snap.is_none_or(|(t, _)| tick != t)
        {
            if let Some((_, at)) = self.last_snap {
                let gap = self.frame_start - at;
                self.snaps += 1;
                self.snap_gap_sum += gap;
                self.snap_gap_max = self.snap_gap_max.max(gap);
            }
            self.last_snap = Some((tick, self.frame_start));
        }
    }

    /// At the bottom of the frame (everything drawn). `pending` and `offset`: see
    /// `Game::debug_stats`.
    pub fn end(&mut self, pending: Option<(usize, f32)>) {
        if !self.on {
            return;
        }
        let now = platform::now_ms();
        let work = now - self.frame_start;
        self.frames += 1;
        self.work_sum += work;
        self.work_max = self.work_max.max(work);
        if now - self.window_start < 1000.0 {
            return;
        }
        let secs = (now - self.window_start) / 1000.0;
        let n = self.frames.max(1) as f64;
        let mut lines = vec![
            format!("fps {:.0}  gap avg {:.1} max {:.1}ms", n / secs, secs * 1000.0 / n, self.gap_max),
            format!("work avg {:.1} max {:.1}ms  ticks/frame max {}", self.work_sum / n, self.work_max, self.ticks_max),
            format!("canvas {:.0}x{:.0} (dpi x{:.2})", screen_width() * screen_dpi_scale(), screen_height() * screen_dpi_scale(), screen_dpi_scale()),
        ];
        if self.snaps > 0 {
            lines.push(format!("snaps gap avg {:.1} max {:.1}ms", self.snap_gap_sum / self.snaps as f64, self.snap_gap_max));
        }
        if let Some((pending, offset)) = pending {
            lines.push(format!("unacked inputs {pending}  correction {offset:.1}px"));
        }
        if let Some((event, draw)) = self.last_jump {
            lines.push(format!("jump: event->frame {}  frame->drawn {draw:.1}ms", event.map_or("?".into(), |ms| format!("{ms:.1}ms"))));
        }
        macroquad::miniquad::info!("[debug] {}", lines.join(" | "));
        // A fresh second (what carries over: what's shown, a jump in progress, the last frame and
        // snapshot seen).
        *self = Self { on: true, jump: self.jump, last_jump: self.last_jump, last_frame: self.last_frame, last_snap: self.last_snap, shown: lines, window_start: now, ..Default::default() };
    }

    pub fn draw(&self, ui: &Ui) {
        if !self.on {
            return;
        }
        let size = 14.0;
        let w = self.shown.iter().map(|l| ui.text_width(l, size)).fold(0.0, f32::max) + ui.px(16.0);
        let x = screen_width() - w - ui.px(8.0);
        let y = ui.px(60.0);
        draw_rectangle(x, y, w, ui.px(18.0) * self.shown.len() as f32 + ui.px(8.0), Color::new(0.0, 0.0, 0.0, 0.7));
        for (k, l) in self.shown.iter().enumerate() {
            ui.text(l, x + ui.px(8.0), y + ui.px(18.0) * (k as f32 + 1.0), size, Color::new(0.6, 1.0, 0.6, 1.0));
        }
    }
}
