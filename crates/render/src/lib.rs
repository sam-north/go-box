//! Procedural pixel art in a Rain World / Noita spirit: everything is generated in
//! code at arena resolution (480x270) — grimy industrial backdrop, mossy brick, rain, procedurally
//! animated bodies, items, explosions, and blood/scorch pixels that stick to the terrain.

pub mod sound;

use sound::Sfx;

use go_box_sim::character::*;
use go_box_sim::sim::*;
use macroquad::prelude::*;

const CLOAK: Color = Color::new(0.27, 0.28, 0.33, 1.0);
const HOOD: Color = Color::new(0.15, 0.16, 0.19, 1.0);
const FACE: Color = Color::new(0.84, 0.78, 0.69, 1.0);
const SHAFT: Color = Color::new(0.47, 0.39, 0.27, 1.0);
const TIP: Color = Color::new(0.85, 0.85, 0.8, 1.0);
const ROCK: Color = Color::new(0.50, 0.48, 0.44, 1.0);
const GRENADE: Color = Color::new(0.30, 0.36, 0.22, 1.0);
const HEAL: Color = Color::new(0.38, 0.86, 1.0, 1.0);
const BLOOD: Color = Color::new(0.50, 0.06, 0.08, 1.0);
const FIRE: Color = Color::new(1.0, 0.62, 0.18, 1.0);
const SCORCH: Color = Color::new(0.08, 0.07, 0.06, 0.9);

/// Scarf colour per fighter slot, so four identical humans can be told apart. Also used by the HUD.
pub const ACCENTS: [Color; N_FIGHTERS] = [
    Color::new(0.70, 0.26, 0.18, 1.0),
    Color::new(0.24, 0.46, 0.80, 1.0),
    Color::new(0.82, 0.68, 0.24, 1.0),
    Color::new(0.58, 0.36, 0.72, 1.0),
];

const SKIN: Color = Color::new(0.86, 0.70, 0.58, 1.0);
const DOG_FUR: Color = Color::new(0.55, 0.38, 0.22, 1.0);
const DOG_DARK: Color = Color::new(0.28, 0.19, 0.12, 1.0);
const GRAV: Color = Color::new(0.45, 0.85, 0.55, 1.0);
const SMOKE: Color = Color::new(0.75, 0.74, 0.70, 0.8);

/// How a character looks as drawn: Classic is the hooded figure the brains were trained as;
/// the named characters show their hair and wear their colour.
pub struct Look {
    pub body: Color,
    /// Back and front leg.
    pub legs: [Color; 2],
    pub arms: Color,
    /// None = hooded (Classic).
    pub hair: Option<Color>,
    /// Scarf, health bar and UI colour.
    pub accent: Color,
    /// Tyler's running sweatband, in his accent colour.
    pub headband: bool,
}

/// How a character looks and sounds: one row per character in `art`.
pub struct CharacterArt {
    /// Its colour in menus and the HUD, and on its scarf and health bar. None: it wears its
    /// seat's colour instead (Classic, so four of them can be told apart).
    pub color: Option<Color>,
    pub body: Color,
    pub legs: [Color; 2],
    pub arms: Color,
    pub hair: Option<Color>,
    pub headband: bool,
    /// Played when it uses its special (None: the special's own effects make the sound).
    pub special_sound: Option<Sfx>,
}

pub fn art(c: Character) -> &'static CharacterArt {
    match c {
        Character::Classic => &CharacterArt { color: None, body: CLOAK, legs: [HOOD, CLOAK], arms: HOOD, hair: None, headband: false, special_sound: None },
        // Brown hair, green-grey jacket; forest green.
        Character::Sam => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.16, 0.50, 0.24, 1.0)),
                body: rgb(62, 78, 64),
                legs: [rgb(32, 36, 44), rgb(44, 48, 58)],
                arms: rgb(50, 64, 52),
                hair: Some(rgb(96, 62, 36)),
                headband: false,
                special_sound: Some(Sfx::BombThrow),
            };
            &ART
        }
        // Reddish-brown hair, work jacket; gold. (The pop-up's poof is its sound.)
        Character::Austin => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.93, 0.72, 0.18, 1.0)),
                body: rgb(84, 76, 66),
                legs: [rgb(38, 40, 50), rgb(50, 52, 64)],
                arms: rgb(70, 63, 55),
                hair: Some(rgb(128, 58, 34)),
                headband: false,
                special_sound: None,
            };
            &ART
        }
        // Blondish-brown hair, light running top, sweatband; runner's neon volt.
        Character::Tyler => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.80, 1.0, 0.16, 1.0)),
                body: rgb(118, 122, 130),
                legs: [rgb(30, 32, 40), rgb(40, 42, 52)],
                arms: SKIN,
                hair: Some(rgb(168, 132, 84)),
                headband: true,
                special_sound: Some(Sfx::Hounds),
            };
            &ART
        }
    }
}

/// The colour that stands for a character in menus and the HUD (a neutral grey for one that
/// wears its seat's colour).
pub fn character_color(c: Character) -> Color {
    art(c).color.unwrap_or(Color::new(0.62, 0.64, 0.70, 1.0))
}

pub fn look(c: Character, seat: usize) -> Look {
    let a = art(c);
    Look { body: a.body, legs: a.legs, arms: a.arms, hair: a.hair, accent: a.color.unwrap_or(ACCENTS[seat]), headband: a.headband }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
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

fn is_metal(a: &Arena, x: i32, y: i32) -> bool {
    let p = vec2(x as f32 + 0.5, y as f32 + 0.5);
    a.platforms.iter().any(|r| r.contains(p))
}

fn solid_px(a: &Arena, x: i32, y: i32) -> bool {
    is_stone(x, y) || is_metal(a, x, y)
}

fn make_background() -> Image {
    let mut img = Image::gen_image_color(W as u16, H as u16, BLACK);
    // Laid out on a 320x180 grid, stretched to the arena.
    let (sx, sy) = (W / 320.0, H / 180.0);
    let px = |v: u32| (v as f32 * sx) as u32;
    let py = |v: u32| (v as f32 * sy) as u32;
    let pipes_v = [(22, 31), (70, 74), (142, 154), (222, 228), (276, 287)].map(|(a, b)| (px(a), px(b)));
    let pipes_h = [(36, 41, 0, 160), (100, 103, 120, 320), (60, 62, 180, 320)].map(|(a, b, c, d)| (py(a), py(b), px(c), px(d)));
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
            let shaft_d = ((fx - fy * 0.6) - 80.0 * sx).abs();
            let shaft = if shaft_d < 40.0 * sx { (1.0 - shaft_d / (40.0 * sx)) * 10.0 * (1.0 - t) } else { 0.0 };
            img.set_pixel(x, y, dither(x, y, r + grime + streak + shaft, g + grime + streak + shaft * 1.1, b + grime + streak + shaft * 1.2));
        }
    }
    img
}

fn make_terrain(a: &Arena) -> Image {
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
                let air_side = !solid_px(a, x - 1, y) && x > 0 || !solid_px(a, x + 1, y) && x < W as i32 - 1 || !solid_px(a, x, y + 1) && y < H as i32 - 1;
                if air_side && !exposed_top {
                    r *= 0.55;
                    g *= 0.55;
                    b *= 0.55;
                }
                img.set_pixel(ux, uy, dither(ux, uy, r, g, b));
            } else if is_metal(a, x, y) {
                let top = !is_metal(a, x, y - 1);
                let bottom = !is_metal(a, x, y + 1);
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
                let mid = is_metal(a, x, y - 2) && is_metal(a, x, y + 2) && !is_metal(a, x, y - 3);
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
    /// Seconds since this fighter last threw (drives the follow-through pose).
    since_throw: f32,
    scarf: [Vec2; 6],
    scarf_prev: [Vec2; 6],
}

pub struct Art {
    bg: Texture2D,
    terrain: Texture2D,
    /// The map the terrain texture shows.
    arena: ArenaId,
    vignette: Texture2D,
    stain_img: Image,
    stain_tex: Texture2D,
    stain_dirty: bool,
    particles: Vec<Particle>,
    rain: Vec<Drop>,
    vis: [FighterVis; N_FIGHTERS],
    /// Recent explosions (position, seconds since) for the flash.
    blasts: Vec<(Vec2, f32, f32)>,
    /// A flag in their colour where each fighter went down (ground position, colour).
    flags: Vec<(Vec2, Color)>,
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
        let rain = (0..200)
            .map(|i| Drop { pos: vec2(hash(i, 0, 1) * W, hash(i, 1, 1) * H), speed: 200.0 + hash(i, 2, 1) * 120.0 })
            .collect();
        Self {
            bg: tex(&make_background()),
            terrain: tex(&make_terrain(ArenaId::default().def())),
            arena: ArenaId::default(),
            vignette: tex(&make_vignette()),
            stain_tex: tex(&stain_img),
            stain_img,
            stain_dirty: false,
            particles: Vec::new(),
            rain,
            vis: Default::default(),
            blasts: Vec::new(),
            flags: Vec::new(),
            time: 0.0,
        }
    }

    /// Draws `arena`'s platforms from now on (rebuilding the terrain if it changed).
    pub fn set_arena(&mut self, arena: ArenaId) {
        if arena != self.arena {
            self.arena = arena;
            self.terrain = tex(&make_terrain(arena.def()));
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.stain_img = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        self.stain_dirty = true;
        self.particles.clear();
        self.blasts.clear();
        self.flags.clear();
        self.vis = Default::default();
    }

    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, pos: Vec2, dir: Vec2, spread: f32, n: usize, color: Color, stick: bool, speed: f32, life: f32, gravity: f32) {
        for i in 0..n {
            let seed = (self.time * 1000.0) as i32 + i as i32 * 7 + self.particles.len() as i32;
            let a = (hash(seed, 0, 31) - 0.5) * spread;
            let s = speed * (0.3 + hash(seed, 1, 31) * 0.9);
            let v = vec2(dir.x * a.cos() - dir.y * a.sin(), dir.x * a.sin() + dir.y * a.cos()) * s;
            let shade = 0.75 + hash(seed, 2, 31) * 0.35;
            let c = Color::new(color.r * shade, color.g * shade, color.b * shade, color.a);
            let life = life * (0.5 + hash(seed, 3, 31));
            self.particles.push(Particle { pos, vel: v, color: c, life, stick, gravity });
        }
    }

    pub fn handle_events(&mut self, m: &Match) {
        for e in &m.events {
            match *e {
                Event::Hit { pos, dir, headshot, .. } => {
                    let n = if headshot { 40 } else { 22 };
                    self.burst(pos, vec2(dir, -0.5).normalize(), 1.6, n, BLOOD, true, 140.0, 2.5, 420.0);
                    let spark = if headshot { Color::new(1.0, 0.95, 0.5, 1.0) } else { Color::new(1.0, 0.95, 0.85, 1.0) };
                    self.burst(pos, vec2(dir, 0.0), 2.5, if headshot { 14 } else { 6 }, spark, false, 120.0, 0.4, 420.0);
                }
                Event::Death { who, pos } => {
                    // Planted on whatever is below the spot they died.
                    let ground = pos + vec2(0.0, terrain_ray(m.arena(), pos, vec2(0.0, 1.0), H));
                    self.flags.push((ground, look(m.f[who].character, who).accent));
                    self.burst(pos, vec2(0.0, -1.0), 6.0, 70, BLOOD, true, 160.0, 2.5, 420.0);
                }
                Event::Jump { pos, .. } | Event::Land { pos } => {
                    self.burst(pos + vec2(0.0, -1.0), vec2(0.0, -1.0), 3.0, 4, Color::new(0.45, 0.43, 0.40, 0.8), false, 40.0, 0.5, 420.0);
                }
                Event::Throw { who } => self.vis[who].since_throw = 0.0,
                Event::Grab { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 2.0, 3, Color::new(0.6, 0.58, 0.52, 0.8), false, 25.0, 0.3, 200.0);
                }
                Event::Explosion { pos } => {
                    self.blasts.push((pos, 0.0, GRENADE_RADIUS));
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 60, FIRE, false, 160.0, 0.5, 120.0);
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 40, Color::new(1.0, 0.92, 0.6, 1.0), false, 90.0, 0.25, 0.0);
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 50, SCORCH, true, 120.0, 1.5, 300.0);
                    self.burst(pos, vec2(0.0, -1.0), 2.0, 30, Color::new(0.25, 0.24, 0.24, 0.5), false, 30.0, 1.5, -25.0);
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
                Event::Spawn { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.0, 8, Color::new(0.8, 0.85, 0.9, 0.7), false, 30.0, 0.5, 60.0);
                }
                Event::Special { who } => {
                    let c = character_color(m.f[who].character);
                    self.burst(m.f[who].center(), vec2(0.0, -1.0), 6.3, 14, c, false, 50.0, 0.35, 0.0);
                }
                Event::Poof { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 26, SMOKE, false, 45.0, 0.6, -30.0);
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 10, character_color(Character::Austin), false, 70.0, 0.3, 0.0);
                }
                Event::BombCrunch { pos } => {
                    // Debris sucked in from all over the map, thickest near the bomb.
                    for i in 0..70 {
                        let seed = (self.time * 1000.0) as i32 + i * 17;
                        let a = hash(seed, 0, 61) * std::f32::consts::TAU;
                        let r = BOMB_PULL_RADIUS * (0.4 + hash(seed, 1, 61).powi(2) * 5.0);
                        let from = pos + vec2(a.cos(), a.sin()) * r;
                        let life = BOMB_CRUNCH as f32 * DT;
                        self.particles.push(Particle { pos: from, vel: (pos - from) / life, color: GRAV, life, stick: false, gravity: 0.0 });
                    }
                }
                Event::BombBlast { pos } => {
                    self.blasts.push((pos, 0.0, BOMB_BLAST_RADIUS));
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 50, GRAV, false, 170.0, 0.45, 0.0);
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 30, Color::new(0.85, 1.0, 0.9, 1.0), false, 90.0, 0.25, 0.0);
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 30, SCORCH, true, 110.0, 1.5, 300.0);
                }
                Event::Bite { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 2.0, 6, Color::new(0.9, 0.9, 0.85, 0.9), false, 50.0, 0.2, 200.0);
                }
                Event::Swing { .. } | Event::Clatter { .. } => {}
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
        for b in self.blasts.iter_mut() {
            b.1 += dt;
        }
        self.blasts.retain(|b| b.1 < 0.25);
        // Lit grenade fuses throw off sparks.
        for it in m.items.iter().filter(|it| it.fuse.is_some()) {
            if hash((self.time * 977.0) as i32, it.pos.x as i32, 41) < dt * 30.0 {
                let c = Color::new(1.0, 0.8, 0.3, 1.0);
                self.particles.push(Particle { pos: it.pos + vec2(0.0, -2.0), vel: vec2(0.0, -20.0), color: c, life: 0.2, stick: false, gravity: 0.0 });
            }
        }
        let arena = self.arena.def();
        let mut stains = Vec::new();
        self.particles.retain_mut(|p| {
            p.vel.y += p.gravity * dt;
            let next = p.pos + p.vel * dt;
            p.life -= dt;
            if solid_px(arena, next.x as i32, next.y as i32) {
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
            if x >= 0 && y >= 0 && x < W as i32 && y < H as i32 && !solid_px(arena, x, y) {
                self.stain_img.set_pixel(x as u32, y as u32, c);
                self.stain_dirty = true;
            }
        }
        for (f, v) in m.f.iter().zip(self.vis.iter_mut()) {
            if !v.init {
                init_vis(v, f);
            }
            v.walk_phase += dt * f.vel.x.abs() * 0.28;
            v.since_throw += dt;
            update_scarf(v, f, dt, self.time);
        }
    }

    pub fn draw(&mut self, m: &Match) {
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

        for &(p, c) in &self.flags {
            draw_flag(p, c, self.time);
        }
        for &h in &m.heals {
            draw_heal(h, self.time);
        }
        for e in &m.effects {
            match e {
                Effect::Bomb(b) => draw_bomb(b, self.time),
                Effect::Dog(d) => draw_dog(d),
            }
        }
        // Loose and flying items (held spears are drawn with their holder).
        for it in m.items.iter().filter(|it| !(it.held_by.is_some() && it.kind == ItemKind::Spear)) {
            draw_item(it, self.time);
        }

        // Dead underneath the living.
        let mut order: Vec<usize> = (0..N_FIGHTERS).filter(|&i| !m.f[i].absent).collect();
        order.sort_by_key(|&i| m.f[i].alive());
        for i in order {
            let f = &m.f[i];
            let flash = f.hitstun > 0 && (f.hitstun / 2) % 2 == 0;
            draw_human(&self.vis[i], f, &look(f.character, i), flash);
        }
        // Held rocks/grenades sit in the hand, in front of the body.
        for it in m.items.iter().filter(|it| it.held_by.is_some() && it.kind != ItemKind::Spear) {
            draw_item(it, self.time);
        }

        for &(pos, t, r) in &self.blasts {
            let k = 1.0 - t / 0.25;
            draw_circle(pos.x, pos.y, r * (0.5 + 0.5 * (1.0 - k)), Color::new(1.0, 0.85, 0.5, 0.35 * k));
            draw_circle(pos.x, pos.y, r * 0.35, Color::new(1.0, 1.0, 0.85, 0.8 * k));
        }
        for p in &self.particles {
            draw_rectangle(p.pos.x.floor(), p.pos.y.floor(), 1.0, 1.0, p.color);
        }

        for (i, f) in m.f.iter().enumerate().filter(|(_, f)| f.alive()) {
            let (x, y) = ((f.pos.x - 6.0).round(), (f.pos.y - 22.0).round());
            draw_rectangle(x, y, 12.0, 1.0, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(x, y, (12.0 * f.hp / MAX_HP).ceil(), 1.0, look(f.character, i).accent);
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

fn draw_item(it: &Item, time: f32) {
    let p = it.pos;
    let (x, y) = (p.x.round() - 1.0, p.y.round() - 1.0);
    match it.kind {
        ItemKind::Rock => {
            draw_rectangle(x, y, 3.0, 3.0, ROCK);
            draw_rectangle(x, y, 1.0, 1.0, Color::new(0.68, 0.66, 0.62, 1.0));
            draw_rectangle(x + 2.0, y + 2.0, 1.0, 1.0, Color::new(0.30, 0.29, 0.27, 1.0));
        }
        ItemKind::Spear => {
            // Lying flat on the ground, or pointing along its flight.
            let dir = if it.vel.length() > 20.0 { it.vel.normalize() } else { vec2(1.0, 0.0) };
            let base = p + vec2(0.0, ITEM_R - 0.5);
            line(base - dir * 6.0, base + dir * 4.0, 1.0, SHAFT);
            line(base + dir * 4.0, base + dir * 6.5, 1.0, TIP);
        }
        ItemKind::Grenade => {
            draw_rectangle(x, y, 3.0, 3.0, GRENADE);
            draw_rectangle(x, y, 1.0, 1.0, Color::new(0.45, 0.52, 0.35, 1.0));
            draw_rectangle(x + 1.0, y - 1.0, 1.0, 1.0, Color::new(0.35, 0.33, 0.30, 1.0));
            // A live fuse blinks red, faster as it burns down.
            if let Some(t) = it.fuse {
                let rate = 4.0 + 16.0 * (1.0 - t as f32 / GRENADE_FUSE as f32);
                if (time * rate).sin() > 0.0 {
                    draw_rectangle(x + 1.0, y + 1.0, 1.0, 1.0, Color::new(1.0, 0.15, 0.1, 1.0));
                    draw_circle(p.x, p.y, 3.5, Color::new(1.0, 0.2, 0.1, 0.25));
                }
            }
        }
    }
}

fn init_vis(v: &mut FighterVis, f: &Fighter) {
    v.init = true;
    v.since_throw = 10.0;
    let neck = vec2(f.pos.x, f.pos.y - 11.0);
    for k in 0..6 {
        v.scarf[k] = neck + vec2(-f.facing * k as f32 * 1.5, 0.0);
        v.scarf_prev[k] = v.scarf[k];
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

fn draw_human(v: &FighterVis, f: &Fighter, look: &Look, flash: bool) {
    let s = f.facing;
    let (x, fy) = (f.pos.x, f.pos.y);
    let scarf = look.accent;
    if !f.alive() {
        line(vec2(x - 6.0 * s, fy - 1.5), vec2(x + 5.0 * s, fy - 1.5), 3.0, look.body);
        draw_circle(x + 7.0 * s, fy - 2.0, 2.5, look.hair.unwrap_or(HOOD));
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
        let c = tint(look.legs[k], flash);
        line(hip, knee, 1.0, c);
        line(knee, foot, 1.0, c);
    }
    // Torso as a soft blob.
    for t in [0.0, 0.5, 1.0] {
        let p = hip.lerp(neck, t);
        draw_circle(p.x, p.y, 1.8, tint(look.body, flash));
    }
    let hand = if f.holding == Some(ItemKind::Spear) { draw_spear(f, neck, flash) } else { free_hand(f, v, neck) };
    let elbow = ik(shoulder, hand, 3.0, 3.5, s);
    line(shoulder, elbow, 1.0, tint(look.arms, flash));
    line(elbow, hand, 1.0, tint(look.arms, flash));
    match look.hair {
        None => {
            // Hooded head with a pale face peeking out.
            draw_circle(head.x, head.y, 2.6, tint(HOOD, flash));
            draw_rectangle((head.x + s * 1.0 - 0.5).round(), head.y.round() - 1.0, 2.0, 2.0, tint(FACE, flash));
        }
        Some(hair) => {
            // Bare head: hair over the top and down the back of the head.
            let (hx, hy) = (head.x.round(), head.y.round());
            draw_circle(head.x, head.y, 2.4, tint(SKIN, flash));
            draw_rectangle(hx - 2.0, hy - 3.0, 5.0, 2.0, tint(hair, flash));
            draw_rectangle(if s > 0.0 { hx - 3.0 } else { hx + 2.0 }, hy - 2.0, 2.0, 3.0, tint(hair, flash));
            if look.headband {
                draw_rectangle(hx - 2.0, hy - 2.0, 5.0, 1.0, tint(look.accent, flash));
            }
        }
    }
    draw_rectangle((head.x + s * 1.5).round(), head.y.round() - 1.0, 1.0, 1.0, Color::new(0.05, 0.05, 0.06, 1.0));
}

/// One of Tyler's dogs, mid-gallop.
fn draw_dog(d: &Dog) {
    let (x, y, s) = (d.pos.x.round(), d.pos.y.round(), d.dir);
    let stride = ((d.pos.x * 0.6).sin() > 0.0) as u8 as f32;
    // Front and back legs take turns lifting.
    for (lx, up) in [(-2.0, stride), (1.0, 1.0 - stride)] {
        draw_rectangle(x + lx * s, y - 2.0, 1.0, 2.0 - up, DOG_DARK);
    }
    draw_rectangle(x - 3.0, y - 4.0, 6.0, 2.0, DOG_FUR);
    // Head and ear out front, tail up behind.
    let hx = if s > 0.0 { x + 2.0 } else { x - 4.0 };
    draw_rectangle(hx, y - 6.0, 2.0, 2.0, DOG_FUR);
    draw_rectangle(if s > 0.0 { hx } else { hx + 1.0 }, y - 7.0, 1.0, 1.0, DOG_DARK);
    draw_rectangle(if s > 0.0 { hx + 1.0 } else { hx }, y - 6.0, 1.0, 1.0, Color::new(0.05, 0.05, 0.06, 1.0));
    draw_rectangle(if s > 0.0 { x - 4.0 } else { x + 3.0 }, y - 5.0 - stride, 1.0, 1.0, DOG_FUR);
}

/// Sam's gravity bomb: a dark, pulsing orb in flight; once it goes off, a ring collapsing from
/// the edge of its reach onto the core.
fn draw_bomb(b: &Bomb, time: f32) {
    let c = b.pos;
    match b.crunch {
        None => {
            draw_circle(c.x, c.y, 4.0 + (time * 30.0).sin() * 0.7, Color::new(GRAV.r, GRAV.g, GRAV.b, 0.25));
            draw_circle(c.x, c.y, 2.0, Color::new(0.04, 0.08, 0.06, 1.0));
            draw_rectangle(c.x.floor(), c.y.floor() - 1.0, 1.0, 1.0, GRAV);
        }
        Some(left) => {
            let k = left as f32 / BOMB_CRUNCH as f32;
            // The pull reaches the whole map: faint rings sweep in from beyond the walls.
            for (r0, a) in [(W, 0.18), (W * 0.6, 0.28)] {
                draw_circle_lines(c.x, c.y, r0 * k, 1.0, Color::new(GRAV.r, GRAV.g, GRAV.b, a * (1.0 - k * 0.5)));
            }
            draw_circle(c.x, c.y, BOMB_PULL_RADIUS * k, Color::new(GRAV.r * 0.3, GRAV.g * 0.3, GRAV.b * 0.3, 0.18 * (1.0 - k)));
            draw_circle_lines(c.x, c.y, BOMB_PULL_RADIUS * k, 1.0, Color::new(GRAV.r, GRAV.g, GRAV.b, 0.9));
            draw_circle(c.x, c.y, 3.0 + 3.0 * (1.0 - k), Color::new(0.02, 0.05, 0.03, 0.95));
            draw_circle_lines(c.x, c.y, 3.5 + 3.0 * (1.0 - k), 1.0, GRAV);
        }
    }
}

/// Draws a held spear for the current swing phase and returns where the hand is.
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
    line(hand + dir * 9.0, hand + dir * 11.5, 1.0, tint(TIP, flash));
    hand
}

/// Where the hand is when not holding a spear: cocked overhead with a rock or grenade,
/// punching, following through after a throw, or hanging loose.
fn free_hand(f: &Fighter, v: &FighterVis, neck: Vec2) -> Vec2 {
    let s = f.facing;
    let (x, fy) = (f.pos.x, f.pos.y);
    if v.since_throw < 0.15 {
        return vec2(x + s * 6.0, fy - 10.0);
    }
    match f.atk_timer {
        t if t > 0 && t <= ATK_WINDUP => vec2(x - s * 1.0, fy - 8.0),
        t if t > ATK_WINDUP && t <= ATK_WINDUP + ATK_ACTIVE => vec2(x + s * 7.0, fy - 8.0),
        _ if f.holding.is_some() => f.hand(),
        _ => neck + vec2(s * 1.5, 5.0),
    }
}

/// A character standing still, feet at (0, 0) in arena pixels, facing right; for menus. The
/// caller sets up a camera (e.g. onto a small render target) around it.
pub fn draw_portrait(c: Character, seat: usize, time: f32) {
    let mut f = Fighter::new(0.0, 1.0);
    f.pos = vec2(0.0, 0.0);
    f.character = c;
    let mut v = FighterVis::default();
    init_vis(&mut v, &f);
    // Let the scarf settle into a natural hang.
    for k in 0..40 {
        update_scarf(&mut v, &f, DT, time - (40 - k) as f32 * DT);
    }
    draw_human(&v, &f, &look(c, seat), false);
}

/// A little flag on a pole, rippling in the wind, planted where a fighter went down.
fn draw_flag(p: Vec2, c: Color, time: f32) {
    let (x, y) = (p.x.round(), p.y.round());
    draw_rectangle(x, y - 13.0, 1.0, 13.0, Color::new(0.55, 0.50, 0.42, 1.0));
    draw_rectangle(x, y - 14.0, 1.0, 1.0, Color::new(0.8, 0.75, 0.6, 1.0));
    for k in 0..6 {
        let wave = ((time * 5.0 - k as f32 * 0.9 + p.x).sin() * 0.8).round();
        let h = 5.0 - k as f32 * 0.5;
        let shade = if wave > 0.0 { 0.8 } else { 1.0 };
        draw_rectangle(x + 1.0 + k as f32, y - 13.0 + wave + (5.0 - h) / 2.0, 1.0, h.round(), Color::new(c.r * shade, c.g * shade, c.b * shade, 1.0));
    }
}
