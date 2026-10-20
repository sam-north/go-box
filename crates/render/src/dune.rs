//! The Sandmaw's desert (the Dune map): twin suns over the dunes, the sand funnel into its mouth,
//! hovering skiffs for ledges; the mouth itself (lips, a ring of teeth, a beak), its tentacles and
//! its acid.

use crate::{dither, ellipse_outline, fbm, fill_ellipse, hash, line, mix};
use go_box_sim::boss::{burst_tick, BossMove, BossShot};
use go_box_sim::sandmaw::{Tentacle, BODY_HALF_W, GLOB_R, THROAT, T_REACH, T_W, T_WARN};
use go_box_sim::arena::{dune_sand_y, skiff_deck, DUNE_SKIFFS};
use go_box_sim::sim::{Arena, Fighter, Match, FLOOR_Y, H, W, WALL};
use macroquad::prelude::*;

type Rgb = (f32, f32, f32);

const SUNS: [(f32, f32, f32); 2] = [(352.0, 52.0, 9.0), (386.0, 70.0, 6.0)];
const FLESH: Color = Color::new(0.72, 0.46, 0.40, 1.0);
const FLESH_DARK: Color = Color::new(0.46, 0.26, 0.24, 1.0);
const FLESH_LIT: Color = Color::new(0.86, 0.62, 0.54, 1.0);
const THROAT_C: Color = Color::new(0.16, 0.04, 0.05, 1.0);
const TOOTH: Color = Color::new(0.93, 0.90, 0.78, 1.0);
const TOOTH_DARK: Color = Color::new(0.60, 0.56, 0.44, 1.0);
const BEAK: Color = Color::new(0.80, 0.66, 0.30, 1.0);
const TENTACLE: Color = Color::new(0.62, 0.40, 0.34, 1.0);
const TENTACLE_DARK: Color = Color::new(0.38, 0.22, 0.20, 1.0);
const SAND: Color = Color::new(0.86, 0.72, 0.48, 1.0);
const ACID: Color = Color::new(0.62, 0.92, 0.22, 1.0);
const ACID_DARK: Color = Color::new(0.30, 0.55, 0.08, 1.0);
const RAGE: Color = Color::new(1.0, 0.25, 0.12, 1.0);

fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t)
}

/// Height of a dune ridge (y of its crest) at `x`, for layer `k` (0 far .. 2 near).
fn ridge(x: f32, k: u32) -> f32 {
    let (base, amp, freq) = [(150.0, 10.0, 0.012), (176.0, 16.0, 0.018), (206.0, 18.0, 0.025)][k as usize];
    base - amp * ((x * freq + k as f32 * 2.1).sin() * 0.6 + (fbm(x * freq * 1.7, k as f32 * 5.0, 61 + k) - 0.5) * 1.6)
}

/// The desert behind the fight.
pub fn make_backdrop() -> Image {
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Sky: pale and hazy at the horizon, bluer up high; the suns' glow.
            let mut c = blend((232.0, 226.0, 204.0), (128.0, 170.0, 206.0), (150.0 - fy) / 150.0);
            for &(sx, sy, r) in &SUNS {
                let d = vec2(fx - sx, fy - sy).length();
                let glow = (1.0 - d / (r * 6.0)).max(0.0);
                c = blend(c, (255.0, 246.0, 214.0), glow * glow * 0.9);
                if d < r {
                    c = blend((255.0, 250.0, 228.0), (255.0, 214.0, 150.0), d / r * 0.6);
                }
            }
            // Mesas far off on the horizon.
            let mesa = 140.0 - if (40..120).contains(&x) { 18.0 + (fbm(fx * 0.05, 0.0, 66) - 0.5) * 6.0 } else if (300..340).contains(&x) { 10.0 } else { 0.0 };
            if fy > mesa && fy < 152.0 {
                c = blend(c, (186.0, 150.0, 140.0), 0.65);
            }
            // The dunes, far to near, each lit on its left slope.
            for k in 0..3 {
                let crest = ridge(fx, k);
                if fy >= crest {
                    // (Lit by the slope over a stretch, not pixel to pixel: that flickered into stripes.)
                    let slope = ridge(fx + 8.0, k) - ridge(fx - 8.0, k);
                    let base = [(214.0, 186.0, 140.0), (210.0, 168.0, 112.0), (200.0, 150.0, 92.0)][k as usize];
                    let lit = 1.0 + (slope * 0.02).clamp(-0.12, 0.12);
                    let ripple = ((fy - crest) * 0.9 + fbm(fx * 0.08, fy * 0.3, 70 + k) * 6.0).sin() * 5.0;
                    let n = (fbm(fx * 0.05, fy * 0.05, 73 + k) - 0.5) * 12.0;
                    c = (base.0 * lit + ripple + n, base.1 * lit + ripple * 0.8 + n, base.2 * lit + ripple * 0.5 + n);
                    if fy - crest < 1.0 {
                        c = (c.0 + 22.0, c.1 + 20.0, c.2 + 16.0);
                    }
                }
            }
            // A moisture vaporator on the left dune, a half-buried wreck on the right.
            let vx = 64.0;
            if (fx - vx).abs() < 2.5 && fy > 150.0 && fy < 196.0 {
                let ring = (y % 9) < 2;
                c = if ring { (90.0, 92.0, 96.0) } else if fx < vx { (176.0, 178.0, 180.0) } else { (130.0, 132.0, 136.0) };
            }
            if (fx - vx).abs() < 6.0 && (fy - 158.0).abs() < 1.5 {
                c = (110.0, 112.0, 116.0);
            }
            let wreck = vec2((fx - 418.0) / 34.0, (fy - 206.0) / 14.0);
            if wreck.length() < 1.0 && fy < 210.0 + (fx - 418.0) * 0.05 {
                let plate = (x / 7 + y / 5) % 3 == 0;
                c = if plate { (120.0, 104.0, 92.0) } else { (150.0, 134.0, 118.0) };
                if wreck.length() > 0.9 {
                    c = (90.0, 78.0, 70.0);
                }
            }
            img.set_pixel(x as u32, y as u32, dither(x as u32, y as u32, c.0, c.1, c.2));
        }
    }
    img
}

/// The Dune's terrain: sand underfoot (its throat dark, its lips flesh), sandstone walls and
/// ceiling, and hovering skiffs for ledges.
pub(crate) fn terrain_px(a: &Arena, x: i32, y: i32) -> Option<Option<Rgb>> {
    let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
    // The skiffs: each deck drawn as the smooth slope it is (the ledges underfoot are fine steps).
    if let Some(&skiff) = DUNE_SKIFFS.iter().find(|s| p.x >= s.0 && p.x < s.1 && p.y >= skiff_deck(**s, p.x) && p.y < skiff_deck(**s, p.x) + 5.0) {
        let dy = (p.y - skiff_deck(skiff, p.x)).floor() as i32;
        let dx = x - skiff.0 as i32;
        let end = dx == 0 || x == skiff.1 as i32 - 1;
        let c = match dy {
            0 => (214.0, 200.0, 170.0),
            1 => (150.0, 46.0, 40.0),
            4 => (70.0, 60.0, 52.0),
            _ if dx % 9 == 4 && dy == 2 => (230.0, 220.0, 196.0),
            _ => (170.0, 154.0, 128.0),
        };
        return Some(Some(if end { (c.0 * 0.6, c.1 * 0.6, c.2 * 0.6) } else { c }));
    }
    // (The steps under a deck, off it: nothing.)
    if a.platforms.iter().any(|s| s.contains(p)) {
        return Some(None);
    }
    // The slopes: sand from the smooth 30-degree line down (over the steps that are underfoot).
    let inside = x >= WALL as i32 && x < (W - WALL) as i32 && y < FLOOR_Y as i32;
    if let Some(sy) = dune_sand_y(p.x).filter(|_| inside) {
        if p.y < sy {
            return a.hills.iter().any(|s| s.contains(p)).then_some(None);
        }
        let below = p.y - sy;
        let ripple = ((p.x * 0.3 + below * 1.1) + fbm(p.x * 0.08, below * 0.2, 86) * 4.0).sin() * 6.0;
        let n = (fbm(p.x * 0.06, p.y * 0.1, 87) - 0.5) * 14.0;
        let deep = (below / 40.0).min(1.0) * 26.0;
        let c = (210.0 + ripple + n - deep, 172.0 + ripple * 0.8 + n - deep, 112.0 + ripple * 0.5 + n - deep * 0.8);
        return Some(Some(if below < 1.0 { (c.0 + 24.0, c.1 + 22.0, c.2 + 18.0) } else { c }));
    }
    if y >= FLOOR_Y as i32 {
        let (t0, t1) = THROAT;
        let mx = W / 2.0;
        let fx = p.x;
        if fx > t0 && fx < t1 {
            // Down its throat.
            let k = 1.0 - ((fx - mx).abs() / (t1 - mx)).powi(2);
            let depth = (y as f32 - FLOOR_Y) / (H - FLOOR_Y);
            return Some(Some(blend((96.0, 30.0, 32.0), (20.0, 4.0, 6.0), k * 0.7 + depth * 0.3)));
        }
        if (fx - mx).abs() < BODY_HALF_W {
            let n = (fbm(fx * 0.3, y as f32 * 0.3, 81) - 0.5) * 16.0;
            return Some(Some((176.0 + n, 112.0 + n, 98.0 + n)));
        }
        let ripple = ((fx * 0.35 + y as f32 * 1.3) + fbm(fx * 0.1, y as f32 * 0.2, 82) * 4.0).sin() * 6.0;
        let n = (fbm(fx * 0.06, y as f32 * 0.2, 83) - 0.5) * 14.0;
        let top = y == FLOOR_Y as i32;
        let c = (204.0 + ripple + n, 166.0 + ripple * 0.8 + n, 108.0 + ripple * 0.5 + n);
        return Some(Some(if top { (c.0 + 22.0, c.1 + 20.0, c.2 + 16.0) } else { c }));
    }
    if y < WALL as i32 || x < WALL as i32 || x >= (W - WALL) as i32 {
        // Sandstone, in strata.
        let band = (y as f32 * 0.5 + fbm(x as f32 * 0.05, y as f32 * 0.05, 84) * 4.0).sin();
        let n = (fbm(x as f32 * 0.15, y as f32 * 0.15, 85) - 0.5) * 18.0;
        let edge = x == WALL as i32 - 1 || x == (W - WALL) as i32 || y == WALL as i32 - 1;
        let c = (170.0 + band * 14.0 + n, 120.0 + band * 10.0 + n, 76.0 + band * 6.0 + n);
        return Some(Some(if edge { (c.0 * 0.6, c.1 * 0.6, c.2 * 0.6) } else { c }));
    }
    None
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn hurt(c: Color, flash: bool) -> Color {
    if flash { mix(c, Color::new(1.0, 0.92, 0.88, c.a), 0.45) } else { c }
}

/// The Sandmaw: its mouth in the floor, the lips raised round it, a ring of teeth, its beak and
/// tentacles at the rim. Before it bursts up, a trembling mound of sand covers it.
pub fn draw_maw(f: &Fighter, m: &Match, time: f32, flash: bool) {
    let Some(b) = &f.boss else { return };
    let (cx, fy) = (f.pos.x, FLOOR_Y);
    let (t0, t1) = THROAT;
    let throat_w = (t1 - t0) / 2.0;
    if m.tick <= burst_tick(m.config.countdown) {
        // Sand over it, trembling as it stirs.
        let shake = ((m.tick as f32 * 2.3).sin() * (m.tick as f32 / burst_tick(m.config.countdown).max(1) as f32)).round();
        fill_ellipse(cx + shake, fy, BODY_HALF_W, 5.0, SAND);
        fill_ellipse(cx + shake, fy + 2.0, BODY_HALF_W - 4.0, 4.0, Color::new(0.76, 0.62, 0.40, 1.0));
        return;
    }
    let dead = !f.alive();
    let (windup, total) = b.mv.timing();
    let t = b.t as f32;
    // How wide open it is (0 shut .. 1 gaping), and how far its beak is up.
    let (open, beak) = if dead {
        (0.15, 0.0)
    } else {
        match b.mv {
            BossMove::Chomp if b.t < windup => (0.4 + 0.6 * ease(t / windup as f32), 0.0),
            BossMove::Chomp => (ease((t - windup as f32) / 16.0) * 0.4, 0.0),
            BossMove::Inhale => (0.4 + 0.6 * ease(t / 10.0).min(ease((total as f32 - t) / 10.0)), 0.0),
            BossMove::Acid => (0.6, ease(t / windup as f32).min(ease((total as f32 - t) / 8.0))),
            _ => (0.4 + 0.06 * (time * 2.0).sin(), 0.12 + 0.06 * (time * 3.0).sin()),
        }
    };
    let s = |c: Color| hurt(if dead { mix(c, Color::new(0.4, 0.38, 0.36, 1.0), 0.5) } else { c }, flash);
    // The throat, a dark hole, red-hot when it's enraged.
    let tall = 3.0 + 7.0 * open;
    fill_ellipse(cx, fy + 1.0, throat_w + 2.0, tall + 1.0, s(FLESH_DARK));
    fill_ellipse(cx, fy + 1.0, throat_w, tall, THROAT_C);
    if b.enraged && !dead {
        let pulse = 0.5 + 0.5 * (time * 6.0).sin();
        fill_ellipse(cx, fy + 2.0, throat_w * 0.6, tall * 0.6, Color::new(RAGE.r, RAGE.g, RAGE.b, 0.25 + 0.2 * pulse));
    }
    // The beak, up out of its throat.
    if beak > 0.01 {
        let h = 4.0 + 22.0 * beak;
        for side in [-1.0f32, 1.0] {
            let base = vec2(cx + side * 5.0, fy + 1.0);
            let tip = vec2(cx + side * 1.0, fy - h);
            draw_triangle(base, vec2(cx, fy + 1.0), tip, s(if side < 0.0 { BEAK } else { mix(BEAK, BLACK, 0.3) }));
        }
    }
    // The lips: a raised ring of flesh round it, higher as it opens.
    for k in 0..((BODY_HALF_W - throat_w) as i32) {
        for side in [-1.0f32, 1.0] {
            let x = cx + side * (throat_w + k as f32);
            let u = k as f32 / (BODY_HALF_W - throat_w);
            let h = ((1.0 - (u * 2.0 - 1.0).abs()) * (5.0 + 6.0 * open)).round();
            let c = if u < 0.35 { FLESH_LIT } else if u > 0.75 { FLESH_DARK } else { FLESH };
            draw_rectangle(x.round(), fy - h, 1.0, h + 2.0, s(c));
        }
    }
    // The ring of teeth round the rim, pointing in (and splaying out as it opens).
    let n = 18;
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        let (ca, sa) = (a.cos(), a.sin());
        let rim = vec2(cx + ca * (throat_w + 1.0), fy + 1.0 + sa * (tall + 1.0));
        let len = 5.0 + 3.0 * open;
        let inward = vec2(-ca, -sa * 0.5 - 0.6 * (1.0 - open)).normalize_or(vec2(0.0, -1.0));
        let tip = rim + inward * len;
        let side = vec2(-inward.y, inward.x) * 1.8;
        let back = sa < 0.0;
        draw_triangle(rim - side, rim + side, tip, s(if back { TOOTH_DARK } else { TOOTH }));
    }
    // Tentacles at the rim, swaying (limp once it's down).
    for k in 0..4 {
        let side = if k < 2 { -1.0 } else { 1.0 };
        let x = cx + side * (BODY_HALF_W - 4.0 - (k % 2) as f32 * 7.0);
        let sway = if dead { 0.0 } else { (time * 2.5 + k as f32 * 1.3).sin() };
        let len = if dead { 4.0 } else { 9.0 + 3.0 * (time * 1.7 + k as f32).sin() };
        let mut p = vec2(x, fy - 2.0);
        for seg in 0..5 {
            let u = seg as f32 / 5.0;
            let q = p + vec2(side * (1.2 + sway * 1.5 * u), -len / 5.0 * (1.0 - u * 0.5));
            line(p, q, 3.0 - u * 2.0, s(if seg % 2 == 0 { TENTACLE } else { TENTACLE_DARK }));
            p = q;
        }
    }
    // Winding up a chomp: the sand round it glows where the bite will reach.
    if b.mv == BossMove::Chomp && b.t < windup && !dead {
        let a = 0.3 + 0.4 * (t / windup as f32) * (0.5 + 0.5 * (time * 18.0).sin());
        ellipse_outline(cx, fy - 2.0, BODY_HALF_W + 10.0, 5.0, 1.0, Color::new(RAGE.r, RAGE.g, RAGE.b, a));
    }
}

/// A tentacle: first the sand bubbling where it'll come up, then it bursting up, sinking back.
pub fn draw_tentacle(t: &Tentacle, time: f32) {
    let (x, fy) = (t.pos.x, t.pos.y);
    if t.t < T_WARN + 4 {
        // The warning: a big churning patch of sand with a pulsing red ring round it (quicker as it
        // gets close), a faint red column as high as it'll reach, and a blinking "!" over that.
        let k = (t.t as f32 / T_WARN as f32).min(1.0);
        let pulse = 0.5 + 0.5 * (time * (10.0 + 18.0 * k)).sin();
        let w = 10.0 + 12.0 * k;
        draw_rectangle(x - T_W / 2.0 - 2.0, fy - T_REACH, T_W + 4.0, T_REACH, Color::new(RAGE.r, RAGE.g, RAGE.b, (0.06 + 0.12 * pulse) * k));
        fill_ellipse(x, fy - 0.5, w, 2.5 + 1.5 * k, Color::new(0.50, 0.36, 0.20, 0.75));
        fill_ellipse(x, fy - 0.5, w * 0.6, 1.5 + k, Color::new(0.30, 0.18, 0.10, 0.8));
        for ring in [0.0, 4.0] {
            ellipse_outline(x, fy - 0.5, w + 2.0 + ring + pulse * 2.0, 3.5 + k + ring * 0.4, 1.0, Color::new(RAGE.r, RAGE.g, RAGE.b, (0.55 + 0.4 * pulse) * if ring > 0.0 { 0.5 } else { 1.0 }));
        }
        for i in 0..12 {
            let u = ((time * 3.0 + hash(i, t.pos.x as i32, 92)) % 1.0 + 1.0) % 1.0;
            let p = vec2(x + (hash(i, 1, 92) - 0.5) * 2.0 * w, fy - 1.0 - u * (6.0 + 8.0 * k));
            draw_rectangle(p.x.round(), p.y.round(), 2.0, 2.0, Color::new(0.88, 0.76, 0.52, 1.0 - u));
        }
        if pulse > 0.4 {
            let top = fy - T_REACH - 10.0;
            draw_rectangle(x - 1.5, top, 3.0, 6.0, Color::new(0.10, 0.02, 0.02, 0.9));
            draw_rectangle(x - 1.0, top + 0.5, 2.0, 5.0, RAGE);
            draw_rectangle(x - 1.0, top + 7.0, 2.0, 2.0, RAGE);
        }
    }
    let reach = t.reach();
    if reach < 1.0 {
        return;
    }
    // Up out of the sand, swaying, thinning to a curled tip; suckers down its side.
    let segs = (reach / 4.0).ceil() as i32;
    let mut p = vec2(x, fy);
    for s in 0..segs {
        let u = s as f32 / segs.max(1) as f32;
        let wob = (time * 9.0 + u * 5.0).sin() * 2.5 * u;
        let q = vec2(x + wob, fy - reach * (s + 1) as f32 / segs as f32);
        let w = 8.0 - 5.0 * u;
        line(p, q, w + 2.0, TENTACLE_DARK);
        line(p, q, w, if s % 2 == 0 { TENTACLE } else { mix(TENTACLE, TENTACLE_DARK, 0.3) });
        if s % 2 == 1 {
            draw_rectangle((q.x - w * 0.3).round(), q.y.round(), 1.0, 1.0, Color::new(0.9, 0.7, 0.62, 1.0));
        }
        p = q;
    }
    line(p, p + vec2(3.0, -2.0), 2.0, TENTACLE);
    fill_ellipse(x, fy, 7.0, 2.0, SAND);

}

/// A glob of acid, dripping.
pub fn draw_glob(s: &BossShot, time: f32) {
    let dir = s.vel.normalize_or(vec2(1.0, 0.0));
    draw_circle(s.pos.x, s.pos.y, GLOB_R + 2.5, Color::new(ACID.r, ACID.g, ACID.b, 0.18));
    for k in 1..4 {
        let p = s.pos - dir * k as f32 * 2.5;
        draw_rectangle(p.x.round(), p.y.round(), 1.0, 1.0, Color::new(ACID.r, ACID.g, ACID.b, 0.7 - k as f32 * 0.18));
    }
    let wob = (time * 20.0).sin() * 0.4;
    fill_ellipse(s.pos.x, s.pos.y, GLOB_R + wob, GLOB_R - wob, ACID_DARK);
    fill_ellipse(s.pos.x - 0.5, s.pos.y - 0.5, GLOB_R - 1.0, GLOB_R - 1.2, ACID);
}

/// The acid's splash (and a tentacle's spray of sand).
pub const SPLAT: Color = ACID;

/// Its inhale: a sandstorm sucked in from both walls toward its mouth, over everyone, hazing the
/// view (thickening as it starts, thinning as it stops).
pub fn draw_sandstorm(f: &Fighter, time: f32) {
    let Some(b) = &f.boss else { return };
    if b.mv != BossMove::Inhale || !f.alive() {
        return;
    }
    let (windup, total) = b.mv.timing();
    let t = b.t as f32;
    let k = ease(t / windup.max(1) as f32).min(ease((total as f32 - t) / 20.0));
    if k <= 0.0 {
        return;
    }
    let mouth = vec2(f.pos.x, FLOOR_Y - 8.0);
    draw_rectangle(0.0, 0.0, W, H, Color::new(0.86, 0.70, 0.46, 0.24 * k));
    for i in 0..320 {
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        let speed = 0.45 + hash(i, 2, 93) * 0.7;
        let u = ((time * speed + hash(i, 3, 93)) % 1.0 + 1.0) % 1.0;
        let from = vec2(if side < 0.0 { WALL } else { W - WALL }, 12.0 + hash(i, 0, 93) * (H - 30.0));
        // In from the wall, curving down into its mouth at the end.
        let p = vec2(from.x + (mouth.x - from.x) * u, from.y + (mouth.y - from.y) * u * u);
        let dir = (mouth - p).normalize_or(vec2(-side, 0.0));
        let len = 3.0 + hash(i, 4, 93) * 7.0 + 6.0 * u;
        let a = (0.35 + 0.4 * hash(i, 5, 93)) * k * (1.0 - u * 0.4);
        let c = if i % 5 == 0 { Color::new(0.70, 0.54, 0.32, a) } else { Color::new(0.94, 0.82, 0.58, a) };
        if i % 7 == 0 {
            draw_rectangle(p.x.round(), p.y.round(), 2.0, 2.0, c);
        } else {
            line(p, p - dir * len, 1.0, c);
        }
    }
    // A swirl of it round its mouth.
    for i in 0..40 {
        let a = time * 6.0 + i as f32 * 0.157;
        let r = 14.0 + 30.0 * hash(i, 6, 93);
        let p = mouth + vec2(a.cos() * r * 1.6, a.sin() * r * 0.35 - 6.0);
        draw_rectangle(p.x.round(), p.y.round(), 1.0, 1.0, Color::new(0.94, 0.82, 0.58, 0.7 * k));
    }
}
