//! The Architect's room: a box made of code. A dark backdrop with a faint grid and columns of
//! glyphs, ledges like lit blocks of code, a floor, ceiling and walls of circuitry, and code
//! raining down it (up it, when he's flipped the room).

use crate::{dither, hash};
use go_box_sim::sim::{Arena, H, W, WALL, FLOOR_Y};
use macroquad::prelude::*;

type Rgb = (f32, f32, f32);

/// Whether a code glyph has a pixel lit here: glyphs are 3x5 in 4x6 cells, each a random pattern.
fn glyph(x: i32, y: i32, seed: u32) -> bool {
    let (cx, cy) = (x.div_euclid(4), y.div_euclid(6));
    let (gx, gy) = (x.rem_euclid(4), y.rem_euclid(6));
    if gx == 3 || gy == 5 {
        return false;
    }
    hash(cx * 7 + gx * 13, cy * 5 + gy * 17, seed) > 0.55
}

/// The room behind everything: black shading to deep green, a faint grid, dim columns of code.
pub fn make_backdrop() -> Image {
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let fy = y as f32 / H;
            let mut c: Rgb = (4.0, 10.0 + 14.0 * (1.0 - (fy - 0.5).abs() * 2.0), 8.0);
            if x % 24 == 0 || y % 24 == 0 {
                c = (c.0 + 6.0, c.1 + 18.0, c.2 + 8.0);
            }
            // Columns of code, some brighter than others.
            let col = x.div_euclid(4);
            let bright = hash(col, 0, 111);
            if bright > 0.45 && glyph(x, y, 112) {
                let k = (bright - 0.45) / 0.55 * (0.4 + 0.6 * hash(col, y.div_euclid(30), 113));
                c = (c.0 + 20.0 * k, c.1 + 90.0 * k, c.2 + 36.0 * k);
            }
            img.set_pixel(x as u32, y as u32, dither(x as u32, y as u32, c.0, c.1, c.2));
        }
    }
    img
}

/// The room's terrain: ledges as lit blocks of code, everything solid as dark circuitry.
pub(crate) fn terrain_px(a: &Arena, x: i32, y: i32) -> Option<Option<Rgb>> {
    let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
    if let Some(s) = a.platforms.iter().find(|s| s.contains(p)) {
        let (dx, dy) = (x - s.x as i32, y - s.y as i32);
        let end = dx == 0 || dx == s.w as i32 - 1;
        let c = match dy {
            0 => (120.0, 255.0, 150.0),
            4 => (20.0, 110.0, 50.0),
            _ if glyph(x, y + 1, 114) => (60.0, 200.0, 100.0),
            _ => (6.0, 36.0, 18.0),
        };
        return Some(Some(if end { (90.0, 230.0, 120.0) } else { c }));
    }
    let solid = y < WALL as i32 || y >= FLOOR_Y as i32 || x < WALL as i32 || x >= (W - WALL) as i32;
    if solid {
        // Circuit board: traces running along it, pads where they turn, a lit edge facing in.
        let edge = y == WALL as i32 - 1 || y == FLOOR_Y as i32 || x == WALL as i32 - 1 || x == (W - WALL) as i32;
        let trace = (y % 5 == 2 && hash(x / 9, y, 115) > 0.3) || (x % 7 == 3 && hash(x, y / 6, 116) > 0.6);
        let c = if edge { (70.0, 230.0, 110.0) } else if trace { (20.0, 90.0, 44.0) } else { (6.0, 18.0, 12.0) };
        return Some(Some(c));
    }
    None
}

/// Code raining down the room (up it, when it's flipped), over the backdrop.
pub fn draw_code_rain(time: f32, flipped: bool) {
    for k in 0..48 {
        let x = (hash(k, 0, 117) * (W - 2.0 * WALL) + WALL).floor();
        let speed = 40.0 + hash(k, 1, 117) * 70.0;
        let len = 6 + (hash(k, 2, 117) * 14.0) as i32;
        let head = ((time * speed + hash(k, 3, 117) * H) % (H + len as f32 * 6.0)) - len as f32 * 3.0;
        for g in 0..len {
            let y = if flipped { H - head + g as f32 * 6.0 } else { head - g as f32 * 6.0 };
            if !(0.0..H).contains(&y) {
                continue;
            }
            let fade = 1.0 - g as f32 / len as f32;
            let c = if g == 0 { Color::new(0.75, 1.0, 0.8, 0.8) } else { Color::new(0.2, 0.9, 0.4, 0.35 * fade) };
            for (bx, by) in [(0, 0), (2, 0), (1, 1), (0, 2), (2, 2), (1, 3)] {
                if hash(k * 31 + g, bx * 4 + by, 118 + ((time * 6.0) as u32 % 3)) > 0.4 {
                    draw_rectangle(x + bx as f32, y.round() + by as f32, 1.0, 1.0, c);
                }
            }
        }
    }
}

/// The room's ledges, drawn live (the Architect reshapes them): lit blocks of code; loose ones
/// edged in amber with warning stripes (they'll drop when the room turns over); live cables
/// crackling along them. Mid-rebuild they glitch away and back.
pub fn draw_room(room: &go_box_sim::architect::Room, m: &go_box_sim::sim::Match, time: f32) {
    use go_box_sim::architect::ShiftKind;
    let glitch = match m.shift_anim() {
        Some((ShiftKind::Rebuild, k)) if k < 1.0 => k,
        _ => 1.0,
    };
    let flick = (time * 30.0) as i32;
    for (n, l) in room.ledges.iter().enumerate() {
        let r = l.rect;
        // (Glitching: jittering about, and blinking out more and more.)
        if glitch < 1.0 && hash(n as i32, flick, 119) > glitch {
            continue;
        }
        let jx = if glitch < 1.0 { ((hash(n as i32, flick, 120) - 0.5) * 8.0 * (1.0 - glitch)).round() } else { 0.0 };
        let (x, y, w, h) = (r.x.round() + jx, r.y.round(), r.w.round(), r.h.round());
        let edge = if l.loose { Color::new(1.0, 0.72, 0.25, 1.0) } else { Color::new(0.47, 1.0, 0.59, 1.0) };
        draw_rectangle(x, y, w, h, Color::new(0.02, 0.14, 0.07, 1.0));
        draw_rectangle(x, y, w, 1.0, edge);
        draw_rectangle(x, y + h - 1.0, w, 1.0, Color::new(edge.r * 0.4, edge.g * 0.4, edge.b * 0.4, 1.0));
        draw_rectangle(x, y, 1.0, h, edge);
        draw_rectangle(x + w - 1.0, y, 1.0, h, edge);
        for gx in (2..(w as i32 - 2)).step_by(2) {
            for gy in 1..(h as i32 - 1) {
                let lit = if l.loose { (gx / 3 + gy) % 4 == 0 } else { glyph(x as i32 + gx, gy + n as i32 * 3, 121) };
                if lit {
                    let c = if l.loose { Color::new(0.85, 0.55, 0.15, 0.9) } else { Color::new(0.24, 0.78, 0.39, 1.0) };
                    draw_rectangle(x + gx as f32, y + gy as f32, 1.0, 1.0, c);
                }
            }
        }
        // Falling: streaks trailing it.
        if l.vy > 0.0 {
            for k in 0..4 {
                let sx = x + 4.0 + k as f32 * (w - 8.0) / 3.0;
                draw_line(sx, y - 2.0, sx, y - 2.0 - (l.vy * 0.04).min(10.0), 1.0, Color::new(edge.r, edge.g, edge.b, 0.4));
            }
        }
        // A live cable: a tangle of wires along it, arcs of current jumping about it.
        if let Some(c) = l.cable_area() {
            let (cx, cy, cw, ch) = (c.x.round() + jx, c.y.round(), c.w.round(), c.h);
            for wire in 0..4 {
                let mut p = vec2(cx, cy + ch * hash(n as i32, wire, 122));
                let steps = (cw / 3.0).max(2.0) as i32;
                for s in 1..=steps {
                    let q = vec2(cx + cw * s as f32 / steps as f32, cy + ch * hash(n as i32 * 31 + wire, s, 123));
                    draw_line(p.x, p.y, q.x, q.y, 1.0, if wire % 2 == 0 { Color::new(0.20, 0.32, 0.42, 1.0) } else { Color::new(0.08, 0.10, 0.12, 1.0) });
                    p = q;
                }
            }
            draw_rectangle(cx - 2.0, cy - 2.0, cw + 4.0, ch + 4.0, Color::new(0.4, 0.85, 1.0, 0.10 + 0.08 * ((time * 25.0).sin() * 0.5 + 0.5)));
            for arc in 0..5 {
                let seed = flick / 2 + arc;
                let mut p = vec2(cx + cw * hash(seed, n as i32, 124), cy + ch * hash(seed, 1, 124));
                let bright = Color::new(0.6, 0.95, 1.0, 0.95);
                for s in 0..4 {
                    let q = p + vec2((hash(seed, s * 2, 125) - 0.5) * 7.0, (hash(seed, s * 2 + 1, 125) - 0.5) * ch);
                    let q = vec2(q.x.clamp(cx, cx + cw), q.y.clamp(cy - 1.0, cy + ch + 1.0));
                    draw_line(p.x, p.y, q.x, q.y, if arc == 0 { 2.0 } else { 1.0 }, bright);
                    p = q;
                }
            }
            // Sparks spitting off it.
            for s in 0..4 {
                let u = ((time * 3.0 + hash(n as i32, s, 126)) % 1.0 + 1.0) % 1.0;
                let p = vec2(cx + cw * hash(n as i32 + (time * 3.0) as i32, s, 127), cy + ch * 0.5 - u * 10.0);
                draw_rectangle(p.x.round(), p.y.round(), 1.0, 1.0, Color::new(0.85, 1.0, 1.0, 1.0 - u));
            }
        }
    }
}
