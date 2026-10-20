//! Procedural sound effects: every sound is synthesised at startup from oscillators and
//! filtered noise (no audio files), then played in response to match events.

use go_box_sim::sim::*;
use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound};
use macroquad::experimental::coroutines::start_coroutine;
use macroquad::math::Vec2;
use std::sync::{Arc, Mutex};

const RATE: u32 = 22050;
/// How often a new gust of Bigworm's sandstorm starts (each lasts a little longer, so they overlap).
const STORM_GUST: f32 = 1.0;

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
        GlueShot,
        Gloop,
        Ready,
        Count,
        Tick,
        Split,
        Meow,
        Scratch,
        Serve,
        Splat,
        ChainThrow,
        Hook,
        Digitize,
        Materialize,
        Thunder,
        Zap,
        Freeze,
        RocketLaunch,
        Dash,
        SwordSwing,
        Slash,
        Shatter,
        Crackle,
        Radio,
        Jet,
        Whistle,
        Possess,
        Release,
        JudoGrab,
        Slam,
        Flex,
        Stomp,
        BigStomp,
        Bugs,
        Skate,
        Tornado,
        ArrowWhistle,
        BookOpen,
        ShieldBlock,
        Backlog,
        BacklogBurst,
        LockIn,
        /// A boss roaring (Bigworm coming up, either boss enraged).
        Roar,
        /// Bigworm's inhale: a sandstorm howling in (played back to back while it lasts).
        Sandstorm,
        /// A zombie's moan.
        Groan,
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
            // Sam's glue gun: a wet, springy "thwup".
            Sfx::GlueShot => {
                s.wobble(0.0, 0.14, 240.0, 30.0, 0.3);
                s.tone(0.0, 0.12, 520.0, 160.0, Wave::Sine, 0.002, 16.0, 0.35);
                s.noise(0.0, 0.08, 1400.0, 500.0, 0.003, 30.0, 0.2);
            }
            // Glue landing on someone (or something): a sticky gloop.
            Sfx::Gloop => {
                s.tone(0.0, 0.16, 140.0, 260.0, Wave::Sine, 0.004, 14.0, 0.45);
                s.wobble(0.02, 0.14, 90.0, 26.0, 0.25);
                s.noise(0.0, 0.1, 700.0, 250.0, 0.004, 22.0, 0.25);
            }
            Sfx::Ready => s.tone(0.0, 0.14, 880.0, 1320.0, Wave::Sine, 0.005, 14.0, 0.22),
            // The 3-2-1 beeps (the start horn, `Start`, is the GO).
            Sfx::Count => s.tone(0.0, 0.16, 660.0, 660.0, Wave::Square, 0.003, 12.0, 0.14),
            // The last five seconds: a sharp, urgent two-note blip each second.
            Sfx::Tick => {
                s.tone(0.0, 0.07, 1175.0, 1175.0, Wave::Square, 0.001, 30.0, 0.14);
                s.tone(0.08, 0.1, 1568.0, 1568.0, Wave::Square, 0.001, 24.0, 0.14);
            }
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
            // Jake going digital: a fast rising arpeggio of square blips over a filtered sweep...
            Sfx::Digitize => {
                for (k, f) in [523.0, 784.0, 1046.0, 1568.0, 2093.0, 3136.0].into_iter().enumerate() {
                    s.tone(k as f32 * 0.04, 0.05, f, f, Wave::Square, 0.001, 30.0, 0.09);
                }
                s.swell(0.0, 0.3, 300.0, 6000.0, 0.2);
            }
            // ...and coming back: the same, falling.
            Sfx::Materialize => {
                for (k, f) in [3136.0, 2093.0, 1568.0, 1046.0, 784.0, 523.0].into_iter().enumerate() {
                    s.tone(k as f32 * 0.04, 0.05, f, f, Wave::Square, 0.001, 30.0, 0.09);
                }
                s.tone(0.24, 0.12, 180.0, 90.0, Wave::Sine, 0.002, 20.0, 0.4);
            }
            // Robert's lightning: electricity, not an explosion. A burst of irregular high crackles over
            // an angry mains buzz, with a fizzing arc whining down.
            Sfx::Thunder => {
                let mut seed = 0x2545_f491u32;
                let mut rnd = || {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    (seed % 10_000) as f32 / 10_000.0
                };
                let mut t = 0.0;
                while t < 0.55 {
                    let len = 0.006 + rnd() * 0.02;
                    s.noise(t, len, 9000.0, 5000.0 + rnd() * 3000.0, 0.0005, 80.0, 0.25 + rnd() * 0.35);
                    t += 0.008 + rnd() * 0.04 * (1.0 + t * 2.0);
                }
                s.tone(0.0, 0.5, 120.0, 100.0, Wave::Saw, 0.003, 5.0, 0.22);
                s.tone(0.0, 0.5, 240.0, 200.0, Wave::Square, 0.003, 6.0, 0.08);
                s.tone(0.0, 0.35, 3200.0, 900.0, Wave::Saw, 0.002, 7.0, 0.06);
            }
            // ...and its sizzle on each person it strikes.
            Sfx::Zap => {
                s.noise(0.0, 0.12, 7000.0, 3000.0, 0.001, 22.0, 0.35);
                s.tone(0.0, 0.1, 1800.0, 600.0, Wave::Saw, 0.001, 25.0, 0.12);
            }
            // Michelle's Eye of the Storm: a firm clunk, then a bright, still chime.
            Sfx::Freeze => {
                s.tone(0.0, 0.1, 140.0, 90.0, Wave::Square, 0.001, 25.0, 0.3);
                s.tone(0.08, 0.4, 1318.0, 1318.0, Wave::Sine, 0.002, 9.0, 0.22);
                s.tone(0.14, 0.4, 1976.0, 1976.0, Wave::Sine, 0.002, 9.0, 0.16);
            }
            // A rocket: a hard thump of ignition and a hissing roar.
            Sfx::RocketLaunch => {
                s.tone(0.0, 0.12, 110.0, 50.0, Wave::Sine, 0.002, 18.0, 0.6);
                s.noise(0.0, 0.5, 2500.0, 900.0, 0.01, 5.0, 0.4);
                s.swell(0.0, 0.4, 600.0, 3000.0, 0.15);
            }
            // A dash: a short, airy whoosh.
            Sfx::Dash => s.noise(0.0, 0.16, 900.0, 3800.0, 0.01, 16.0, 0.3),
            // The giant sword: a heavy, rising whoosh with a low hum of the blade...
            Sfx::SwordSwing => {
                s.noise(0.0, 0.24, 500.0, 4200.0, 0.03, 9.0, 0.42);
                s.tone(0.0, 0.2, 260.0, 520.0, Wave::Sine, 0.02, 10.0, 0.12);
            }
            // ...and a sharp, ringing cut when it lands.
            Sfx::Slash => {
                s.noise(0.0, 0.1, 7000.0, 2500.0, 0.001, 30.0, 0.55);
                s.tone(0.0, 0.3, 1650.0, 1600.0, Wave::Tri, 0.001, 12.0, 0.12);
                s.tone(0.0, 0.1, 180.0, 90.0, Wave::Sine, 0.001, 25.0, 0.4);
            }
            // A Molotov breaking: glass, then the whump of the fire catching.
            Sfx::Shatter => {
                s.noise(0.0, 0.2, 9000.0, 4000.0, 0.001, 18.0, 0.45);
                for (k, f) in [2600.0, 3300.0, 4100.0].into_iter().enumerate() {
                    s.tone(k as f32 * 0.02, 0.12, f, f * 0.97, Wave::Tri, 0.001, 30.0, 0.07);
                }
                s.noise(0.05, 0.4, 500.0, 200.0, 0.03, 6.0, 0.4);
            }
            // Fire: a few quiet crackles (played over and over while anything burns).
            Sfx::Crackle => {
                for (t, v) in [(0.0, 0.25), (0.05, 0.12), (0.11, 0.2), (0.19, 0.1)] {
                    s.noise(t, 0.012, 6000.0, 2500.0, 0.0005, 100.0, v);
                }
                s.noise(0.0, 0.25, 400.0, 300.0, 0.05, 6.0, 0.06);
            }
            // Calling in the air strike over a crackly ham radio: a squelch, a burst of garbled
            // talk, and a "roger" beep.
            Sfx::Radio => {
                s.noise(0.0, 0.05, 7000.0, 5000.0, 0.001, 30.0, 0.3);
                let syllables = [(0.08, 240.0), (0.17, 300.0), (0.24, 220.0), (0.36, 280.0), (0.44, 330.0), (0.52, 250.0), (0.66, 300.0), (0.74, 210.0), (0.83, 270.0)];
                for (t, f) in syllables {
                    s.tone(t, 0.07, f, f * 0.85, Wave::Square, 0.008, 14.0, 0.11);
                    s.tone(t, 0.07, f * 3.1, f * 2.8, Wave::Saw, 0.008, 18.0, 0.04);
                    s.noise(t, 0.07, 2600.0, 1800.0, 0.008, 16.0, 0.12);
                }
                s.noise(0.0, 1.0, 3000.0, 3000.0, 0.01, 1.0, 0.04);
                s.tone(0.96, 0.09, 1100.0, 1100.0, Wave::Sine, 0.003, 4.0, 0.12);
                s.noise(1.05, 0.06, 7000.0, 5000.0, 0.001, 30.0, 0.25);
            }
            // The jet: a roar swelling in and passing over, with a high whine.
            Sfx::Jet => {
                s.swell(0.0, 1.6, 300.0, 3200.0, 0.35);
                s.noise(1.5, 1.2, 2500.0, 400.0, 0.02, 2.5, 0.3);
                s.tone(0.2, 2.2, 2100.0, 1300.0, Wave::Saw, 0.6, 1.5, 0.04);
            }
            // A bomb falling: the classic descending whistle.
            Sfx::Whistle => s.tone(0.0, 1.0, 1900.0, 650.0, Wave::Sine, 0.05, 1.2, 0.16),
            // Vid diving into someone: a deep, warping bass drop under a rush of glitchy blips...
            Sfx::Possess => {
                s.tone(0.0, 0.45, 160.0, 45.0, Wave::Sine, 0.005, 5.0, 0.55);
                s.swell(0.0, 0.3, 4000.0, 300.0, 0.25);
                for (k, f) in [1760.0, 1320.0, 2093.0, 990.0, 1568.0].into_iter().enumerate() {
                    s.tone(k as f32 * 0.035, 0.03, f, f, Wave::Square, 0.001, 50.0, 0.08);
                }
            }
            // ...and stepping back out: the same, rising.
            Sfx::Release => {
                s.tone(0.0, 0.4, 50.0, 170.0, Wave::Sine, 0.01, 5.0, 0.45);
                s.swell(0.0, 0.3, 300.0, 4000.0, 0.22);
                for (k, f) in [990.0, 1568.0, 1320.0, 2093.0, 1760.0].into_iter().enumerate() {
                    s.tone(0.1 + k as f32 * 0.035, 0.03, f, f, Wave::Square, 0.001, 50.0, 0.08);
                }
            }
            // Doug's grab: a snap of cloth gripped, then the whoosh of someone going over.
            Sfx::JudoGrab => {
                s.noise(0.0, 0.04, 3500.0, 1500.0, 0.001, 60.0, 0.4);
                s.tone(0.0, 0.05, 220.0, 160.0, Wave::Tri, 0.001, 40.0, 0.2);
                s.noise(0.05, 0.25, 500.0, 2600.0, 0.03, 10.0, 0.35);
            }
            // ...and them hitting the mat: a heavy, flat body thud.
            Sfx::Slam => {
                s.tone(0.0, 0.25, 95.0, 40.0, Wave::Sine, 0.001, 12.0, 0.9);
                s.noise(0.0, 0.18, 1200.0, 200.0, 0.001, 20.0, 0.6);
                s.tone(0.0, 0.06, 240.0, 120.0, Wave::Square, 0.001, 45.0, 0.15);
            }
            // Travis bulking up: a low, straining swell rising to a grunt.
            Sfx::Flex => {
                s.tone(0.0, 0.35, 70.0, 130.0, Wave::Saw, 0.08, 4.0, 0.3);
                s.tone(0.0, 0.35, 140.0, 260.0, Wave::Square, 0.08, 5.0, 0.08);
                s.noise(0.2, 0.15, 600.0, 300.0, 0.01, 20.0, 0.3);
            }
            // Each of his steps: a deep boom with a rattle of grit...
            Sfx::Stomp => {
                s.tone(0.0, 0.22, 70.0, 32.0, Wave::Sine, 0.001, 12.0, 0.75);
                s.noise(0.0, 0.12, 700.0, 150.0, 0.001, 25.0, 0.35);
            }
            // ...and landing from a jump: bigger, with a rumble rolling off.
            Sfx::BigStomp => {
                s.tone(0.0, 0.45, 60.0, 26.0, Wave::Sine, 0.001, 6.0, 0.95);
                s.noise(0.0, 0.5, 900.0, 90.0, 0.001, 7.0, 0.55);
                s.wobble(0.05, 0.35, 45.0, 18.0, 0.2);
            }
            // Carrie's laptop: a clack as it opens, then a swarm of buzzing rising out of it.
            Sfx::Bugs => {
                s.tone(0.0, 0.03, 1400.0, 900.0, Wave::Square, 0.001, 80.0, 0.15);
                s.noise(0.0, 0.03, 4000.0, 2000.0, 0.001, 90.0, 0.3);
                for (k, f) in [190.0, 230.0, 260.0, 310.0].into_iter().enumerate() {
                    s.tone(0.05 + k as f32 * 0.04, 0.6, f, f * 1.15, Wave::Saw, 0.08, 4.0, 0.07);
                }
                s.wobble(0.05, 0.6, 210.0, 33.0, 0.12);
            }
            // Matt's skateboard: the clack of it hitting the ground and a fast rolling rumble.
            Sfx::Skate => {
                s.tone(0.0, 0.04, 900.0, 500.0, Wave::Square, 0.001, 70.0, 0.2);
                s.noise(0.0, 0.03, 3000.0, 1500.0, 0.001, 90.0, 0.35);
                s.noise(0.03, 0.4, 500.0, 900.0, 0.01, 5.0, 0.4);
                s.wobble(0.03, 0.4, 70.0, 40.0, 0.15);
            }
            // Zack's tornadoes spinning up: a rising, howling rush of wind.
            Sfx::Tornado => {
                s.swell(0.0, 0.9, 250.0, 1800.0, 0.35);
                s.noise(0.3, 0.9, 1400.0, 500.0, 0.05, 3.0, 0.3);
                s.tone(0.1, 0.9, 180.0, 320.0, Wave::Saw, 0.2, 2.5, 0.05);
            }
            // Kaleb whistling his arrow: a quick rising slide and a high note, like Yondu.
            Sfx::ArrowWhistle => {
                s.tone(0.0, 0.16, 900.0, 2100.0, Wave::Sine, 0.01, 6.0, 0.22);
                s.tone(0.16, 0.14, 2100.0, 2050.0, Wave::Sine, 0.005, 9.0, 0.2);
                s.noise(0.0, 0.3, 4000.0, 3000.0, 0.02, 8.0, 0.03);
            }
            // Vid opening his bag of books: a rustle of pages and a rising shimmer of magic.
            Sfx::BookOpen => {
                for k in 0..4 {
                    s.noise(k as f32 * 0.05, 0.05, 3000.0, 1500.0, 0.002, 40.0, 0.18);
                }
                s.tone(0.1, 0.6, 600.0, 1200.0, Wave::Sine, 0.1, 3.0, 0.1);
                s.tone(0.1, 0.6, 900.0, 1800.0, Wave::Sine, 0.1, 3.0, 0.06);
            }
            // An attack glancing off his shield: a bright, ringing ping.
            Sfx::ShieldBlock => {
                s.tone(0.0, 0.25, 1760.0, 1700.0, Wave::Sine, 0.001, 10.0, 0.22);
                s.tone(0.0, 0.2, 2640.0, 2600.0, Wave::Sine, 0.001, 14.0, 0.1);
            }
            // Kyle putting it all off: a shuffle of papers into a pile.
            Sfx::Backlog => {
                for k in 0..3 {
                    s.noise(k as f32 * 0.07, 0.07, 2500.0, 1200.0, 0.005, 25.0, 0.25);
                }
                s.tone(0.0, 0.08, 300.0, 220.0, Wave::Tri, 0.002, 30.0, 0.12);
            }
            // ...and it all going at once: a flurry of whooshes and slaps.
            Sfx::BacklogBurst => {
                for k in 0..5 {
                    let t = k as f32 * 0.03;
                    s.noise(t, 0.12, 1200.0, 4000.0, 0.005, 18.0, 0.3);
                    s.tone(t, 0.08, 160.0, 70.0, Wave::Sine, 0.001, 30.0, 0.25);
                }
            }
            // Weston locking in: a sharp click, then a low hum swelling as the things round him stir.
            // A huge beast roaring: a held, rumbling growl (low voices, a rasp of breath, all fluttering
            // in the throat), echoing back off the dunes.
            Sfx::Roar => {
                s.growl(0.0, 1.5, 78.0, 23.0, 0.42);
                s.growl(0.0, 1.5, 117.0, 29.0, 0.18);
                s.growl(0.05, 1.4, 58.0, 17.0, 0.3);
                s.echo(0.26, 0.45, 3);
            }
            // A zombie's moan: a low, wavering voice, breathy, sagging at the end.
            Sfx::Groan => {
                s.growl(0.0, 0.9, 92.0, 6.0, 0.28);
                s.tone(0.0, 0.9, 150.0, 115.0, Wave::Tri, 0.15, 2.0, 0.12);
                s.noise(0.0, 0.9, 700.0, 400.0, 0.15, 2.0, 0.12);
            }
            // The sandstorm: gusting wind (its cutoff rising and falling) over a low roar, with a
            // thin whistle; faded in and out so back to back it runs on without a seam.
            Sfx::Sandstorm => {
                s.noise(0.0, 0.7, 500.0, 1900.0, 0.35, 0.0, 0.28);
                s.noise(0.7, 0.7, 1900.0, 600.0, 0.0, 1.6, 0.28);
                s.noise(0.0, 1.4, 260.0, 220.0, 0.35, 0.8, 0.3);
                s.tone(0.1, 1.2, 900.0, 1250.0, Wave::Sine, 0.4, 1.4, 0.025);
            }
            Sfx::LockIn => {
                s.tone(0.0, 0.03, 2200.0, 1800.0, Wave::Square, 0.001, 80.0, 0.15);
                s.noise(0.0, 0.02, 6000.0, 4000.0, 0.001, 100.0, 0.25);
                s.tone(0.04, 0.6, 110.0, 220.0, Wave::Saw, 0.15, 3.0, 0.12);
                s.tone(0.04, 0.6, 660.0, 990.0, Wave::Sine, 0.2, 3.0, 0.08);
                s.wobble(0.04, 0.6, 330.0, 9.0, 0.06);
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
        // (The glide is the same ratio every sample, so a multiply rather than a `powf` each.)
        let (mut f, glide) = (f0 as f64, glide(f0, f1, len));
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            phase = (phase + f as f32 / RATE as f32).fract();
            f *= glide;
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
        let (mut cutoff, glide) = (c0 as f64, glide(c0, c1, len));
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let c = cutoff as f32;
            let k = 1.0 - (-std::f32::consts::TAU * c / RATE as f32).exp();
            let x = self.rand();
            y += (x - y) * k;
            // The filter takes the edge off the level, more so at low cutoffs; make some back.
            let gain = (1.0 + 2000.0 / c).sqrt().min(4.0);
            *self.at(i0 + i) += y * gain * vol * Self::env(t, len, attack, decay);
            cutoff *= glide;
        }
    }

    /// Noise that grows louder and brighter, then stops dead.
    fn swell(&mut self, start: f32, len: f32, c0: f32, c1: f32, vol: f32) {
        let n = (len * RATE as f32) as usize;
        let i0 = (start * RATE as f32) as usize;
        let mut y = 0.0f32;
        let (mut cutoff, glide) = (c0 as f64, glide(c0, c1, len));
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let c = cutoff as f32;
            let k = 1.0 - (-std::f32::consts::TAU * c / RATE as f32).exp();
            let x = self.rand();
            y += (x - y) * k;
            let a = (t / len).powi(2) * ((len - t) / 0.004).clamp(0.0, 1.0);
            *self.at(i0 + i) += y * 2.0 * vol * a;
            cutoff *= glide;
        }
    }

    /// A held growl: a saw at `f` (drifting a little) and a rasp of low noise, both fluttering at
    /// `rate` Hz, swelling in, held, and dying away over its last third.
    fn growl(&mut self, start: f32, len: f32, f: f32, rate: f32, vol: f32) {
        let n = (len * RATE as f32) as usize;
        let i0 = (start * RATE as f32) as usize;
        let (mut phase, mut y) = (0.0f32, 0.0f32);
        let k = 1.0 - (-std::f32::consts::TAU * 700.0 / RATE as f32).exp();
        for i in 0..n {
            let t = i as f32 / RATE as f32;
            let fi = f * (1.0 + 0.04 * (t * 2.3).sin() + 0.02 * (t * 7.1).sin());
            phase = (phase + fi / RATE as f32).fract();
            y += (self.rand() - y) * k;
            let flutter = 0.6 + 0.4 * (t * rate * std::f32::consts::TAU).sin();
            let env = (t / 0.15).min(1.0) * ((len - t) / (len * 0.35)).clamp(0.0, 1.0);
            *self.at(i0 + i) += ((phase * 2.0 - 1.0) * 0.7 + y * 2.2) * flutter * env * vol;
        }
    }

    /// Echoes of everything so far: `times` quieter copies, each `delay` seconds after the last.
    fn echo(&mut self, delay: f32, gain: f32, times: usize) {
        let d = (delay * RATE as f32) as usize;
        let dry = self.s.clone();
        for k in 1..=times {
            let g = gain.powi(k as i32);
            for (i, v) in dry.iter().enumerate() {
                *self.at(i + d * k) += v * g;
            }
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

/// What an exponential glide from `from` to `to` over `len` seconds multiplies by each sample
/// (in f64: a sound tens of thousands of samples long would drift off pitch in f32).
fn glide(from: f32, to: f32, len: f32) -> f64 {
    (to as f64 / from as f64).powf(1.0 / (len as f64 * RATE as f64))
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
    fire_acc: f32,
    wind_acc: f32,
    /// Bigworm's sandstorm: time since the last gust started.
    storm_acc: f32,
    special_cd: Option<u32>,
    /// The countdown number last shown (3, 2, 1), to beep on each.
    count: Option<u32>,
    /// Whole seconds left in the match, once it's down to the last few (for the ticks).
    ending: Option<u32>,
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
        Self { sounds, muted: false, master: 0.8, effects: 1.0, last: vec![-1.0; Sfx::ALL.len()], now: 0.0, step_dist: [0.0; N_BODIES], fuse_acc: 0.0, fire_acc: 0.0, wind_acc: 0.0, storm_acc: STORM_GUST, special_cd: None, count: None, ending: None }
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
                        Source::Slash => Sfx::Slash,
                        // (The fire's own crackle is the sound.)
                        Source::Fire => Sfx::Punch,
                        Source::Sushi => Sfx::Splat,
                        // The blast or the bite is the sound; just add a thump.
                        Source::Grenade | Source::Special => Sfx::Punch,
                        Source::Boss => Sfx::Slam,
                        Source::Ember => Sfx::Scratch,
                    };
                    self.at(s, pos, ear, 0.9);
                    if headshot {
                        self.at(Sfx::Headshot, pos, ear, 1.0);
                    }
                }
                Event::Death { pos, .. } => self.at(Sfx::Death, pos, ear, 1.0),
                Event::Jump { who, pos } if Some(who) != own => self.at(Sfx::Jump, pos, ear, 0.25),
                Event::Swing { who } if Some(who) != own => {
                    let sword = m.f[who].holding == Some(ItemKind::Sword);
                    self.at(if sword { Sfx::SwordSwing } else { Sfx::Swing }, m.f[who].center(), ear, if sword { 0.6 } else { 0.2 });
                }
                Event::Jump { .. } | Event::Swing { .. } => {}
                Event::Land { pos } => self.at(Sfx::Land, pos, ear, 0.4),
                Event::Throw { who } => self.at(Sfx::Throw, m.f[who].center(), ear, 0.9),
                Event::Grab { pos } => self.at(Sfx::Grab, pos, ear, 0.8),
                Event::Explosion { pos } => self.play(Sfx::Explosion, 1.0 - (pos.distance(ear) / W * 0.3)),
                Event::Heal { pos, .. } => self.at(Sfx::Heal, pos, ear, 1.0),
                Event::Spawn { pos } => self.at(Sfx::Spawn, pos, ear, 0.8),
                Event::Special { who } => {
                    if let Some(s) = crate::special_sound(&m.f[who]) {
                        self.at(s, m.f[who].center(), ear, 1.0);
                    }
                }
                Event::Poof { pos } => self.at(Sfx::Poof, pos, ear, 0.9),
                Event::Glued { pos } => self.at(Sfx::Gloop, pos, ear, 0.9),
                Event::Bite { pos } => self.at(Sfx::Bite, pos, ear, 0.9),
                Event::Scratch { pos } => self.at(Sfx::Scratch, pos, ear, 0.6),
                Event::Splat { pos } => self.at(Sfx::Splat, pos, ear, 1.0),
                Event::Hook { pos } => self.at(Sfx::Hook, pos, ear, 1.0),
                Event::RocketLaunch { pos } => self.at(Sfx::RocketLaunch, pos, ear, 1.0),
                Event::Dash { who, pos } if Some(who) != own => self.at(Sfx::Dash, pos, ear, 0.6),
                Event::Dash { .. } => {}
                Event::Ignite { pos } => self.at(Sfx::Shatter, pos, ear, 1.0),
                Event::RadioCall { pos } => self.at(Sfx::Radio, pos, ear, 1.0),
                Event::JetPass { .. } => self.play(Sfx::Jet, 0.9),
                Event::BombDrop { pos } => self.at(Sfx::Whistle, pos, ear, 0.8),
                Event::Zap { pos, .. } => self.at(Sfx::Zap, pos, ear, 0.8),
                Event::Blocked { pos } => self.at(Sfx::ShieldBlock, pos, ear, 0.8),
                // (Their special's own sound covers these.)
                Event::EyeOfTheStorm { .. } | Event::LaptopOpen { .. } | Event::Awaken { .. } => {}
                Event::BacklogBurst { pos } => self.at(Sfx::BacklogBurst, pos, ear, 1.0),
                Event::ArrowCall { pos } => self.at(Sfx::ArrowWhistle, pos, ear, 0.9),
                Event::Slam { pos } => self.at(Sfx::Slam, pos, ear, 1.0),
                // The Gatecrasher smashing through the wall; Bigworm coming up out of the sand with a roar.
                Event::BossBurst { .. } if m.f[BOSS].boss.is_some_and(|b| b.kind == go_box_sim::boss::BossKind::Sandmaw) => {
                    self.play(Sfx::Roar, 1.0);
                    self.play(Sfx::Stomp, 0.6);
                }
                Event::BossBurst { .. } => {
                    self.play(Sfx::Explosion, 1.0);
                    self.play(Sfx::BigStomp, 1.0);
                }
                Event::BossSlam { pos } => self.at(Sfx::BigStomp, pos, ear, 1.0),
                Event::BossRoar { pos } => self.at(Sfx::Roar, pos, ear, 1.0),
                Event::Swallowed { pos } => {
                    self.at(Sfx::BigStomp, pos, ear, 1.0);
                    self.at(Sfx::Bite, pos, ear, 1.0);
                }
                Event::ShiftWarn => self.play(Sfx::Possess, 0.9),
                Event::Shift => {
                    self.play(Sfx::BigStomp, 1.0);
                    self.play(Sfx::Release, 0.8);
                }
                Event::ZombieIn { pos } => self.at(Sfx::Groan, pos, ear, 0.55),
                Event::KnockedDown { pos } => self.at(Sfx::Slam, pos, ear, 0.7),
                Event::Bitten { pos, .. } => {
                    self.at(Sfx::Bite, pos, ear, 1.0);
                    self.at(Sfx::Groan, pos, ear, 1.0);
                }
                Event::Revived { pos, .. } => {
                    self.at(Sfx::Heal, pos, ear, 1.0);
                    self.play(Sfx::Ready, 0.7);
                }
                Event::BossSplat { pos, sand } => self.at(if sand { Sfx::Stomp } else { Sfx::Splat }, pos, ear, 0.8),
                Event::Quake { pos, big } => self.at(if big { Sfx::BigStomp } else { Sfx::Stomp }, pos, ear, if big { 1.0 } else { 0.7 }),
                Event::Hologram { pos, out } => self.at(if out { Sfx::Materialize } else { Sfx::Digitize }, pos, ear, 1.0),
                Event::Matrix { pos, out } => self.at(if out { Sfx::Release } else { Sfx::Possess }, pos, ear, 1.0),
                Event::Clatter { pos, kind } => {
                    let s = match kind {
                        ItemKind::Rock => Sfx::ClatterRock,
                        ItemKind::Spear => Sfx::ClatterSpear,
                        ItemKind::Grenade => Sfx::ClatterGrenade,
                        ItemKind::Sushi => Sfx::Splat,
                        ItemKind::Phone => Sfx::ClatterGrenade,
                        ItemKind::RocketLauncher => Sfx::ClatterSpear,
                        ItemKind::Sword => Sfx::ClatterSpear,
                        ItemKind::Molotov => Sfx::ClatterGrenade,
                        ItemKind::AirStrike => Sfx::ClatterGrenade,
                    };
                    self.at(s, pos, ear, 0.7);
                }
            }
        }
    }

    /// Your own predicted jumps and swings, played without waiting for the server.
    /// `holding`: what you've got in hand (a sword swings with its own whoosh).
    pub fn local(&mut self, events: &[Event], holding: Option<ItemKind>) {
        for e in events {
            match e {
                Event::Jump { .. } => self.play(Sfx::Jump, 0.3),
                Event::Swing { .. } if holding == Some(ItemKind::Sword) => self.play(Sfx::SwordSwing, 0.7),
                Event::Swing { .. } => self.play(Sfx::Swing, 0.3),
                Event::Dash { .. } => self.play(Sfx::Dash, 0.5),
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
        // The last five seconds tick down out loud.
        let left = m.config.max_ticks.saturating_sub(m.play_ticks()).div_ceil(TICK_RATE);
        let ending = (!m.counting_down() && (1..=ENDING_SECS).contains(&left)).then_some(left);
        if ending != self.ending {
            if ending.is_some() {
                self.play(Sfx::Tick, 0.9);
            }
            self.ending = ending;
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
        // Anything burning crackles (louder the closer it is).
        let fire = m.effects.iter().filter(|e| matches!(e, go_box_sim::character::Effect::Fire(_))).map(|e| e.pos()).min_by(|a, b| a.distance(ear).total_cmp(&b.distance(ear)));
        match fire {
            Some(p) => {
                self.fire_acc += dt;
                if self.fire_acc >= 0.22 {
                    self.fire_acc = 0.0;
                    self.at(Sfx::Crackle, p, ear, 0.5);
                }
            }
            None => self.fire_acc = 0.0,
        }
        // A tornado howls on (the nearest one; a new gust as the last dies away).
        let wind = m.effects.iter().filter(|e| matches!(e, go_box_sim::character::Effect::Tornado(_))).map(|e| e.pos()).min_by(|a, b| a.distance(ear).total_cmp(&b.distance(ear)));
        match wind {
            Some(p) => {
                self.wind_acc += dt;
                if self.wind_acc >= 0.9 {
                    self.wind_acc = 0.0;
                    self.at(Sfx::Tornado, p, ear, 0.35);
                }
            }
            None => self.wind_acc = 0.0,
        }
        // Bigworm inhaling: the sandstorm howls on, a gust after a gust (the first right away).
        let inhaling = m.f[BOSS].alive() && m.f[BOSS].boss.is_some_and(|b| b.mv == go_box_sim::boss::BossMove::Inhale && b.t >= b.mv.timing().0 / 2);
        if inhaling {
            self.storm_acc += dt;
            if self.storm_acc >= STORM_GUST {
                self.storm_acc = 0.0;
                self.play(Sfx::Sandstorm, 0.75);
            }
        } else {
            self.storm_acc = STORM_GUST;
        }
        let cd = own.map(|s| &m.f[s]).filter(|f| f.alive() && f.special().is_some()).map(|f| f.special_cd);
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
        self.ending = None;
    }
}

/// The match's last seconds tick down (a sound each second, and the clock pulses: see the HUD).
pub const ENDING_SECS: u32 = 5;

fn vec2_mid() -> Vec2 {
    Vec2::new(W / 2.0, H / 2.0)
}
