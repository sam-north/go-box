//! Procedural sound effects: every sound is synthesised at startup from oscillators and
//! filtered noise (no audio files), then played in response to match events.

use go_box_sim::sim::*;
use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound};
use macroquad::experimental::coroutines::start_coroutine;
use macroquad::math::Vec2;
use std::sync::{Arc, Mutex};

const RATE: u32 = 22050;

go_box_sim::enum_all! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Sfx {
        Jump,
        Land,
        Step,
        Swing,
        Punch,
        Stab,
        SpearHit,
        RockHit,
        Headshot,
        Death,
        Throw,
        Grab,
        ClatterRock,
        ClatterSpear,
        ClatterGrenade,
        Explosion,
        FuseTick,
        Heal,
        Spawn,
        Poof,
        Hounds,
        Bite,
        BombThrow,
        BombCrunch,
        BombBlast,
        Ready,
        Count,
        Split,
        Meow,
        Scratch,
        Serve,
        Splat,
        ChainThrow,
        Hook,
        Start,
        Win,
        Lose,
        Click,
    }
}

impl Sfx {
    /// The waveform, as mono samples in -1..1.
    fn synth(self) -> Vec<f32> {
        let mut s = Buf::new(self as u32);
        match self {
            // A soft blip: triangle, not square, and quiet; you hear it a lot.
            Sfx::Jump => s.tone(0.0, 0.07, 260.0, 480.0, Wave::Tri, 0.004, 28.0, 0.12),
            Sfx::Land => s.noise(0.0, 0.06, 400.0, 120.0, 0.002, 50.0, 0.3),
            Sfx::Step => s.noise(0.0, 0.035, 700.0, 300.0, 0.002, 90.0, 0.22),
            // A muffled puff of air: low-passed and quiet. Swings are constant; a hit is the loud part.
            Sfx::Swing => s.noise(0.0, 0.09, 260.0, 520.0, 0.02, 35.0, 0.12),
            Sfx::Punch => {
                s.noise(0.0, 0.11, 1600.0, 200.0, 0.001, 32.0, 0.7);
                s.tone(0.0, 0.11, 130.0, 55.0, Wave::Sine, 0.001, 26.0, 0.7);
            }
            Sfx::Stab => {
                s.noise(0.0, 0.08, 4000.0, 900.0, 0.001, 40.0, 0.55);
                s.tone(0.0, 0.07, 320.0, 140.0, Wave::Saw, 0.001, 35.0, 0.25);
            }
            Sfx::SpearHit => {
                s.tone(0.0, 0.14, 190.0, 85.0, Wave::Sine, 0.001, 20.0, 0.8);
                s.noise(0.0, 0.06, 2500.0, 600.0, 0.001, 50.0, 0.5);
            }
            Sfx::RockHit => {
                s.tone(0.0, 0.07, 560.0, 300.0, Wave::Sine, 0.001, 38.0, 0.6);
                s.noise(0.0, 0.03, 5000.0, 2000.0, 0.001, 90.0, 0.4);
            }
            Sfx::Headshot => {
                s.tone(0.0, 0.45, 1320.0, 1320.0, Wave::Sine, 0.001, 9.0, 0.3);
                s.tone(0.0, 0.45, 1985.0, 1985.0, Wave::Sine, 0.001, 12.0, 0.15);
            }
            Sfx::Death => {
                s.tone(0.0, 0.55, 300.0, 55.0, Wave::Saw, 0.005, 5.0, 0.28);
                s.noise(0.0, 0.3, 900.0, 150.0, 0.002, 9.0, 0.35);
            }
            Sfx::Throw => s.noise(0.0, 0.12, 2000.0, 5000.0, 0.025, 28.0, 0.25),
            Sfx::Grab => s.tone(0.0, 0.05, 660.0, 990.0, Wave::Square, 0.002, 40.0, 0.12),
            Sfx::ClatterRock => {
                s.tone(0.0, 0.05, 720.0, 420.0, Wave::Sine, 0.001, 60.0, 0.35);
                s.noise(0.0, 0.02, 5000.0, 3000.0, 0.001, 120.0, 0.25);
            }
            Sfx::ClatterSpear => {
                s.tone(0.0, 0.1, 880.0, 840.0, Wave::Tri, 0.001, 35.0, 0.18);
                s.noise(0.0, 0.03, 3000.0, 1500.0, 0.001, 80.0, 0.2);
            }
            Sfx::ClatterGrenade => s.tone(0.0, 0.05, 1250.0, 1150.0, Wave::Square, 0.001, 60.0, 0.1),
            Sfx::Explosion => {
                s.noise(0.0, 1.0, 3200.0, 90.0, 0.002, 4.0, 1.0);
                s.tone(0.0, 0.7, 85.0, 32.0, Wave::Sine, 0.002, 5.0, 0.9);
            }
            Sfx::FuseTick => s.tone(0.0, 0.018, 1900.0, 1900.0, Wave::Square, 0.001, 120.0, 0.12),
            Sfx::Heal => {
                for (k, f) in [660.0, 880.0, 1320.0].into_iter().enumerate() {
                    s.tone(k as f32 * 0.07, 0.25, f, f, Wave::Sine, 0.003, 12.0, 0.22);
                }
            }
            Sfx::Spawn => s.tone(0.0, 0.14, 1500.0, 2300.0, Wave::Sine, 0.01, 18.0, 0.08),
            Sfx::Poof => {
                s.tone(0.0, 0.16, 300.0, 1900.0, Wave::Sine, 0.003, 14.0, 0.25);
                s.noise(0.0, 0.16, 3500.0, 1500.0, 0.01, 18.0, 0.18);
            }
            Sfx::Hounds => {
                for k in 0..3 {
                    let t = k as f32 * 0.13;
                    let f = 560.0 - k as f32 * 50.0;
                    s.tone(t, 0.09, f, f * 0.5, Wave::Saw, 0.004, 20.0, 0.3);
                    s.noise(t, 0.06, 1800.0, 700.0, 0.002, 35.0, 0.25);
                }
            }
            Sfx::Bite => {
                s.noise(0.0, 0.05, 3000.0, 1200.0, 0.001, 60.0, 0.45);
                s.tone(0.0, 0.07, 210.0, 120.0, Wave::Square, 0.001, 30.0, 0.18);
            }
            Sfx::BombThrow => {
                s.wobble(0.0, 0.3, 150.0, 22.0, 0.35);
                s.tone(0.0, 0.3, 70.0, 90.0, Wave::Square, 0.01, 9.0, 0.08);
            }
            Sfx::BombCrunch => {
                // A rising suck that cuts off when it blows.
                let len = BOMB_CRUNCH_SECS;
                s.swell(0.0, len, 150.0, 4500.0, 0.6);
                s.tone(0.0, len, 60.0, 420.0, Wave::Sine, len, 0.0, 0.5);
            }
            Sfx::BombBlast => {
                s.tone(0.0, 0.8, 55.0, 28.0, Wave::Sine, 0.002, 4.0, 1.0);
                s.noise(0.0, 0.7, 1800.0, 70.0, 0.002, 5.0, 0.7);
                s.tone(0.0, 0.3, 110.0, 40.0, Wave::Square, 0.002, 10.0, 0.18);
            }
            Sfx::Ready => s.tone(0.0, 0.14, 880.0, 1320.0, Wave::Sine, 0.005, 14.0, 0.22),
            // The 3-2-1 beeps (the start horn, `Start`, is the GO).
            Sfx::Count => s.tone(0.0, 0.16, 660.0, 660.0, Wave::Square, 0.003, 12.0, 0.14),
            // Seth's minis popping out: two quick rising blips.
            Sfx::Split => {
                s.tone(0.0, 0.08, 520.0, 1040.0, Wave::Tri, 0.003, 18.0, 0.25);
                s.tone(0.09, 0.08, 620.0, 1240.0, Wave::Tri, 0.003, 18.0, 0.25);
            }
            // Corwin's cats let loose: a couple of rising-falling meows.
            Sfx::Meow => {
                s.tone(0.0, 0.22, 700.0, 1100.0, Wave::Saw, 0.02, 9.0, 0.16);
                s.tone(0.18, 0.25, 900.0, 600.0, Wave::Saw, 0.02, 8.0, 0.14);
            }
            Sfx::Scratch => s.noise(0.0, 0.04, 6000.0, 3500.0, 0.001, 70.0, 0.22),
            // Justin's table: a bright "ding" of service.
            Sfx::Serve => {
                s.tone(0.0, 0.5, 1568.0, 1568.0, Wave::Sine, 0.001, 7.0, 0.25);
                s.tone(0.0, 0.5, 3136.0, 3136.0, Wave::Sine, 0.001, 10.0, 0.08);
            }
            // Squishy: a low wet thud with a soft smear of noise.
            Sfx::Splat => {
                s.tone(0.0, 0.12, 180.0, 70.0, Wave::Sine, 0.002, 22.0, 0.55);
                s.noise(0.0, 0.14, 900.0, 300.0, 0.004, 20.0, 0.45);
                s.wobble(0.0, 0.1, 120.0, 40.0, 0.2);
            }
            // Ryan's chain: a fast metallic rattle.
            Sfx::ChainThrow => {
                for k in 0..5 {
                    s.tone(k as f32 * 0.03, 0.03, 2400.0 + k as f32 * 150.0, 1800.0, Wave::Square, 0.001, 60.0, 0.08);
                }
                s.noise(0.0, 0.16, 6000.0, 3000.0, 0.005, 20.0, 0.18);
            }
            // The hook biting: a clank.
            Sfx::Hook => {
                s.tone(0.0, 0.18, 980.0, 900.0, Wave::Square, 0.001, 18.0, 0.18);
                s.tone(0.0, 0.18, 1470.0, 1400.0, Wave::Sine, 0.001, 16.0, 0.18);
                s.noise(0.0, 0.05, 5000.0, 2000.0, 0.001, 60.0, 0.3);
            }
            Sfx::Start => {
                s.tone(0.0, 0.12, 440.0, 440.0, Wave::Square, 0.003, 10.0, 0.14);
                s.tone(0.16, 0.3, 660.0, 660.0, Wave::Square, 0.003, 6.0, 0.16);
            }
            Sfx::Win => {
                for (k, f) in [523.0, 659.0, 784.0, 1046.0].into_iter().enumerate() {
                    s.tone(k as f32 * 0.1, 0.35, f, f, Wave::Square, 0.003, 7.0, 0.12);
                }
            }
            Sfx::Lose => {
                for (k, f) in [392.0, 330.0, 262.0].into_iter().enumerate() {
                    s.tone(k as f32 * 0.16, 0.4, f, f * 0.98, Wave::Tri, 0.003, 5.0, 0.25);
                }
            }
            Sfx::Click => s.tone(0.0, 0.025, 1000.0, 900.0, Wave::Square, 0.001, 80.0, 0.1),
        }
        s.finish()
    }
}

const BOMB_CRUNCH_SECS: f32 = go_box_sim::character::BOMB_CRUNCH as f32 * DT;

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Square,
    Saw,
    Tri,
}

/// A mono sample buffer that sounds are layered into.
struct Buf {
    s: Vec<f32>,
    seed: u32,
}

impl Buf {
    fn new(seed: u32) -> Self {
        Self { s: Vec::new(), seed: seed.wrapping_mul(2_654_435_761) | 1 }
    }

    fn at(&mut self, i: usize) -> &mut f32 {
        if i >= self.s.len() {
            self.s.resize(i + 1, 0.0);
        }
        &mut self.s[i]
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    }

    /// Attack then exponential decay, with a short fade at the very end so nothing clicks.
    fn env(t: f32, len: f32, attack: f32, decay: f32) -> f32 {
        let a = if attack > 0.0 { (t / attack).min(1.0) } else { 1.0 };
        let tail = ((len - t) / 0.006).clamp(0.0, 1.0);
        a * (-(t - attack).max(0.0) * decay).exp() * tail
    }

    /// An oscillator gliding exponentially from `f0` to `f1` Hz.
    #[allow(clippy::too_many_arguments)]
    fn tone(&mut self, start: f32, len: f32, f0: f32, f1: f32, wave: Wave, attack: f32, decay: f32, vol: f32) {
        let n = (len * RATE as f32) as usize;
        let i0 = (start * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let f = f0 * (f1 / f0).powf(t / len);
            phase = (phase + f / RATE as f32).fract();
            let v = match wave {
                Wave::Sine => (phase * std::f32::consts::TAU).sin(),
                Wave::Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Wave::Saw => phase * 2.0 - 1.0,
                Wave::Tri => 1.0 - 4.0 * (phase - 0.5).abs(),
            };
            *self.at(i0 + i) += v * vol * Self::env(t, len, attack, decay);
        }
    }

    /// White noise through a one-pole low-pass whose cutoff glides from `c0` to `c1` Hz.
    #[allow(clippy::too_many_arguments)]
    fn noise(&mut self, start: f32, len: f32, c0: f32, c1: f32, attack: f32, decay: f32, vol: f32) {
        let n = (len * RATE as f32) as usize;
        let i0 = (start * RATE as f32) as usize;
        let mut y = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let c = c0 * (c1 / c0).powf(t / len);
            let k = 1.0 - (-std::f32::consts::TAU * c / RATE as f32).exp();
            let x = self.rand();
            y += (x - y) * k;
            // The filter takes the edge off the level, more so at low cutoffs; make some back.
            let gain = (1.0 + 2000.0 / c).sqrt().min(4.0);
            *self.at(i0 + i) += y * gain * vol * Self::env(t, len, attack, decay);
        }
    }

    /// Noise that grows louder and brighter, then stops dead.
    fn swell(&mut self, start: f32, len: f32, c0: f32, c1: f32, vol: f32) {
        let n = (len * RATE as f32) as usize;
        let i0 = (start * RATE as f32) as usize;
        let mut y = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let c = c0 * (c1 / c0).powf(t / len);
            let k = 1.0 - (-std::f32::consts::TAU * c / RATE as f32).exp();
            let x = self.rand();
            y += (x - y) * k;
            let a = (t / len).powi(2) * ((len - t) / 0.004).clamp(0.0, 1.0);
            *self.at(i0 + i) += y * 2.0 * vol * a;
        }
    }

    /// A low sine with a fast vibrato: a heavy, warbling "vwomm".
    fn wobble(&mut self, start: f32, len: f32, f: f32, rate: f32, vol: f32) {
        let n = (len * RATE as f32) as usize;
        let i0 = (start * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let fi = f * (1.0 + 0.25 * (t * rate * std::f32::consts::TAU).sin()) * (1.0 + t * 1.5);
            phase = (phase + fi / RATE as f32).fract();
            *self.at(i0 + i) += (phase * std::f32::consts::TAU).sin() * vol * Self::env(t, len, 0.02, 6.0);
        }
    }

    /// Soft-clips the mix into range.
    fn finish(self) -> Vec<f32> {
        self.s.into_iter().map(|v| v.tanh()).collect()
    }
}

/// 16-bit mono PCM WAV.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data_len = samples.len() as u32 * 2;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for &v in samples {
        b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    b
}

/// The sound board: the synthesised sounds plus the little bit of state needed to play them
/// from match events (footsteps, fuse ticks, "special ready").
pub struct Sounds {
    /// Filled in the background as each sound finishes loading (the browser decodes them
    /// asynchronously, and a sound that never decodes must not hold up the game).
    sounds: Arc<Mutex<Vec<Option<Sound>>>>,
    pub muted: bool,
    /// Overall volume (0..1): everything the game plays (sound effects now, music later).
    pub master: f32,
    /// Sound-effects volume (0..1), under `master`.
    pub effects: f32,
    /// When each sound last started (seconds), so a burst of identical events plays once.
    last: Vec<f64>,
    now: f64,
    step_dist: [f32; N_BODIES],
    fuse_acc: f32,
    special_cd: Option<u32>,
    /// The countdown number last shown (3, 2, 1), to beep on each.
    count: Option<u32>,
}

impl Sounds {
    /// Synthesises every sound and starts loading them; they become playable over the next few
    /// frames.
    pub fn load() -> Self {
        let sounds = Arc::new(Mutex::new(vec![None; Sfx::ALL.len()]));
        let slots = sounds.clone();
        start_coroutine(async move {
            for s in Sfx::ALL {
                if let Ok(sound) = load_sound_from_bytes(&wav(&s.synth())).await {
                    slots.lock().unwrap()[s as usize] = Some(sound);
                }
            }
        });
        Self { sounds, muted: false, master: 0.8, effects: 1.0, last: vec![-1.0; Sfx::ALL.len()], now: 0.0, step_dist: [0.0; N_BODIES], fuse_acc: 0.0, special_cd: None, count: None }
    }

    /// Plays `s` at `vol` (0..1, before the master volume).
    pub fn play(&mut self, s: Sfx, vol: f32) {
        let k = s as usize;
        if self.muted || self.now - self.last[k] < 0.035 {
            return;
        }
        self.last[k] = self.now;
        if let Some(sound) = &self.sounds.lock().unwrap()[k] {
            play_sound(sound, PlaySoundParams { looped: false, volume: (vol * self.master * self.effects).clamp(0.0, 1.0) });
        }
    }

    /// Sounds far from `ear` (your fighter, or the middle of the arena) are a bit quieter.
    fn at(&mut self, s: Sfx, pos: Vec2, ear: Vec2, vol: f32) {
        let d = pos.distance(ear) / W;
        self.play(s, vol * (1.0 - d * 0.6).max(0.4));
    }

    /// Plays the sounds for this frame's events. `own` is your seat: your own jumps and swings
    /// were already played the moment you pressed the key (see `local`), so they're skipped
    /// here.
    pub fn events(&mut self, m: &Match, events: &[Event], own: Option<usize>) {
        let ear = own.map_or(vec2_mid(), |s| m.f[s].center());
        for e in events {
            match *e {
                Event::Hit { pos, headshot, source, .. } => {
                    let s = match source {
                        Source::Fist => Sfx::Punch,
                        Source::Stab => Sfx::Stab,
                        Source::SpearThrow => Sfx::SpearHit,
                        Source::Rock => Sfx::RockHit,
                        Source::Sushi => Sfx::Splat,
                        // The blast or the bite is the sound; just add a thump.
                        Source::Grenade | Source::Special => Sfx::Punch,
                    };
                    self.at(s, pos, ear, 0.9);
                    if headshot {
                        self.at(Sfx::Headshot, pos, ear, 1.0);
                    }
                }
                Event::Death { pos, .. } => self.at(Sfx::Death, pos, ear, 1.0),
                Event::Jump { who, pos } if Some(who) != own => self.at(Sfx::Jump, pos, ear, 0.25),
                Event::Swing { who } if Some(who) != own => self.at(Sfx::Swing, m.f[who].center(), ear, 0.2),
                Event::Jump { .. } | Event::Swing { .. } => {}
                Event::Land { pos } => self.at(Sfx::Land, pos, ear, 0.4),
                Event::Throw { who } => self.at(Sfx::Throw, m.f[who].center(), ear, 0.9),
                Event::Grab { pos } => self.at(Sfx::Grab, pos, ear, 0.8),
                Event::Explosion { pos } => self.play(Sfx::Explosion, 1.0 - (pos.distance(ear) / W * 0.3)),
                Event::Heal { pos, .. } => self.at(Sfx::Heal, pos, ear, 1.0),
                Event::Spawn { pos } => self.at(Sfx::Spawn, pos, ear, 0.8),
                Event::Special { who } => {
                    if let Some(s) = crate::art(m.f[who].character).special_sound {
                        self.at(s, m.f[who].center(), ear, 1.0);
                    }
                }
                Event::Poof { pos } => self.at(Sfx::Poof, pos, ear, 0.9),
                Event::BombCrunch { pos } => self.at(Sfx::BombCrunch, pos, ear, 1.0),
                Event::BombBlast { pos } => self.play(Sfx::BombBlast, 1.0 - (pos.distance(ear) / W * 0.3)),
                Event::Bite { pos } => self.at(Sfx::Bite, pos, ear, 0.9),
                Event::Scratch { pos } => self.at(Sfx::Scratch, pos, ear, 0.6),
                Event::Splat { pos } => self.at(Sfx::Splat, pos, ear, 1.0),
                Event::Hook { pos } => self.at(Sfx::Hook, pos, ear, 1.0),
                Event::Clatter { pos, kind } => {
                    let s = match kind {
                        ItemKind::Rock => Sfx::ClatterRock,
                        ItemKind::Spear => Sfx::ClatterSpear,
                        ItemKind::Grenade => Sfx::ClatterGrenade,
                        ItemKind::Sushi => Sfx::Splat,
                    };
                    self.at(s, pos, ear, 0.7);
                }
            }
        }
    }

    /// Your own predicted jumps and swings, played without waiting for the server.
    pub fn local(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::Jump { .. } => self.play(Sfx::Jump, 0.3),
                Event::Swing { .. } => self.play(Sfx::Swing, 0.3),
                _ => {}
            }
        }
    }

    /// Sounds that come from state rather than events: footsteps, lit fuses ticking faster
    /// as they burn, and your special coming back.
    pub fn frame(&mut self, dt: f32, m: &Match, own: Option<usize>) {
        self.now += dt as f64;
        // A finished match is a still frame behind the scoreboard: nothing in it makes sound.
        if m.over {
            return;
        }
        // 3, 2, 1 (a beep each), then GO (the start horn).
        let count = m.counting_down().then(|| (m.config.countdown - m.tick).div_ceil(TICK_RATE));
        if count != self.count {
            match count {
                Some(_) => self.play(Sfx::Count, 0.9),
                None if self.count.is_some() => self.play(Sfx::Start, 1.0),
                None => {}
            }
            self.count = count;
        }
        let ear = own.map_or(vec2_mid(), |s| m.f[s].center());
        for i in 0..N_BODIES {
            let f = &m.f[i];
            if !f.alive() || !f.grounded || f.vel.x.abs() < 20.0 {
                self.step_dist[i] = 0.0;
                continue;
            }
            self.step_dist[i] += f.vel.x.abs() * dt;
            if self.step_dist[i] > 16.0 {
                self.step_dist[i] = 0.0;
                let vol = if Some(i) == own { 0.5 } else { 0.3 };
                self.at(Sfx::Step, f.pos, ear, vol);
            }
        }
        let burn = m.items.iter().filter_map(|it| it.fuse).map(|t| 1.0 - t as f32 / GRENADE_FUSE as f32).fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b))));
        match burn {
            Some(b) => {
                self.fuse_acc += dt * (6.0 + 16.0 * b);
                if self.fuse_acc >= 1.0 {
                    self.fuse_acc -= 1.0;
                    self.play(Sfx::FuseTick, 0.8);
                }
            }
            None => self.fuse_acc = 0.0,
        }
        let cd = own.map(|s| &m.f[s]).filter(|f| f.alive() && f.character.special().is_some()).map(|f| f.special_cd);
        if let (Some(prev), Some(0)) = (self.special_cd, cd)
            && prev > 0
        {
            self.play(Sfx::Ready, 0.9);
        }
        self.special_cd = cd;
    }

    /// Forget per-match state (between matches).
    pub fn reset(&mut self) {
        self.step_dist = [0.0; N_BODIES];
        self.fuse_acc = 0.0;
        self.special_cd = None;
        self.count = None;
    }
}

fn vec2_mid() -> Vec2 {
    Vec2::new(W / 2.0, H / 2.0)
}
