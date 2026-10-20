//! The Gatecrasher's hall (the boss map): a torchlit dungeon wall with a great hole smashed through
//! the middle of it, the boss itself standing in the hole, and its shockwaves.

use crate::{dither, ellipse_outline, fbm, fill_ellipse, hash, ik, line, mix};
use go_box_sim::boss::{burst_tick, BossMove, BossShot, BossState, Shockwave, FIST_X, TOSS_AT, WAVE_H};
use go_box_sim::sim::{Fighter, Match, FLOOR_Y, H, W};
use macroquad::prelude::*;

/// The hole in the back wall: its middle, and half its width and height (its foot is the floor).
const HOLE: (f32, f32) = (W / 2.0, 190.0);
const HOLE_R: (f32, f32) = (54.0, 82.0);
/// The torches on the wall, either side.
pub const TORCHES: [Vec2; 4] = [vec2(56.0, 110.0), vec2(150.0, 86.0), vec2(330.0, 86.0), vec2(424.0, 110.0)];

const SKIN: Color = Color::new(0.40, 0.34, 0.42, 1.0);
const SKIN_LIT: Color = Color::new(0.54, 0.47, 0.55, 1.0);
const SKIN_DARK: Color = Color::new(0.24, 0.20, 0.26, 1.0);
const IRON: Color = Color::new(0.30, 0.31, 0.34, 1.0);
const IRON_LIT: Color = Color::new(0.55, 0.56, 0.58, 1.0);
const LEATHER: Color = Color::new(0.34, 0.22, 0.14, 1.0);
const HORN: Color = Color::new(0.86, 0.80, 0.66, 1.0);
const EYES: Color = Color::new(1.0, 0.70, 0.20, 1.0);
const RAGE: Color = Color::new(1.0, 0.22, 0.10, 1.0);
const BRICK: (f32, f32, f32) = (62.0, 56.0, 60.0);

/// How far into the hole a pixel is (< 1 inside), with a jagged, broken edge.
fn in_hole(x: f32, y: f32) -> f32 {
    let (dx, dy) = ((x - HOLE.0) / HOLE_R.0, (y - HOLE.1) / HOLE_R.1);
    let a = dy.atan2(dx);
    let jag = 1.0 + (fbm(a * 3.0 + 10.0, 0.0, 41) - 0.5) * 0.35 + (hash((a * 14.0) as i32, 0, 42) - 0.5) * 0.12;
    (dx * dx + dy * dy).sqrt() / jag
}

/// A big block of the dungeon wall at a pixel (stone, mortar, grime).
fn wall_px(x: i32, y: i32) -> (f32, f32, f32) {
    let row = y / 9;
    let off = (row % 2) * 9;
    let col = (x + off) / 18;
    let mortar = y % 9 == 0 || (x + off) % 18 == 0;
    let tone = hash(col, row, 43) * 16.0 - 8.0;
    let n = (fbm(x as f32 * 0.06, y as f32 * 0.06, 44) - 0.5) * 26.0;
    let (mut r, mut g, mut b) = (BRICK.0 + tone + n, BRICK.1 + tone + n, BRICK.2 + tone + n);
    if mortar {
        (r, g, b) = (r - 20.0, g - 20.0, b - 18.0);
    }
    // Darker up high, out of the torchlight.
    let k = 0.55 + 0.45 * (y as f32 / H).min(1.0);
    (r * k, g * k, b * k)
}

/// The hall's back wall, smashed through in the middle, with pillars and banners; and the patch of
/// wall that fills the hole until the boss smashes through it (only the hole's pixels).
pub fn make_backdrop() -> (Image, Image) {
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    let mut patch = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let (ux, uy) = (x as u32, y as u32);
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut c = wall_px(x, y);
            // Pillars, and banners between them.
            let pillar = [24, 196, 268, 440].iter().any(|&px| (px..px + 16).contains(&x));
            if pillar {
                let edge = [24, 196, 268, 440].iter().any(|&px| x == px || x == px + 15);
                let n = (fbm(fx * 0.1, fy * 0.04, 45) - 0.5) * 20.0;
                c = (78.0 + n, 72.0 + n, 74.0 + n);
                if edge || y % 40 < 3 {
                    c = (c.0 * 0.6, c.1 * 0.6, c.2 * 0.6);
                }
            }
            for &bx in &[96.0f32, 360.0] {
                let (bx0, bx1) = (bx, bx + 24.0);
                // (Notched at the foot.)
                let bottom = 160.0 - (12.0 - (fx - bx0 - 12.0).abs()) * 0.8;
                if fx >= bx0 && fx < bx1 && fy > 24.0 && fy < bottom {
                    let stripe = (fx - bx0 - 12.0).abs() < 3.0;
                    let n = (fbm(fx * 0.2, fy * 0.1, 46) - 0.5) * 18.0;
                    c = if stripe { (150.0 + n, 120.0 + n, 50.0) } else { (96.0 + n, 22.0 + n * 0.3, 30.0) };
                    if fx < bx0 + 1.0 || fx >= bx1 - 1.0 {
                        c = (c.0 * 0.6, c.1 * 0.6, c.2 * 0.6);
                    }
                }
            }
            // Cracks running out from the hole.
            let h = in_hole(fx, fy);
            if h > 1.0 && h < 1.5 {
                let a = (fy - HOLE.1).atan2(fx - HOLE.0);
                let ray = (fbm(a * 7.0, h * 2.0, 47) - 0.5).abs();
                if ray < 0.008 * (1.5 - h) / 0.5 {
                    c = (c.0 * 0.5, c.1 * 0.5, c.2 * 0.5);
                }
            }
            // The broken rim: lighter, chipped stone.
            if (1.0..1.08).contains(&h) {
                c = (c.0 * 1.35, c.1 * 1.3, c.2 * 1.3);
            }
            // The torches' warm glow on the wall.
            let glow: f32 = TORCHES.iter().map(|t| (1.0 - t.distance(vec2(fx, fy)) / 70.0).max(0.0)).sum();
            c = (c.0 + glow * 60.0, c.1 + glow * 30.0, c.2 + glow * 6.0);
            if h < 1.0 {
                // Through the hole: a dark cavern, red with fire far below.
                let deep = (fy - 110.0) / 150.0;
                let n = fbm(fx * 0.08, fy * 0.08, 48);
                let hole = (10.0 + deep * 60.0 * n, 6.0 + deep * 12.0 * n, 10.0 + deep * 6.0);
                img.set_pixel(ux, uy, dither(ux, uy, hole.0, hole.1, hole.2));
                patch.set_pixel(ux, uy, dither(ux, uy, c.0, c.1, c.2));
            } else {
                img.set_pixel(ux, uy, dither(ux, uy, c.0, c.1, c.2));
            }
        }
    }
    (img, patch)
}

/// The torches' flames (flickering).
pub fn draw_torches(time: f32) {
    for (k, t) in TORCHES.iter().enumerate() {
        draw_rectangle(t.x - 1.0, t.y, 3.0, 6.0, Color::new(0.25, 0.18, 0.12, 1.0));
        draw_rectangle(t.x - 2.0, t.y - 1.0, 5.0, 2.0, IRON);
        let f = (time * 11.0 + k as f32 * 2.3).sin() * 0.5 + 0.5;
        let g = (time * 17.0 + k as f32).sin() * 0.5 + 0.5;
        draw_circle(t.x + 0.5, t.y - 6.0, 7.0 + f * 1.5, Color::new(1.0, 0.55, 0.15, 0.10));
        fill_ellipse(t.x + 0.5, t.y - 3.5, 2.2, 3.5 + f, Color::new(0.95, 0.42, 0.10, 1.0));
        fill_ellipse(t.x + 0.5 + (g - 0.5), t.y - 3.0, 1.2, 2.0 + g, Color::new(1.0, 0.85, 0.40, 1.0));
    }
}

/// Before the boss smashes through: the wall over the hole, shaking harder as it gets close.
pub fn draw_patch(patch: &Texture2D, m: &Match) {
    let burst = burst_tick(m.config.countdown);
    if m.f[go_box_sim::sim::BOSS].boss.is_none() || m.tick > burst {
        return;
    }
    let close = 1.0 - (burst - m.tick) as f32 / burst.max(1) as f32;
    let shake = if close > 0.3 { ((m.tick as f32 * 2.7).sin() * 1.5 * close).round() } else { 0.0 };
    draw_texture(patch, shake, 0.0, WHITE);
}

/// Thick limb, outlined.
fn limb(a: Vec2, b: Vec2, t: f32, c: Color) {
    line(a, b, t + 2.0, SKIN_DARK);
    line(a, b, t, c);
}

/// A fist (or what it holds): an iron-knuckled lump.
fn fist(p: Vec2, flash: bool) {
    fill_ellipse(p.x, p.y, 8.5, 7.5, hurt(SKIN_DARK, flash));
    fill_ellipse(p.x, p.y, 7.5, 6.5, hurt(SKIN, flash));
    draw_rectangle((p.x - 6.0).round(), (p.y - 6.0).round(), 12.0, 3.0, hurt(IRON, flash));
    for k in 0..3 {
        draw_rectangle((p.x - 5.0 + k as f32 * 4.0).round(), (p.y - 6.0).round(), 2.0, 1.0, hurt(IRON_LIT, flash));
    }
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where each fist is (left, right) and how far its head is thrown back (0..1), for what it's doing.
fn pose(b: &BossState, at: Vec2, time: f32) -> (Vec2, Vec2, f32) {
    let bob = (time * 2.2).sin() * 1.5;
    let rest = |s: f32| vec2(at.x + s * (FIST_X + 2.0), at.y - 8.0 + bob * 0.3);
    let (windup, total) = b.mv.timing();
    let t = b.t as f32;
    let w = windup as f32;
    let mut fists = [rest(-1.0), rest(1.0)];
    let mut roar = 0.0;
    let arm = if b.side < 0.0 { 0 } else { 1 };
    match b.mv {
        BossMove::Rest => {
            // The boulder it tosses between attacks: reach back, hurl, settle.
            let k = TOSS_AT as f32;
            let back = vec2(at.x + b.side * 14.0, at.y - 120.0);
            let out = vec2(at.x + b.side * 76.0, at.y - 92.0);
            fists[arm] = if t < k { rest(b.side).lerp(back, ease(t / k)) } else if t < k + 5.0 { back.lerp(out, ease((t - k) / 4.0)) } else { out.lerp(rest(b.side), ease((t - k - 5.0) / 12.0)) };
        }
        BossMove::Slam => {
            for (k, s) in [-1.0f32, 1.0].into_iter().enumerate() {
                let up = vec2(at.x + s * 20.0, at.y - 128.0);
                fists[k] = if t < w { rest(s).lerp(up, ease(t / w)) } else { rest(s) + vec2(0.0, 2.0) };
            }
            roar = if t < w { ease(t / w) * 0.4 } else { 0.0 };
        }
        BossMove::Volley | BossMove::Boulders => {
            let back = vec2(at.x + b.side * 16.0, at.y - 124.0);
            let out = vec2(at.x + b.side * 78.0, at.y - 92.0);
            fists[arm] = if t < w {
                rest(b.side).lerp(back, ease(t / w))
            } else {
                // Throwing: out, back up for the next (boulders come one after another), out again.
                let k = ((t - w) % 10.0) / 10.0;
                if b.mv == BossMove::Boulders && t < total as f32 - 12.0 { back.lerp(out, ease(k * 2.0)).lerp(back, ease(k * 2.0 - 1.0)) } else { out.lerp(rest(b.side), ease((t - w) / 20.0)) }
            };
        }
        BossMove::Swat => {
            let cocked = vec2(at.x - b.side * 8.0, at.y - 104.0);
            let swept = vec2(at.x + b.side * 92.0, at.y - 96.0);
            fists[arm] = if t < w { rest(b.side).lerp(cocked, ease(t / w)) } else if t < w + 8.0 { cocked.lerp(swept, ease((t - w) / 6.0)) } else { swept.lerp(rest(b.side), ease((t - w - 8.0) / 14.0)) };
        }
        BossMove::Strike => {
            for (k, s) in [-1.0f32, 1.0].into_iter().enumerate() {
                fists[k] = rest(s).lerp(vec2(at.x + s * 64.0, at.y - 118.0), ease(t / 20.0).min(ease((total as f32 - t) / 15.0)));
            }
            roar = ease(t / 20.0).min(ease((total as f32 - t) / 15.0));
        }
        BossMove::Spray => {
            // Braced, head thrown back and mouth wide, spewing.
            for (k, s) in [-1.0f32, 1.0].into_iter().enumerate() {
                fists[k] = rest(s) + vec2(s * 8.0, -10.0) * ease(t / w);
            }
            roar = ease(t / w).min(ease((total as f32 - t) / 10.0));
        }
        BossMove::Missiles => {
            // Hunched, fists planted: the missiles come out of its shoulders.
            for (k, s) in [-1.0f32, 1.0].into_iter().enumerate() {
                fists[k] = rest(s) + vec2(s * -4.0, 3.0) * ease(t / w);
            }
            roar = 0.3 * ease(t / w);
        }
        // (The Sandmaw's; not this boss's.)
        BossMove::Tentacles | BossMove::Inhale | BossMove::Chomp | BossMove::Acid => {}
    }
    (fists[0], fists[1], roar)
}

/// The Gatecrasher: a horned brute of stone and iron filling the hole in the wall. `flash` when it
/// was just hurt; slumped once it's down.
pub fn draw_boss(f: &Fighter, m: &Match, time: f32, flash: bool) {
    let Some(b) = &f.boss else { return };
    if m.tick <= burst_tick(m.config.countdown) {
        return;
    }
    let dead = !f.alive();
    let at = f.pos;
    let breathe = if dead { 0.0 } else { (time * 2.2).sin() * 1.0 };
    let slump = if dead { 14.0 } else { 0.0 };
    let (lf, rf, roar) = if dead { (vec2(at.x - 44.0, at.y - 6.0), vec2(at.x + 44.0, at.y - 6.0), 0.0) } else { pose(b, at, time) };
    let skin = if dead { mix(SKIN, SKIN_DARK, 0.5) } else { SKIN };
    let s = |c: Color| hurt(c, flash);
    // Legs and feet.
    for side in [-1.0f32, 1.0] {
        let x = at.x + side * 15.0;
        draw_rectangle((x - 8.0).round(), at.y - 36.0, 16.0, 36.0, s(SKIN_DARK));
        draw_rectangle((x - 7.0).round(), at.y - 36.0, 14.0, 34.0, s(skin));
        draw_rectangle((x - 7.0 + side * 2.0).round(), at.y - 22.0, 6.0, 3.0, s(IRON));
        draw_rectangle((x - 10.0 + side * 2.0).round(), at.y - 5.0, 20.0, 5.0, s(SKIN_DARK));
    }
    // Torso: widening up to the shoulders.
    let (top, hip) = (at.y - 88.0 + breathe + slump, at.y - 38.0);
    for y in top as i32..hip as i32 {
        let k = (y as f32 - top) / (hip - top);
        let half = 36.0 - k * 12.0;
        draw_rectangle((at.x - half).round(), y as f32, (half * 2.0).round(), 1.0, s(SKIN_DARK));
        draw_rectangle((at.x - half + 1.0).round(), y as f32, (half * 2.0 - 2.0).round(), 1.0, s(if k < 0.35 { SKIN_LIT } else { skin }));
    }
    // Chest plates, belly, belt and loincloth.
    for side in [-1.0f32, 1.0] {
        draw_rectangle((at.x + side * 12.0 - 9.0).round(), top + 10.0, 18.0, 12.0, s(SKIN_DARK));
        draw_rectangle((at.x + side * 12.0 - 8.0).round(), top + 10.0, 16.0, 10.0, s(SKIN_LIT));
    }
    for k in 0..3 {
        draw_rectangle(at.x - 8.0, top + 26.0 + k as f32 * 6.0, 16.0, 1.0, s(SKIN_DARK));
    }
    draw_rectangle(at.x - 26.0, hip - 4.0, 52.0, 6.0, s(LEATHER));
    draw_rectangle(at.x - 5.0, hip - 5.0, 10.0, 8.0, s(IRON_LIT));
    draw_rectangle(at.x - 12.0, hip + 2.0, 24.0, 12.0, s(LEATHER));
    // Enraged: glowing cracks across its chest.
    if b.enraged && !dead {
        let pulse = 0.6 + 0.4 * (time * 8.0).sin();
        let c = Color::new(RAGE.r, RAGE.g, RAGE.b, pulse);
        line(vec2(at.x - 20.0, top + 8.0), vec2(at.x - 8.0, top + 30.0), 1.0, c);
        line(vec2(at.x - 8.0, top + 30.0), vec2(at.x - 14.0, top + 44.0), 1.0, c);
        line(vec2(at.x + 18.0, top + 14.0), vec2(at.x + 6.0, top + 36.0), 1.0, c);
    }
    // Arms, from the shoulders to the fists, elbows out.
    let shoulders = [vec2(at.x - 34.0, top + 8.0), vec2(at.x + 34.0, top + 8.0)];
    for (k, (sh, hand)) in shoulders.into_iter().zip([lf, rf]).enumerate() {
        let out = if k == 0 { 1.0 } else { -1.0 };
        let elbow = ik(sh, hand, 40.0, 40.0, out);
        limb(sh, elbow, 11.0, s(skin));
        limb(elbow, hand, 10.0, s(skin));
        draw_rectangle((elbow.x - 5.0).round(), (elbow.y - 2.0).round(), 10.0, 4.0, s(IRON));
        fist(hand, flash);
    }
    // What it's about to throw, in its hand.
    let holding = if b.mv == BossMove::Rest { b.t <= TOSS_AT } else { b.t < b.mv.timing().0 + 1 };
    if !dead && holding {
        let hand = if b.side < 0.0 { lf } else { rf };
        match b.mv {
            BossMove::Volley => {
                fill_ellipse(hand.x, hand.y - 8.0, 4.0, 4.0, Color::new(0.30, 0.36, 0.22, 1.0));
                draw_rectangle(hand.x - 1.0, hand.y - 14.0, 2.0, 2.0, Color::new(1.0, 0.8, 0.3, 0.5 + 0.5 * (time * 20.0).sin()));
            }
            BossMove::Boulders | BossMove::Rest => fill_ellipse(hand.x, hand.y - 8.0, 5.0, 4.0, Color::new(0.62, 0.60, 0.55, 1.0)),
            _ => {}
        }
    }
    // Spiked pauldrons over the shoulders.
    for sh in shoulders {
        fill_ellipse(sh.x, sh.y - 2.0, 13.0, 9.0, s(IRON));
        fill_ellipse(sh.x, sh.y - 4.0, 11.0, 6.0, s(IRON_LIT));
        for d in [-6.0, 0.0, 6.0] {
            let base = vec2(sh.x + d, sh.y - 9.0);
            line(base, base + vec2(d * 0.3, -6.0), 2.0, s(HORN));
        }
    }
    // The head: low between the shoulders, horns, a heavy brow, tusks; thrown back roaring.
    let head = vec2(at.x, top - 4.0 + slump * 0.5 - roar * 4.0);
    draw_rectangle((head.x - 14.0).round(), (head.y - 6.0).round(), 28.0, 22.0, s(SKIN_DARK));
    draw_rectangle((head.x - 13.0).round(), (head.y - 5.0).round(), 26.0, 20.0, s(skin));
    draw_rectangle((head.x - 13.0).round(), (head.y + 1.0).round(), 26.0, 3.0, s(SKIN_DARK));
    for side in [-1.0f32, 1.0] {
        let root = head + vec2(side * 12.0, -4.0);
        line(root, root + vec2(side * 7.0, -6.0), 4.0, s(HORN));
        line(root + vec2(side * 7.0, -6.0), root + vec2(side * 9.0, -15.0), 3.0, s(HORN));
        line(root + vec2(side * 9.0, -15.0), root + vec2(side * 7.0, -20.0), 2.0, s(HORN));
    }
    let (wind, _) = b.mv.timing();
    let winding = b.mv != BossMove::Rest && b.t < wind;
    let eye = if dead { SKIN_DARK } else if b.enraged || winding { RAGE } else { EYES };
    for side in [-1.0f32, 1.0] {
        let e = head + vec2(side * 6.0 - 2.0, 4.0);
        draw_rectangle(e.x.round(), e.y.round(), 4.0, 2.0, eye);
        if !dead {
            draw_circle(e.x + 2.0, e.y + 1.0, if winding { 5.0 } else { 3.5 }, Color::new(eye.r, eye.g, eye.b, 0.18));
        }
    }
    // The jaw: open when it roars.
    let jaw = (roar * 5.0).round();
    draw_rectangle((head.x - 8.0).round(), (head.y + 10.0).round(), 16.0, 2.0 + jaw, Color::new(0.12, 0.04, 0.05, 1.0));
    for side in [-1.0f32, 1.0] {
        draw_rectangle((head.x + side * 7.0 - 1.0).round(), (head.y + 8.0 + jaw).round(), 2.0, 4.0, s(HORN));
    }
    // Spraying: its mouth blazes.
    if b.mv == BossMove::Spray && !dead {
        let m = vec2(head.x, head.y + 12.0 + jaw * 0.5);
        let k = (b.t as f32 / wind.max(1) as f32).min(1.0);
        let flick = 0.8 + 0.2 * (time * 30.0).sin();
        draw_circle(m.x, m.y, (6.0 + 3.0 * flick) * k, Color::new(1.0, 0.45, 0.1, 0.25 * k));
        fill_ellipse(m.x, m.y, 4.0 * k, (1.5 + jaw * 0.4) * k, Color::new(1.0, 0.8, 0.35, flick * k));
    }
    // Winding up a slam: the floor glows where its fists will land.
    if b.mv == BossMove::Slam && winding {
        let k = b.t as f32 / wind as f32;
        for side in [-1.0f32, 1.0] {
            let x = at.x + side * FIST_X;
            let a = 0.25 + 0.45 * k * (0.5 + 0.5 * (time * 18.0).sin());
            fill_ellipse(x, FLOOR_Y - 0.5, 22.0, 3.0, Color::new(RAGE.r, RAGE.g, RAGE.b, a * 0.4));
            ellipse_outline(x, FLOOR_Y - 0.5, 22.0, 3.0, 1.0, Color::new(RAGE.r, RAGE.g, RAGE.b, a));
        }
    }
}

/// A shockwave: a crest of broken floor and dust racing along it.
/// `ground` is the height of the floor (or the sand) at an x: the crack trails along it.
pub fn draw_shockwave(w: &Shockwave, time: f32, sand: bool, ground: &dyn Fn(f32) -> f32) {
    let x = w.pos.x;
    // The crack it tears in the floor behind it, glowing with magma and cooling further back (in
    // sand, a furrow of churned sand).
    let (crack, ember, glow, hot) = if sand {
        ((0.50, 0.36, 0.20), (0.92, 0.80, 0.56), (0.95, 0.82, 0.58), (0.70, 0.52, 0.30))
    } else {
        ((1.0, 0.35, 0.08), (1.0, 0.55, 0.2), (1.0, 0.40, 0.10), (1.0, 0.45, 0.12))
    };
    let trail = (x - w.start).abs().min(150.0);
    for k in 0..trail as i32 {
        let px = x - w.dir * k as f32;
        let y = ground(px);
        let fade = 1.0 - k as f32 / 150.0;
        let wob = (hash(px as i32, 0, 53) * 2.0).floor();
        draw_rectangle(px.round(), y - 1.0 - wob, 1.0, 1.0 + wob, Color::new(crack.0, crack.1 + if sand { 0.0 } else { 0.3 * fade }, crack.2, 0.85 * fade));
        if k % 3 == 0 {
            draw_rectangle(px.round(), y - 3.0 - wob, 1.0, 1.0, Color::new(ember.0, ember.1, ember.2, 0.35 * fade));
        }
    }
    // Everything from here leans with the ground under it (up a slope, the crest stands out from
    // the sand at right angles to it, not straight up): each piece is laid out along the ground,
    // and stands along its normal there.
    let at = |px: f32| vec2(px, ground(px));
    let normal = |px: f32| vec2((ground(px + 1.0) - ground(px - 1.0)) / 2.0, -1.0).normalize();
    // A wide glow along the ground under the crest, so it reads from across the hall.
    for k in 0..12 {
        let (a, b) = (at(x - w.dir * k as f32 * 2.0), at(x - w.dir * (k + 1) as f32 * 2.0));
        let up = normal(a.x) * 1.0;
        line(a + up, b + up, 6.0, Color::new(glow.0, glow.1, glow.2, 0.30 * (1.0 - k as f32 / 12.0)));
        if k < 6 {
            line(a + up, b + up, 3.0, Color::new(1.0, 0.70, 0.30, 0.45));
        }
    }
    // The crest: a wall of heaved-up ground, tallest at the front, trailing off behind; jagged and
    // shuddering, outlined dark.
    let tall = WAVE_H + 14.0;
    for k in 0..14 {
        let px = x - w.dir * k as f32 * 2.0;
        let (base, n) = (at(px), normal(px));
        let jag = 0.7 + 0.3 * (time * 34.0 + k as f32 * 1.9).sin();
        let h = (tall - k as f32 * 1.7).max(1.0) * jag;
        let c = mix(Color::new(0.95, 0.84, 0.64, 1.0), Color::new(0.45, 0.36, 0.30, 0.6), k as f32 / 14.0);
        line(base, base + n * (h + 1.0), 3.0, Color::new(0.10, 0.08, 0.07, c.a));
        line(base, base + n * h, 2.0, c);
        // Hot underneath.
        line(base, base + n * 3.0, 2.0, Color::new(hot.0, hot.1, hot.2, c.a));
    }
    // Chunks of ground flung up off the front (up off the slope, where it slopes).
    let (front, n) = (at(x), normal(x));
    // (The way it's going, along the ground.)
    let along = vec2(-n.y, n.x) * if -n.y * w.dir < 0.0 { -1.0 } else { 1.0 };
    for k in 0..6 {
        let t = (time * 5.0 + k as f32 * 0.17).fract();
        let side = if k % 2 == 0 { 1.0 } else { 0.3 };
        let p = front + along * (3.0 + k as f32 * 2.5) * side + n * (tall * 0.6 + t * 18.0 - t * t * 22.0);
        let s = if k < 2 { 3.0 } else { 2.0 };
        draw_rectangle(p.x.round(), p.y.round(), s, s, Color::new(0.10, 0.08, 0.07, 1.0 - t));
        draw_rectangle(p.x.round(), p.y.round(), s - 1.0, s - 1.0, Color::new(0.62, 0.54, 0.46, 1.0 - t));
    }
}

/// An ember (a hot glowing spark with a short tail) or a missile (smoking, its tip blinking).
pub fn draw_boss_shot(s: &BossShot, time: f32) {
    let dir = s.vel.normalize_or(vec2(1.0, 0.0));
    if s.kind == go_box_sim::boss::ShotKind::Missile {
        let tail = s.pos - dir * 6.0;
        line(tail, s.pos, 3.0, Color::new(0.10, 0.10, 0.12, 1.0));
        line(tail, s.pos, 2.0, Color::new(0.42, 0.44, 0.48, 1.0));
        let blink = (time * 14.0).sin() > 0.0;
        draw_rectangle((s.pos.x - 1.0).round(), (s.pos.y - 1.0).round(), 2.0, 2.0, if blink { Color::new(1.0, 0.15, 0.1, 1.0) } else { Color::new(0.5, 0.05, 0.05, 1.0) });
        let flame = tail - dir * (2.0 + (time * 40.0).sin().abs() * 2.0);
        line(tail, flame, 2.0, Color::new(1.0, 0.75, 0.3, 1.0));
        draw_circle(s.pos.x, s.pos.y, 5.0, Color::new(1.0, 0.2, 0.1, 0.12));
        return;
    }
    draw_circle(s.pos.x, s.pos.y, 4.0, Color::new(1.0, 0.45, 0.10, 0.22));
    for k in 1..4 {
        let p = s.pos - dir * k as f32 * 2.0;
        draw_rectangle(p.x.round(), p.y.round(), 1.0, 1.0, Color::new(1.0, 0.45, 0.12, 0.8 - k as f32 * 0.22));
    }
    draw_rectangle((s.pos.x - 1.0).round(), (s.pos.y - 1.0).round(), 2.0, 2.0, Color::new(1.0, 0.85, 0.40, 1.0));
}

/// Lightened a little when it's just been hurt (it's hit so often a full flash would leave it white).
fn hurt(c: Color, flash: bool) -> Color {
    if flash { mix(c, Color::new(1.0, 0.92, 0.88, c.a), 0.45) } else { c }
}
