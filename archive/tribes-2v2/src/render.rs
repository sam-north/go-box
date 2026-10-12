//! Procedural pixel art in a Rain World / Noita spirit: everything is generated in
//! code at 320x180 — grimy industrial backdrop, mossy brick, rain, procedurally
//! animated bodies, rocks, acid, and blood/acid pixels that stick to the terrain.

use crate::sim::*;
use macroquad::prelude::*;

const CLOAK: Color = Color::new(0.27, 0.28, 0.33, 1.0);
const HOOD: Color = Color::new(0.15, 0.16, 0.19, 1.0);
const FACE: Color = Color::new(0.84, 0.78, 0.69, 1.0);
const SHAFT: Color = Color::new(0.47, 0.39, 0.27, 1.0);
const LIZ_BODY: Color = Color::new(0.25, 0.26, 0.30, 1.0);
const LIZ_LEG: Color = Color::new(0.17, 0.18, 0.21, 1.0);
const ROCK: Color = Color::new(0.50, 0.48, 0.44, 1.0);
const ACID: Color = Color::new(0.70, 1.0, 0.25, 1.0);
const HEAL: Color = Color::new(0.38, 0.86, 1.0, 1.0);
const BLOOD_H: Color = Color::new(0.50, 0.06, 0.08, 1.0);
const BLOOD_BITER: Color = Color::new(0.58, 0.14, 0.42, 1.0);
const BLOOD_SPITTER: Color = Color::new(0.42, 0.52, 0.10, 1.0);

/// Signature colour per kind: scarf for humans, head for creatures. Also used by the HUD.
pub fn accent(kind: Kind) -> Color {
    match kind {
        Kind::Spear => Color::new(0.62, 0.24, 0.16, 1.0),
        Kind::Thrower => Color::new(0.22, 0.55, 0.60, 1.0),
        Kind::Biter => Color::new(0.98, 0.35, 0.67, 1.0),
        Kind::Spitter => Color::new(0.72, 0.92, 0.22, 1.0),
    }
}

fn blood(kind: Kind) -> Color {
    match kind {
        Kind::Spear | Kind::Thrower => BLOOD_H,
        Kind::Biter => BLOOD_BITER,
        Kind::Spitter => BLOOD_SPITTER,
    }
}

const SPINE: usize = 10;
const SPINE_SEG: f32 = 2.2;
const SPINE_R: [f32; SPINE] = [3.0, 2.6, 2.8, 2.8, 2.6, 2.1, 1.6, 1.2, 0.8, 0.5];

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263) ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xff_ffff) as f32 / 16_777_215.0
}

fn vnoise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let a = hash(xi, yi, seed) + (hash(xi + 1, yi, seed) - hash(xi, yi, seed)) * sx;
    let b = hash(xi, yi + 1, seed) + (hash(xi + 1, yi + 1, seed) - hash(xi, yi + 1, seed)) * sx;
    a + (b - a) * sy
}

fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    let (mut v, mut amp, mut f) = (0.0, 0.5, 1.0);
    for o in 0..4 {
        v += vnoise(x * f, y * f, seed + o) * amp;
        amp *= 0.5;
        f *= 2.0;
    }
    v
}

const BAYER: [[f32; 4]; 4] = [[0.0, 8.0, 2.0, 10.0], [12.0, 4.0, 14.0, 6.0], [3.0, 11.0, 1.0, 9.0], [15.0, 7.0, 13.0, 5.0]];

/// Ordered-dither a colour (0..255 floats) down to a coarse palette for a crunchy pixel look.
fn dither(x: u32, y: u32, r: f32, g: f32, b: f32) -> Color {
    let t = (BAYER[(y % 4) as usize][(x % 4) as usize] / 16.0 - 0.5) * 6.0;
    let q = |v: f32| (((v + t) / 6.0).round() * 6.0).clamp(0.0, 255.0) as u8;
    rgb(q(r), q(g), q(b))
}

fn is_stone(x: i32, y: i32) -> bool {
    let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
    SOLIDS.iter().any(|r| r.contains(p))
}

fn is_metal(x: i32, y: i32) -> bool {
    let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
    PLATFORMS.iter().any(|r| r.contains(p))
}

fn solid_px(x: i32, y: i32) -> bool {
    is_stone(x, y) || is_metal(x, y)
}

fn make_background() -> Image {
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    let pipes_v = [(22, 31), (70, 74), (142, 154), (222, 228), (276, 287)];
    let pipes_h = [(36, 41, 0, 160), (100, 103, 120, 320), (60, 62, 180, 320)];
    for y in 0..H as u32 {
        for x in 0..W as u32 {
            let (fx, fy) = (x as f32, y as f32);
            let t = fy / H;
            let (mut r, mut g, mut b) = (13.0 + 10.0 * t, 16.0 + 8.0 * t, 21.0 + 4.0 * t);
            // Distant industrial pipes.
            for &(x0, x1) in &pipes_v {
                if x >= x0 && x < x1 {
                    let edge = if x == x0 { 9.0 } else if x == x1 - 1 { -6.0 } else { 0.0 };
                    let joint = if (y + x0) % 23 < 2 { 6.0 } else { 0.0 };
                    r = 27.0 + edge + joint;
                    g = 31.0 + edge + joint;
                    b = 35.0 + edge + joint;
                }
            }
            for &(y0, y1, xa, xb) in &pipes_h {
                if y >= y0 && y < y1 && x >= xa && x < xb {
                    let edge = if y == y0 { 8.0 } else { 0.0 };
                    r = 24.0 + edge;
                    g = 28.0 + edge;
                    b = 31.0 + edge;
                }
            }
            // Hanging vines / cables from the ceiling.
            let vine_len = (hash(x as i32, 0, 7) * 40.0) as u32;
            if hash(x as i32, 1, 9) > 0.93 && y < 8 + vine_len {
                let sway = (fy * 0.3).sin() > 0.6;
                if !sway {
                    r = 20.0;
                    g = 34.0;
                    b = 24.0;
                }
            }
            // Grime, drip streaks, and a soft shaft of light from the upper left.
            let n = fbm(fx * 0.05, fy * 0.05, 3);
            let grime = (n - 0.5) * 18.0;
            let streak = if hash(x as i32, 2, 11) > 0.8 && fy > hash(x as i32, 3, 13) * 60.0 { -4.0 } else { 0.0 };
            let shaft_d = ((fx - fy * 0.6) - 80.0).abs();
            let shaft = if shaft_d < 40.0 { (1.0 - shaft_d / 40.0) * 10.0 * (1.0 - t) } else { 0.0 };
            img.set_pixel(x, y, dither(x, y, r + grime + streak + shaft, g + grime + streak + shaft * 1.1, b + grime + streak + shaft * 1.2));
        }
    }
    img
}

fn make_terrain() -> Image {
    let mut img = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let (ux, uy) = (x as u32, y as u32);
            if is_stone(x, y) {
                let row = y / 5;
                let off = (row % 2) * 5;
                let col = (x + off) / 10;
                let mortar = y % 5 == 0 || (x + off) % 10 == 0;
                let tone = hash(col, row, 21) * 14.0 - 7.0;
                let n = (fbm(x as f32 * 0.08, y as f32 * 0.08, 5) - 0.5) * 22.0;
                let (mut r, mut g, mut b) = (56.0 + tone + n, 50.0 + tone + n, 45.0 + tone + n);
                if mortar {
                    r -= 16.0;
                    g -= 16.0;
                    b -= 14.0;
                }
                // Depth below an exposed top surface, for moss.
                let mut depth = 0;
                while depth < 5 && is_stone(x, y - depth - 1) {
                    depth += 1;
                }
                let exposed_top = depth < 5 && !is_stone(x, y - depth - 1) && y - depth - 1 >= 0;
                let moss_len = (fbm(x as f32 * 0.15, 0.0, 8) * 7.0) as i32 - 1 + (hash(x, 4, 3) * 2.0) as i32;
                if exposed_top && depth == 0 {
                    r = 92.0 + n;
                    g = 86.0 + n;
                    b = 74.0 + n;
                }
                if exposed_top && depth < moss_len {
                    r = 54.0 + n;
                    g = 82.0 + n + hash(x, y, 5) * 16.0;
                    b = 42.0 + n;
                }
                let air_side = !solid_px(x - 1, y) && x > 0 || !solid_px(x + 1, y) && x < W as i32 - 1 || !solid_px(x, y + 1) && y < H as i32 - 1;
                if air_side && !exposed_top {
                    r *= 0.55;
                    g *= 0.55;
                    b *= 0.55;
                }
                img.set_pixel(ux, uy, dither(ux, uy, r, g, b));
            } else if is_metal(x, y) {
                let top = !is_metal(x, y - 1);
                let bottom = !is_metal(x, y + 1);
                let rust = fbm(x as f32 * 0.12, y as f32 * 0.3, 17) > 0.58;
                let (mut r, mut g, mut b) = (44.0, 50.0, 58.0);
                if rust {
                    (r, g, b) = (88.0, 56.0, 40.0);
                }
                if top {
                    (r, g, b) = (r + 34.0, g + 34.0, b + 34.0);
                }
                if bottom {
                    (r, g, b) = (r * 0.5, g * 0.5, b * 0.5);
                }
                let mid = is_metal(x, y - 2) && is_metal(x, y + 2) && !is_metal(x, y - 3);
                if mid && x % 8 == 3 {
                    (r, g, b) = (100.0, 104.0, 108.0);
                }
                img.set_pixel(ux, uy, dither(ux, uy, r, g, b));
            }
        }
    }
    img
}

fn make_vignette() -> Image {
    let mut img = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
    for y in 0..H as u32 {
        for x in 0..W as u32 {
            let d = vec2((x as f32 / W - 0.5) * 2.0, (y as f32 / H - 0.5) * 2.0).length() / std::f32::consts::SQRT_2;
            let a = ((d - 0.45) / 0.55).clamp(0.0, 1.0);
            let a = a * a * 0.75 + (BAYER[(y % 4) as usize][(x % 4) as usize] / 16.0 - 0.5) * 0.04;
            img.set_pixel(x, y, Color::new(0.02, 0.03, 0.05, a.clamp(0.0, 1.0)));
        }
    }
    img
}


struct Particle {
    pos: Vec2,
    vel: Vec2,
    color: Color,
    life: f32,
    stick: bool,
    gravity: f32,
}

struct Drop {
    pos: Vec2,
    speed: f32,
}

#[derive(Default)]
struct FighterVis {
    init: bool,
    walk_phase: f32,
    /// Seconds since this fighter last threw or spat (drives the follow-through pose).
    since_ranged: f32,
    scarf: [Vec2; 6],
    scarf_prev: [Vec2; 6],
    spine: [Vec2; SPINE],
    feet: [Vec2; 4],
    feet_target: [Vec2; 4],
}

pub struct Art {
    bg: Texture2D,
    terrain: Texture2D,
    vignette: Texture2D,
    stain_img: Image,
    stain_tex: Texture2D,
    stain_dirty: bool,
    particles: Vec<Particle>,
    rain: Vec<Drop>,
    vis: [FighterVis; N_FIGHTERS],
    time: f32,
}

fn tex(img: &Image) -> Texture2D {
    let t = Texture2D::from_image(img);
    t.set_filter(FilterMode::Nearest);
    t
}

impl Art {
    pub fn new() -> Self {
        let stain_img = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        let rain = (0..90)
            .map(|i| Drop { pos: vec2(hash(i, 0, 1) * W, hash(i, 1, 1) * H), speed: 200.0 + hash(i, 2, 1) * 120.0 })
            .collect();
        Self {
            bg: tex(&make_background()),
            terrain: tex(&make_terrain()),
            vignette: tex(&make_vignette()),
            stain_tex: tex(&stain_img),
            stain_img,
            stain_dirty: false,
            particles: Vec::new(),
            rain,
            vis: Default::default(),
            time: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.stain_img = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        self.stain_dirty = true;
        self.particles.clear();
        self.vis = Default::default();
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, pos: Vec2, dir: Vec2, spread: f32, n: usize, color: Color, stick: bool, speed: f32, life: f32) {
        for i in 0..n {
            let seed = (self.time * 1000.0) as i32 + i as i32 * 7 + self.particles.len() as i32;
            let a = (hash(seed, 0, 31) - 0.5) * spread;
            let s = speed * (0.3 + hash(seed, 1, 31) * 0.9);
            let v = vec2(dir.x * a.cos() - dir.y * a.sin(), dir.x * a.sin() + dir.y * a.cos()) * s;
            let shade = 0.75 + hash(seed, 2, 31) * 0.35;
            let c = Color::new(color.r * shade, color.g * shade, color.b * shade, color.a);
            let life = life * (0.5 + hash(seed, 3, 31));
            self.particles.push(Particle { pos, vel: v, color: c, life, stick, gravity: 420.0 });
        }
    }

    pub fn handle_events(&mut self, m: &Match) {
        for e in &m.events {
            match *e {
                Event::Hit { victim, pos, dir } => {
                    self.burst(pos, vec2(dir, -0.5).normalize(), 1.6, 22, blood(m.f[victim].kind), true, 140.0, 2.5);
                    self.burst(pos, vec2(dir, 0.0), 2.5, 6, Color::new(1.0, 0.95, 0.85, 1.0), false, 120.0, 0.4);
                }
                Event::Death { who, pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.0, 70, blood(m.f[who].kind), true, 160.0, 2.5);
                }
                Event::Jump { pos } | Event::Land { pos } => {
                    self.burst(pos + vec2(0.0, -1.0), vec2(0.0, -1.0), 3.0, 4, Color::new(0.45, 0.43, 0.40, 0.8), false, 40.0, 0.5);
                }
                Event::Throw { who } | Event::Spit { who } => self.vis[who].since_ranged = 0.0,
                Event::Splash { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 5.0, 14, ACID, true, 70.0, 1.5);
                }
                Event::HealSpawn { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.0, 10, HEAL, false, 30.0, 0.6);
                }
                Event::Heal { who, pos } => {
                    // Motes drift up out of the fruit and off the healed fighter.
                    let c = m.f[who].center();
                    for i in 0..18 {
                        let seed = (self.time * 1000.0) as i32 + i * 13;
                        let from = if i % 3 == 0 { pos } else { c + vec2((hash(seed, 0, 51) - 0.5) * 8.0, (hash(seed, 1, 51) - 0.5) * 10.0) };
                        let vel = vec2((hash(seed, 2, 51) - 0.5) * 20.0, -15.0 - hash(seed, 3, 51) * 25.0);
                        self.particles.push(Particle { pos: from, vel, color: HEAL, life: 0.5 + hash(seed, 4, 51) * 0.6, stick: false, gravity: -20.0 });
                    }
                }
            }
        }
    }

    pub fn update(&mut self, dt: f32, m: &Match) {
        self.time += dt;
        for d in self.rain.iter_mut() {
            d.pos += vec2(-0.12, 1.0) * d.speed * dt;
            if d.pos.y > H {
                d.pos.y -= H;
                d.pos.x = (d.pos.x + 137.0) % W;
            }
            if d.pos.x < 0.0 {
                d.pos.x += W;
            }
        }
        // Acid globs leave a faint fizzing trail.
        for a in &m.acid {
            if hash((self.time * 977.0) as i32, a.owner as i32, 41) < dt * 40.0 {
                let c = Color::new(ACID.r, ACID.g, ACID.b, 0.6);
                self.particles.push(Particle { pos: a.pos, vel: a.vel * 0.1, color: c, life: 0.25, stick: false, gravity: 60.0 });
            }
        }
        let mut stains = Vec::new();
        self.particles.retain_mut(|p| {
            p.vel.y += p.gravity * dt;
            let next = p.pos + p.vel * dt;
            p.life -= dt;
            if solid_px(next.x as i32, next.y as i32) {
                if p.stick {
                    stains.push((p.pos, p.color));
                }
                return false;
            }
            p.pos = next;
            p.life > 0.0 && p.pos.x >= 0.0 && p.pos.x < W && p.pos.y >= 0.0 && p.pos.y < H
        });
        for (pos, c) in stains {
            let (x, y) = (pos.x as i32, pos.y as i32);
            if x >= 0 && y >= 0 && x < W as i32 && y < H as i32 && !solid_px(x, y) {
                self.stain_img.set_pixel(x as u32, y as u32, c);
                self.stain_dirty = true;
            }
        }
        for (f, v) in m.f.iter().zip(self.vis.iter_mut()) {
            if !v.init {
                init_vis(v, f);
            }
            v.walk_phase += dt * f.vel.x.abs() * 0.28;
            v.since_ranged += dt;
            match f.kind.tribe() {
                Tribe::Humans => update_scarf(v, f, dt, self.time),
                Tribe::Creatures => update_lizard(v, f, dt),
            }
        }
    }

    pub fn draw(&mut self, m: &Match, debug: bool) {
        if self.stain_dirty {
            self.stain_tex.update(&self.stain_img);
            self.stain_dirty = false;
        }
        draw_texture(&self.bg, 0.0, 0.0, WHITE);
        for d in &self.rain {
            draw_line(d.pos.x, d.pos.y, d.pos.x + 0.6, d.pos.y - 4.0, 1.0, Color::new(0.55, 0.62, 0.72, 0.22));
        }
        draw_texture(&self.terrain, 0.0, 0.0, WHITE);
        draw_texture(&self.stain_tex, 0.0, 0.0, WHITE);

        if debug {
            for f in m.f.iter().filter(|f| f.alive()) {
                let eye = f.eye();
                for i in 0..N_RAYS {
                    let d = ray_dir(i);
                    let t = terrain_ray(eye, d, RAY_MAX);
                    draw_line(eye.x, eye.y, eye.x + d.x * t, eye.y + d.y * t, 1.0, Color::new(0.3, 0.9, 1.0, 0.12));
                }
                if f.atk_timer > 0 {
                    let b = f.attack_box();
                    let c = if f.attack_active() { Color::new(1.0, 0.2, 0.2, 0.5) } else { Color::new(1.0, 1.0, 0.2, 0.3) };
                    draw_rectangle_lines(b.x, b.y, b.w, b.h, 1.0, c);
                }
            }
        }

        // Dead underneath the living.
        let mut order: Vec<usize> = (0..N_FIGHTERS).collect();
        order.sort_by_key(|&i| m.f[i].alive());
        for i in order {
            let f = &m.f[i];
            let flash = f.hitstun > 0 && (f.hitstun / 2) % 2 == 0;
            match f.kind.tribe() {
                Tribe::Humans => draw_human(&self.vis[i], f, flash),
                Tribe::Creatures => draw_lizard(&self.vis[i], f, flash),
            }
        }

        for &h in &m.heals {
            draw_heal(h, self.time);
        }
        for r in &m.rocks {
            draw_rock(r.pos);
        }
        for a in &m.acid {
            draw_rectangle(a.pos.x.round() - 1.0, a.pos.y.round() - 1.0, 2.0, 2.0, ACID);
            draw_circle(a.pos.x, a.pos.y, 2.5, Color::new(ACID.r, ACID.g, ACID.b, 0.15));
        }
        for p in &self.particles {
            draw_rectangle(p.pos.x.floor(), p.pos.y.floor(), 1.0, 1.0, p.color);
        }

        for f in m.f.iter().filter(|f| f.alive()) {
            let (x, y) = ((f.pos.x - 6.0).round(), (f.pos.y - 22.0).round());
            draw_rectangle(x, y, 12.0, 1.0, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(x, y, (12.0 * f.hp / MAX_HP).ceil(), 1.0, accent(f.kind));
        }
        draw_texture(&self.vignette, 0.0, 0.0, WHITE);
    }
}

/// A glowing bulb of fruit, gently bobbing and pulsing so it reads from across the arena.
fn draw_heal(p: Vec2, time: f32) {
    let bob = ((time * 3.0 + p.x * 0.1).sin() * 1.0).round();
    let pulse = 0.10 + 0.06 * (time * 4.0 + p.x).sin();
    let (x, y) = (p.x.round(), p.y.round() + bob);
    draw_circle(x, y, 6.0, Color::new(HEAL.r, HEAL.g, HEAL.b, pulse));
    draw_rectangle(x, y - 3.0, 1.0, 1.0, Color::new(0.25, 0.35, 0.25, 1.0));
    draw_rectangle(x - 1.0, y - 2.0, 3.0, 4.0, HEAL);
    draw_rectangle(x - 2.0, y - 1.0, 5.0, 2.0, HEAL);
    draw_rectangle(x - 1.0, y - 1.0, 1.0, 1.0, Color::new(0.92, 1.0, 1.0, 1.0));
    draw_rectangle(x, y + 1.0, 2.0, 1.0, Color::new(HEAL.r * 0.6, HEAL.g * 0.6, HEAL.b * 0.7, 1.0));
}

fn draw_rock(p: Vec2) {
    let (x, y) = (p.x.round() - 1.0, p.y.round() - 1.0);
    draw_rectangle(x, y, 3.0, 3.0, ROCK);
    draw_rectangle(x, y, 1.0, 1.0, Color::new(0.68, 0.66, 0.62, 1.0));
    draw_rectangle(x + 2.0, y + 2.0, 1.0, 1.0, Color::new(0.30, 0.29, 0.27, 1.0));
}

fn init_vis(v: &mut FighterVis, f: &Fighter) {
    v.init = true;
    v.since_ranged = 10.0;
    let neck = vec2(f.pos.x, f.pos.y - 11.0);
    for k in 0..6 {
        v.scarf[k] = neck + vec2(-f.facing * k as f32 * 1.5, 0.0);
        v.scarf_prev[k] = v.scarf[k];
    }
    for k in 0..SPINE {
        v.spine[k] = vec2(f.pos.x - f.facing * (k as f32 * SPINE_SEG - 4.0), f.pos.y - 6.0 + k as f32 * 0.3);
    }
    for k in 0..4 {
        v.feet[k] = vec2(f.pos.x + (k as f32 - 1.5) * 3.0, f.pos.y);
        v.feet_target[k] = v.feet[k];
    }
}

fn update_scarf(v: &mut FighterVis, f: &Fighter, dt: f32, time: f32) {
    let lean = (f.vel.x * 0.02).clamp(-1.5, 1.5);
    v.scarf[0] = vec2(f.pos.x + lean, f.pos.y - 11.0);
    let wind = vec2(-12.0 + (time * 2.3).sin() * 8.0, 0.0);
    for k in 1..6 {
        let cur = v.scarf[k];
        let vel = (cur - v.scarf_prev[k]) * 0.92;
        v.scarf_prev[k] = cur;
        v.scarf[k] = cur + vel + (vec2(0.0, 90.0) + wind) * dt * dt;
    }
    for _ in 0..3 {
        for k in 1..6 {
            let d = v.scarf[k] - v.scarf[k - 1];
            let len = d.length().max(0.001);
            v.scarf[k] = v.scarf[k - 1] + d / len * 1.6;
        }
    }
}

fn update_lizard(v: &mut FighterVis, f: &Fighter, dt: f32) {
    let s = f.facing;
    let mut head = vec2(f.pos.x + s * 4.0, f.pos.y - 7.0);
    if !f.alive() {
        head = vec2(v.spine[0].x, (v.spine[0].y + 60.0 * dt).min(f.pos.y - 2.0));
    } else if f.atk_timer > 0 && f.atk_timer <= ATK_WINDUP {
        head += vec2(-s * 2.0, -1.5);
    } else if f.attack_active() {
        head += vec2(s * if f.kind.ranged() { 5.0 } else { 9.0 }, 0.5);
    } else if v.since_ranged < 0.15 {
        // Spit recoil: head snaps up and back.
        head += vec2(-s * 1.5, -2.0);
    }
    // Ease the head toward its target so lunges read as motion, not teleports.
    v.spine[0] += (head - v.spine[0]) * (1.0 - (-30.0 * dt).exp());
    for k in 1..SPINE {
        v.spine[k].y += 40.0 * dt;
        let d = v.spine[k] - v.spine[k - 1];
        let len = d.length().max(0.001);
        v.spine[k] = v.spine[k - 1] + d / len * SPINE_SEG;
        let mut guard = 0;
        while solid_px(v.spine[k].x as i32, (v.spine[k].y + 1.0) as i32) && guard < 6 {
            v.spine[k].y -= 1.0;
            guard += 1;
        }
    }
    // Procedural stepping: a foot stays planted until its hip drifts too far away.
    let hips = [1, 1, 5, 5];
    for k in 0..4 {
        let hip = v.spine[hips[k]];
        let side = if k % 2 == 0 { 1.5 } else { -1.5 };
        if f.grounded {
            let desired = vec2(hip.x + side + f.vel.x * 0.12, f.pos.y);
            let other_moving = v.feet[k ^ 1].distance(v.feet_target[k ^ 1]) > 0.5;
            if v.feet_target[k].distance(desired) > 6.0 && !other_moving {
                v.feet_target[k] = desired;
            }
            if !f.alive() {
                v.feet_target[k] = hip + vec2(side, 2.0);
            }
        } else {
            v.feet_target[k] = hip + vec2(side - f.vel.x * 0.02, 4.0);
        }
        v.feet[k] += (v.feet_target[k] - v.feet[k]) * (1.0 - (-28.0 * dt).exp());
    }
}

/// Two-bone IK: returns the knee/elbow for a limb from `a` to `b`.
fn ik(a: Vec2, b: Vec2, l1: f32, l2: f32, bend: f32) -> Vec2 {
    let d = b - a;
    let len = d.length().clamp(0.01, l1 + l2 - 0.01);
    let dir = d.normalize_or_zero();
    let x = (l1 * l1 - l2 * l2 + len * len) / (2.0 * len);
    let h = (l1 * l1 - x * x).max(0.0).sqrt();
    a + dir * x + vec2(-dir.y, dir.x) * h * bend
}

fn line(a: Vec2, b: Vec2, t: f32, c: Color) {
    draw_line(a.x, a.y, b.x, b.y, t, c);
}

fn tint(c: Color, flash: bool) -> Color {
    if flash { Color::new(1.0, 0.95, 0.9, c.a) } else { c }
}

fn draw_human(v: &FighterVis, f: &Fighter, flash: bool) {
    let s = f.facing;
    let (x, fy) = (f.pos.x, f.pos.y);
    let scarf = accent(f.kind);
    if !f.alive() {
        line(vec2(x - 6.0 * s, fy - 1.5), vec2(x + 5.0 * s, fy - 1.5), 3.0, CLOAK);
        draw_circle(x + 7.0 * s, fy - 2.0, 2.5, HOOD);
        if f.kind == Kind::Spear {
            line(vec2(x - 9.0, fy - 0.5), vec2(x + 6.0, fy - 0.5), 1.0, SHAFT);
        }
        for k in 1..6 {
            line(v.scarf[k - 1], v.scarf[k], 1.0, scarf);
        }
        return;
    }
    let speed = f.vel.x.abs();
    let lean = (f.vel.x * 0.02).clamp(-1.5, 1.5);
    let hip = vec2(x, fy - 6.0);
    let neck = vec2(x + lean, fy - 11.0);
    let shoulder = neck + vec2(0.0, 1.0);
    let head = neck + vec2(s * 0.5, -2.5);

    // Scarf behind everything.
    for k in 1..6 {
        line(v.scarf[k - 1], v.scarf[k], 1.0, tint(scarf, flash));
    }
    // Legs.
    for k in 0..2 {
        let foot = if f.grounded {
            let ph = v.walk_phase + k as f32 * std::f32::consts::PI;
            let amt = (speed / 45.0).min(1.5);
            vec2(x + ph.sin() * 3.0 * amt, fy - (-ph.cos()).max(0.0) * 2.0 * amt.min(1.0))
        } else {
            vec2(x + (k as f32 - 0.5) * 3.0 - f.vel.x * 0.01, fy - 2.0)
        };
        let knee = ik(hip, foot, 3.4, 3.4, -s);
        let c = tint(if k == 0 { HOOD } else { CLOAK }, flash);
        line(hip, knee, 1.0, c);
        line(knee, foot, 1.0, c);
    }
    // Torso as a soft blob.
    for t in [0.0, 0.5, 1.0] {
        let p = hip.lerp(neck, t);
        draw_circle(p.x, p.y, 1.8, tint(CLOAK, flash));
    }
    let hand = match f.kind {
        Kind::Spear => draw_spear(f, neck, flash),
        _ => thrower_hand(f, v, neck),
    };
    let elbow = ik(shoulder, hand, 3.0, 3.5, s);
    line(shoulder, elbow, 1.0, tint(HOOD, flash));
    line(elbow, hand, 1.0, tint(HOOD, flash));
    // Hooded head with a pale face peeking out.
    draw_circle(head.x, head.y, 2.6, tint(HOOD, flash));
    draw_rectangle((head.x + s * 1.0 - 0.5).round(), head.y.round() - 1.0, 2.0, 2.0, tint(FACE, flash));
    draw_rectangle((head.x + s * 1.5).round(), head.y.round() - 1.0, 1.0, 1.0, Color::new(0.05, 0.05, 0.06, 1.0));
}

/// Draws the spear for the current swing phase and returns where the hand is.
fn draw_spear(f: &Fighter, neck: Vec2, flash: bool) -> Vec2 {
    let s = f.facing;
    let (x, fy) = (f.pos.x, f.pos.y);
    let rest = (neck + vec2(s * 1.5, 3.0), vec2(s * 0.55, -0.83));
    let (hand, dir) = match f.atk_timer {
        0 => rest,
        t if t <= ATK_WINDUP => (vec2(x - s * 2.0, fy - 8.5), vec2(s, -0.05)),
        t if t <= ATK_WINDUP + ATK_ACTIVE => (vec2(x + s * 6.0, fy - 8.0), vec2(s, 0.0)),
        t => {
            let k = (t - ATK_WINDUP - ATK_ACTIVE) as f32 / (ATK_TOTAL - ATK_WINDUP - ATK_ACTIVE) as f32;
            (vec2(x + s * 6.0, fy - 8.0).lerp(rest.0, k), vec2(s, 0.0).lerp(rest.1, k).normalize())
        }
    };
    line(hand - dir * 4.0, hand + dir * 9.0, 1.0, tint(SHAFT, flash));
    line(hand + dir * 9.0, hand + dir * 11.5, 1.0, tint(Color::new(0.85, 0.85, 0.8, 1.0), flash));
    hand
}

/// Where the rock-thrower's hand is: cocked overhead with a rock, punching, following
/// through after a throw, or hanging loose.
fn thrower_hand(f: &Fighter, v: &FighterVis, neck: Vec2) -> Vec2 {
    let s = f.facing;
    let (x, fy) = (f.pos.x, f.pos.y);
    if v.since_ranged < 0.15 {
        return vec2(x + s * 6.0, fy - 10.0);
    }
    match f.atk_timer {
        t if t > 0 && t <= ATK_WINDUP => vec2(x - s * 1.0, fy - 8.0),
        t if t > ATK_WINDUP && t <= ATK_WINDUP + ATK_ACTIVE => vec2(x + s * 7.0, fy - 8.0),
        _ if f.holding.is_some() => f.hand(),
        _ => neck + vec2(s * 1.5, 5.0),
    }
}

fn draw_lizard(v: &FighterVis, f: &Fighter, flash: bool) {
    let s = f.facing;
    let alive = f.alive();
    let head_c = if alive { accent(f.kind) } else { Color::new(0.40, 0.36, 0.38, 1.0) };
    // Far legs first, then body, then near legs.
    let hips = [1, 1, 5, 5];
    let draw_leg = |k: usize| {
        let hip = v.spine[hips[k]];
        let knee = ik(hip, v.feet[k], 3.0, 3.5, s);
        let c = tint(if k % 2 == 0 { LIZ_LEG } else { LIZ_BODY }, flash);
        line(hip, knee, 1.0, c);
        line(knee, v.feet[k], 1.0, c);
    };
    draw_leg(1);
    draw_leg(3);
    for k in (1..SPINE).rev() {
        draw_circle(v.spine[k].x, v.spine[k].y, SPINE_R[k], tint(LIZ_BODY, flash));
    }
    // Faint dorsal stripe catching the light.
    for k in 1..SPINE - 2 {
        let p = v.spine[k];
        draw_rectangle(p.x.round(), (p.y - SPINE_R[k]).round(), 1.0, 1.0, tint(Color::new(0.38, 0.40, 0.45, 1.0), flash));
    }
    draw_leg(0);
    draw_leg(2);
    let h = v.spine[0];
    // Spitter's throat sac swells as its acid rebuilds, glowing when ready.
    if f.kind == Kind::Spitter && alive {
        let r = 0.8 + f.saliva * 1.4;
        let a = if f.saliva >= 1.0 { 0.9 } else { 0.45 };
        draw_circle(h.x - s * 0.5, h.y + 2.5, r, Color::new(ACID.r * 0.8, ACID.g * 0.8, ACID.b, a));
    }
    // Head: glow, skull, jaws.
    if alive {
        draw_circle(h.x, h.y, 6.0, Color::new(head_c.r, head_c.g, head_c.b, 0.08));
    }
    let open = match f.atk_timer {
        _ if v.since_ranged < 0.2 => 2.6,
        0 => 0.4,
        t if t <= ATK_WINDUP => 2.2,
        t if t <= ATK_WINDUP + ATK_ACTIVE => 0.0,
        _ => 0.8,
    };
    let hc = tint(head_c, flash);
    if open > 1.0 {
        let tooth = Color::new(0.95, 0.92, 0.85, 1.0);
        draw_rectangle((h.x + s * 3.0).round(), (h.y - open + 0.5).round(), 1.0, 1.0, tooth);
        draw_rectangle((h.x + s * 3.0).round(), (h.y + open).round(), 1.0, 1.0, tooth);
    }
    line(h, h + vec2(s * 4.5, -0.5 - open), 2.0, hc);
    line(h, h + vec2(s * 4.0, 1.0 + open), 1.5, hc);
    draw_circle(h.x, h.y, 2.8, hc);
}
