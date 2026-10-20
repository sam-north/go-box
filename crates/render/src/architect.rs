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
