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
/// Michelle's colour, and her freeze's glow (a little paler); Robert's lightning.
const MICHELLE: Color = Color::new(0.45, 0.40, 1.0, 1.0);
const PIN: Color = Color::new(0.66, 0.62, 1.0, 1.0);
const BOLT: Color = Color::new(0.75, 0.85, 1.0, 1.0);
const HOLO_PINK: Color = Color::new(1.0, 0.45, 0.85, 1.0);
/// Vid's falling code.
const CODE: Color = Color::new(0.30, 0.95, 0.42, 1.0);
/// Fire: the base of the flames, and their tips.
const FLAME: Color = Color::new(0.80, 0.36, 0.12, 1.0);
const FLAME_TIP: Color = Color::new(0.95, 0.72, 0.32, 1.0);
const EMBER_BASE: Color = Color::new(0.48, 0.15, 0.07, 1.0);
/// The giant sword's steel.
const STEEL: Color = Color::new(0.68, 0.70, 0.72, 1.0);
const STEEL_DARK: Color = Color::new(0.42, 0.44, 0.47, 1.0);
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
/// Travis's weighted vest, its strap and a pouch; Doug's black belt.
const VEST: Color = Color::new(0.16, 0.17, 0.15, 1.0);
const VEST_STRAP: Color = Color::new(0.48, 0.42, 0.30, 1.0);
const VEST_POUCH: Color = Color::new(0.26, 0.28, 0.22, 1.0);
const BELT: Color = Color::new(0.06, 0.06, 0.07, 1.0);
/// Matt's skateboard deck and wheels.
/// Zack's tornadoes.
/// Kaleb's arrow: gold, with a red fin.
const ARROW_GOLD: Color = Color::new(1.0, 0.80, 0.25, 1.0);
const ARROW_FIN: Color = Color::new(0.85, 0.12, 0.10, 1.0);
/// Vid's reading: the glow of his eyes, shield and books; the books' covers and pages.
const EYE_GLOW: Color = Color::new(0.62, 0.92, 1.0, 1.0);
const BOOK_COVERS: [Color; 3] = [Color::new(0.55, 0.16, 0.18, 1.0), Color::new(0.18, 0.28, 0.55, 1.0), Color::new(0.20, 0.42, 0.24, 1.0)];
const PAGE: Color = Color::new(0.94, 0.91, 0.82, 1.0);
const TORNADO_GREY: Color = Color::new(0.62, 0.64, 0.66, 1.0);
const DECK: Color = Color::new(0.20, 0.55, 0.85, 1.0);
const WHEEL: Color = Color::new(0.92, 0.88, 0.70, 1.0);
/// Carrie's Buc-ee's beaver suit: the red cap and its brim, the tail, the belly.
const BUCEE_RED: Color = Color::new(0.82, 0.12, 0.14, 1.0);
const BUCEE_BRIM: Color = Color::new(0.62, 0.08, 0.10, 1.0);
const BUCEE_TAIL: Color = Color::new(0.36, 0.24, 0.15, 1.0);
const BUCEE_TAIL_DARK: Color = Color::new(0.24, 0.16, 0.10, 1.0);
const BUCEE_BELLY: Color = Color::new(0.88, 0.74, 0.52, 1.0);
/// Carrie's bugs: a spider's body and legs; a horsefly's body, eyes and wings.
const SPIDER: Color = Color::new(0.10, 0.09, 0.09, 1.0);
const SPIDER_LEG: Color = Color::new(0.22, 0.20, 0.19, 1.0);
const FLY_BODY: Color = Color::new(0.24, 0.26, 0.20, 1.0);
const FLY_EYE: Color = Color::new(0.75, 0.22, 0.12, 1.0);
const FLY_WING: Color = Color::new(0.85, 0.90, 0.95, 0.55);
/// Dust kicked up by Travis's steps (and a landing from Doug's throw).
const DUST: Color = Color::new(0.62, 0.56, 0.46, 0.85);
/// Travis's quake rings: moss green and earth brown.
const QUAKE_MOSS: Color = Color::new(0.52, 0.68, 0.30, 1.0);
const QUAKE_EARTH: Color = Color::new(0.62, 0.46, 0.26, 1.0);
/// How fast someone Doug threw spins in the air (radians a tick).
const TUMBLE_SPIN: f32 = 0.24;
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
    /// A long goatee hanging off the chin (Vid).
    pub goatee: bool,
    pub stubble: Option<Color>,
    pub belt: bool,
    pub beaver_suit: bool,
    pub mustache: bool,
    pub backwards_cap: Option<Color>,
    pub grey_flecks: bool,
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
    /// Clipped very short, close to the head all round (Doug).
    Buzz,
    /// Combed straight back, flat and shiny (Zack).
    Slick,
    /// Wild and swept back, thin on top (Vid).
    Wild,
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
    /// A long goatee hanging off the chin (Vid).
    pub goatee: bool,
    /// A shadow of short beard along the jaw, in this colour (blended into the skin).
    pub stubble: Option<Color>,
    /// A black belt round the waist (Doug's judo gi).
    pub belt: bool,
    /// A Buc-ee's beaver suit (Carrie): a tan belly, a flat tail behind, and the red cap.
    pub beaver_suit: bool,
    /// A light mustache under the nose (Matt).
    pub mustache: bool,
    /// A ballcap worn backwards, in this colour (Kyle).
    pub backwards_cap: Option<Color>,
    /// Salt-and-pepper: flecks of grey through the hair and beard (Shawn).
    pub grey_flecks: bool,
    /// Played when it uses its special (None: the special's own effects make the sound).
    pub special_sound: Option<Sfx>,
}

pub fn art(c: Character) -> &'static CharacterArt {
    match c {
        Character::Classic => &CharacterArt { color: None, body: CLOAK, legs: [HOOD, CLOAK], arms: HOOD, hair: None, headband: false, skin: SKIN, hair_style: HairStyle::Short, beard: false, glasses: false, goatee: false, stubble: None, belt: false, beaver_suit: false, mustache: false, backwards_cap: None, grey_flecks: false, special_sound: None },
        // Brown hair and stubble, green-grey jacket; forest green.
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
                goatee: false,
                stubble: Some(rgb(96, 62, 36)),
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::GlueShot),
            };
            &ART
        }
        // Reddish-brown hair and stubble, work jacket; gold. (The pop-up's poof is its sound.)
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
                goatee: false,
                stubble: Some(rgb(122, 60, 36)),
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
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
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::Hounds),
            };
            &ART
        }
        // Dark chestnut hair (between brown and red) and stubble, charcoal hoodie; teal. (His minis are drawn from the same look.)
        Character::Seth => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.20, 0.78, 0.85, 1.0)),
                body: rgb(58, 60, 66),
                legs: [rgb(30, 32, 38), rgb(42, 44, 52)],
                arms: rgb(50, 52, 58),
                hair: Some(rgb(84, 48, 34)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                goatee: false,
                stubble: Some(rgb(84, 48, 34)),
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
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
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
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
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
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
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::ChainThrow),
            };
            &ART
        }
        // Shaved head and stubble, a dark work jacket; electric blue.
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
                goatee: false,
                stubble: Some(rgb(104, 84, 68)),
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::Thunder),
            };
            &ART
        }
        // Dirty-blonde hair and stubble under a backwards ballcap, glasses, a tidy polo; mint.
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
                glasses: true,
                goatee: false,
                stubble: Some(rgb(150, 116, 70)),
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: Some(rgb(36, 88, 70)),
                grey_flecks: false,
                special_sound: Some(Sfx::Backlog),
            };
            &ART
        }
        // Wild red hair swept back (thin on top), glasses, a long goatee, a long black coat; matrix
        // green.
        // (Reading, he sits with his book, eyes glowing, in a shield.)
        Character::Vid => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.30, 0.92, 0.45, 1.0)),
                body: rgb(22, 24, 24),
                legs: [rgb(16, 18, 18), rgb(28, 30, 30)],
                arms: rgb(30, 32, 32),
                hair: Some(rgb(178, 70, 38)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Wild,
                beard: false,
                glasses: true,
                goatee: true,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::BookOpen),
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
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: None,
            };
            &ART
        }
        // Very short blackish-brown hair and stubble, a white judo gi with a black belt; ivory.
        Character::Doug => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.95, 0.92, 0.80, 1.0)),
                body: rgb(226, 224, 214),
                legs: [rgb(198, 196, 186), rgb(214, 212, 202)],
                arms: rgb(210, 208, 198),
                hair: Some(rgb(38, 30, 26)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Buzz,
                beard: false,
                glasses: false,
                goatee: false,
                stubble: Some(rgb(44, 34, 28)),
                belt: true,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::JudoGrab),
            };
            &ART
        }
        // Short brown hair, a grey gym tank (bare arms) and joggers; magenta. Bulked up, he wears
        // a weighted vest over it.
        Character::Travis => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.90, 0.26, 0.92, 1.0)),
                body: rgb(128, 128, 134),
                legs: [rgb(28, 28, 32), rgb(40, 40, 46)],
                arms: SKIN,
                hair: Some(rgb(104, 70, 42)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::Flex),
            };
            &ART
        }
        // Long straight black hair, fair skin, always in a Buc-ee's beaver suit (brown fur, tan
        // belly, flat tail, red cap); Buc-ee's yellow.
        Character::Carrie => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(1.0, 0.86, 0.10, 1.0)),
                body: rgb(138, 90, 50),
                legs: [rgb(108, 70, 40), rgb(124, 80, 46)],
                arms: rgb(126, 82, 46),
                hair: Some(rgb(16, 14, 16)),
                headband: false,
                skin: rgb(244, 222, 208),
                hair_style: HairStyle::Long,
                beard: false,
                glasses: false,
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: true,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::Bugs),
            };
            &ART
        }
        // Dark brown hair and a light mustache, a red flannel and chinos; skater coral. (His
        // skateboard comes out for the special.)
        Character::Matt => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(1.0, 0.44, 0.34, 1.0)),
                body: rgb(150, 52, 46),
                legs: [rgb(132, 112, 84), rgb(150, 128, 96)],
                arms: rgb(128, 42, 40),
                hair: Some(rgb(58, 38, 26)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: true,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::Skate),
            };
            &ART
        }
        // Dark hair slicked back, a groomed beard along the jaw into an extended goatee, a slate
        // storm-chaser jacket; storm blue.
        Character::Zack => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.62, 0.84, 1.0, 1.0)),
                body: rgb(66, 84, 98),
                legs: [rgb(34, 38, 48), rgb(46, 50, 62)],
                arms: rgb(58, 74, 88),
                hair: Some(rgb(40, 30, 24)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Slick,
                beard: false,
                glasses: false,
                goatee: true,
                stubble: Some(rgb(40, 30, 24)),
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::Tornado),
            };
            &ART
        }
        // Dirty-blond hair and stubble, a long red-brown leather coat; Ravager red. (His arrow is
        // gold.)
        Character::Kaleb => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.80, 0.16, 0.14, 1.0)),
                body: rgb(112, 44, 36),
                legs: [rgb(42, 34, 32), rgb(56, 46, 42)],
                arms: rgb(98, 38, 32),
                hair: Some(rgb(168, 130, 82)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: false,
                glasses: false,
                goatee: false,
                stubble: Some(rgb(140, 104, 64)),
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                // (Each call of the arrow whistles; see `Event::ArrowCall`.)
                special_sound: None,
            };
            &ART
        }
        // Long straight brown hair, a soft lavender cardigan; storm indigo. (Her freeze glows in
        // it, and an eye opens over her as she casts it.)
        Character::Michelle => {
            const ART: CharacterArt = CharacterArt {
                color: Some(MICHELLE),
                body: rgb(176, 160, 200),
                legs: [rgb(44, 46, 62), rgb(58, 60, 80)],
                arms: rgb(162, 146, 188),
                hair: Some(rgb(112, 74, 46)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Long,
                beard: false,
                glasses: false,
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: false,
                special_sound: Some(Sfx::Freeze),
            };
            &ART
        }
        // Short black hair going grey and a full beard to match (salt and pepper), a long black
        // coat; silver.
        Character::Shawn => {
            const ART: CharacterArt = CharacterArt {
                color: Some(Color::new(0.80, 0.82, 0.88, 1.0)),
                body: rgb(30, 32, 34),
                legs: [rgb(22, 24, 26), rgb(34, 36, 38)],
                arms: rgb(38, 40, 42),
                hair: Some(rgb(48, 46, 46)),
                headband: false,
                skin: SKIN,
                hair_style: HairStyle::Short,
                beard: true,
                glasses: false,
                goatee: false,
                stubble: None,
                belt: false,
                beaver_suit: false,
                mustache: false,
                backwards_cap: None,
                grey_flecks: true,
                special_sound: None,
            };
            &ART
        }
    }
}

/// The colour that stands for a character in menus and the HUD (a neutral grey for one that
/// wears its seat's colour).
/// The sound fighter `f`'s special makes: their character's, or for one Ryan picked up
/// (Hobbyist), the sound of whoever has it (his old chain if nobody does).
pub fn special_sound(f: &Fighter) -> Option<Sfx> {
    let Some(sp) = f.special().filter(|_| f.hobby.is_some()) else { return art(f.character).special_sound };
    match Character::ALL.into_iter().find(|c| c.special().is_some_and(|s| s.id() == sp.id())) {
        Some(c) => art(c).special_sound,
        None => Some(Sfx::ChainThrow),
    }
}

pub fn character_color(c: Character) -> Color {
    art(c).color.unwrap_or(Color::new(0.62, 0.64, 0.70, 1.0))
}

pub fn look(c: Character, seat: usize) -> Look {
    let a = art(c);
    Look { body: a.body, legs: a.legs, arms: a.arms, hair: a.hair, accent: a.color.unwrap_or(ACCENTS[seat]), headband: a.headband, skin: a.skin, hair_style: a.hair_style, beard: a.beard, glasses: a.glasses, goatee: a.goatee, stubble: a.stubble, belt: a.belt, beaver_suit: a.beaver_suit, mustache: a.mustache, backwards_cap: a.backwards_cap, grey_flecks: a.grey_flecks }
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


/// Something drawn only for looks, made from an event: the server neither keeps nor sends it.
/// (A new special's flash or ring goes here: an `Event` from the sim, a variant here made in
/// `handle_events`, drawn in `draw`.)
enum Visual {
    /// Robert's bolts, from his hand to each foe they struck.
    Bolts(Lightning),
    /// Michelle's Eye of the Storm, and whose (the eye is drawn over her).
    Eye(Freeze, usize),
    /// Carrie's laptop, and whose (it's drawn in her hands).
    Laptop(Laptop, usize),
    /// One of Travis's steps, rippling out.
    Quake(Quake),
}

impl Visual {
    /// How long it lasts (sim ticks).
    fn lifetime(&self) -> f32 {
        (match self {
            Visual::Bolts(_) => LIGHTNING_TICKS,
            Visual::Eye(..) => PIN_TICKS,
            Visual::Laptop(..) => LAPTOP_TICKS,
            Visual::Quake(_) => QUAKE_TICKS,
        }) as f32
    }

    fn set_ticks(&mut self, t: u32) {
        match self {
            Visual::Bolts(l) => l.ticks = t,
            Visual::Eye(o, _) => o.ticks = t,
            Visual::Laptop(l, _) => l.ticks = t,
            Visual::Quake(q) => q.ticks = t,
        }
    }
}

/// Robert's bolts: from his hand to where each foe was when it struck.
struct Lightning {
    from: Vec2,
    to: Vec<Vec2>,
    /// Shapes the forks.
    seed: u32,
    ticks: u32,
}

/// Michelle's Eye of the Storm: where she cast it (the shockwave starts there).
struct Freeze {
    at: Vec2,
    ticks: u32,
}

/// Carrie's laptop, in her hands.
struct Laptop {
    at: Vec2,
    ticks: u32,
}

/// One of Travis's steps, rippling out to its reach.
struct Quake {
    at: Vec2,
    radius: f32,
    ticks: u32,
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
    /// Every map's terrain, built once at startup (building one takes long enough to hitch, and
    /// it used to happen right as a match's countdown began).
    terrains: Vec<Texture2D>,
    /// The map the terrain texture shows.
    arena: ArenaId,
    vignette: Texture2D,
    stain_img: Image,
    stain_tex: Texture2D,
    /// The part of the stain image changed since it was last uploaded (min x, min y, max x, max y),
    /// so only that goes to the GPU, not the whole half-megabyte image every frame of a fight.
    stain_dirty: Option<(u32, u32, u32, u32)>,
    particles: Vec<Particle>,
    rain: Vec<Drop>,
    vis: [FighterVis; N_BODIES],
    /// Seth's minis are drawn full size into these, then shrunk onto the arena (drawing them
    /// straight at a smaller size would lose their 1px limbs).
    minis: Vec<RenderTarget>,
    /// Someone Doug threw is drawn into one of these (one per seat), then turned as they tumble.
    tumblers: Vec<RenderTarget>,
    /// Recent explosions (position, seconds since) for the flash.
    blasts: Vec<(Vec2, f32, f32)>,
    /// Jake's hologram beams and Vid's code: (middle, seconds since, coming out?, Vid's code?).
    holos: Vec<(Vec2, f32, bool, bool)>,
    /// A flag in their colour where each fighter went down (ground position, colour).
    flags: Vec<(Vec2, Color)>,
    /// What's drawn only for looks, from events (the server doesn't keep it), and how long each
    /// has left (in sim ticks).
    visuals: Vec<(Visual, f32)>,
    time: f32,
}

fn tex(img: &Image) -> Texture2D {
    let t = Texture2D::from_image(img);
    t.set_filter(FilterMode::Nearest);
    t
}

impl Art {
    pub fn new() -> Self {
        let terrains: Vec<Texture2D> = ArenaId::ALL.iter().map(|a| tex(&make_terrain(a.def()))).collect();
        let stain_img = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        let rain = (0..200)
            .map(|i| Drop { pos: vec2(hash(i, 0, 1) * W, hash(i, 1, 1) * H), speed: 200.0 + hash(i, 2, 1) * 120.0 })
            .collect();
        Self {
            bg: tex(&make_background()),
            terrain: terrains[ArenaId::default() as usize].clone(),
            terrains,
            arena: ArenaId::default(),
            vignette: tex(&make_vignette()),
            stain_tex: tex(&stain_img),
            stain_img,
            stain_dirty: None,
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
            tumblers: (0..N_FIGHTERS)
                .map(|_| {
                    let t = render_target(MINI_SPRITE.0, MINI_SPRITE.1);
                    t.texture.set_filter(FilterMode::Nearest);
                    t
                })
                .collect(),
            blasts: Vec::new(),
            holos: Vec::new(),
            flags: Vec::new(),
            visuals: Vec::new(),
            time: 0.0,
        }
    }

    /// Draws `arena`'s platforms from now on (rebuilding the terrain if it changed).
    pub fn set_arena(&mut self, arena: ArenaId) {
        if arena != self.arena {
            self.arena = arena;
            self.terrain = self.terrains[arena as usize].clone();
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.stain_img = Image::gen_image_color(W as u16, H as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        self.stain_dirty = Some((0, 0, W as u32 - 1, H as u32 - 1));
        self.particles.clear();
        self.blasts.clear();
        self.flags.clear();
        self.visuals.clear();
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
                Event::Matrix { pos, out } => {
                    self.holos.push((pos, 0.0, out, true));
                    self.burst(pos, vec2(0.0, 1.0), 1.0, 20, CODE, false, 50.0, 0.5, 60.0);
                }
                Event::Hologram { pos, out } => {
                    self.holos.push((pos, 0.0, out, false));
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 18, HOLO, false, 40.0, 0.5, -20.0);
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 8, HOLO_PINK, false, 30.0, 0.4, -20.0);
                }
                Event::Slam { pos } => {
                    self.burst(pos + vec2(0.0, 5.0), vec2(-1.0, -0.4).normalize(), 0.7, 8, DUST, false, 60.0, 0.4, 150.0);
                    self.burst(pos + vec2(0.0, 5.0), vec2(1.0, -0.4).normalize(), 0.7, 8, DUST, false, 60.0, 0.4, 150.0);
                }
                Event::Quake { pos, big } => {
                    // Dust thrown out both ways along the ground, and grit kicked up.
                    let n = if big { 14 } else { 6 };
                    for side in [-1.0, 1.0] {
                        self.burst(pos + vec2(side * 2.0, -1.0), vec2(side, -0.35).normalize(), 0.6, n, DUST, false, if big { 110.0 } else { 70.0 }, 0.4, 200.0);
                    }
                    self.burst(pos + vec2(0.0, -1.0), vec2(0.0, -1.0), 1.2, n / 2, Color::new(0.40, 0.36, 0.30, 1.0), false, if big { 110.0 } else { 60.0 }, 0.5, 420.0);
                    let radius = if big { QUAKE_BIG_R } else { QUAKE_R };
                    self.visuals.push((Visual::Quake(Quake { at: pos, radius, ticks: QUAKE_TICKS }), QUAKE_TICKS as f32));
                }
                Event::BacklogBurst { pos } => {
                    // Paperwork everywhere.
                    self.burst(pos + vec2(0.0, -12.0), vec2(0.0, -1.0), 6.3, 18, PAGE, false, 90.0, 0.6, 200.0);
                }
                Event::Blocked { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 10, EYE_GLOW, false, 60.0, 0.25, 0.0);
                }
                Event::ArrowCall { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 8, ARROW_GOLD, false, 40.0, 0.3, 0.0);
                }
                Event::Zap { from, pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 10, BOLT, false, 90.0, 0.2, 0.0);
                    // One bolt per foe struck, all from one cast: grouped into one set of forks.
                    let fresh = LIGHTNING_TICKS as f32;
                    match self.visuals.last_mut() {
                        Some((Visual::Bolts(l), left)) if l.from == from && *left == fresh => l.to.push(pos),
                        _ => {
                            let seed = (from.x as u32).wrapping_mul(73_856_093) ^ (from.y as u32).wrapping_mul(19_349_663) ^ (self.time * 1000.0) as u32;
                            self.visuals.push((Visual::Bolts(Lightning { from, to: vec![pos], seed, ticks: LIGHTNING_TICKS }), fresh));
                        }
                    }
                }
                Event::EyeOfTheStorm { who, at } => {
                    let v = Visual::Eye(Freeze { at, ticks: PIN_TICKS }, who);
                    let left = v.lifetime();
                    self.visuals.push((v, left));
                }
                Event::LaptopOpen { who } => {
                    let v = Visual::Laptop(Laptop { at: laptop_at(&m.f[who]), ticks: LAPTOP_TICKS }, who);
                    let left = v.lifetime();
                    self.visuals.push((v, left));
                }
                Event::Ignite { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 2.5, 24, FLAME, false, 90.0, 0.5, 120.0);
                    self.burst(pos, vec2(0.0, -1.0), 3.0, 10, Color::new(0.55, 0.62, 0.50, 0.9), false, 110.0, 0.3, 380.0);
                }
                Event::RadioCall { .. } | Event::JetPass { .. } | Event::BombDrop { .. } => {}
                Event::Dash { who, pos } => {
                    // A puff of dust kicked up behind.
                    let back = -m.f[who].facing;
                    self.burst(pos + vec2(0.0, -2.0), vec2(back, -0.3).normalize(), 0.8, 10, SMOKE, false, 70.0, 0.35, 0.0);
                }
                Event::RocketLaunch { pos } => {
                    self.burst(pos, vec2(0.0, -1.0), 6.3, 14, SMOKE, false, 40.0, 0.5, -20.0);
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
        // Look-only visuals count down on their own clock; Carrie's laptop stays in her hands.
        for (v, left) in self.visuals.iter_mut() {
            *left -= dt * TICK_RATE as f32;
            v.set_ticks(left.max(0.0).ceil() as u32);
            if let Visual::Laptop(l, who) = v {
                l.at = laptop_at(&m.f[*who]);
                if !m.f[*who].present() {
                    *left = 0.0;
                }
            }
        }
        self.visuals.retain(|(_, left)| *left > 0.0);
        for h in self.holos.iter_mut() {
            h.1 += dt;
        }
        self.holos.retain(|h| h.1 < HOLO_SECS);
        // Fires throw up embers and a little smoke.
        for e in &m.effects {
            if let Effect::Fire(fire) = e {
                let seed = (self.time * 1000.0) as i32;
                if hash(seed, fire.at.x as i32, 77) < dt * 30.0 {
                    let x = fire.at.x + (hash(seed, 3, 78) - 0.5) * fire.half_w * 2.0;
                    let (c, life) = if hash(seed, 4, 79) < 0.7 { (FLAME_TIP, 0.5) } else { (Color::new(0.35, 0.33, 0.32, 0.5), 1.0) };
                    self.particles.push(Particle { pos: vec2(x, fire.at.y - 4.0), vel: vec2((hash(seed, 5, 80) - 0.5) * 10.0, -25.0), color: c, life, stick: false, gravity: -15.0 });
                }
            }
        }
        // Tornadoes kick up dust at their feet, flung round the way they spin.
        for e in &m.effects {
            if let Effect::Tornado(t) = e {
                let seed = (self.time * 1000.0) as i32;
                if hash(seed, t.at.x as i32, 83) < dt * 40.0 {
                    let side = if hash(seed, 2, 84) < 0.5 { -1.0 } else { 1.0 };
                    let vel = vec2(t.spin * 40.0 + side * 15.0, -30.0 - hash(seed, 3, 85) * 40.0);
                    self.particles.push(Particle { pos: t.at + vec2(side * 3.0, -1.0), vel, color: DUST, life: 0.6, stick: false, gravity: 60.0 });
                }
            }
        }
        // Kaleb's arrow leaves a trail of golden sparks in flight.
        for e in &m.effects {
            if let Effect::Arrow(a) = e
                && matches!(a.state, ArrowState::Out | ArrowState::Back)
            {
                let back = a.pos - a.dir * 8.0;
                self.particles.push(Particle { pos: back, vel: -a.dir * 20.0, color: ARROW_GOLD, life: 0.25, stick: false, gravity: 0.0 });
            }
        }
        // Rockets leave a smoke trail.
        for e in &m.effects {
            if let Effect::Rocket(r) = e {
                let back = r.pos - r.vel.normalize_or_zero() * 5.0;
                self.particles.push(Particle { pos: back, vel: vec2(0.0, -6.0), color: Color::new(0.62, 0.62, 0.60, 0.5), life: 0.6, stick: false, gravity: -10.0 });
            }
        }
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
                let (x, y) = (x as u32, y as u32);
                self.stain_img.set_pixel(x, y, c);
                self.stain_dirty = Some(match self.stain_dirty {
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                    None => (x, y, x, y),
                });
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

    /// Draws each of Seth's minis, and anyone Doug has thrown, full size into its own picture, for
    /// `draw` to shrink (a mini) or turn (a tumbling body) onto the arena. Call before setting the
    /// arena camera (it sets its own).
    pub fn draw_sprites(&self, m: &Match) {
        for (i, f) in m.f.iter().enumerate() {
            let target = if i >= N_FIGHTERS {
                &self.minis[i - N_FIGHTERS]
            } else if tumble(m, i).is_some() {
                &self.tumblers[i]
            } else {
                continue;
            };
            if f.absent {
                continue;
            }
            let (w, h) = (MINI_SPRITE.0 as f32, MINI_SPRITE.1 as f32);
            let mut cam = Camera2D { target: vec2(0.0, h * (0.5 - MINI_FEET)), zoom: vec2(2.0 / w, 2.0 / h), ..Default::default() };
            cam.render_target = Some(target.clone());
            set_camera(&cam);
            clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
            // The same fighter at full size, standing on the picture's origin.
            let mut big = f.clone();
            big.pos = Vec2::ZERO;
            big.scale = 1.0;
            let mut v = self.vis[i].clone();
            for p in v.scarf.iter_mut() {
                *p -= f.pos;
            }
            // (Not flashing all the while they're down: that's a long stun.)
            let flash = f.hitstun > 0 && (f.hitstun / 2) % 2 == 0 && tumble(m, i).is_none();
            draw_human(&v, &big, &look(f.character, f.team(i)), flash);
        }
    }

    /// A body drawn into its own picture (a mini, or someone Doug threw), placed on the arena at
    /// its size and turned as it tumbles.
    fn draw_sprite(&self, m: &Match, i: usize) {
        let f = &m.f[i];
        let tex = if i >= N_FIGHTERS { &self.minis[i - N_FIGHTERS].texture } else { &self.tumblers[i].texture };
        let (w, h) = (MINI_SPRITE.0 as f32 * f.scale, MINI_SPRITE.1 as f32 * f.scale);
        let (angle, drop) = tumble(m, i).unwrap_or((0.0, 0.0));
        let at = vec2(f.pos.x - w / 2.0, f.pos.y - h * MINI_FEET + drop * f.scale);
        let pivot = f.center() + vec2(0.0, drop * f.scale);
        let params = DrawTextureParams { dest_size: Some(vec2(w, h)), rotation: angle, pivot: Some(pivot), ..Default::default() };
        draw_texture_ex(tex, at.x, at.y, WHITE, params);
        // Down after a throw: stars going round their head.
        if let Some(j) = judo_of(m, i).filter(|j| j.landed()) {
            let head = pivot + vec2(angle.sin(), -angle.cos()) * 7.0 * f.scale;
            for k in 0..3 {
                let a = self.time * 7.0 + k as f32 * std::f32::consts::TAU / 3.0;
                let p = head + vec2(a.cos() * 3.5, a.sin() * 1.2 - 2.0);
                let fade = (j.down as f32 / 10.0).min(1.0);
                draw_rectangle(p.x.round(), p.y.round(), 1.0, 1.0, Color::new(1.0, 0.92, 0.45, fade));
            }
        }
    }

    pub fn draw(&mut self, m: &Match) {
        if let Some((x0, y0, x1, y1)) = self.stain_dirty.take() {
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let part = self.stain_img.sub_image(Rect::new(x0 as f32, y0 as f32, w as f32, h as f32));
            self.stain_tex.update_part(&part, x0 as i32, y0 as i32, w as i32, h as i32);
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
                Effect::Pet(p) => {
                    // Frozen by Michelle: a ring of her glow at its feet.
                    if p.frozen > 0 {
                        let fade = (p.frozen as f32 / 20.0).min(1.0);
                        ellipse_outline(p.pos.x, p.pos.y - 0.5, 5.0, 2.0, 1.0, Color::new(PIN.r, PIN.g, PIN.b, 0.9 * fade));
                    }
                    match p.breed {
                        Breed::Hound => draw_dog(p),
                        Breed::Cat(coat) => draw_cat(p, coat),
                        Breed::Spider(_) => draw_spider(p),
                        Breed::Fly(k) => draw_fly(p, k, self.time),
                    }
                }
                Effect::Table(t) => draw_table(t),
                Effect::Chain(c) => draw_chain(c, chain_anchor(&m.f[c.owner])),
                Effect::Rocket(r) => draw_rocket(r, self.time),
                Effect::Fire(fire) => draw_fire(fire, self.time),
                Effect::Plane(p) => draw_plane(p, self.time),
                Effect::AirBomb(b) => draw_air_bomb(b),
                // (Drawn as the thrown fighter tumbling.)
                Effect::Judo(_) => {}
                // (Never sent: see `Effect::on_wire`.)
                Effect::Lunge(_) => {}
                Effect::Tornado(t) => draw_tornado(t, self.time),
                // (Drawn over the fighters, below.)
                Effect::Arrow(_) => {}
                Effect::Book(b) => draw_book(b, self.time),
            }
        }
        // Travis's quake rings, on the ground under everyone.
        for (v, _) in &self.visuals {
            if let Visual::Quake(q) = v {
                draw_quake(q, self.time);
            }
        }
        // Loose and flying items (held spears and swords are drawn with their holder).
        for it in m.items.iter().filter(|it| !(it.held_by.is_some() && matches!(it.kind, ItemKind::Spear | ItemKind::Sword))) {
            draw_item(it, self.time);
        }
        // Jake inside a phone: a chevron over the one he's in, and faint ones over the others he could
        // hop to.
        if m.f.iter().any(|f| f.digital > 0) {
            for it in m.items.iter().filter(|it| it.kind == ItemKind::Phone) {
                let bob = ((self.time * 5.0).sin() * 1.0).round();
                let c = if it.carries.is_some() { HOLO_PINK } else { Color::new(HOLO_PINK.r, HOLO_PINK.g, HOLO_PINK.b, 0.3) };
                let (x, y) = (it.pos.x.round(), it.pos.y.round() - 7.0 + bob);
                for (k, w) in [5.0, 3.0, 1.0].into_iter().enumerate() {
                    draw_rectangle(x - (w - 1.0) / 2.0, y + k as f32, w, 1.0, c);
                }
            }
        }

        // Michelle's freeze: a glowing ring at the feet of everyone stuck. Tyler's haste and Matt's
        // skateboard: speed streaks.
        for f in m.f.iter().filter(|f| f.present()) {
            if f.pinned > 0 {
                let pulse = 0.5 + 0.5 * (self.time * 8.0).sin();
                let fade = (f.pinned as f32 / 20.0).min(1.0);
                let w = 9.0 * f.scale + pulse;
                fill_ellipse(f.pos.x, f.pos.y - 0.5, w + 3.0, 4.0, Color::new(PIN.r, PIN.g, PIN.b, 0.15 * fade));
                fill_ellipse(f.pos.x, f.pos.y - 0.5, w, 3.0, Color::new(PIN.r, PIN.g, PIN.b, 0.4 * fade));
                ellipse_outline(f.pos.x, f.pos.y - 0.5, w, 3.0, 1.0, Color::new(PIN.r, PIN.g, PIN.b, fade));
                ellipse_outline(f.pos.x, f.pos.y - 0.5, w + 3.0, 4.0, 1.0, Color::new(PIN.r, PIN.g, PIN.b, 0.5 * pulse * fade));
                for k in 0..3 {
                    let x = f.pos.x + (k as f32 - 1.0) * 5.0 * f.scale;
                    let h = 3.0 + 3.0 * ((self.time * 6.0 + k as f32 * 2.1).sin() * 0.5 + 0.5);
                    draw_rectangle(x.round(), (f.pos.y - h).round(), 1.0, h.round(), Color::new(PIN.r, PIN.g, PIN.b, 0.75 * fade));
                }
            }
            if f.bulked > 0 && f.grounded {
                draw_bulk_reach(f, self.time);
            }
            if (f.haste > 0 || f.skate > 0) && f.vel.x.abs() > 60.0 {
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
            // A mini (shrunk to its size) or someone tumbling from Doug's throw: their own picture
            // (from `draw_sprites`).
            if i >= N_FIGHTERS || tumble(m, i).is_some() {
                self.draw_sprite(m, i);
                continue;
            }
            let flash = f.hitstun > 0 && (f.hitstun / 2) % 2 == 0;
            if f.ridden > 0 {
                draw_ridden(&self.vis[i], f, self.time);
            } else {
                draw_human(&self.vis[i], f, &look(f.character, i), flash);
            }
        }
        // Kyle's backlog: his inbox over his head, piling up.
        for f in m.f.iter().filter(|f| f.present() && f.backlog > 0) {
            draw_inbox(f, self.time);
        }
        // Vid reading: his shield, over him.
        for f in m.f.iter().filter(|f| f.present() && f.reading > 0) {
            draw_book_shield(f, self.time);
        }
        for e in &m.effects {
            if let Effect::Arrow(a) = e {
                draw_arrow(a, self.time);
            }
        }
        for (v, _) in &self.visuals {
            match v {
                Visual::Bolts(l) => draw_lightning(l, self.time),
                Visual::Eye(o, who) => draw_eye(o, &m.f[*who], self.time),
                Visual::Laptop(l, who) => draw_laptop(l, &m.f[*who], self.time),
                // (Under the fighters, above.)
                Visual::Quake(_) => {}
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
        for it in m.items.iter().filter(|it| it.held_by.is_some() && !matches!(it.kind, ItemKind::Spear | ItemKind::Sword)) {
            draw_item(it, self.time);
        }

        for &(pos, t, out, code) in &self.holos {
            if code {
                draw_code_burst(pos, t, out, self.time);
            } else {
                draw_hologram(pos, t, out);
            }
        }
        for &(pos, t, r) in &self.blasts {
            let k = 1.0 - t / 0.25;
            draw_circle(pos.x, pos.y, r * (0.5 + 0.5 * (1.0 - k)), Color::new(1.0, 0.85, 0.5, 0.35 * k));
            draw_circle(pos.x, pos.y, r * 0.35, Color::new(1.0, 1.0, 0.85, 0.8 * k));
        }
        for p in &self.particles {
            draw_rectangle(p.pos.x.floor(), p.pos.y.floor(), 1.0, 1.0, p.color);
        }

        // Health bars; a mini's is shorter, sits lower (it's smaller) and runs out of its own (smaller)
        // health.
        for (i, f) in m.f.iter().enumerate().filter(|(_, f)| f.present()) {
            let mini = i >= N_FIGHTERS;
            let (w, full) = if mini { (8.0, MINI_HP) } else { (12.0, MAX_HP) };
            let (x, y) = ((f.pos.x - w / 2.0).round(), (f.pos.y - 22.0 * f.scale).round());
            draw_rectangle(x, y, w, 1.0, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(x, y, (w * f.hp / full).ceil(), 1.0, look(f.character, f.team(i)).accent);
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

/// A soft glow pulsing under a rare item lying on the ground, so it's easy to spot.
fn item_glow(p: Vec2, c: Color, time: f32) {
    let pulse = 0.5 + 0.5 * (time * 4.0 + p.x * 0.1).sin();
    draw_circle(p.x, p.y - 2.0, 7.0 + pulse * 2.0, Color::new(c.r, c.g, c.b, 0.06 + 0.06 * pulse));
    draw_circle(p.x, p.y - 2.0, 4.0 + pulse, Color::new(c.r, c.g, c.b, 0.10 + 0.08 * pulse));
}

/// A filled rectangle with a soft 1px outline round it (a dark shade of its own colour, partly
/// see-through), so a small item reads on any backdrop without a hard black border.
fn outlined(x: f32, y: f32, w: f32, h: f32, c: Color) {
    draw_rectangle(x - 1.0, y - 1.0, w + 2.0, h + 2.0, Color::new(c.r * 0.3, c.g * 0.3, c.b * 0.3, 0.6));
    draw_rectangle(x, y, w, h, c);
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
            // A little bigger than a rock, outlined so it stands out: a ridged olive body sitting
            // on the ground, with the lever and pin ring on top.
            let (gx, gy) = (p.x.round() - 2.0, (p.y + ITEM_R).round() - 4.0);
            outlined(gx, gy, 4.0, 4.0, GRENADE);
            draw_rectangle(gx, gy, 1.0, 1.0, Color::new(0.48, 0.56, 0.36, 1.0));
            draw_rectangle(gx, gy + 2.0, 4.0, 1.0, Color::new(0.22, 0.27, 0.16, 1.0));
            draw_rectangle(gx + 1.0, gy - 1.0, 2.0, 1.0, Color::new(0.55, 0.53, 0.50, 1.0));
            draw_rectangle(gx + 3.0, gy - 2.0, 1.0, 1.0, Color::new(0.78, 0.76, 0.70, 1.0));
            // A live fuse blinks red, faster as it burns down.
            if let Some(t) = it.fuse {
                let rate = 4.0 + 16.0 * (1.0 - t as f32 / GRENADE_FUSE as f32);
                if (time * rate).sin() > 0.0 {
                    draw_rectangle(gx + 1.0, gy + 1.0, 2.0, 1.0, Color::new(1.0, 0.15, 0.1, 1.0));
                    draw_circle(p.x, p.y, 4.5, Color::new(1.0, 0.2, 0.1, 0.25));
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
        ItemKind::Sword => {
            // Lying flat, or flying tip first.
            let dir = if it.vel.length() > 20.0 { it.vel.normalize() } else { vec2(1.0, 0.0) };
            let base = if it.vel.length() > 20.0 { p } else { p + vec2(0.0, ITEM_R - 0.5) };
            draw_blade(base - dir * 7.0, dir, false);
        }
        // These three are rare and worth grabbing: bigger than the rest, outlined, with a soft glow
        // while they're lying around.
        ItemKind::Molotov => {
            let (bx, by) = (p.x.round(), p.y.round() + 1.0);
            if it.held_by.is_none() {
                item_glow(p, FLAME, time);
            }
            let glass = Color::new(0.24, 0.40, 0.24, 1.0);
            outlined(bx - 1.0, by - 4.0, 3.0, 4.0, glass);
            outlined(bx, by - 5.0, 1.0, 1.0, glass);
            draw_rectangle(bx - 1.0, by - 3.0, 1.0, 2.0, Color::new(0.42, 0.58, 0.40, 1.0));
            draw_rectangle(bx - 1.0, by - 2.0, 3.0, 1.0, Color::new(0.62, 0.50, 0.30, 1.0));
            draw_rectangle(bx, by - 7.0, 1.0, 2.0, Color::new(0.70, 0.62, 0.48, 1.0));
            let flick = ((time * 13.0 + p.x).sin() * 0.5 + 0.5).round();
            draw_rectangle(bx, by - 8.0 - flick, 1.0, 1.0 + flick, FLAME_TIP);
        }
        ItemKind::AirStrike => {
            let (bx, by) = (p.x.round(), p.y.round() + 1.0);
            let blink = (time * 4.0).sin() > 0.0;
            if it.held_by.is_none() {
                item_glow(p, Color::new(0.95, 0.35, 0.25, 1.0), time);
            }
            // A field radio: a squat box with a speaker grille, a whip antenna and a blinking light.
            outlined(bx - 2.0, by - 4.0, 5.0, 4.0, Color::new(0.30, 0.33, 0.26, 1.0));
            draw_rectangle(bx - 1.0, by - 3.0, 2.0, 1.0, Color::new(0.14, 0.15, 0.13, 1.0));
            draw_rectangle(bx - 1.0, by - 1.0, 2.0, 1.0, Color::new(0.14, 0.15, 0.13, 1.0));
            draw_rectangle(bx + 2.0, by - 8.0, 1.0, 4.0, Color::new(0.10, 0.10, 0.10, 1.0));
            let light = if blink { Color::new(1.0, 0.30, 0.20, 1.0) } else { Color::new(0.40, 0.14, 0.10, 1.0) };
            draw_rectangle(bx + 1.0, by - 3.0, 1.0, 1.0, light);
            if blink {
                draw_circle(bx + 1.5, by - 2.5, 2.0, Color::new(1.0, 0.3, 0.2, 0.25));
            }
        }
        ItemKind::RocketLauncher => {
            let (bx, by) = (p.x.round(), p.y.round() + 1.0);
            if it.held_by.is_none() {
                item_glow(p, Color::new(0.95, 0.82, 0.40, 1.0), time);
            }
            // A battered olive tube with a yellow warning band, a dark muzzle and a grip.
            outlined(bx - 5.0, by - 3.0, 10.0, 2.0, Color::new(0.34, 0.38, 0.26, 1.0));
            draw_rectangle(bx - 5.0, by - 3.0, 10.0, 1.0, Color::new(0.46, 0.50, 0.36, 1.0));
            draw_rectangle(bx - 1.0, by - 3.0, 2.0, 2.0, Color::new(0.82, 0.68, 0.26, 1.0));
            draw_rectangle(bx + 4.0, by - 3.0, 1.0, 2.0, Color::new(0.06, 0.06, 0.06, 1.0));
            draw_rectangle(bx - 3.0, by - 1.0, 1.0, 1.0, Color::new(0.20, 0.18, 0.15, 1.0));
        }
        ItemKind::Sushi => {
            // A piece of salmon nigiri: rice, fish on top, a band of nori.
            draw_rectangle(x - 1.0, y + 1.0, 4.0, 2.0, RICE);
            draw_rectangle(x - 1.0, y, 4.0, 1.0, SALMON);
            draw_rectangle(x + 1.0, y, 1.0, 3.0, NORI);
        }
    }
}

/// The throw fighter `i` is in the middle of, if Doug has thrown them.
fn judo_of(m: &Match, i: usize) -> Option<&Judo> {
    m.effects.iter().find_map(|e| match e {
        Effect::Judo(j) if j.victim == i => Some(j),
        _ => None,
    })
}

/// Someone Doug threw: how far they're turned (radians, clockwise) and how far down to draw them
/// (lying flat, so they're on the ground and not floating at waist height). Spinning in the air;
/// landed, flat on their back, then getting up over the last part.
fn tumble(m: &Match, i: usize) -> Option<(f32, f32)> {
    let j = judo_of(m, i)?;
    if !m.f[i].alive() {
        return None;
    }
    let angle = if j.landed() {
        let lying = (j.down as f32 / (JUDO_DOWN as f32 * 0.4)).min(1.0);
        j.dir * std::f32::consts::FRAC_PI_2 * lying
    } else {
        j.dir * j.flown as f32 * TUMBLE_SPIN
    };
    Some((angle, if j.landed() { 5.0 * angle.sin().abs() } else { 0.0 }))
}

/// One of Travis's steps: like the freeze's ring but softer and in earth colours, a flat ring
/// on the ground that runs out from his feet to exactly how far the step reaches, with tufts of
/// moss and grit thrown up around its edge.
fn draw_quake(q: &Quake, time: f32) {
    let t = 1.0 - q.ticks as f32 / QUAKE_TICKS as f32;
    let fade = 1.0 - t * t;
    let (x, y) = (q.at.x, q.at.y - 0.5);
    let reach = q.radius * (0.35 + 0.65 * t.sqrt());
    let tall = 2.5 + q.radius * 0.05;
    fill_ellipse(x, y, reach, tall, Color::new(QUAKE_MOSS.r, QUAKE_MOSS.g, QUAKE_MOSS.b, 0.16 * fade));
    ellipse_outline(x, y, reach, tall, 1.0, Color::new(QUAKE_MOSS.r, QUAKE_MOSS.g, QUAKE_MOSS.b, 0.75 * fade));
    ellipse_outline(x, y, reach * 0.7, tall * 0.7, 1.0, Color::new(QUAKE_EARTH.r, QUAKE_EARTH.g, QUAKE_EARTH.b, 0.55 * fade));
    // Tufts thrown up at the edge of the ring, both ways.
    for side in [-1.0, 1.0] {
        for k in 0..2 {
            let d = reach - k as f32 * 4.0;
            let h = (1.0 + 2.0 * fade * ((time * 9.0 + k as f32 * 1.7 + side).sin() * 0.5 + 0.5)).round();
            let c = if k == 0 { QUAKE_MOSS } else { QUAKE_EARTH };
            draw_rectangle((x + side * d).round(), (q.at.y - h).round(), 1.0, h, Color::new(c.r, c.g, c.b, 0.8 * fade));
        }
    }
}

/// Travis bulked up: a faint ring on the ground showing how far each step reaches.
fn draw_bulk_reach(f: &Fighter, time: f32) {
    let pulse = 0.5 + 0.5 * (time * 4.0).sin();
    let fade = (f.bulked as f32 / 20.0).min(1.0);
    let (x, y) = (f.pos.x, f.pos.y - 0.5);
    let tall = 2.5 + QUAKE_R * 0.05;
    fill_ellipse(x, y, QUAKE_R, tall, Color::new(QUAKE_MOSS.r, QUAKE_MOSS.g, QUAKE_MOSS.b, (0.05 + 0.03 * pulse) * fade));
    ellipse_outline(x, y, QUAKE_R, tall, 1.0, Color::new(QUAKE_EARTH.r, QUAKE_EARTH.g, QUAKE_EARTH.b, (0.22 + 0.12 * pulse) * fade));
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

/// Points round a unit circle, worked out once: ellipses are drawn from these rather than with
/// macroquad's own (which allocates and does its trig afresh every call, and a tornado draws
/// over a hundred a frame).
static UNIT_RING: std::sync::LazyLock<[Vec2; 20]> = std::sync::LazyLock::new(|| {
    std::array::from_fn(|k| {
        let a = k as f32 / 20.0 * std::f32::consts::TAU;
        vec2(a.cos(), a.sin())
    })
});

/// The ring's points for an ellipse this wide: every other one for a small one.
fn ring(x: f32, y: f32, w: f32, h: f32) -> impl Iterator<Item = (Vec2, Vec2)> {
    let step = if w < 8.0 { 2 } else { 1 };
    let at = move |k: usize| {
        let u = UNIT_RING[k % 20];
        vec2(x + u.x * w, y + u.y * h)
    };
    (0..20).step_by(step).map(move |k| (at(k), at(k + step)))
}

/// A filled ellipse (centre, half width, half height).
fn fill_ellipse(x: f32, y: f32, w: f32, h: f32, c: Color) {
    let mid = vec2(x, y);
    for (a, b) in ring(x, y, w, h) {
        draw_triangle(mid, a, b, c);
    }
}

/// An ellipse's outline (centre, half width, half height).
fn ellipse_outline(x: f32, y: f32, w: f32, h: f32, thickness: f32, c: Color) {
    for (a, b) in ring(x, y, w, h) {
        draw_line(a.x, a.y, b.x, b.y, thickness, c);
    }
}

/// `a`, `t` of the way to `b`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
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
    // Vid reading sits on the ground: everything above the legs sits lower.
    let reading = f.reading > 0;
    let sit = if reading { 3.0 } else { 0.0 };
    let hip = vec2(x, fy - 6.0 + sit);
    let neck = vec2(x + lean, fy - 11.0 + sit);
    let shoulder = neck + vec2(0.0, 1.0);
    let head = neck + vec2(s * 0.5, -2.5);
    // Travis bulked up: thick arms and legs, a broad chest, and a weighted vest.
    let bulk = f.bulked > 0;
    let limb = if bulk { 2.0 } else { 1.0 };

    // Scarf behind everything.
    for k in 1..6 {
        line(v.scarf[k - 1], v.scarf[k], 1.0, tint(scarf, flash));
    }
    if look.beaver_suit {
        // The flat beaver tail, sticking out behind, low; criss-crossed darker.
        let (tx, ty) = (if s > 0.0 { x.round() - 5.0 } else { x.round() + 1.0 }, fy.round() - 4.0);
        draw_rectangle(tx, ty, 4.0, 2.0, tint(BUCEE_TAIL, flash));
        draw_rectangle(tx + 1.0, ty, 1.0, 1.0, tint(BUCEE_TAIL_DARK, flash));
        draw_rectangle(tx + 2.0, ty + 1.0, 1.0, 1.0, tint(BUCEE_TAIL_DARK, flash));
    }
    // Matt's skateboard: a deck under his feet on two wheels, his feet planted front and back.
    let skating = f.skate > 0;
    if skating {
        let (bx, by) = (x.round() - 4.0, fy.round() - 1.0);
        draw_rectangle(bx, by, 9.0, 1.0, tint(DECK, flash));
        draw_rectangle(if s > 0.0 { bx + 8.0 } else { bx }, by - 1.0, 1.0, 1.0, tint(DECK, flash));
        draw_rectangle(bx + 1.0, by + 1.0, 1.0, 1.0, tint(WHEEL, flash));
        draw_rectangle(bx + 7.0, by + 1.0, 1.0, 1.0, tint(WHEEL, flash));
    }
    // Legs.
    for k in 0..2 {
        let foot = if reading {
            // Sitting cross-legged: feet out in front, on the ground.
            vec2(x + s * (2.0 + 2.0 * k as f32), fy - 1.0)
        } else if skating {
            vec2(x + (k as f32 - 0.5) * 4.0 * s, fy - 1.5)
        } else if f.grounded {
            let ph = v.walk_phase + k as f32 * std::f32::consts::PI;
            let amt = (speed / 45.0).min(1.5);
            vec2(x + ph.sin() * 3.0 * amt, fy - (-ph.cos()).max(0.0) * 2.0 * amt.min(1.0))
        } else {
            vec2(x + (k as f32 - 0.5) * 3.0 - f.vel.x * 0.01, fy - 2.0)
        };
        let knee = ik(hip, foot, 3.4, 3.4, -s);
        let c = tint(look.legs[k], flash);
        line(hip, knee, limb, c);
        line(knee, foot, limb, c);
    }
    // Torso as a soft blob (broad in the chest, bulked up).
    for t in [0.0, 0.5, 1.0] {
        let p = hip.lerp(neck, t);
        draw_circle(p.x, p.y, if bulk { 1.8 + 0.8 * t } else { 1.8 }, tint(look.body, flash));
    }
    if bulk {
        // The weighted vest: a dark plate front and back, a strap across, and a pouch in front.
        let (vx, vy) = ((neck.x - 3.0).round(), (neck.y + 0.5).round());
        draw_rectangle(vx, vy, 6.0, 4.0, tint(VEST, flash));
        draw_rectangle(vx, vy + 2.0, 6.0, 1.0, tint(VEST_STRAP, flash));
        draw_rectangle(if s > 0.0 { vx + 4.0 } else { vx }, vy + 1.0, 2.0, 2.0, tint(VEST_POUCH, flash));
    }
    if look.beaver_suit {
        // The suit's tan belly, out front.
        draw_rectangle(if s > 0.0 { x.round() } else { x.round() - 1.0 }, fy.round() - 9.0, 2.0, 3.0, tint(BUCEE_BELLY, flash));
    }
    if look.belt {
        draw_rectangle((hip.x - 2.0).round(), (hip.y - 1.0).round(), 5.0, 1.0, tint(BELT, flash));
        draw_rectangle((hip.x + s * 2.0).round(), hip.y.round(), 1.0, 2.0, tint(BELT, flash));
    }
    let hand = if reading {
        // Holding his book open in front of him.
        neck + vec2(s * 3.5, 3.5)
    } else {
        match f.holding {
            Some(ItemKind::Spear) => draw_spear(f, neck, flash),
            Some(ItemKind::Sword) => draw_sword(f, neck, flash),
            _ => free_hand(f, v, neck),
        }
    };
    if reading {
        draw_open_book(hand, s, flash);
    }
    let elbow = ik(shoulder, hand, 3.0, 3.5, s);
    line(shoulder, elbow, limb, tint(look.arms, flash));
    line(elbow, hand, limb, tint(look.arms, flash));
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
            match look.hair_style {
                // Thin on top: just a wisp.
                HairStyle::Wild => px(-2.0, -3.0, 3.0, 1.0, hair),
                HairStyle::Shaved | HairStyle::Buzz => px(-2.0, -3.0, 5.0, 1.0, hair),
                _ => px(-2.0, -3.0, 5.0, 2.0, hair),
            }
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
                HairStyle::Buzz => px(-3.0, -2.0, 1.0, 2.0, hair),
                HairStyle::Slick => {
                    // Combed straight back off the forehead, flat and shiny, a little long behind.
                    px(-3.0, -2.0, 2.0, 3.0, hair);
                    px(-4.0, -1.0, 1.0, 2.0, hair);
                    px(0.0, -3.0, 2.0, 1.0, tint(mix(hair, WHITE, 0.25), flash));
                }
                HairStyle::Wild => {
                    px(-4.0, -3.0, 2.0, 3.0, hair);
                    px(-5.0, -4.0, 2.0, 1.0, hair);
                    px(-5.0, -1.0, 1.0, 2.0, hair);
                    px(-3.0, 0.0, 2.0, 2.0, hair);
                    px(1.0, -4.0, 1.0, 1.0, hair);
                }
                HairStyle::Emo => {
                    px(-3.0, -3.0, 6.0, 1.0, hair);
                    px(-3.0, -2.0, 2.0, 4.0, hair);
                    // The fringe, swept forward and down over the eyes.
                    px(-1.0, -2.0, 4.0, 2.0, hair);
                    px(2.0, 0.0, 1.0, 1.0, hair);
                }
            }
            if let Some(stubble) = look.stubble {
                // Along the jaw from the ear to the chin, a shade darker than the skin.
                let c = tint(mix(look.skin, stubble, 0.45), flash);
                px(-1.0, 1.0, 4.0, 1.0, c);
                px(0.0, 2.0, 3.0, 1.0, c);
            }
            if look.mustache {
                px(0.0, 1.0, 2.0, 1.0, tint(mix(look.skin, look.hair.unwrap_or(HOOD), 0.75), flash));
            }
            if look.goatee {
                px(1.0, 2.0, 2.0, 2.0, hair);
                px(1.0, 4.0, 1.0, 2.0, hair);
            }
            if look.beard {
                px(-1.0, 1.0, 4.0, 2.0, hair);
                px(0.0, 3.0, 3.0, 1.0, hair);
            }
            if look.grey_flecks {
                // Salt and pepper: grey through the hair, at the temple and in the beard.
                let grey = tint(Color::new(0.68, 0.68, 0.70, 1.0), flash);
                px(-1.0, -3.0, 1.0, 1.0, grey);
                px(1.0, -2.0, 1.0, 1.0, grey);
                px(-3.0, -1.0, 1.0, 1.0, grey);
                if look.beard {
                    px(0.0, 2.0, 1.0, 1.0, grey);
                    px(2.0, 1.0, 1.0, 1.0, grey);
                    px(1.0, 3.0, 1.0, 1.0, grey);
                }
            }
            if look.headband {
                draw_rectangle(hx - 2.0, hy - 2.0, 5.0, 1.0, tint(look.accent, flash));
            }
            if let Some(cap) = look.backwards_cap {
                // A ballcap turned round: a tall crown over the top (lit on the front, a button on
                // top), and the brim sticking well out behind his head.
                let dark = tint(mix(cap, Color::new(0.0, 0.0, 0.0, 1.0), 0.4), flash);
                px(-3.0, -5.0, 6.0, 3.0, tint(cap, flash));
                px(0.0, -5.0, 2.0, 1.0, tint(mix(cap, Color::new(1.0, 1.0, 1.0, 1.0), 0.3), flash));
                px(0.0, -6.0, 1.0, 1.0, dark);
                px(-7.0, -3.0, 4.0, 1.0, dark);
                px(-6.0, -2.0, 2.0, 1.0, dark);
            }
            if look.beaver_suit {
                // Buc-ee's red cap, the brim out front, a spot of yellow badge on it.
                px(-2.0, -4.0, 5.0, 2.0, tint(BUCEE_RED, flash));
                px(3.0, -3.0, 2.0, 1.0, tint(BUCEE_BRIM, flash));
                px(1.0, -4.0, 1.0, 1.0, tint(look.accent, flash));
            }
        }
    }
    // The eye (hidden under an emo fringe).
    if !(look.hair.is_some() && look.hair_style == HairStyle::Emo) {
        let (ex, ey) = ((head.x + s * 1.5).round(), head.y.round() - 1.0);
        if reading {
            // Reading, his eyes glow (like the Avatar State).
            draw_circle(ex + 0.5, ey + 0.5, 2.5, Color::new(EYE_GLOW.r, EYE_GLOW.g, EYE_GLOW.b, 0.35));
            draw_rectangle(ex, ey, 1.0, 1.0, Color::new(1.0, 1.0, 1.0, 1.0));
        } else {
            draw_rectangle(ex, ey, 1.0, 1.0, Color::new(0.05, 0.05, 0.06, 1.0));
        }
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
    let stride = if c.eating { 0.0 } else { ((c.pos.x * 0.8).sin() > 0.0) as u8 as f32 };
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

/// Kyle's backlog: an inbox tray over his head with a sheet of paper for everything he's put off,
/// the stack shaking as it grows, and a bar of how long until it all goes.
fn draw_inbox(f: &Fighter, time: f32) {
    let (x, y) = (f.pos.x.round(), (f.pos.y - f.height() - 8.0).round());
    let n = f.inbox.min(10) as f32;
    let shake = if n >= 4.0 { ((time * 30.0).sin() * 0.5).round() } else { 0.0 };
    // The tray.
    draw_rectangle(x - 4.0, y, 9.0, 2.0, Color::new(0.42, 0.40, 0.36, 1.0));
    draw_rectangle(x - 4.0, y - 1.0, 1.0, 1.0, Color::new(0.42, 0.40, 0.36, 1.0));
    draw_rectangle(x + 4.0, y - 1.0, 1.0, 1.0, Color::new(0.42, 0.40, 0.36, 1.0));
    // The pile, a sheet a thing, each a little askew.
    for k in 0..n as i32 {
        let off = if k % 2 == 0 { 0.0 } else { 1.0 } + shake;
        let c = if k % 3 == 2 { Color::new(0.86, 0.84, 0.76, 1.0) } else { PAGE };
        draw_rectangle(x - 3.0 + off, y - 1.0 - k as f32, 6.0, 1.0, c);
    }
    // Time left.
    let left = f.backlog as f32 / BACKLOG_TICKS as f32;
    draw_rectangle(x - 4.0, y + 3.0, (9.0 * left).ceil(), 1.0, Color::new(0.45, 0.95, 0.70, 0.9));
}

/// Vid's book, open in his hands: two pale pages in a dark cover, glowing faintly.
fn draw_open_book(at: Vec2, s: f32, flash: bool) {
    let (x, y) = (at.x.round(), at.y.round());
    draw_circle(x, y - 1.0, 4.0, Color::new(EYE_GLOW.r, EYE_GLOW.g, EYE_GLOW.b, 0.12));
    let left = if s > 0.0 { x - 1.0 } else { x - 3.0 };
    draw_rectangle(left, y - 2.0, 4.0, 3.0, tint(BOOK_COVERS[0], flash));
    draw_rectangle(left, y - 2.0, 4.0, 2.0, tint(PAGE, flash));
    draw_rectangle(left + 2.0, y - 2.0, 1.0, 2.0, tint(Color::new(0.75, 0.72, 0.62, 1.0), flash));
}

/// One of Vid's books, flapping its covers like wings as it flies.
fn draw_book(b: &Book, time: f32) {
    let (x, y) = (b.pos.x.round(), b.pos.y.round());
    let cover = BOOK_COVERS[b.k as usize % BOOK_COVERS.len()];
    let open = (time * 14.0 + b.k as f32 * 2.0).sin() > 0.0;
    draw_circle(x, y, 3.0, Color::new(EYE_GLOW.r, EYE_GLOW.g, EYE_GLOW.b, 0.15));
    if open {
        // Spread out: both covers, pages between.
        draw_rectangle(x - 2.0, y - 1.0, 5.0, 2.0, cover);
        draw_rectangle(x - 1.0, y - 1.0, 3.0, 1.0, PAGE);
    } else {
        // Snapped shut: a little block, the page edge showing.
        draw_rectangle(x - 1.0, y - 2.0, 3.0, 3.0, cover);
        draw_rectangle(x + 1.0, y - 1.0, 1.0, 2.0, PAGE);
    }
}

/// Vid reading: a shimmering bubble of a shield round him.
fn draw_book_shield(f: &Fighter, time: f32) {
    let c = f.center() + vec2(0.0, 2.0);
    let pulse = 0.5 + 0.5 * (time * 5.0).sin();
    let fade = (f.reading as f32 / 20.0).min(1.0);
    let r = 11.0 + pulse;
    draw_circle(c.x, c.y, r, Color::new(EYE_GLOW.r, EYE_GLOW.g, EYE_GLOW.b, (0.08 + 0.05 * pulse) * fade));
    ellipse_outline(c.x, c.y, r, r, 1.0, Color::new(EYE_GLOW.r, EYE_GLOW.g, EYE_GLOW.b, (0.5 + 0.3 * pulse) * fade));
}

/// Kaleb's arrow: a slim golden shaft with a bright tip and a red fin, pointing the way it flies,
/// glowing; bobbing gently while it hovers over his head. It flickers as its time runs out.
fn draw_arrow(a: &Arrow, time: f32) {
    if a.ticks < 40 && (a.ticks / 4) % 2 == 0 {
        return;
    }
    let bob = if a.state == ArrowState::Hover { vec2(0.0, (time * 4.0).sin()) } else { Vec2::ZERO };
    let tip = a.pos + bob;
    let d = a.dir.normalize_or(vec2(1.0, 0.0));
    draw_circle(tip.x - d.x * 4.0, tip.y - d.y * 4.0, 5.0, Color::new(ARROW_GOLD.r, ARROW_GOLD.g, ARROW_GOLD.b, 0.18));
    line(tip - d * 9.0, tip - d * 1.0, 1.0, ARROW_GOLD);
    line(tip - d * 1.5, tip, 1.0, Color::new(1.0, 0.97, 0.80, 1.0));
    // The fin, across the tail.
    let n = vec2(-d.y, d.x);
    line(tip - d * 9.0 + n * 1.5, tip - d * 7.0 - n * 1.5, 1.0, ARROW_FIN);
}

/// One of Zack's tornadoes: a grey column from the floor to the ceiling, narrow at the bottom and
/// flaring out toward the top, swaying, with streaks of wind whirling round it the way it
/// spins. It fades in and out.
fn draw_tornado(t: &Tornado, time: f32) {
    let age = (TORNADO_TICKS - t.ticks.min(TORNADO_TICKS)) as f32;
    let fade = (age / 12.0).min(1.0).min(t.ticks as f32 / 20.0);
    let (bottom, top) = (t.at.y - 1.0, WALL);
    let rows = ((bottom - top) / 4.0) as i32;
    for k in 0..=rows {
        let h = k as f32 / rows as f32;
        let y = bottom - h * (bottom - top);
        let w = 2.5 + h * 4.0 + h * h * 12.0;
        let cx = t.at.x + (time * 2.5 + k as f32 * 0.25).sin() * (1.0 + h * 5.0);
        fill_ellipse(cx, y, w, 1.8, Color::new(TORNADO_GREY.r, TORNADO_GREY.g, TORNADO_GREY.b, 0.16 * fade));
        ellipse_outline(cx, y, w, 1.8, 1.0, Color::new(TORNADO_GREY.r, TORNADO_GREY.g, TORNADO_GREY.b, 0.45 * fade));
        // A streak whipping round this ring (only on the front half, so it reads as turning).
        let a = time * 9.0 * t.spin + k as f32 * 0.9;
        if a.sin() > 0.0 {
            draw_rectangle((cx + a.cos() * w).round(), y.round(), 2.0, 1.0, Color::new(0.92, 0.94, 0.96, 0.75 * fade));
        }
    }
}

/// One of Carrie's spiders: a black body, four legs splayed out each side, stepping as it runs.
/// (Drawn a little bigger than it is, so it's easy to see.)
fn draw_spider(sp: &Pet) {
    let (x, y, s) = (sp.pos.x.round(), sp.pos.y.round(), sp.dir);
    let step = (sp.pos.x * 0.9).sin() > 0.0;
    draw_rectangle(x - 1.0, y - 3.0, 3.0, 2.0, SPIDER);
    for side in [-1.0, 1.0] {
        let kx = if side < 0.0 { x - 2.0 } else { x + 2.0 };
        // Knees up by the body, feet out on the ground; the pairs take turns lifting.
        let (near, far) = if step { (1.0, 0.0) } else { (0.0, 1.0) };
        draw_rectangle(kx, y - 3.0 - near, 1.0, 1.0, SPIDER_LEG);
        draw_rectangle(kx + side, y - 2.0, 1.0, 1.0, SPIDER_LEG);
        draw_rectangle(kx + side, y - 1.0 - far, 1.0, 1.0, SPIDER_LEG);
        draw_rectangle(kx, y - 1.0, 1.0, 1.0, SPIDER_LEG);
    }
    // A glint of eyes at the front.
    draw_rectangle(if s > 0.0 { x + 1.0 } else { x - 1.0 }, y - 3.0, 1.0, 1.0, FLY_EYE);
}

/// One of Carrie's horseflies: a fat dark body with big red eyes and a blur of wings. (Drawn a
/// little bigger than it is, so it's easy to see.)
fn draw_fly(fly: &Pet, k: u8, time: f32) {
    let (x, y, s) = (fly.pos.x.round(), fly.pos.y.round(), fly.dir);
    draw_rectangle(x - 2.0, y - 3.0, 4.0, 2.0, FLY_BODY);
    draw_rectangle(if s > 0.0 { x - 2.0 } else { x + 1.0 }, y - 2.0, 1.0, 1.0, Color::new(0.40, 0.42, 0.30, 1.0));
    draw_rectangle(if s > 0.0 { x + 2.0 } else { x - 3.0 }, y - 3.0, 1.0, 2.0, FLY_EYE);
    let up = (time * 45.0 + k as f32).sin() > 0.0;
    let wy = if up { y - 5.0 } else { y - 4.0 };
    draw_rectangle(if s > 0.0 { x - 2.0 } else { x }, wy, 3.0, 1.0, FLY_WING);
    draw_rectangle(if s > 0.0 { x - 1.0 } else { x }, wy - 1.0 + up as u8 as f32, 1.0, 1.0, FLY_WING);
}

/// Carrie's laptop, open in her hands: a dark base with the screen lit up and glowing, flickering
/// with code (where the bugs come from).
fn draw_laptop(l: &Laptop, carrie: &Fighter, time: f32) {
    let fade = (l.ticks as f32 / 12.0).min(1.0);
    let s = carrie.facing;
    let (x, y) = (l.at.x.round(), l.at.y.round());
    let screen_glow = Color::new(0.55, 0.85, 1.0, 1.0);
    draw_circle(x, y - 2.0, 5.0, Color::new(screen_glow.r, screen_glow.g, screen_glow.b, 0.18 * fade));
    draw_rectangle(x - 2.0, y, 5.0, 1.0, Color::new(0.30, 0.31, 0.34, fade));
    // The screen, tilted back toward her.
    let sx = if s > 0.0 { x - 2.0 } else { x };
    draw_rectangle(sx, y - 4.0, 3.0, 4.0, Color::new(0.20, 0.21, 0.24, fade));
    for row in 0..3 {
        let on = (time * 12.0 + row as f32 * 2.3).sin() > -0.3;
        let c = if on { screen_glow } else { Color::new(0.25, 0.55, 0.75, 1.0) };
        draw_rectangle(if s > 0.0 { sx + 1.0 } else { sx }, y - 3.0 + row as f32, 2.0, 1.0, Color::new(c.r, c.g, c.b, fade));
    }
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

/// Michelle's Eye of the Storm: a shockwave ring sweeping out across the arena as she casts it,
/// and while it holds, a great eye open over her, glowing, so everyone sees who did it.
fn draw_eye(o: &Freeze, michelle: &Fighter, time: f32) {
    let t = (PIN_TICKS - o.ticks.min(PIN_TICKS)) as f32 * DT;
    let fade = (o.ticks as f32 * DT / 0.3).min(1.0);
    for (lag, width) in [(0.0, 2.0), (0.08, 1.0)] {
        let k = ((t - lag) / 0.5).clamp(0.0, 1.0);
        if k > 0.0 && k < 1.0 {
            draw_circle_lines(o.at.x, o.at.y, 8.0 + k * W, width, Color::new(PIN.r, PIN.g, PIN.b, 0.9 * (1.0 - k)));
        }
    }
    if !michelle.present() {
        return;
    }
    // Opening over the first moment, closing as it ends.
    let open = (t / 0.25).min(1.0).min(o.ticks as f32 * DT / 0.25);
    let pulse = 0.5 + 0.5 * (time * 4.0).sin();
    // (Up above her health bar and name, so it's never hidden.)
    let c = vec2(michelle.pos.x, michelle.pos.y - 54.0);
    let (w, h) = (9.0, 4.5 * open);
    draw_circle(c.x, c.y, 9.0 + pulse, Color::new(PIN.r, PIN.g, PIN.b, (0.12 + 0.06 * pulse) * fade));
    // A faint beam down to her.
    draw_rectangle(c.x - 1.0, c.y + 4.0, 2.0, 34.0, Color::new(PIN.r, PIN.g, PIN.b, 0.15 * fade));
    if h >= 0.5 {
        fill_ellipse(c.x, c.y, w, h, Color::new(0.93, 0.92, 1.0, 0.95 * fade));
        // The iris and pupil, looking out over the arena, and a glint.
        let r = (2.8 * open).min(h);
        draw_circle(c.x, c.y, r, Color::new(MICHELLE.r, MICHELLE.g, MICHELLE.b, fade));
        draw_circle(c.x, c.y, r * 0.45, Color::new(0.05, 0.04, 0.12, fade));
        draw_rectangle((c.x + 1.0).round(), (c.y - 1.0).round(), 1.0, 1.0, Color::new(1.0, 1.0, 1.0, fade));
    }
    // The lids.
    ellipse_outline(c.x, c.y, w, h.max(0.5), 1.0, Color::new(MICHELLE.r, MICHELLE.g, MICHELLE.b, fade));
}

/// Someone Vid is riding: a black silhouette of them with green 0s and 1s raining down through
/// it, bright at the head of each stream.
fn draw_ridden(v: &FighterVis, f: &Fighter, time: f32) {
    let black = Color::new(0.02, 0.03, 0.02, 1.0);
    let dark = Look { body: black, legs: [black, black], arms: black, hair: Some(black), accent: Color::new(0.05, 0.25, 0.08, 1.0), headband: false, skin: black, hair_style: HairStyle::Short, beard: false, glasses: false, goatee: false, stubble: None, belt: false, beaver_suit: false, mustache: false, backwards_cap: None, grey_flecks: false };
    draw_human(v, f, &dark, false);
    let (x0, top, h) = (f.pos.x.round() - 3.0, (f.pos.y - f.height()).round() - 1.0, f.height() + 1.0);
    for col in 0..4 {
        let x = x0 + col as f32 * 2.0;
        let head = (time * 18.0 + col as f32 * 5.3) % (h + 6.0);
        for k in 0..5 {
            let y = head - k as f32 * 3.0;
            if y < 0.0 || y > h - 1.0 {
                continue;
            }
            let c = if k == 0 { Color::new(0.75, 1.0, 0.80, 1.0) } else { Color::new(CODE.r, CODE.g, CODE.b, 1.0 - k as f32 * 0.18) };
            // A "1" is a short stroke, a "0" a little block.
            if hash(col, (time * 6.0) as i32 + k, 33) < 0.5 {
                draw_rectangle(x, top + y, 1.0, 2.0, c);
            } else {
                draw_rectangle(x, top + y, 2.0, 2.0, c);
            }
        }
    }
}

/// Vid going into (or out of) someone: a column of green code, collapsing in or opening out.
fn draw_code_burst(c: Vec2, t: f32, out: bool, time: f32) {
    let k = (t / HOLO_SECS).clamp(0.0, 1.0);
    let open = if out { k } else { 1.0 - k };
    let fade = if out { 1.0 - k } else { 1.0 };
    for col in -3..=3 {
        let x = c.x + col as f32 * 2.0 * open.max(0.15);
        for row in 0..6 {
            let y = c.y - 9.0 + ((row as f32 * 3.0 + time * 40.0 + col as f32 * 7.0) % 18.0);
            let a = (0.9 - row as f32 * 0.12) * fade;
            draw_rectangle(x.round(), y.round(), 1.0, 2.0, Color::new(CODE.r, CODE.g, CODE.b, a));
        }
    }
    draw_circle(c.x, c.y, 10.0 * open + 2.0, Color::new(CODE.r, CODE.g, CODE.b, 0.12 * fade));
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

/// A rocket: a dull grey body with a rust-red nose, pointing where it's going, a flickering flame
/// behind. (Its smoke trail is particles, from `update`.)
fn draw_rocket(r: &Rocket, time: f32) {
    let d = r.vel.normalize_or(vec2(1.0, 0.0));
    let back = r.pos - d * 4.0;
    line(back, r.pos, 2.0, Color::new(0.45, 0.46, 0.44, 1.0));
    line(r.pos, r.pos + d * 1.5, 2.0, Color::new(0.62, 0.24, 0.16, 1.0));
    let flick = 1.5 + ((time * 50.0).sin() * 0.5 + 0.5) * 2.0;
    line(back, back - d * flick, 2.0, Color::new(1.0, 0.62, 0.18, 0.9));
    draw_circle(back.x, back.y, 2.5, Color::new(1.0, 0.6, 0.2, 0.2));
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

/// The giant sword from its grip `at`, pointing `dir`: a leather hilt, a guard across, then a
/// long, broad blade with a darker edge. (`flash`: tinted when its holder is hit.)
fn draw_blade(at: Vec2, dir: Vec2, flash: bool) {
    let side = vec2(-dir.y, dir.x);
    line(at - dir * 3.0, at, 1.0, tint(Color::new(0.30, 0.22, 0.16, 1.0), flash));
    line(at - side * 2.5, at + side * 2.5, 1.0, tint(Color::new(0.55, 0.46, 0.26, 1.0), flash));
    line(at + dir, at + dir * 15.0, 3.0, tint(STEEL_DARK, flash));
    line(at + dir, at + dir * 15.0, 1.5, tint(STEEL, flash));
    line(at + dir * 15.0, at + dir * 17.5, 1.0, tint(STEEL, flash));
}

/// A held giant sword for the current swing phase (returns where the hand is): held at the hip
/// with the blade angled up in front (a ready stance), raised up and back on the wind-up, then a big arc over and down in front (with a pale
/// trail through the air), settling back after.
fn draw_sword(f: &Fighter, _neck: Vec2, flash: bool) -> Vec2 {
    let s = f.facing;
    let (x, fy) = (f.pos.x, f.pos.y);
    // Angle in degrees: 90 straight up, 0 straight ahead, negative pointing down in front.
    let dir = |deg: f32| {
        let a = deg.to_radians();
        vec2(s * a.cos(), -a.sin())
    };
    let rest = (vec2(x + s * 3.0, fy - 7.0), 55.0);
    let (hand, deg) = match f.atk_timer {
        0 => rest,
        t if t <= ATK_WINDUP => {
            let k = t as f32 / ATK_WINDUP as f32;
            (rest.0.lerp(vec2(x - s * 1.0, fy - 12.0), k), 55.0 + 85.0 * k)
        }
        t if t <= ATK_WINDUP + ATK_ACTIVE => {
            let k = (t - ATK_WINDUP) as f32 / ATK_ACTIVE as f32;
            let deg = 140.0 - 185.0 * k;
            // The trail: where the blade swept through, fading behind it.
            for j in 1..6 {
                let d = dir(deg + j as f32 * 14.0);
                let h = vec2(x + s * 4.0, fy - 9.0);
                let a = 0.35 - j as f32 * 0.06;
                line(h + d * 6.0, h + d * 17.0, 1.0, Color::new(0.85, 0.88, 0.92, a));
            }
            (vec2(x + s * 4.0, fy - 9.0), deg)
        }
        t => {
            let k = (t - ATK_WINDUP - ATK_ACTIVE) as f32 / (ATK_TOTAL - ATK_WINDUP - ATK_ACTIVE) as f32;
            (vec2(x + s * 4.0, fy - 9.0).lerp(rest.0, k), -45.0 + (rest.1 + 45.0) * k)
        }
    };
    draw_blade(hand, dir(deg), flash);
    hand
}

/// Burning ground: a strip of flickering flames, dark at the roots and yellow at the tips, with
/// a low glow; it dies down at the end.
fn draw_fire(fire: &Fire, time: f32) {
    let fade = (fire.ticks as f32 * DT / 0.6).min(1.0);
    let x0 = (fire.at.x - fire.half_w).round();
    let y = fire.at.y.round();
    draw_rectangle(x0, y - 1.0, (fire.half_w * 2.0).round(), 1.0, Color::new(EMBER_BASE.r, EMBER_BASE.g, EMBER_BASE.b, fade));
    fill_ellipse(fire.at.x, y - 3.0, fire.half_w + 4.0, 6.0, Color::new(FLAME.r, FLAME.g, FLAME.b, 0.10 * fade));
    let n = (fire.half_w * 2.0 / 2.0) as i32;
    let frame = (time * 14.0) as i32;
    for k in 0..n {
        let x = x0 + k as f32 * 2.0;
        let h = ((1.5 + hash(k, frame, 91) * 5.0) * fade).round();
        if h < 1.0 {
            continue;
        }
        let w = if hash(k, frame, 92) < 0.5 { 1.0 } else { 2.0 };
        draw_rectangle(x, y - h, w, h, Color::new(FLAME.r, FLAME.g, FLAME.b, fade));
        draw_rectangle(x, y - h, w, (h * 0.4).ceil(), Color::new(FLAME_TIP.r, FLAME_TIP.g, FLAME_TIP.b, fade));
    }
}

/// The air strike's jet: a dark, swept-wing silhouette heading `dir`, with a contrail behind.
fn draw_plane(p: &Plane, time: f32) {
    if p.delay > 0 {
        return;
    }
    let (x, y, s) = (p.pos.x.round(), p.pos.y.round(), p.dir);
    let body = Color::new(0.30, 0.33, 0.37, 1.0);
    let dark = Color::new(0.18, 0.20, 0.23, 1.0);
    // A rectangle in jet space (dx forward), mirrored for a jet flying left.
    let px = |dx: f32, dy: f32, w: f32, h: f32, c: Color| draw_rectangle(if s > 0.0 { x + dx } else { x - dx - w + 1.0 }, y + dy, w, h, c);
    px(-8.0, -1.0, 15.0, 2.0, body);
    px(7.0, 0.0, 2.0, 1.0, body);
    px(-2.0, 1.0, 6.0, 1.0, dark);
    px(-4.0, 2.0, 4.0, 1.0, dark);
    px(-8.0, -3.0, 2.0, 2.0, dark);
    px(3.0, -1.0, 2.0, 1.0, Color::new(0.55, 0.62, 0.66, 1.0));
    // Exhaust and contrail.
    let flick = if (time * 40.0).sin() > 0.0 { 1.0 } else { 2.0 };
    px(-8.0 - flick, -1.0, flick, 1.0, FLAME_TIP);
    let tail = x - s * 10.0;
    draw_line(tail, y, tail - s * 60.0, y, 1.0, Color::new(0.85, 0.85, 0.82, 0.18));
}

/// An air-strike bomb: a dark olive casing with fins, nose down along its fall.
fn draw_air_bomb(b: &AirBomb) {
    let d = b.vel.normalize_or(vec2(0.0, 1.0));
    line(b.pos - d * 3.0, b.pos + d * 2.0, 2.0, Color::new(0.28, 0.30, 0.22, 1.0));
    let side = vec2(-d.y, d.x);
    line(b.pos - d * 3.0 - side * 1.5, b.pos - d * 3.0 + side * 1.5, 1.0, Color::new(0.20, 0.21, 0.17, 1.0));
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
