//! The Cafe map's art: the coffee shop and its storefront, the courtyard, their ledges and walls, and the train past the window.

use crate::{dither, fbm, hash};
use go_box_sim::arena::{CAFE_BAR, CAFE_DIVIDER, CAFE_HANGING, CAFE_PLANT_SHELF, CAFE_SHELVES, CAFE_TABLES, COURTYARD_AWNING, COURTYARD_BALCONIES, COURTYARD_TABLES};
use go_box_sim::sim::{Aabb, Arena, FLOOR_Y, H, W, WALL};
use macroquad::prelude::*;

type Rgb = (f32, f32, f32);

/// The storefront window (x0, y0, x1, y1), the bars across it, and the door with its pane of glass.
const WINDOW: (i32, i32, i32, i32) = (104, 58, 232, 204);
const MULLIONS: [i32; 2] = [146, 189];
const TRANSOM: i32 = 80;
const DOOR: (i32, i32, i32, i32) = (240, 92, 284, 254);
const DOOR_GLASS: (i32, i32, i32, i32) = (244, 98, 280, 236);
const TRAIN_TOP: f32 = 121.0;
const RAIL: i32 = 134;
const LOT: i32 = 138;
const CURB: i32 = 182;
/// Where the courtyard's two buildings meet, and their rooflines.
const SEAM: i32 = 346;
const LEFT_ROOF: i32 = 46;
const RIGHT_ROOF: i32 = 34;

#[derive(Clone, Copy)]
enum Zone {
    Inside,
    Glass,
    Courtyard,
}

fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t)
}

fn scale(c: Rgb, k: f32) -> Rgb {
    (c.0 * k, c.1 * k, c.2 * k)
}

fn lift(c: Rgb, d: f32) -> Rgb {
    (c.0 + d, c.1 + d, c.2 + d)
}

/// Hazed and dimmed like every map's backdrop, so the fight reads in front of it.
fn grade(c: Rgb, zone: Zone) -> Rgb {
    let (fog, to, dim) = match zone {
        Zone::Inside => (0.46, (40.0, 34.0, 36.0), 0.7),
        Zone::Glass => (0.5, (46.0, 54.0, 78.0), 0.68),
        Zone::Courtyard => (0.46, (38.0, 38.0, 54.0), 0.72),
    };
    let grey = c.0 * 0.3 + c.1 * 0.59 + c.2 * 0.11;
    scale(blend(blend(c, (grey, grey, grey), 0.3), to, fog), dim)
}

fn zone(x: i32, y: i32) -> Zone {
    if x >= CAFE_DIVIDER.x as i32 {
        Zone::Courtyard
    } else if in_glass(x, y) {
        Zone::Glass
    } else {
        Zone::Inside
    }
}

fn in_glass(x: i32, y: i32) -> bool {
    let (x0, y0, x1, y1) = WINDOW;
    let pane = x >= x0 + 2 && x < x1 - 2 && y >= y0 + 2 && y < y1 - 2 && !MULLIONS.iter().any(|&m| x >= m && x < m + 2) && !(TRANSOM..TRANSOM + 2).contains(&y);
    let (dx0, dy0, dx1, dy1) = DOOR_GLASS;
    pane || (x >= dx0 && x < dx1 && y >= dy0 && y < dy1)
}

/// The backdrop as it's painted, and which of its pixels are in front of the train (everything but the far view through the glass).
struct Canvas {
    px: Vec<Rgb>,
    near: Vec<bool>,
}

impl Canvas {
    fn new() -> Self {
        let n = (W * H) as usize;
        Self { px: vec![(0.0, 0.0, 0.0); n], near: vec![true; n] }
    }

    fn at(x: i32, y: i32) -> Option<usize> {
        ((0..W as i32).contains(&x) && (0..H as i32).contains(&y)).then(|| (y * W as i32 + x) as usize)
    }

    fn put(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = Self::at(x, y) {
            self.px[i] = c;
            self.near[i] = true;
        }
    }

    fn far(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = Self::at(x, y) {
            self.px[i] = c;
            self.near[i] = false;
        }
    }

    fn get(&self, x: i32, y: i32) -> Rgb {
        Self::at(x, y).map_or((0.0, 0.0, 0.0), |i| self.px[i])
    }

    fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, c);
            }
        }
    }

    fn line(&mut self, (x0, y0): (i32, i32), (x1, y1): (i32, i32), c: Rgb) {
        let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.put((x0 as f32 + (x1 - x0) as f32 * t).round() as i32, (y0 as f32 + (y1 - y0) as f32 * t).round() as i32, c);
        }
    }

    /// Light falling off round (cx, cy) out to `r`, leaving what's near as it was.
    fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, k: f32) {
        for y in (cy - r) as i32..=(cy + r) as i32 {
            for x in (cx - r) as i32..=(cx + r) as i32 {
                let Some(i) = Self::at(x, y) else { continue };
                let d = vec2(x as f32 + 0.5, y as f32 + 0.5).distance(vec2(cx, cy));
                if d < r {
                    let f = (1.0 - d / r).powi(2) * k;
                    let p = &mut self.px[i];
                    *p = (p.0 + c.0 * f, p.1 + c.1 * f, p.2 + c.2 * f);
                }
            }
        }
    }
}

/// The backdrop, and which pixels go on the front layer (drawn over the train).
pub(crate) fn make_backdrop() -> (Image, Vec<bool>) {
    let mut cv = Canvas::new();
    outside(&mut cv);
    inside(&mut cv);
    courtyard(&mut cv);
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let (r, g, b) = grade(cv.get(x, y), zone(x, y));
            img.set_pixel(x as u32, y as u32, dither(x as u32, y as u32, r, g, b));
        }
    }
    (img, cv.near)
}

fn dusk(y: f32) -> Rgb {
    let stops = [(0.0, (26.0, 32.0, 74.0)), (60.0, (50.0, 50.0, 104.0)), (98.0, (124.0, 78.0, 112.0)), (124.0, (228.0, 140.0, 96.0))];
    let mut c = stops[0].1;
    for s in stops.windows(2) {
        let ((y0, c0), (y1, c1)) = (s[0], s[1]);
        if y >= y0 {
            c = blend(c0, c1, ((y - y0) / (y1 - y0)).clamp(0.0, 1.0));
        }
    }
    c
}

fn brick(x: i32, y: i32, seed: u32) -> Rgb {
    let row = y.div_euclid(4);
    let off = if row.rem_euclid(2) == 0 { 0 } else { 4 };
    let col = (x + off).div_euclid(8);
    let n = (fbm(x as f32 * 0.07, y as f32 * 0.07, seed) - 0.5) * 24.0;
    if y.rem_euclid(4) == 3 || (x + off).rem_euclid(8) == 7 {
        return (88.0 + n, 78.0 + n, 70.0 + n);
    }
    let t = hash(col, row, seed);
    let mut c = (112.0 + t * 30.0 + n, 60.0 + t * 16.0 + n * 0.6, 46.0 + t * 10.0 + n * 0.5);
    if hash(col, row, seed + 1) > 0.88 {
        c = scale(c, 0.72);
    }
    if y.rem_euclid(4) == 0 {
        c = lift(c, 8.0);
    }
    let wash = fbm(x as f32 * 0.03, y as f32 * 0.05, seed + 2);
    if wash > 0.62 {
        c = blend(c, (190.0, 178.0, 164.0), ((wash - 0.62) * 2.0).min(0.5));
    }
    c
}

fn wood(x: i32, y: i32, base: Rgb, seed: u32) -> Rgb {
    let grain = hash(x / 5 + y * 13, y, seed) * 14.0 - 7.0 + (fbm(x as f32 * 0.2, y as f32 * 1.3, seed) - 0.5) * 16.0;
    lift(base, grain)
}

/// What's seen through the glass: a dusk sky over distant trees, the railway, a parking lot of cars facing the shop, and the patio outside.
fn outside(cv: &mut Canvas) {
    let far_cars: [(i32, Rgb, bool); 7] = [
        (114, (150.0, 40.0, 36.0), false),
        (136, (196.0, 196.0, 192.0), true),
        (160, (40.0, 70.0, 130.0), false),
        (184, (140.0, 144.0, 150.0), false),
        (208, (36.0, 36.0, 40.0), true),
        (254, (40.0, 110.0, 110.0), false),
        (274, (196.0, 160.0, 56.0), false),
    ];
    let near_cars: [(i32, Rgb, bool); 5] = [(122, (140.0, 144.0, 150.0), false), (152, (40.0, 70.0, 130.0), true), (206, (196.0, 196.0, 192.0), false), (252, (150.0, 40.0, 36.0), false), (276, (36.0, 36.0, 40.0), true)];
    let (x0, y0, x1, y1) = (WINDOW.0, WINDOW.1, DOOR.2, DOOR_GLASS.3);
    for y in y0..y1 {
        for x in x0..x1 {
            if !in_glass(x, y) {
                continue;
            }
            let (fx, fy) = (x as f32, y as f32);
            let mut c = dusk(fy);
            if fy < 100.0 && hash(x, y, 501) > 0.996 {
                c = lift(c, 70.0);
            }
            let trees = 108.0 + fbm(fx * 0.08, 0.0, 503) * 12.0;
            let roofs = 100.0 + hash(x / 14, 0, 505) * 14.0 + if hash(x / 14, 1, 505) > 0.7 { 6.0 } else { 0.0 };
            if fy >= trees.min(roofs) {
                c = if fy >= trees { (46.0, 40.0, 64.0) } else { (56.0, 48.0, 74.0) };
            }
            if y > RAIL - 12 && y < RAIL && x.rem_euclid(48) == 30 || y == RAIL - 19 {
                c = (34.0, 32.0, 46.0);
            }
            if y == RAIL {
                c = (156.0, 150.0, 152.0);
            } else if y > RAIL && y < LOT {
                c = lift((70.0, 64.0, 66.0), hash(x, y, 507) * 12.0);
            }
            if y < LOT {
                cv.far(x, y, c);
                continue;
            }
            c = lift((52.0, 54.0, 62.0), hash(x / 2, y, 509) * 8.0);
            if (146..152).contains(&y) && (x - 102).rem_euclid(24) == 0 || (168..178).contains(&y) && (x - 107).rem_euclid(30) == 0 {
                c = (150.0, 150.0, 146.0);
            }
            if y >= CURB {
                c = if y < CURB + 2 { (164.0, 160.0, 150.0) } else if (x - 110).rem_euclid(26) == 0 { (100.0, 98.0, 94.0) } else { lift((128.0, 124.0, 116.0), hash(x, y, 511) * 10.0) };
            }
            for &(cx, body, lit) in &far_cars {
                if let Some(p) = car_px(x - (cx - 7), y - 141, 14, 11, body, lit) {
                    c = p;
                }
            }
            for &(cx, body, lit) in &near_cars {
                if let Some(p) = car_px(x - (cx - 11), y - 160, 22, 18, body, lit) {
                    c = p;
                }
            }
            cv.put(x, y, c);
        }
    }
    for lamp in [126, 218] {
        for y in 92..158 {
            if in_glass(lamp, y) {
                cv.put(lamp, y, (40.0, 42.0, 48.0));
            }
        }
        for x in lamp..lamp + 6 {
            if in_glass(x, 92) {
                cv.put(x, 92, (40.0, 42.0, 48.0));
            }
        }
        for x in lamp + 3..lamp + 7 {
            if in_glass(x, 93) {
                cv.put(x, 93, (255.0, 226.0, 160.0));
            }
        }
        cv.glow(lamp as f32 + 5.0, 94.0, 14.0, (120.0, 90.0, 40.0), 0.8);
    }
    // Patio tables just outside the glass.
    for (tx, top) in [(168, 192), (262, 212)] {
        patio_table(cv, tx, top, &|x, y| in_glass(x, y));
    }
    for (x0, x1) in [(250, 262), (266, 278)] {
        for y in 222..236 {
            for x in x0..x1 {
                if in_glass(x, y) {
                    cv.put(x, y, if y < 228 { (60.0, 104.0, 56.0) } else { (110.0, 76.0, 52.0) });
                }
            }
        }
    }
}

/// A car seen from the front (`lx`, `ly` inside its w x h box): None outside it.
fn car_px(lx: i32, ly: i32, w: i32, h: i32, body: Rgb, lit: bool) -> Option<Rgb> {
    if lx < 0 || lx >= w || ly < 0 || ly >= h {
        return None;
    }
    let cabin = h * 4 / 9;
    if ly < cabin {
        let inset = 2 + (cabin - ly) / 3;
        if lx < inset || lx >= w - inset {
            return None;
        }
        if ly == 0 {
            return Some(body);
        }
        return Some(blend((64.0, 74.0, 116.0), (150.0, 104.0, 118.0), ly as f32 / cabin as f32 * 0.7));
    }
    let (by, bh) = (ly - cabin, h - cabin);
    if by >= bh - 2 {
        return Some(if lx < 3 || lx >= w - 3 { (18.0, 18.0, 20.0) } else { (30.0, 30.0, 34.0) });
    }
    if by == bh - 3 {
        return Some(blend(body, (40.0, 40.0, 44.0), 0.6));
    }
    if by == 1 && (lx <= 2 || lx >= w - 3) && lx != 0 && lx != w - 1 {
        return Some(if lit { (255.0, 240.0, 190.0) } else { (206.0, 200.0, 176.0) });
    }
    if (1..=2).contains(&by) && (lx - w / 2).abs() <= w / 6 {
        return Some((28.0, 28.0, 32.0));
    }
    Some(if by == 0 { lift(body, 18.0) } else { body })
}

/// A patio table under a striped umbrella (its top at `top`, centred on `cx`) with a chair either side, where `ok` allows.
fn patio_table(cv: &mut Canvas, cx: i32, top: i32, ok: &dyn Fn(i32, i32) -> bool) {
    let dot = |cv: &mut Canvas, x: i32, y: i32, c: Rgb| {
        if ok(x, y) {
            cv.put(x, y, c);
        }
    };
    let (iron, seat) = ((30.0, 34.0, 32.0), (44.0, 50.0, 46.0));
    for row in 0..5 {
        for x in cx - 3 - row * 2..=cx + 3 + row * 2 {
            let stripe = ((x - cx + 40) / 3) % 2 == 0;
            dot(cv, x, top - 14 + row, if row == 4 { (150.0, 60.0, 50.0) } else if stripe { (200.0, 70.0, 56.0) } else { (226.0, 214.0, 190.0) });
        }
    }
    for y in top - 9..top {
        dot(cv, cx, y, (90.0, 84.0, 76.0));
    }
    for x in cx - 6..=cx + 6 {
        dot(cv, x, top, (70.0, 76.0, 72.0));
        dot(cv, x, top + 1, iron);
    }
    for y in top + 2..top + 10 {
        dot(cv, cx, y, iron);
    }
    for x in cx - 3..=cx + 3 {
        dot(cv, x, top + 10, iron);
    }
    for side in [-1, 1] {
        let mid = cx + side * 11;
        for x in mid - 2..=mid + 2 {
            dot(cv, x, top + 4, seat);
            dot(cv, x, top + 5, iron);
        }
        for y in top - 2..top + 4 {
            dot(cv, mid + side * 3, y, seat);
            dot(cv, mid + side * 2, y, if y == top - 2 { iron } else { seat });
        }
        for (k, y) in (top + 6..top + 11).enumerate() {
            dot(cv, mid - side * (2 + k as i32 / 3), y, iron);
            dot(cv, mid + side * 3, y, iron);
        }
    }
}

/// The coffee shop's back wall: exposed brick, the ceiling's ducts and pipes, pendant lamps, the bar's machines, shelves of mugs and plants, and the storefront.
fn inside(cv: &mut Canvas) {
    let divider = CAFE_DIVIDER.x as i32;
    for y in WALL as i32..FLOOR_Y as i32 {
        for x in WALL as i32..divider {
            if in_glass(x, y) {
                let streak = (x + y).rem_euclid(70);
                if streak < 4 || (7..9).contains(&streak) {
                    let i = (y * W as i32 + x) as usize;
                    cv.px[i] = lift(cv.px[i], if streak < 4 { 14.0 } else { 7.0 });
                }
                continue;
            }
            cv.put(x, y, if y < 44 { ceiling(x, y) } else { brick(x, y, 401) });
        }
    }
    storefront(cv);
    chalkboard(cv);
    bar_top(cv);
    shelves(cv);
    furniture(cv);
    plants(cv);
    lamps(cv);
}

fn ceiling(x: i32, y: i32) -> Rgb {
    let mut c = if x.rem_euclid(32) < 3 { (42.0, 40.0, 42.0) } else { (30.0, 28.0, 30.0) };
    if y < 12 && x.rem_euclid(48) == 20 {
        c = (72.0, 72.0, 74.0);
    }
    if (12..26).contains(&y) {
        // The big insulated duct: foil wrapped round it, banded, lit from above.
        let t = (y - 12) as f32 / 13.0;
        let light = 0.4 + 0.8 * (1.0 - t).powf(1.4) * (t * std::f32::consts::PI).sin().max(0.25);
        let crinkle = hash(x / 2, y, 403) * 16.0 - 8.0;
        c = (150.0 * light + crinkle, 152.0 * light + crinkle, 158.0 * light + crinkle);
        if (x + 6).rem_euclid(24) < 2 {
            c = scale(c, 0.68);
        }
    }
    match y {
        28 | 29 => c = if x.rem_euclid(16) == 0 { (150.0, 146.0, 138.0) } else { (198.0, 192.0, 180.0) },
        30 => c = (150.0, 146.0, 138.0),
        33 => c = (160.0, 46.0, 40.0),
        34 => c = (112.0, 32.0, 30.0),
        35 if x.rem_euclid(56) == 30 => c = (170.0, 140.0, 70.0),
        37 => c = (180.0, 110.0, 66.0),
        38 => c = (120.0, 72.0, 42.0),
        41 => c = (18.0, 18.0, 20.0),
        42 | 43 => c = (24.0, 20.0, 20.0),
        _ if y > 26 && y < 41 && x.rem_euclid(40) == 12 => c = (64.0, 64.0, 66.0),
        _ => {}
    }
    c
}

fn storefront(cv: &mut Canvas) {
    let (x0, y0, x1, y1) = WINDOW;
    for y in y0..y1 {
        for x in x0..x1 {
            if !in_glass(x, y) {
                let edge = y == y0 || MULLIONS.contains(&x) || y == TRANSOM;
                cv.put(x, y, if edge { (66.0, 66.0, 72.0) } else { (34.0, 34.0, 38.0) });
            }
        }
    }
    for y in y1..y1 + 3 {
        for x in x0 - 2..x1 + 2 {
            cv.put(x, y, if y == y1 { (180.0, 130.0, 86.0) } else { wood(x, y, (136.0, 94.0, 60.0), 413) });
        }
    }
    // The door: a wooden frame round a tall pane of glass, a kick panel, a brass handle.
    let (dx0, dy0, dx1, dy1) = DOOR;
    for y in dy0..dy1 {
        for x in dx0..dx1 {
            if in_glass(x, y) {
                continue;
            }
            let bevel = (x == DOOR_GLASS.0 - 1 || x == DOOR_GLASS.2) && y >= DOOR_GLASS.1 && y < DOOR_GLASS.3 || y == DOOR_GLASS.1 - 1 || y == DOOR_GLASS.3;
            let panel = (x == dx0 + 6 || x == dx1 - 7) && (240..250).contains(&y) || (y == 240 || y == 249) && x > dx0 + 5 && x < dx1 - 6;
            let c = if x == dx0 || x == dx1 - 1 { (70.0, 44.0, 28.0) } else if bevel || panel { (92.0, 60.0, 36.0) } else { wood(x, y, (128.0, 84.0, 50.0), 415) };
            cv.put(x, y, c);
        }
    }
    for y in 154..178 {
        cv.put(DOOR_GLASS.0 + 3, y, (206.0, 170.0, 90.0));
    }
    for y in [154, 177] {
        cv.put(DOOR_GLASS.0 + 2, y, (150.0, 120.0, 60.0));
    }
    // An OPEN sign hung inside the door, and a neon coffee cup in the window.
    cv.fill(254, 110, 270, 118, (26.0, 26.0, 30.0));
    for x in 256..268 {
        if x % 3 != 1 {
            cv.put(x, 113, (255.0, 80.0, 70.0));
            cv.put(x, 115, (255.0, 80.0, 70.0));
        }
    }
    cv.glow(262.0, 114.0, 9.0, (120.0, 30.0, 30.0), 0.6);
    let neon = (255.0, 200.0, 160.0);
    let mut cup = Vec::new();
    for x in 116..129 {
        cup.extend([(x, 96), (x, 107)]);
    }
    for y in 96..107 {
        cup.extend([(116, y), (128, y)]);
    }
    for y in 98..104 {
        cup.push((131, y));
    }
    cup.extend([(129, 98), (130, 98), (129, 103), (130, 103)]);
    for x in 113..132 {
        cup.push((x, 109));
    }
    for (k, sx) in [119, 122, 125].into_iter().enumerate() {
        for y in 86..94 {
            cup.push((sx + if (y + k as i32) % 4 < 2 { 0 } else { 1 }, y));
        }
    }
    for &(x, y) in &cup {
        cv.glow(x as f32 + 0.5, y as f32 + 0.5, 5.0, (110.0, 44.0, 24.0), 0.7);
    }
    for (x, y) in cup {
        cv.put(x, y, neon);
    }
}

fn chalkboard(cv: &mut Canvas) {
    let (x0, y0, x1, y1) = (22, 60, 90, 100);
    for y in y0..y1 {
        for x in x0..x1 {
            let frame = x < x0 + 2 || x >= x1 - 2 || y < y0 + 2 || y >= y1 - 2;
            let c = if frame { wood(x, y, (112.0, 76.0, 46.0), 417) } else { lift((32.0, 42.0, 38.0), hash(x, y, 419) * 6.0) };
            cv.put(x, y, c);
        }
    }
    let chalk = (196.0, 196.0, 186.0);
    for x in 30..64 {
        if x % 4 != 3 {
            cv.put(x, 65, chalk);
            cv.put(x, 66, chalk);
        }
    }
    for r in 0..5 {
        let y = 72 + r * 5;
        for x in 27..72 {
            if hash(x / 3, r, 421) > 0.3 && x % 3 != 0 {
                cv.put(x, y, scale(chalk, 0.8));
            }
        }
        cv.put(80, y, chalk);
        cv.put(82, y, chalk);
    }
}

/// On the bar: an espresso machine with cups warming on top, a grinder, a tip jar and a case of pastries.
fn bar_top(cv: &mut Canvas) {
    let top = CAFE_BAR.y as i32;
    cv.fill(16, top - 15, 41, top - 1, (172.0, 174.0, 180.0));
    cv.fill(16, top - 8, 41, top - 1, (118.0, 120.0, 126.0));
    cv.fill(16, top - 16, 41, top - 15, (52.0, 52.0, 56.0));
    for x in [19, 24, 29] {
        cv.fill(x, top - 19, x + 3, top - 16, (222.0, 218.0, 208.0));
    }
    for g in [22, 32] {
        cv.fill(g, top - 9, g + 3, top - 7, (90.0, 90.0, 96.0));
        cv.fill(g + 1, top - 7, g + 6, top - 6, (28.0, 26.0, 24.0));
    }
    cv.fill(27, top - 13, 30, top - 11, (160.0, 40.0, 36.0));
    cv.line((39, top - 10), (41, top - 4), (200.0, 200.0, 206.0));
    cv.fill(16, top - 2, 41, top - 1, (40.0, 40.0, 44.0));
    cv.fill(44, top - 8, 51, top - 1, (42.0, 42.0, 46.0));
    cv.fill(45, top - 16, 50, top - 8, (128.0, 140.0, 140.0));
    cv.fill(45, top - 12, 50, top - 8, (82.0, 52.0, 32.0));
    cv.fill(56, top - 6, 60, top - 1, (140.0, 160.0, 160.0));
    cv.fill(57, top - 4, 59, top - 2, (90.0, 140.0, 80.0));
    let (px0, px1, py0) = (66, 96, top - 14);
    for y in py0..top - 1 {
        for x in px0..px1 {
            let frame = x == px0 || x == px1 - 1 || y == py0 || y == top - 2;
            cv.put(x, y, if frame { (64.0, 64.0, 68.0) } else if y == py0 + 6 { (150.0, 150.0, 150.0) } else { (88.0, 100.0, 104.0) });
        }
    }
    for x in [69, 76, 83, 90] {
        cv.fill(x, py0 + 3, x + 4, py0 + 6, (192.0, 124.0, 58.0));
        cv.put(x + 1, py0 + 3, (226.0, 170.0, 96.0));
        cv.fill(x, py0 + 9, x + 4, py0 + 10, (110.0, 64.0, 40.0));
        cv.fill(x, py0 + 10, x + 4, py0 + 12, (200.0, 190.0, 160.0));
    }
}

/// What sits on the shelves (they're ledges: the terrain draws them), and the brackets and ropes holding them up.
fn shelves(cv: &mut Canvas) {
    let iron = (40.0, 40.0, 44.0);
    for s in CAFE_SHELVES {
        let (x0, x1, y) = (s.x as i32, (s.x + s.w) as i32, (s.y + s.h) as i32);
        for bx in [x0 + 3, x1 - 4] {
            for d in 0..5 {
                cv.put(bx, y + d, iron);
            }
            cv.line((bx, y + 4), (bx + if bx < (x0 + x1) / 2 { 3 } else { -3 }, y), iron);
        }
    }
    let rope = (176.0, 150.0, 110.0);
    for s in CAFE_HANGING {
        for x in [s.x as i32 + 2, (s.x + s.w) as i32 - 3] {
            for y in 26..s.y as i32 {
                cv.put(x, y, if y % 5 == 0 { scale(rope, 0.8) } else { rope });
            }
        }
    }
    let low = CAFE_SHELVES[0].y as i32;
    cv.fill(18, low - 3, 21, low, (222.0, 216.0, 206.0));
    cv.put(21, low - 2, (222.0, 216.0, 206.0));
    for bx in [24, 30] {
        cv.fill(bx, low - 7, bx + 5, low, (170.0, 136.0, 96.0));
        cv.fill(bx, low - 4, bx + 5, low - 3, (222.0, 210.0, 180.0));
    }
    for jx in [38, 44] {
        cv.fill(jx, low - 6, jx + 4, low - 5, (60.0, 60.0, 64.0));
        cv.fill(jx, low - 5, jx + 4, low, (150.0, 170.0, 170.0));
        cv.fill(jx, low - 3, jx + 4, low, (90.0, 58.0, 36.0));
    }
    potted(cv, 56, low, 0);
    let high = CAFE_SHELVES[1].y as i32;
    for (k, x) in [46, 49, 52].into_iter().enumerate() {
        cv.fill(x, high - 3, x + 3, high, if k == 1 { (120.0, 170.0, 190.0) } else { (222.0, 216.0, 206.0) });
    }
    for x in [47, 50] {
        cv.fill(x, high - 6, x + 3, high - 3, (222.0, 216.0, 206.0));
    }
    potted(cv, 62, high, 3);
    for (k, x) in (74..88).step_by(2).enumerate() {
        let tall = 7 + (hash(k as i32, 0, 423) * 3.0) as i32;
        let spine = [(150.0, 50.0, 46.0), (50.0, 80.0, 130.0), (60.0, 110.0, 70.0), (200.0, 170.0, 90.0)][k % 4];
        cv.fill(x, high - tall, x + 2, high, spine);
    }
    let rack = CAFE_HANGING[0].y as i32;
    for (k, y) in (rack - 4..rack).enumerate() {
        cv.fill(182 + k as i32, y, 194, y + 1, [(150.0, 50.0, 46.0), (220.0, 210.0, 190.0), (50.0, 80.0, 130.0), (200.0, 170.0, 90.0)][k]);
    }
    potted(cv, 220, rack, 3);
    let beam = CAFE_HANGING[1].y as i32;
    potted(cv, 102, beam, 3);
    potted(cv, 154, beam, 3);
    for jx in [124, 132] {
        cv.fill(jx, beam - 5, jx + 4, beam, (150.0, 170.0, 170.0));
        cv.fill(jx, beam - 3, jx + 4, beam, (180.0, 110.0, 60.0));
    }
    let sill = CAFE_PLANT_SHELF.y as i32;
    for (x, kind) in [(154, 1), (168, 0), (184, 2), (200, 3), (212, 4)] {
        potted(cv, x, sill, kind);
    }
}

/// A terracotta pot standing on `floor` with a plant in it: leaves (0), a snake plant (1), a fern (2), trailing pothos (3) or a cactus (4).
fn potted(cv: &mut Canvas, x: i32, floor: i32, kind: u8) {
    let (dark, light) = ((46.0, 86.0, 48.0), (78.0, 124.0, 62.0));
    match kind {
        1 => {
            for (k, h) in [9, 12, 10, 7].into_iter().enumerate() {
                for y in floor - 4 - h..floor - 4 {
                    cv.put(x + k as i32, y, if y % 3 == 0 { (150.0, 150.0, 70.0) } else { dark });
                }
            }
        }
        2 => {
            for k in -4..=4i32 {
                cv.line((x + 2, floor - 4), (x + 2 + k, floor - 9 + k.abs() / 2), if k % 2 == 0 { light } else { dark });
            }
        }
        3 => {
            cv.fill(x, floor - 7, x + 5, floor - 4, dark);
            for (k, len) in [8, 12, 6].into_iter().enumerate() {
                let vx = x + k as i32 * 2;
                for d in 0..len {
                    cv.put(vx + (d / 4) % 2, floor + 1 + d, if d % 3 == 0 { light } else { dark });
                }
            }
        }
        4 => {
            cv.fill(x + 2, floor - 12, x + 4, floor - 4, (60.0, 110.0, 66.0));
            cv.fill(x, floor - 9, x + 1, floor - 6, (60.0, 110.0, 66.0));
            cv.fill(x + 5, floor - 10, x + 6, floor - 7, (60.0, 110.0, 66.0));
        }
        _ => {
            for (dx, dy) in [(0, -6), (2, -8), (4, -6), (1, -5), (3, -5), (2, -6), (-1, -5), (5, -5)] {
                cv.put(x + dx, floor + dy, if dy < -6 { light } else { dark });
                cv.put(x + dx, floor + dy + 1, dark);
            }
        }
    }
    cv.fill(x - 1, floor - 4, x + 6, floor - 3, (196.0, 112.0, 74.0));
    cv.fill(x, floor - 3, x + 5, floor, (170.0, 90.0, 60.0));
}

/// The tables' legs and the chairs round them (behind the tops, which are ledges), and a tall fig in a pot by the bar.
fn furniture(cv: &mut Canvas) {
    let leg = (96.0, 64.0, 40.0);
    let floor = FLOOR_Y as i32;
    for t in CAFE_TABLES {
        let (x0, x1, y) = (t.x as i32, (t.x + t.w) as i32, (t.y + t.h) as i32);
        for lx in [x0 + 3, x1 - 5] {
            cv.fill(lx, y, lx + 2, floor, leg);
        }
        cv.fill(x0 + 5, floor - 10, x1 - 5, floor - 9, leg);
        for (seat, back) in [(x0 - 9, x0 - 9), (x1 + 2, x1 + 8)] {
            cv.fill(seat, floor - 19, seat + 7, floor - 17, (84.0, 56.0, 36.0));
            cv.fill(back, floor - 34, back + 1, floor - 17, (84.0, 56.0, 36.0));
            cv.fill(back - 1, floor - 34, back + 2, floor - 33, (104.0, 70.0, 44.0));
            for lx in [seat, seat + 6] {
                cv.fill(lx, floor - 17, lx + 1, floor, (70.0, 46.0, 30.0));
            }
        }
    }
    let fig = (106, floor);
    cv.fill(fig.0 - 5, fig.1 - 12, fig.0 + 6, fig.1, (214.0, 208.0, 196.0));
    cv.fill(fig.0 - 6, fig.1 - 13, fig.0 + 7, fig.1 - 12, (236.0, 232.0, 222.0));
    cv.line((fig.0, fig.1 - 13), (fig.0 - 1, fig.1 - 40), (84.0, 60.0, 40.0));
    for (k, (lx, ly)) in [(-6, -44), (4, -46), (-8, -34), (6, -36), (-4, -26), (5, -24), (0, -50), (-2, -38)].into_iter().enumerate() {
        let (cx, cy) = (fig.0 + lx, fig.1 + ly);
        for y in cy - 3..=cy + 3 {
            for x in cx - 3..=cx + 3 {
                let d = ((x - cx) as f32 / 3.2).powi(2) + ((y - cy) as f32 / 3.6).powi(2);
                if d <= 1.0 {
                    let c = if x == cx { (96.0, 140.0, 76.0) } else if k % 2 == 0 { (50.0, 94.0, 50.0) } else { (64.0, 110.0, 58.0) };
                    cv.put(x, y, c);
                }
            }
        }
    }
}

/// Plants hung from the ceiling in macrame, trailing vines.
fn plants(cv: &mut Canvas) {
    for (x, pot) in [(270, 52), (112, 44)] {
        for y in 26..pot {
            let d = (y - 26) % 8;
            cv.put(x - 2 + d.min(8 - d) / 2, y, (200.0, 186.0, 160.0));
            cv.put(x + 2 - d.min(8 - d) / 2, y, (200.0, 186.0, 160.0));
        }
        cv.fill(x - 3, pot, x + 4, pot + 6, (214.0, 208.0, 196.0));
        cv.fill(x - 4, pot, x + 5, pot + 1, (236.0, 232.0, 222.0));
        for (k, len) in [14, 20, 10, 17].into_iter().enumerate() {
            let vx = x - 3 + k as i32 * 2;
            for d in 0..len {
                cv.put(vx + (d / 5) % 2, pot + 5 + d, if d % 3 == 0 { (78.0, 124.0, 62.0) } else { (46.0, 86.0, 48.0) });
            }
        }
    }
}

/// Industrial pendant lamps and bare bulbs on cords from the duct, each throwing warm light round it.
fn lamps(cv: &mut Canvas) {
    let cord = (22.0, 22.0, 24.0);
    for (x, y) in [(142, 54), (226, 54), (60, 50)] {
        for cy in 26..y - 3 {
            cv.put(x, cy, cord);
        }
        for (row, half) in [(y - 3, 1), (y - 2, 2), (y - 1, 3)] {
            for dx in -half..=half {
                cv.put(x + dx, row, if dx == -half || row == y - 3 { (70.0, 100.0, 84.0) } else { (40.0, 64.0, 52.0) });
            }
        }
        cv.put(x, y, (255.0, 236.0, 180.0));
        cv.glow(x as f32 + 0.5, y as f32 + 4.0, 34.0, (70.0, 46.0, 22.0), 0.9);
    }
    for (x, y) in [(122, 66), (168, 72), (196, 62), (252, 68)] {
        for cy in 26..y {
            cv.put(x, cy, cord);
        }
        cv.fill(x, y, x + 1, y + 2, (250.0, 210.0, 130.0));
        cv.glow(x as f32 + 0.5, y as f32 + 1.0, 9.0, (110.0, 80.0, 36.0), 0.6);
    }
}

/// The courtyard: two buildings (no windows: doors, an iron stair, balconies), string lights strung over it, planters and bistro chairs.
fn courtyard(cv: &mut Canvas) {
    let (x0, x1) = (CAFE_DIVIDER.x as i32, (W - WALL) as i32);
    for y in WALL as i32..FLOOR_Y as i32 {
        for x in x0..x1 {
            cv.put(x, y, facade(x, y));
        }
    }
    let floor = FLOOR_Y as i32;
    for y in (CAFE_DIVIDER.y + CAFE_DIVIDER.h) as i32..floor {
        for x in x0..x0 + CAFE_DIVIDER.w as i32 {
            cv.put(x, y, if y >= floor - 2 { (70.0, 62.0, 56.0) } else { (24.0, 20.0, 20.0) });
        }
    }
    for y in 112..floor {
        for x in 452..x1 {
            let n = fbm(x as f32 * 0.16, y as f32 * 0.1, 431) + (x - 452) as f32 * 0.012 - (floor - y) as f32 * 0.0008;
            if n > 0.56 {
                cv.put(x, y, if hash(x, y, 433) > 0.5 { (50.0, 92.0, 48.0) } else { (72.0, 116.0, 60.0) });
            }
        }
    }
    let [landing, balcony, top_left, top_right] = COURTYARD_BALCONIES;
    door(cv, 318, 196, 340, floor, (40.0, 84.0, 66.0));
    door(cv, 312, 80, 332, top_left.y as i32, (52.0, 80.0, 130.0));
    door(cv, 362, 116, 380, balcony.y as i32, (140.0, 64.0, 50.0));
    door(cv, 440, 152, 456, landing.y as i32, (60.0, 60.0, 66.0));
    door(cv, 440, 80, 456, top_right.y as i32, (130.0, 110.0, 60.0));
    door(cv, 446, 222, 464, floor, (60.0, 60.0, 66.0));
    let iron = (34.0, 38.0, 40.0);
    // The blade sign over the shop, on its bracket.
    cv.line((300, 150), (316, 150), iron);
    cv.fill(302, 152, 314, 162, (210.0, 196.0, 160.0));
    cv.fill(305, 155, 310, 160, (90.0, 56.0, 36.0));
    cv.fill(310, 156, 312, 158, (90.0, 56.0, 36.0));
    for (x, y) in [(338, 160), (404, 168), (414, 92), (320, 66)] {
        cv.fill(x, y, x + 2, y + 3, iron);
        cv.fill(x, y + 1, x + 2, y + 3, (255.0, 220.0, 150.0));
        cv.glow(x as f32 + 1.0, y as f32 + 2.0, 14.0, (110.0, 76.0, 36.0), 0.7);
    }
    for b in [landing, balcony, top_left, top_right] {
        railing(cv, b.x as i32, (b.x + b.w) as i32, b.y as i32);
    }
    flight(cv, (356, floor), (landing.x as i32, landing.y as i32));
    flight(cv, (landing.x as i32, landing.y as i32), ((balcony.x + balcony.w) as i32, balcony.y as i32));
    flight(cv, ((balcony.x + balcony.w) as i32, balcony.y as i32), (top_right.x as i32, top_right.y as i32));
    for t in COURTYARD_TABLES {
        let cx = (t.x + t.w / 2.0) as i32;
        cv.fill(cx - 1, (t.y + t.h) as i32, cx + 1, floor - 3, iron);
        for (dx, dy) in [(-3, 1), (-2, 2), (2, 2), (3, 1)] {
            cv.put(cx + dx, floor - dy, iron);
        }
        cv.fill(cx - 2, floor - 3, cx + 2, floor - 2, iron);
        for side in [-1, 1] {
            let seat = cx + side * ((t.w / 2.0) as i32 + 6);
            cv.fill(seat - 3, floor - 20, seat + 4, floor - 18, iron);
            let back = seat + side * 3;
            for y in floor - 34..floor - 18 {
                cv.put(back, y, iron);
            }
            cv.put(back - side, floor - 35, iron);
            cv.put(back - side * 2, floor - 34, iron);
            cv.line((seat - side * 3, floor - 18), (seat - side * 4, floor - 1), iron);
            cv.line((back, floor - 18), (back + side, floor - 1), iron);
        }
    }
    cv.fill(298, floor - 9, 314, floor, (110.0, 76.0, 52.0));
    cv.fill(298, floor - 10, 314, floor - 9, (140.0, 98.0, 66.0));
    leaves(cv, &[(302.0, 240.0, 5.0), (309.0, 239.0, 5.0), (305.0, 235.0, 4.0)]);
    cv.fill(398, floor - 12, 409, floor, (176.0, 96.0, 64.0));
    cv.fill(397, floor - 13, 410, floor - 12, (200.0, 118.0, 80.0));
    cv.line((403, floor - 13), (402, floor - 32), (80.0, 62.0, 46.0));
    cv.line((402, floor - 26), (397, floor - 34), (80.0, 62.0, 46.0));
    leaves(cv, &[(402.0, 214.0, 8.0), (395.0, 218.0, 6.0), (409.0, 219.0, 6.0), (403.0, 207.0, 6.0)]);
    for (from, to, sag) in [((x0 + 9, 14), (x1 - 2, 22), 9.0), ((x0 + 9, 34), (x1 - 2, 44), 11.0)] {
        lights(cv, from, to, sag);
    }
}

fn facade(x: i32, y: i32) -> Rgb {
    let n = (fbm(x as f32 * 0.09, y as f32 * 0.09, 435) - 0.5) * 18.0;
    let (roof, wall, cap) = if x < SEAM { (LEFT_ROOF, (166.0, 112.0, 82.0), (198.0, 160.0, 128.0)) } else { (RIGHT_ROOF, (118.0, 130.0, 104.0), (164.0, 170.0, 148.0)) };
    if y < roof {
        let mut c = dusk(y as f32 + 40.0);
        if hash(x, y, 437) > 0.995 {
            c = lift(c, 70.0);
        }
        return c;
    }
    if y < roof + 2 {
        return cap;
    }
    if x == SEAM || x == SEAM + 1 {
        return (70.0, 70.0, 64.0);
    }
    if y >= FLOOR_Y as i32 - 8 {
        return lift(scale(wall, 0.72), n);
    }
    lift(wall, n)
}

fn door(cv: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, paint: Rgb) {
    for y in y0..y1 {
        for x in x0..x1 {
            let frame = x == x0 || x == x1 - 1 || y == y0;
            let panel = (x == x0 + 3 || x == x1 - 4) && y > y0 + 3 && y < y1 - 3 || (y == y0 + 3 || y == y1 - 4 || y == (y0 + y1) / 2) && x > x0 + 2 && x < x1 - 3;
            cv.put(x, y, if frame { (54.0, 50.0, 48.0) } else if panel { scale(paint, 0.72) } else { paint });
        }
    }
    cv.fill(x1 - 4, (y0 + y1) / 2 + 2, x1 - 2, (y0 + y1) / 2 + 4, (206.0, 170.0, 90.0));
    cv.fill(x0 - 1, y0 - 2, x1 + 1, y0, (90.0, 84.0, 78.0));
}

/// An iron railing standing on a balcony along x0..x1, its slab's top at `floor`, with a flower box at one end.
fn railing(cv: &mut Canvas, x0: i32, x1: i32, floor: i32) {
    let iron = (32.0, 34.0, 36.0);
    for x in x0..x1 {
        cv.put(x, floor - 10, (64.0, 66.0, 70.0));
        cv.put(x, floor - 3, iron);
        if (x - x0) % 4 == 0 || x == x1 - 1 {
            for y in floor - 9..floor {
                cv.put(x, y, iron);
            }
        }
    }
    cv.fill(x1 - 10, floor - 14, x1 - 2, floor - 10, (110.0, 76.0, 52.0));
    for x in x1 - 10..x1 - 2 {
        cv.put(x, floor - 15, if x % 2 == 0 { (200.0, 60.0, 70.0) } else { (60.0, 110.0, 56.0) });
    }
}

/// A flight of iron stairs from `low` up to `high`: open treads on a stringer, a handrail over it.
fn flight(cv: &mut Canvas, low: (i32, i32), high: (i32, i32)) {
    let (iron, tread) = ((40.0, 42.0, 46.0), (104.0, 106.0, 112.0));
    let steps = (low.1 - high.1) / 6;
    let dir = (high.0 - low.0).signum();
    for k in 1..=steps {
        let t = k as f32 / steps as f32;
        let x = (low.0 as f32 + (high.0 - low.0) as f32 * t).round() as i32;
        let y = low.1 - k * 6;
        for d in 0..6 {
            cv.put(x - dir * d, y, tread);
        }
        if k % 2 == 0 {
            for d in 1..10 {
                cv.put(x, y - d, iron);
            }
        }
    }
    cv.line(low, high, iron);
    cv.line((low.0, low.1 + 1), (high.0, high.1 + 1), iron);
    cv.line((low.0, low.1 - 10), (high.0, high.1 - 10), (70.0, 72.0, 78.0));
}

fn leaves(cv: &mut Canvas, blobs: &[(f32, f32, f32)]) {
    let (x0, x1) = (blobs.iter().map(|b| b.0 - b.2).fold(f32::MAX, f32::min) as i32 - 2, blobs.iter().map(|b| b.0 + b.2).fold(f32::MIN, f32::max) as i32 + 2);
    let (y0, y1) = (blobs.iter().map(|b| b.1 - b.2).fold(f32::MAX, f32::min) as i32 - 2, blobs.iter().map(|b| b.1 + b.2).fold(f32::MIN, f32::max) as i32 + 2);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
            let rag = (fbm(p.x * 0.4, p.y * 0.4, 441) - 0.5) * 3.0;
            let inside = blobs.iter().map(|&(bx, by, r)| r + rag - p.distance(vec2(bx, by))).fold(f32::MIN, f32::max);
            if inside > 0.0 {
                let n = fbm(p.x * 0.3, p.y * 0.3, 443);
                cv.put(x, y, if inside < 1.5 && n > 0.5 { (104.0, 130.0, 70.0) } else { (40.0 + n * 30.0, 74.0 + n * 40.0, 42.0 + n * 14.0) });
            }
        }
    }
}

/// A strand of warm bulbs sagging between two points.
fn lights(cv: &mut Canvas, from: (i32, i32), to: (i32, i32), sag: f32) {
    let n = to.0 - from.0;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let x = from.0 + i;
        let y = (from.1 as f32 + (to.1 - from.1) as f32 * t + sag * (t * std::f32::consts::PI).sin()).round() as i32;
        cv.put(x, y, (30.0, 28.0, 30.0));
        if i % 7 == 3 {
            cv.put(x, y + 1, (255.0, 214.0, 140.0));
            cv.glow(x as f32 + 0.5, y as f32 + 1.5, 4.0, (120.0, 90.0, 40.0), 0.5);
        }
    }
}

/// The Cafe's ledges, walls, ceiling and floor, one pixel (as `city_px`).
pub(crate) fn terrain_px(a: &Arena, x: i32, y: i32) -> Option<Option<Rgb>> {
    let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
    if CAFE_BAR.contains(p) {
        return Some(Some(bar_px(x, y)));
    }
    if CAFE_DIVIDER.contains(p) && y >= WALL as i32 {
        // Painted brick, pale against the hazy backdrop either side, so it reads as solid.
        let lintel = y >= (CAFE_DIVIDER.y + CAFE_DIVIDER.h) as i32 - 5;
        let (left, right) = (x == CAFE_DIVIDER.x as i32, x == (CAFE_DIVIDER.x + CAFE_DIVIDER.w) as i32 - 1);
        let c = if lintel {
            if y == (CAFE_DIVIDER.y + CAFE_DIVIDER.h) as i32 - 5 { (130.0, 132.0, 138.0) } else if y % 5 == 2 && x % 3 == 1 { (120.0, 122.0, 128.0) } else { (70.0, 72.0, 78.0) }
        } else if left {
            (238.0, 232.0, 220.0)
        } else if right {
            (112.0, 106.0, 98.0)
        } else {
            let mortar = y.rem_euclid(4) == 3 || (x + if y.div_euclid(4) % 2 == 0 { 0 } else { 4 }).rem_euclid(8) == 7;
            let n = (fbm(x as f32 * 0.2, y as f32 * 0.1, 451) - 0.5) * 14.0;
            if mortar { (156.0 + n, 148.0 + n, 136.0 + n) } else { (204.0 + n, 196.0 + n, 182.0 + n) }
        };
        return Some(Some(c));
    }
    if let Some(s) = a.platforms.iter().find(|s| s.contains(p)) {
        return Some(ledge_px(s, x - s.x as i32, y - s.y as i32, x, y));
    }
    if y < WALL as i32 {
        let c = if x < CAFE_DIVIDER.x as i32 + CAFE_DIVIDER.w as i32 {
            if y == WALL as i32 - 1 { (50.0, 48.0, 48.0) } else { lift((92.0, 90.0, 88.0), (fbm(x as f32 * 0.1, y as f32 * 0.3, 453) - 0.5) * 18.0 - (y % 3 == 0) as u8 as f32 * 8.0) }
        } else if y == 0 || y == WALL as i32 - 1 {
            (70.0, 72.0, 78.0)
        } else if y == 3 && x % 8 == 0 {
            (100.0, 102.0, 108.0)
        } else {
            (44.0, 46.0, 52.0)
        };
        return Some(Some(c));
    }
    if x < WALL as i32 {
        let c = brick(x, y, 455);
        return Some(Some(if x == WALL as i32 - 1 { lift(c, 16.0) } else { c }));
    }
    if x >= (W - WALL) as i32 {
        let block = (y / 6) % 2 == 0;
        let c = if y % 6 == 5 { (90.0, 92.0, 80.0) } else if block { (150.0, 150.0, 128.0) } else { (128.0, 132.0, 112.0) };
        return Some(Some(if x == (W - WALL) as i32 { scale(c, 0.7) } else { c }));
    }
    if y >= FLOOR_Y as i32 {
        return Some(Some(floor_px(x, y)));
    }
    None
}

/// The bar: a thick honey-wood counter, a paneled front with a brass foot rail, and two stools pulled up to it.
fn bar_px(x: i32, y: i32) -> Rgb {
    let (lx, ly) = (x - CAFE_BAR.x as i32, y - CAFE_BAR.y as i32);
    for cx in [56, 80] {
        let seat = (14..16).contains(&ly) && (x - cx).abs() <= 3;
        let stem = x == cx && (16..34).contains(&ly);
        let ring = ly == 26 && (x - cx).abs() <= 2;
        let base = (34..36).contains(&ly) && (x - cx).abs() <= 2;
        if seat {
            return if ly == 14 { (110.0, 52.0, 44.0) } else { (78.0, 34.0, 30.0) };
        }
        if stem || ring || base {
            return (54.0, 54.0, 58.0);
        }
    }
    let mut c = match ly {
        0 => (204.0, 158.0, 104.0),
        1 | 2 => wood(x, y, (172.0, 122.0, 78.0), 457),
        3 => (110.0, 74.0, 46.0),
        8 => (128.0, 86.0, 54.0),
        26 => (200.0, 162.0, 86.0),
        32.. => (40.0, 30.0, 24.0),
        _ if lx % 6 == 0 => (62.0, 40.0, 26.0),
        _ => wood(x, y, if (lx / 6) % 2 == 0 { (98.0, 64.0, 42.0) } else { (90.0, 58.0, 38.0) }, 459),
    };
    if (27..30).contains(&ly) && lx % 23 == 11 {
        c = (150.0, 120.0, 64.0);
    }
    if lx == CAFE_BAR.w as i32 - 1 {
        c = scale(c, 0.65);
    }
    c
}

/// One pixel of a ledge `s` (`lx`, `d` inside it): None leaves it clear (a rounded corner, a hole in a grate).
fn ledge_px(s: &Aabb, lx: i32, d: i32, x: i32, y: i32) -> Option<Rgb> {
    let w = s.w as i32;
    let corner = (lx == 0 || lx == w - 1) && (d == 0 || d == 4);
    let plank = |base: Rgb| match d {
        0 => lift(base, 38.0),
        1 | 2 => wood(x, y, base, 461),
        3 => scale(base, 0.8),
        _ => scale(base, 0.45),
    };
    if CAFE_TABLES.contains(s) {
        return (!corner).then(|| plank((166.0, 118.0, 74.0)));
    }
    if CAFE_SHELVES.contains(s) {
        return Some(plank((116.0, 78.0, 50.0)));
    }
    if *s == CAFE_PLANT_SHELF {
        return Some(plank((176.0, 140.0, 96.0)));
    }
    if CAFE_HANGING.contains(s) {
        let knot = lx == 2 || lx == w - 3;
        return Some(if knot && d < 4 { (196.0, 170.0, 120.0) } else { plank((140.0, 98.0, 62.0)) });
    }
    if COURTYARD_TABLES.contains(s) || *s == COURTYARD_BALCONIES[0] {
        let (rim, mesh, under) = if *s == COURTYARD_BALCONIES[0] { ((140.0, 144.0, 150.0), (84.0, 88.0, 94.0), (44.0, 46.0, 50.0)) } else { ((96.0, 118.0, 102.0), (48.0, 64.0, 56.0), (30.0, 38.0, 34.0)) };
        if corner && *s != COURTYARD_BALCONIES[0] {
            return None;
        }
        return match d {
            0 => Some(rim),
            4 => Some(under),
            _ if lx > 0 && lx < w - 1 && (lx + d) % 2 == 0 => None,
            _ => Some(mesh),
        };
    }
    if *s == COURTYARD_AWNING {
        let base = if (lx / 4) % 2 == 0 { (222.0, 210.0, 178.0) } else { (44.0, 106.0, 78.0) };
        return match d {
            0 => Some(lift(base, 22.0)),
            4 if lx % 4 == 3 => None,
            4 => Some(scale(base, 0.75)),
            _ => Some(scale(base, 1.0 - d as f32 * 0.04)),
        };
    }
    let concrete = lift((150.0, 144.0, 134.0), (hash(x / 2, y, 463) - 0.5) * 10.0);
    Some(match d {
        0 => (184.0, 178.0, 166.0),
        1 => concrete,
        4 => (70.0, 68.0, 64.0),
        _ => scale(concrete, 0.8),
    })
}

/// The cafe's plank floor, a steel threshold under the wall, and the courtyard's cobblestones.
fn floor_px(x: i32, y: i32) -> Rgb {
    let d = y - FLOOR_Y as i32;
    let (cafe_end, gap_end) = (CAFE_DIVIDER.x as i32, (CAFE_DIVIDER.x + CAFE_DIVIDER.w) as i32);
    if x < cafe_end {
        if d == 0 {
            return (186.0, 138.0, 92.0);
        }
        let row = (d - 1) / 3;
        let base = scale((150.0, 102.0, 64.0), 1.0 - d as f32 * 0.035);
        if (d - 1) % 3 == 2 || (x + row * 13).rem_euclid(28) == 0 {
            return scale(base, 0.6);
        }
        return wood(x, y, base, 465);
    }
    if x < gap_end {
        return if d == 0 { (160.0, 162.0, 168.0) } else { (60.0, 60.0, 64.0) };
    }
    let row = d / 4;
    let off = row * 3;
    if d % 4 == 3 || (x + off).rem_euclid(6) == 5 {
        return (44.0, 42.0, 46.0);
    }
    let t = hash((x + off).div_euclid(6), row, 467);
    let c = (100.0 + t * 30.0, 96.0 + t * 26.0, 92.0 + t * 20.0);
    let c = if d == 0 || (x + off).rem_euclid(6) == 0 && d % 4 == 0 { lift(c, 22.0) } else { c };
    scale(c, 1.0 - d as f32 * 0.025)
}

/// The commuter train that passes behind the window every so often, left to right (drawn between the backdrop and its front layer).
pub(crate) fn draw_train(time: f32) {
    const PERIOD: f32 = 22.0;
    const SPEED: f32 = 60.0;
    const CAR: f32 = 40.0;
    const CARS: usize = 5;
    let len = CARS as f32 * (CAR + 1.0);
    let head = (WINDOW.0 as f32 - 6.0 + (time % PERIOD) * SPEED).round();
    if head - len > DOOR.2 as f32 {
        return;
    }
    let c = |rgb: Rgb| {
        let g = grade(rgb, Zone::Glass);
        Color::new(g.0 / 255.0, g.1 / 255.0, g.2 / 255.0, 1.0)
    };
    let top = TRAIN_TOP;
    for k in 0..CARS {
        let x1 = head - k as f32 * (CAR + 1.0);
        let x0 = x1 - CAR;
        let nose = if k == 0 { 4.0 } else { 0.0 };
        draw_rectangle(x0, top + 1.0, CAR - nose, 11.0, c((150.0, 154.0, 162.0)));
        draw_rectangle(x0 + 1.0, top, CAR - 2.0 - nose, 1.0, c((112.0, 116.0, 124.0)));
        draw_rectangle(x0, top + 8.0, CAR - nose, 2.0, c((50.0, 84.0, 150.0)));
        for w in 0..6 {
            let wx = x0 + 2.0 + w as f32 * 6.0;
            if wx + 4.0 > x1 - nose {
                break;
            }
            let door = w == 2 || w == 5;
            draw_rectangle(wx, top + 3.0, 4.0, if door { 8.0 } else { 3.0 }, if door { c((96.0, 100.0, 108.0)) } else { c((214.0, 190.0, 130.0)) });
        }
        draw_rectangle(x0, top + 11.0, CAR, 1.0, c((60.0, 62.0, 68.0)));
        for b in [x0 + 4.0, x1 - 10.0] {
            draw_rectangle(b, top + 12.0, 6.0, 1.0, c((28.0, 28.0, 32.0)));
        }
        if k == 1 {
            draw_line(x0 + 18.0, top, x0 + 22.0, top - 5.0, 1.0, c((40.0, 40.0, 46.0)));
            draw_line(x0 + 26.0, top, x0 + 22.0, top - 5.0, 1.0, c((40.0, 40.0, 46.0)));
        }
    }
    for r in 0..11 {
        let reach = (r as f32 * 0.6).min(4.0);
        draw_rectangle(head - 4.0, top + 1.0 + r as f32, reach, 1.0, c(if r < 4 { (40.0, 46.0, 60.0) } else { (150.0, 154.0, 162.0) }));
    }
    draw_rectangle(head - 4.0, top + 8.0, 4.0, 2.0, c((50.0, 84.0, 150.0)));
    draw_rectangle(head - 1.0, top + 10.0, 1.0, 1.0, c((255.0, 244.0, 210.0)));
}
