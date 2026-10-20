//! The Outbreak map's backdrop: a graveyard under a full moon, a chapel and its bell tower behind,
//! rows of headstones, dead trees, an iron fence, and a green mist low along the ground.

use crate::{dither, fbm, hash};
use go_box_sim::sim::{FLOOR_Y, H, W};
use macroquad::prelude::*;

type Rgb = (f32, f32, f32);

const MOON: (f32, f32, f32) = (330.0, 58.0, 20.0);

fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t)
}

/// Whether a headstone (or a cross) stands at this pixel, in one of the rows (`row` 0 far .. 2 near).
fn headstone(x: f32, y: f32, row: u32) -> bool {
    let (base, spacing, h) = [(206.0, 26.0, 9.0), (226.0, 34.0, 13.0), (250.0, 46.0, 18.0)][row as usize];
    let cell = ((x + row as f32 * 13.0) / spacing).floor();
    let cx = cell * spacing + spacing * (0.3 + 0.4 * hash(cell as i32, row as i32, 101)) - row as f32 * 13.0;
    let w = h * (0.45 + 0.2 * hash(cell as i32, 1, 101));
    let tall = h * (0.8 + 0.4 * hash(cell as i32, 2, 101));
    let top = base - tall;
    let dx = x - cx;
    if hash(cell as i32, 3, 101) < 0.3 {
        // A cross.
        (dx.abs() < 1.5 && y > top && y < base) || ((y - (top + tall * 0.3)).abs() < 1.5 && dx.abs() < w * 0.5)
    } else {
        // A rounded stone.
        let r = w * 0.5;
        dx.abs() < r && y < base && (y > top + r || (dx * dx + (y - top - r).powi(2)) < r * r)
    }
}

/// The night behind the fight.
pub fn make_backdrop() -> Image {
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Sky: deep navy up high, a sickly violet glow low down.
            let mut c = blend((10.0, 12.0, 30.0), (48.0, 30.0, 58.0), fy / 200.0);
            if hash(x, y, 102) > 0.996 && fy < 150.0 {
                c = (200.0, 200.0, 220.0);
            }
            // The moon, its halo, its seas.
            let d = vec2(fx - MOON.0, fy - MOON.1).length();
            c = blend(c, (120.0, 130.0, 150.0), (1.0 - d / (MOON.2 * 4.0)).max(0.0).powi(2) * 0.6);
            if d < MOON.2 {
                let sea = fbm(fx * 0.12, fy * 0.12, 103) > 0.55;
                c = if sea { (178.0, 184.0, 176.0) } else { (226.0, 230.0, 214.0) };
            }
            // Hills far off.
            let hill = 168.0 - 14.0 * (fx * 0.013).sin() - (fbm(fx * 0.02, 0.0, 104) - 0.5) * 18.0;
            if fy > hill {
                c = (22.0, 22.0, 38.0);
            }
            // The chapel and its bell tower, in silhouette.
            let chapel = (fx > 196.0 && fx < 284.0 && fy > 150.0) || (fx > 222.0 && fx < 258.0 && fy > 108.0) || ((fx - 240.0).abs() < 18.0 - (108.0 - fy) * 0.8 && fy > 86.0 && fy <= 108.0) || ((fx > 196.0 && fx < 284.0) && fy > 150.0 - (44.0 - (fx - 240.0).abs()) * 0.5);
            if chapel {
                c = (14.0, 14.0, 24.0);
                // A lit window or two.
                let win = ((fx - 240.0).abs() < 4.0 && (fy - 124.0).abs() < 6.0) || (((fx - 212.0).abs() < 3.0 || (fx - 268.0).abs() < 3.0) && (fy - 172.0).abs() < 6.0);
                if win {
                    c = (150.0, 120.0, 60.0);
                }
            }
            // Dead trees, each a trunk forking into bare branches.
            for &(tx, ty, s) in &[(54.0f32, 214.0f32, 1.0f32), (420.0, 210.0, 1.2), (150.0, 220.0, 0.7)] {
                let u = (ty - fy) / (60.0 * s);
                if u > 0.0 && u < 1.0 {
                    let w = (3.0 - 2.5 * u) * s;
                    let sway = (u * 4.0 + tx).sin() * 3.0 * u;
                    let branch = |k: f32| (fx - tx - sway - k * u * u * 24.0 * s).abs() < (1.4 - u) * s + 0.4 && u > 0.35;
                    if (fx - tx - sway).abs() < w || branch(1.0) || branch(-1.0) || branch(0.6) && u > 0.6 {
                        c = (16.0, 14.0, 20.0);
                    }
                }
            }
            // Rows of headstones, far (fainter) to near.
            for row in 0..3 {
                if headstone(fx, fy, row) {
                    let k = [0.55, 0.75, 1.0][row as usize];
                    let lit = (fbm(fx * 0.3, fy * 0.3, 105 + row) - 0.5) * 14.0;
                    c = (48.0 * k + lit, 50.0 * k + lit, 62.0 * k + lit);
                }
            }
            // An iron fence along the back of the ground.
            if (fy - 214.0).abs() < 0.6 || (fy - 222.0).abs() < 0.6 || (x % 9 == 0 && fy > 206.0 && fy < 232.0) {
                if !(fx > 196.0 && fx < 284.0) {
                    c = (24.0, 24.0, 30.0);
                }
            }
            // Mist low along the ground, green-tinged, drifting in bands.
            let mist = ((fy - 200.0) / (FLOOR_Y - 200.0)).clamp(0.0, 1.0) * (0.35 + 0.4 * fbm(fx * 0.02, fy * 0.08, 106));
            c = blend(c, (90.0, 120.0, 96.0), mist);
            img.set_pixel(x as u32, y as u32, dither(x as u32, y as u32, c.0, c.1, c.2));
        }
    }
    img
}
