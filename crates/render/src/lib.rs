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
const RICE: Color = Color::new(0.96, 0.95, 0.90, 1.0);
/// Jake's hologram light, and how long the beam lasts going into or out of the phone.
const HOLO: Color = Color::new(0.35, 0.95, 1.0, 1.0);
/// Kyle's pin glow, and Robert's lightning.
const PIN: Color = Color::new(0.55, 1.0, 0.75, 1.0);
const BOLT: Color = Color::new(0.75, 0.85, 1.0, 1.0);
const HOLO_PINK: Color = Color::new(1.0, 0.45, 0.85, 1.0);
const HOLO_SECS: f32 = 0.45;
const SALMON: Color = Color::new(1.0, 0.52, 0.34, 1.0);
const NORI: Color = Color::new(0.10, 0.16, 0.12, 1.0);
const WOOD: Color = Color::new(0.52, 0.34, 0.20, 1.0);
const WOOD_DARK: Color = Color::new(0.34, 0.22, 0.13, 1.0);
/// Cat coats: ginger, grey, black, cream, and a tabby brown.
const COATS: [(Color, Color); 5] = [
    (Color::new(0.90, 0.55, 0.22, 1.0), Color::new(0.70, 0.38, 0.14, 1.0)),
    (Color::new(0.58, 0.60, 0.64, 1.0), Color::new(0.40, 0.42, 0.46, 1.0)),
    (Color::new(0.14, 0.13, 0.15, 1.0), Color::new(0.05, 0.05, 0.06, 1.0)),
    (Color::new(0.92, 0.88, 0.78, 1.0), Color::new(0.75, 0.68, 0.56, 1.0)),
    (Color::new(0.55, 0.42, 0.30, 1.0), Color::new(0.32, 0.24, 0.17, 1.0)),
];
/// Sam's glue: off-white, a little yellow.
const GLUE: Color = Color::new(0.96, 0.93, 0.78, 1.0);
const SMOKE: Color = Color::new(0.75, 0.74, 0.70, 0.8);
/// The protected-start shield.
const SHIELD: Color = Color::new(0.55, 0.85, 1.0, 1.0);
/// The full-size picture a mini is drawn into (pixels), and how far down it its feet sit (0..1).
const MINI_SPRITE: (u32, u32) = (40, 48);
const MINI_FEET: f32 = 38.0 / 48.0;

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
    pub skin: Color,
    pub hair_style: HairStyle,
    pub beard: bool,
    pub glasses: bool,
}

/// How hair is cut (drawn over a bare head).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HairStyle {
    /// Over the top and down the back of the head.
    Short,
    /// Down past the neck to the shoulders (Corwin).
    Long,
    /// Straight, with a fringe, covering the ears (Justin).
    OverEars,
    /// Swept down over one eye, longer at the back (Jake).
    Emo,
    /// Shaved to stubble (Robert).
    Shaved,
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
    pub skin: Color,
    pub hair_style: HairStyle,
    pub beard: bool,
    pub glasses: bool,
    /// Played when it uses its special (None: the special's own effects make the sound).
    pub special_sound: Option<Sfx>,
}

pub fn art(c: Character) -> &'static CharacterArt {
    match c {
        Character::Classic => &CharacterArt { color: None, body: CLOAK, legs: [HOOD, CLOAK], arms: HOOD, hair: None, headband: false, skin: SKIN, hair_style: HairStyle::Short, beard: false, glasses: false, special_sound: None },
        // Brown hair, green-grey jacket; forest green.
        Character::Sam => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.16, 0.50, 0.24, 1.0)),
                body: rgb(62, 78, 64),
                legs: [rgb(32, 36, 44), rgb(44, 48, 58)],
                arms: rgb(50, 64, 52),
                hair: Some(rgb(96, 62, 36)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                special_sound: Some(Sfx::GlueShot),
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
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
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
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                special_sound: Some(Sfx::Hounds),
            };
            &ART
        }
        // Near-black hair, charcoal hoodie; teal. (His minis are drawn from the same look.)
        Character::Seth => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.20, 0.78, 0.85, 1.0)),
                body: rgb(58, 60, 66),
                legs: [rgb(30, 32, 38), rgb(42, 44, 52)],
                arms: rgb(50, 52, 58),
                hair: Some(rgb(34, 28, 24)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                special_sound: Some(Sfx::Split),
            };
            &ART
        }
        // Long black hair and a full beard (no braids), a leather jerkin; crimson.
        Character::Corwin => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.86, 0.22, 0.24, 1.0)),
                body: rgb(98, 70, 48),
                legs: [rgb(40, 36, 34), rgb(54, 48, 44)],
                arms: rgb(84, 60, 42),
                hair: Some(rgb(22, 20, 22)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Long,
                beard: true,
                glasses: false,
                special_sound: Some(Sfx::Meow),
            };
            &ART
        }
        // Pale, straight blonde hair over the ears, a light shirt; violet.
        Character::Justin => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.68, 0.48, 1.0, 1.0)),
                body: rgb(196, 198, 206),
                legs: [rgb(46, 50, 66), rgb(60, 64, 82)],
                arms: rgb(176, 178, 188),
                hair: Some(rgb(226, 196, 112)),
                headband: false,
                skin: rgb(240, 214, 196),
                hair_style: HairStyle::OverEars,
                beard: false,
                glasses: false,
                special_sound: Some(Sfx::Serve),
            };
            &ART
        }
        // Glasses, short brown hair, a hoodie; orange.
        Character::Ryan => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(1.0, 0.50, 0.12, 1.0)),
                body: rgb(70, 74, 86),
                legs: [rgb(36, 38, 46), rgb(48, 50, 60)],
                arms: rgb(60, 64, 74),
                hair: Some(rgb(112, 76, 46)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: true,
                special_sound: Some(Sfx::ChainThrow),
            };
            &ART
        }
        // Shaved head, a dark work jacket; electric blue.
        Character::Robert => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.38, 0.62, 1.0, 1.0)),
                body: rgb(48, 54, 72),
                legs: [rgb(32, 34, 42), rgb(44, 46, 56)],
                arms: rgb(56, 62, 82),
                hair: Some(rgb(164, 132, 112)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Shaved,
                beard: false,
                glasses: false,
                special_sound: Some(Sfx::Thunder),
            };
            &ART
        }
        // Dirty-blonde hair, a tidy polo; mint.
        Character::Kyle => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.45, 0.95, 0.70, 1.0)),
                body: rgb(88, 110, 98),
                legs: [rgb(58, 52, 44), rgb(72, 66, 56)],
                arms: SKIN,
                hair: Some(rgb(196, 166, 102)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                special_sound: Some(Sfx::Organize),
            };
            &ART
        }
        // Blonde emo fringe over his eyes, a black hoodie; hot pink.
        Character::Jake => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(1.0, 0.38, 0.72, 1.0)),
                body: rgb(30, 30, 36),
                legs: [rgb(22, 22, 28), rgb(34, 34, 42)],
                arms: rgb(40, 40, 48),
                hair: Some(rgb(236, 214, 140)),
                headband: false,
                skin: rgb(232, 200, 178),
                hair_style: HairStyle::Emo,
                beard: false,
                glasses: false,
                special_sound: None,
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
    Look { body: a.body, legs: a.legs, arms: a.arms, hair: a.hair, accent: a.color.unwrap_or(ACCENTS[seat]), headband: a.headband, skin: a.skin, hair_style: a.hair_style, beard: a.beard, glasses: a.glasses }
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

#[derive(Default, Clone)]
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
    vis: [FighterVis; N_BODIES],
    /// Seth's minis are drawn full size into these, then shrunk onto the arena (drawing them
    /// straight at a smaller size would lose their 1px limbs).
    minis: Vec<RenderTarget>,
    /// Recent explosions (position, seconds since) for the flash.
    blasts: Vec<(Vec2, f32, f32)>,
    /// Jake's hologram beams: (his middle, seconds since, coming out?).
    holos: Vec<(Vec2, f32, bool)>,
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
            minis: (0..MAX_MINIS)
                .map(|_| {
                    let t = render_target(MINI_SPRITE.0, MINI_SPRITE.1);
                    t.texture.set_filter(FilterMode::Linear);
                    t
                })
                .collect(),
            blasts: Vec::new(),
            holos: Vec::new(),
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
        self.holos.clear();
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
                    // (Players and bots get a flag; minis just vanish.)
                    if who < N_FIGHTERS {
                        let ground = pos + vec2(0.0, terrain_ray(m.arena(), pos, vec2(0.0, 1.0), H));
                        self.flags.push((ground, look(m.f[who].character, who).accent));
                    }
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
                Event::Glued { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 12, GLUE, true, 50.0, 0.6, 300.0);
                }
                Event::Bite { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 2.0, 6, Color::new(0.9, 0.9, 0.85, 0.9), false, 50.0, 0.2, 200.0);
                }
                Event::Scratch { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 2.0, 3, Color::new(0.95, 0.85, 0.80, 0.9), false, 40.0, 0.15, 200.0);
                }
                Event::Hologram { pos, out } => {
                    self.holos.push((pos, 0.0, out));
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 18, HOLO, false, 40.0, 0.5, -20.0);
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 8, HOLO_PINK, false, 30.0, 0.4, -20.0);
                }
                Event::Zap { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 10, BOLT, false, 90.0, 0.2, 0.0);
                }
                Event::Hook { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 8, Color::new(1.0, 0.95, 0.8, 1.0), false, 90.0, 0.2, 0.0);
                }
                Event::Splat { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 3.0, 10, RICE, true, 70.0, 0.5, 380.0);
                    self.burst(pos, vec2(0.0, -1.0), 3.0, 5, SALMON, true, 60.0, 0.5, 380.0);
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
        for h in self.holos.iter_mut() {
            h.1 += dt;
        }
        self.holos.retain(|h| h.1 < HOLO_SECS);
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
            if f.absent {
                // An empty mini slot: start fresh when the next one appears.
                *v = FighterVis::default();
                continue;
            }
            if !v.init {
                init_vis(v, f);
            }
            v.walk_phase += dt * f.vel.x.abs() * 0.28;
            v.since_throw += dt;
            update_scarf(v, f, dt, self.time);
        }
    }

    /// Draws each of Seth's minis full size into its own picture, for `draw` to shrink onto the
    /// arena. Call before setting the arena camera (it sets its own).
    pub fn draw_minis(&self, m: &Match) {
        for (k, f) in m.f[N_FIGHTERS..].iter().enumerate() {
            if f.absent {
                continue;
            }
            let (w, h) = (MINI_SPRITE.0 as f32, MINI_SPRITE.1 as f32);
            let mut cam = Camera2D { target: vec2(0.0, h * (0.5 - MINI_FEET)), zoom: vec2(2.0 / w, 2.0 / h), ..Default::default() };
            cam.render_target = Some(self.minis[k].clone());
            set_camera(&cam);
            clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
            // The same fighter at full size, standing on the picture's origin.
            let mut big = f.clone();
            big.pos = Vec2::ZERO;
            big.scale = 1.0;
            let mut v = self.vis[N_FIGHTERS + k].clone();
            for p in v.scarf.iter_mut() {
                *p -= f.pos;
            }
            let flash = f.hitstun > 0 && (f.hitstun / 2) % 2 == 0;
            draw_human(&v, &big, &look(f.character, f.team(N_FIGHTERS + k)), flash);
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
                Effect::Glue(g) => draw_glue(g, m, self.time),
                Effect::Pet(p) => match p.breed {
                    Breed::Hound => draw_dog(p),
                    Breed::Cat(coat) => draw_cat(p, coat),
                },
                Effect::Table(t) => draw_table(t),
                Effect::Chain(c) => draw_chain(c, chain_anchor(&m.f[c.owner])),
                // (Drawn over the fighters, below.)
                Effect::Lightning(_) => {}
                Effect::Order(_) => {}
            }
        }
        // Loose and flying items (held spears are drawn with their holder).
        for it in m.items.iter().filter(|it| !(it.held_by.is_some() && it.kind == ItemKind::Spear)) {
            draw_item(it, self.time);
        }

        // Kyle's pin: a glowing ring at the feet of everyone stuck. Tyler's haste: speed streaks.
        for f in m.f.iter().filter(|f| f.present()) {
            if f.pinned > 0 {
                let pulse = 0.5 + 0.5 * (self.time * 8.0).sin();
                let fade = (f.pinned as f32 / 20.0).min(1.0);
                let w = 9.0 * f.scale + pulse;
                draw_ellipse(f.pos.x, f.pos.y - 0.5, w + 3.0, 4.0, 0.0, Color::new(PIN.r, PIN.g, PIN.b, 0.15 * fade));
                draw_ellipse(f.pos.x, f.pos.y - 0.5, w, 3.0, 0.0, Color::new(PIN.r, PIN.g, PIN.b, 0.4 * fade));
                draw_ellipse_lines(f.pos.x, f.pos.y - 0.5, w, 3.0, 0.0, 1.0, Color::new(PIN.r, PIN.g, PIN.b, fade));
                draw_ellipse_lines(f.pos.x, f.pos.y - 0.5, w + 3.0, 4.0, 0.0, 1.0, Color::new(PIN.r, PIN.g, PIN.b, 0.5 * pulse * fade));
                for k in 0..3 {
                    let x = f.pos.x + (k as f32 - 1.0) * 5.0 * f.scale;
                    let h = 3.0 + 3.0 * ((self.time * 6.0 + k as f32 * 2.1).sin() * 0.5 + 0.5);
                    draw_rectangle(x.round(), (f.pos.y - h).round(), 1.0, h.round(), Color::new(PIN.r, PIN.g, PIN.b, 0.75 * fade));
                }
            }
            if f.haste > 0 && f.vel.x.abs() > 60.0 {
                let a = look(f.character, 0).accent;
                let back = -f.vel.x.signum();
                for (k, dy) in [3.0, 7.0, 11.0].into_iter().enumerate() {
                    let len = 4.0 + 3.0 * ((self.time * 20.0 + k as f32).sin() * 0.5 + 0.5);
                    let x0 = f.pos.x + back * (5.0 + k as f32);
                    draw_line(x0, f.pos.y - dy, x0 + back * len, f.pos.y - dy, 1.0, Color::new(a.r, a.g, a.b, 0.5));
                }
            }
        }

        // Dead underneath the living.
        let mut order: Vec<usize> = (0..N_BODIES).filter(|&i| !m.f[i].absent && m.f[i].digital == 0).collect();
        order.sort_by_key(|&i| m.f[i].alive());
        for i in order {
            let f = &m.f[i];
            if i >= N_FIGHTERS {
                // A mini: its full-size picture (from `update`), shrunk to its size.
                let (w, h) = (MINI_SPRITE.0 as f32 * f.scale, MINI_SPRITE.1 as f32 * f.scale);
                let params = DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() };
                draw_texture_ex(&self.minis[i - N_FIGHTERS].texture, f.pos.x - w / 2.0, f.pos.y - h * MINI_FEET, WHITE, params);
                continue;
            }
            let flash = f.hitstun > 0 && (f.hitstun / 2) % 2 == 0;
            draw_human(&self.vis[i], f, &look(f.character, i), flash);
        }
        for e in &m.effects {
            if let Effect::Lightning(l) = e {
                draw_lightning(l, self.time);
            }
            if let Effect::Order(o) = e {
                draw_order(o, &m.f[o.owner], self.time);
            }
        }
        // The protected start: a pulsing shield around everyone, fading out as it ends.
        if m.protected() {
            let left = (m.config.countdown + m.config.spawn_grace - m.tick) as f32 * DT;
            let fade = (left / 0.5).min(1.0);
            let pulse = 0.5 + 0.5 * (self.time * 7.0).sin();
            for f in m.f.iter().filter(|f| f.present()) {
                let c = f.center();
                let a = (0.25 + 0.35 * pulse) * fade;
                draw_circle(c.x, c.y, 11.0 + pulse, Color::new(SHIELD.r, SHIELD.g, SHIELD.b, a * 0.25));
                draw_circle_lines(c.x, c.y, 11.0 + pulse, 1.0, Color::new(SHIELD.r, SHIELD.g, SHIELD.b, a));
            }
        }
        // Held rocks/grenades sit in the hand, in front of the body.
        for it in m.items.iter().filter(|it| it.held_by.is_some() && it.kind != ItemKind::Spear) {
            draw_item(it, self.time);
        }

        for &(pos, t, out) in &self.holos {
            draw_hologram(pos, t, out);
        }
        for &(pos, t, r) in &self.blasts {
            let k = 1.0 - t / 0.25;
            draw_circle(pos.x, pos.y, r * (0.5 + 0.5 * (1.0 - k)), Color::new(1.0, 0.85, 0.5, 0.35 * k));
            draw_circle(pos.x, pos.y, r * 0.35, Color::new(1.0, 1.0, 0.85, 0.8 * k));
        }
        for p in &self.particles {
            draw_rectangle(p.pos.x.floor(), p.pos.y.floor(), 1.0, 1.0, p.color);
        }

        // Health bars (not minis: one hit is all they take).
        for (i, f) in m.f.iter().enumerate().take(N_FIGHTERS).filter(|(_, f)| f.present()) {
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
        ItemKind::Phone => {
            // A smartphone: dark case, screen lit up bright (and pulsing) while Jake is inside.
            draw_rectangle(x, y - 1.0, 3.0, 4.0, Color::new(0.10, 0.10, 0.12, 1.0));
            let screen = if it.carries.is_some() {
                let k = 0.6 + 0.4 * (time * 9.0).sin();
                draw_circle(p.x, p.y, 4.0, Color::new(HOLO.r, HOLO.g, HOLO.b, 0.22 * k));
                Color::new(HOLO.r * k + (1.0 - k), HOLO.g * k + (1.0 - k), HOLO.b, 1.0)
            } else {
                Color::new(0.25, 0.32, 0.45, 1.0)
            };
            draw_rectangle(x + 1.0, y, 1.0, 2.0, screen);
        }
        ItemKind::Sushi => {
            // A piece of salmon nigiri: rice, fish on top, a band of nori.
            draw_rectangle(x - 1.0, y + 1.0, 4.0, 2.0, RICE);
            draw_rectangle(x - 1.0, y, 4.0, 1.0, SALMON);
            draw_rectangle(x + 1.0, y, 1.0, 3.0, NORI);
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
    // A punch (fists, nothing in hand): a comically big boxing glove on the end of the arm.
    if f.holding.is_none() && f.atk_timer > 0 && f.atk_timer <= ATK_WINDUP + ATK_ACTIVE {
        draw_glove(hand, s, f.atk_timer > ATK_WINDUP, flash);
    }
    match look.hair {
        None => {
            // Hooded head with a pale face peeking out.
            draw_circle(head.x, head.y, 2.6, tint(HOOD, flash));
            draw_rectangle((head.x + s * 1.0 - 0.5).round(), head.y.round() - 1.0, 2.0, 2.0, tint(FACE, flash));
        }
        Some(hair) => {
            // Bare head: hair over the top and down the back of the head.
            let (hx, hy) = (head.x.round(), head.y.round());
            let hair = tint(hair, flash);
            // A rectangle relative to the head, drawn as if facing right and mirrored when not.
            let px = |dx: f32, dy: f32, w: f32, h: f32, c: Color| draw_rectangle(if s > 0.0 { hx + dx } else { hx + 1.0 - dx - w }, hy + dy, w, h, c);
            draw_circle(head.x, head.y, 2.4, tint(look.skin, flash));
            // Over the top (just a shadow of stubble when shaved).
            px(-2.0, -3.0, 5.0, if look.hair_style == HairStyle::Shaved { 1.0 } else { 2.0 }, hair);
            match look.hair_style {
                HairStyle::Short => px(-3.0, -2.0, 2.0, 3.0, hair),
                HairStyle::Long => {
                    px(-3.0, -2.0, 2.0, 6.0, hair);
                    px(-4.0, 0.0, 1.0, 4.0, hair);
                }
                HairStyle::OverEars => {
                    px(-3.0, -2.0, 2.0, 3.0, hair);
                    px(-1.0, -1.0, 2.0, 2.0, hair);
                    px(2.0, -2.0, 1.0, 1.0, hair);
                }
                HairStyle::Shaved => {}
                HairStyle::Emo => {
                    px(-3.0, -3.0, 6.0, 1.0, hair);
                    px(-3.0, -2.0, 2.0, 4.0, hair);
                    // The fringe, swept forward and down over the eyes.
                    px(-1.0, -2.0, 4.0, 2.0, hair);
                    px(2.0, 0.0, 1.0, 1.0, hair);
                }
            }
            if look.beard {
                px(-1.0, 1.0, 4.0, 2.0, hair);
                px(0.0, 3.0, 3.0, 1.0, hair);
            }
            if look.headband {
                draw_rectangle(hx - 2.0, hy - 2.0, 5.0, 1.0, tint(look.accent, flash));
            }
        }
    }
    // The eye (hidden under an emo fringe).
    if !(look.hair.is_some() && look.hair_style == HairStyle::Emo) {
        draw_rectangle((head.x + s * 1.5).round(), head.y.round() - 1.0, 1.0, 1.0, Color::new(0.05, 0.05, 0.06, 1.0));
    }
    if look.glasses {
        // A dark frame across the eye and a pale glint of lens in front of it.
        let (hx, hy) = (head.x.round(), head.y.round());
        let px = |dx: f32, w: f32, c: Color| draw_rectangle(if s > 0.0 { hx + dx } else { hx + 1.0 - dx - w }, hy - 1.0, w, 1.0, c);
        px(-1.0, 2.0, tint(Color::new(0.10, 0.10, 0.12, 1.0), flash));
        px(2.0, 1.0, tint(Color::new(0.80, 0.90, 1.0, 1.0), flash));
    }
}

/// One of Tyler's dogs, mid-gallop.
fn draw_dog(d: &Pet) {
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

/// One of Corwin's cats, tail up, scampering.
fn draw_cat(c: &Pet, coat: u8) {
    let (x, y, s) = (c.pos.x.round(), c.pos.y.round(), c.dir);
    let (fur, dark) = COATS[coat as usize % COATS.len()];
    let stride = ((c.pos.x * 0.8).sin() > 0.0) as u8 as f32;
    for (lx, up) in [(-2.0, stride), (1.0, 1.0 - stride)] {
        draw_rectangle(x + lx * s, y - 1.0, 1.0, 1.0 - up, dark);
    }
    draw_rectangle(x - 2.0, y - 3.0, 4.0, 2.0, fur);
    // Head with two ears out front, and a tall tail behind.
    let hx = if s > 0.0 { x + 1.0 } else { x - 3.0 };
    draw_rectangle(hx, y - 4.0, 2.0, 2.0, fur);
    draw_rectangle(hx, y - 5.0, 1.0, 1.0, dark);
    draw_rectangle(hx + 1.0, y - 5.0, 1.0, 1.0, dark);
    let tx = if s > 0.0 { x - 3.0 } else { x + 2.0 };
    draw_rectangle(tx, y - 5.0 - stride, 1.0, 3.0, dark);
}

/// Robert's lightning: a jagged bolt to each foe, with a couple of short forks branching off, all
/// flickering (re-shaped a few times a second) and fading out.
fn draw_lightning(l: &Lightning, time: f32) {
    let fade = (l.ticks as f32 / 20.0).min(1.0);
    let frame = (time * 24.0) as i32;
    for (b, &to) in l.to.iter().enumerate() {
        let d = to - l.from;
        let n = ((d.length() / 10.0) as i32).clamp(3, 30);
        let side = vec2(-d.y, d.x).normalize_or_zero();
        let jag = |k: i32, salt: i32| (hash(k + salt * 97, frame + b as i32 * 13, l.seed) - 0.5) * 8.0;
        let mut prev = l.from;
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let p = if k == n { to } else { l.from + d * t + side * jag(k, 0) };
            draw_line(prev.x, prev.y, p.x, p.y, 3.0, Color::new(BOLT.r, BOLT.g, BOLT.b, 0.25 * fade));
            draw_line(prev.x, prev.y, p.x, p.y, 1.0, Color::new(1.0, 1.0, 1.0, 0.95 * fade));
            // Now and then a fork splits off.
            if k < n && hash(k, b as i32, l.seed ^ 0x5a5a) < 0.18 {
                let dir = (d.normalize_or_zero() + side * (hash(k, 7, l.seed) - 0.5) * 2.5).normalize_or_zero();
                let mut q = p;
                for s in 0..3 {
                    let r = q + dir * 5.0 + side * jag(k * 5 + s, 3) * 0.5;
                    draw_line(q.x, q.y, r.x, r.y, 1.0, Color::new(BOLT.r, BOLT.g, BOLT.b, 0.8 * fade));
                    q = r;
                }
            }
            prev = p;
        }
        draw_circle(to.x, to.y, 4.0, Color::new(BOLT.r, BOLT.g, BOLT.b, 0.3 * fade));
    }
    draw_circle(l.from.x, l.from.y, 3.0, Color::new(1.0, 1.0, 1.0, 0.6 * fade));
}

/// Kyle's organize, on Kyle: a shockwave ring sweeping out across the arena as he casts it, then
/// while it holds a pillar of light and little blocks orbiting him, so everyone sees who did it.
fn draw_order(o: &Order, kyle: &Fighter, time: f32) {
    let t = (PIN_TICKS - o.ticks.min(PIN_TICKS)) as f32 * DT;
    let fade = (o.ticks as f32 * DT / 0.3).min(1.0);
    for (lag, width) in [(0.0, 2.0), (0.08, 1.0)] {
        let k = ((t - lag) / 0.5).clamp(0.0, 1.0);
        if k > 0.0 && k < 1.0 {
            draw_circle_lines(o.at.x, o.at.y, 8.0 + k * W, width, Color::new(PIN.r, PIN.g, PIN.b, 0.9 * (1.0 - k)));
        }
    }
    if !kyle.present() {
        return;
    }
    let c = kyle.center();
    let pulse = 0.5 + 0.5 * (time * 6.0).sin();
    draw_rectangle(c.x - 5.0, c.y - 40.0, 10.0, 40.0 + kyle.height() * 0.5, Color::new(PIN.r, PIN.g, PIN.b, (0.10 + 0.08 * pulse) * fade));
    draw_rectangle(c.x - 2.0, c.y - 60.0, 4.0, 60.0 + kyle.height() * 0.5, Color::new(PIN.r, PIN.g, PIN.b, (0.12 + 0.1 * pulse) * fade));
    draw_circle(c.x, c.y, 11.0 + pulse * 2.0, Color::new(PIN.r, PIN.g, PIN.b, 0.18 * fade));
    draw_circle_lines(c.x, c.y, 11.0 + pulse * 2.0, 1.0, Color::new(PIN.r, PIN.g, PIN.b, 0.9 * fade));
    for k in 0..4 {
        let a = time * 3.0 + k as f32 * std::f32::consts::FRAC_PI_2;
        let p = c + vec2(a.cos() * 13.0, a.sin() * 6.0);
        draw_rectangle((p.x - 1.0).round(), (p.y - 1.0).round(), 2.0, 2.0, Color::new(1.0, 1.0, 1.0, fade));
        draw_rectangle_lines((p.x - 2.0).round(), (p.y - 2.0).round(), 4.0, 4.0, 1.0, Color::new(PIN.r, PIN.g, PIN.b, fade));
    }
}

/// Jake breaking up into (or coming back out of) light: a stack of flickering scan lines the
/// height of a fighter, squeezing down to nothing going in, opening up coming out.
fn draw_hologram(c: Vec2, t: f32, out: bool) {
    let k = (t / HOLO_SECS).clamp(0.0, 1.0);
    let open = if out { k } else { 1.0 - k };
    let fade = if out { 1.0 - k } else { 1.0 };
    for row in 0..9 {
        let y = c.y - 8.0 + row as f32 * 2.0;
        let flicker = ((t * 60.0 + row as f32 * 1.7).sin() * 0.5 + 0.5) * 0.6 + 0.4;
        let w = (5.0 + 3.0 * ((row * 7) % 3) as f32) * open.max(0.08);
        let col = if row % 2 == 0 { HOLO } else { HOLO_PINK };
        draw_rectangle((c.x - w).round(), y.round(), (w * 2.0).round().max(1.0), 1.0, Color::new(col.r, col.g, col.b, 0.85 * flicker * fade));
    }
    draw_circle(c.x, c.y, 10.0 * open + 2.0, Color::new(HOLO.r, HOLO.g, HOLO.b, 0.12 * fade));
}

/// Ryan's chain: steel links from his chest to the hook.
fn draw_chain(c: &Chain, from: Vec2) {
    let d = c.tip - from;
    let n = (d.length() / 2.0) as i32;
    for k in 0..n {
        let p = from + d * (k as f32 / n.max(1) as f32);
        let col = if k % 2 == 0 { Color::new(0.72, 0.74, 0.78, 1.0) } else { Color::new(0.42, 0.44, 0.48, 1.0) };
        draw_rectangle(p.x.floor(), p.y.floor(), 1.0, 1.0, col);
    }
    // The hook: a little barbed point.
    let (x, y) = (c.tip.x.floor(), c.tip.y.floor());
    let s = if d.x >= 0.0 { 1.0 } else { -1.0 };
    draw_rectangle(x - 1.0, y - 1.0, 2.0, 2.0, Color::new(0.85, 0.87, 0.90, 1.0));
    draw_rectangle(x - s, y + 1.0, 1.0, 1.0, Color::new(0.85, 0.87, 0.90, 1.0));
}

/// Justin's table: a wooden top on two legs, with a little cloth runner.
fn draw_table(t: &Table) {
    let b = t.body();
    let (x, y) = (b.x.round(), b.y.round());
    draw_rectangle(x, y, b.w, 2.0, WOOD);
    draw_rectangle(x, y + 2.0, b.w, 1.0, WOOD_DARK);
    draw_rectangle(x + b.w * 0.5 - 4.0, y, 8.0, 1.0, Color::new(0.85, 0.20, 0.22, 1.0));
    for lx in [x + 2.0, x + b.w - 4.0] {
        draw_rectangle(lx, y + 3.0, 2.0, b.h - 3.0, WOOD_DARK);
    }
}

/// Sam's glue: a wobbling glob in flight; stuck, a blob with a sagging strand to everyone it holds
/// (sagging less the more it's stretched), and a splotch on each of them. It fades as it dries.
fn draw_glue(g: &Glue, m: &Match, time: f32) {
    let fade = if g.stuck { (g.ticks as f32 * DT / 0.6).min(1.0) } else { 1.0 };
    let c = |a: f32| Color::new(GLUE.r, GLUE.g, GLUE.b, a * fade);
    if !g.stuck {
        let r = 2.0 + (time * 25.0).sin() * 0.4;
        draw_circle(g.pos.x, g.pos.y, r + 1.0, c(0.35));
        draw_circle(g.pos.x, g.pos.y, r, c(1.0));
        draw_rectangle(g.pos.x.floor() - 1.0, g.pos.y.floor() - 1.0, 1.0, 1.0, Color::new(1.0, 1.0, 1.0, 1.0));
        let back = g.pos - g.vel.normalize_or_zero() * 4.0;
        draw_line(g.pos.x, g.pos.y, back.x, back.y, 1.0, c(0.6));
        return;
    }
    let a = g.pos;
    for &h in &g.held {
        let f = &m.f[h as usize];
        if !f.present() {
            continue;
        }
        let b = f.center();
        let d = b - a;
        let slack = (1.0 - (d.length() - TETHER_REST).max(0.0) / (TETHER_BREAK - TETHER_REST)).clamp(0.0, 1.0);
        let sag = vec2(0.0, 6.0 * slack);
        let mut prev = a;
        for k in 1..=8 {
            let t = k as f32 / 8.0;
            let p = a + d * t + sag * (4.0 * t * (1.0 - t));
            draw_line(prev.x, prev.y, p.x, p.y, 2.0, c(0.35));
            draw_line(prev.x, prev.y, p.x, p.y, 1.0, c(0.9));
            prev = p;
        }
        draw_circle(b.x, b.y, 3.0, c(0.55));
    }
    draw_circle(a.x, a.y, 4.0, c(0.4));
    draw_circle(a.x, a.y, 2.5, c(0.95));
    // A drip hanging off the blob.
    let drip = 1.0 + ((time * 2.0 + a.x).sin() * 0.5 + 0.5) * 2.0;
    draw_rectangle(a.x.floor(), a.y.floor() + 2.0, 1.0, drip.round(), c(0.8));
}

/// A big, beat-up boxing glove at `hand`, facing `s`: weathered oxblood leather in the game's muted
/// palette, blocky (wider than tall) with a stubby thumb, a worn tan cuff and a darker underside.
/// Smaller while wound up; thrown, full size with a faint smear behind it.
fn draw_glove(hand: Vec2, s: f32, out: bool, flash: bool) {
    let (len, h) = if out { (7.0, 5.0) } else { (5.0, 4.0) };
    let leather = tint(Color::new(0.47, 0.19, 0.16, 1.0), flash);
    let shade = tint(Color::new(0.29, 0.12, 0.11, 1.0), flash);
    let worn = tint(Color::new(0.58, 0.30, 0.24, 1.0), flash);
    let cuff = tint(Color::new(0.55, 0.50, 0.40, 1.0), flash);
    let (hx, hy) = (hand.x.round(), (hand.y - h / 2.0).round());
    // A rectangle in glove space (dx forward from the wrist), mirrored when facing left.
    let px = |dx: f32, dy: f32, w: f32, hh: f32, c: Color| draw_rectangle(if s > 0.0 { hx + dx } else { hx + 1.0 - dx - w }, hy + dy, w, hh, c);
    // Cuff, then the fist: square-ish, its front corners knocked off.
    px(-2.0, 0.0, 2.0, h, cuff);
    px(0.0, 0.0, len - 1.0, h, leather);
    px(len - 1.0, 1.0, 1.0, h - 2.0, leather);
    px(0.0, h - 1.0, len - 1.0, 1.0, shade);
    // Stubby thumb on top, and a scuff along the knuckles.
    px(1.0, -1.0, 2.0, 1.0, leather);
    px(len - 3.0, 1.0, 2.0, 1.0, worn);
    if out {
        for dy in [1.0, h - 2.0] {
            px(-6.0, dy, 3.0, 1.0, Color::new(0.75, 0.74, 0.70, 0.25));
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
