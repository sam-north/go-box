//! The pictures in "How to Play": a little scripted match, played over and over. On the left
//! someone throws a grenade past the items lying at their feet; on the right Vid reads, his books
//! going after the one standing next to him. Drawn whole into its own picture; the popup shows
//! the two parts of it (`ITEMS_VIEW`, `SPECIAL_VIEW`).

use go_box_render::Art;
use go_box_sim::brain::Brain;
use go_box_sim::character::Character;
use go_box_sim::sim::{Action, DT, Event, FLOOR_Y, H, ITEM_R, Item, ItemKind, Match, MatchConfig, N_FIGHTERS, TICK_RATE, W};
use macroquad::prelude::*;

/// The items on show, in the order they lie on the floor (left to right).
pub const ITEMS: [ItemKind; 4] = [ItemKind::Grenade, ItemKind::Molotov, ItemKind::Spear, ItemKind::RocketLauncher];
/// Where the items are lying.
const ITEM_XS: [f32; 4] = [22.0, 34.0, 48.0, 64.0];
/// The parts of the arena the popup shows (16:9).
pub const ITEMS_VIEW: Rect = Rect::new(12.0, 196.0, 160.0, 64.0);
pub const SPECIAL_VIEW: Rect = Rect::new(358.0, 196.0, 120.0, 64.0);

const THROWER: usize = 0;
const VID: usize = 1;
const DUMMY: usize = 2;
/// When the thrower throws (they're handed a fresh grenade a little before each).
const THROWS: [u32; 2] = [TICK_RATE / 2, TICK_RATE * 3];
/// When Vid opens his book.
const READ_AT: u32 = TICK_RATE / 3;
/// How long one go lasts before it starts again.
const LOOP: u32 = TICK_RATE * 5;

pub struct HowTo {
    m: Match,
    art: Art,
    brain: Brain,
    acc: f32,
    pub rt: RenderTarget,
}

impl HowTo {
    pub fn new() -> Self {
        let rt = render_target(W as u32, H as u32);
        rt.texture.set_filter(FilterMode::Nearest);
        Self { m: Self::fresh(), art: Art::new(), brain: Brain { w: vec![0.0; Brain::N_WEIGHTS] }, acc: 0.0, rt }
    }

    fn fresh() -> Match {
        let mut m = Match::new(1, &MatchConfig { heals: 0..=0, countdown: 0, spawn_grace: 0, unstick: false, ..MatchConfig::default() });
        m.items.clear();
        for (&kind, &x) in ITEMS.iter().zip(&ITEM_XS) {
            m.items.push(Item::new(kind, vec2(x, FLOOR_Y - ITEM_R)));
        }
        for (i, (c, x, facing)) in [(Character::Classic, 92.0, 1.0), (Character::Vid, 404.0, 1.0), (Character::Classic, 430.0, -1.0)].into_iter().enumerate() {
            m.f[i].character = c;
            m.f[i].pos = vec2(x, FLOOR_Y);
            m.f[i].facing = facing;
        }
        // (The one Vid's books go after just stands there.)
        m.f[DUMMY].special_cd = u32::MAX / 2;
        for i in DUMMY + 1..N_FIGHTERS {
            m.remove(i);
        }
        m
    }

    /// Plays on in real time, then draws it into `rt`.
    pub fn frame(&mut self, dt: f32) {
        self.acc = (self.acc + dt).min(0.25);
        let mut events = Vec::new();
        while self.acc >= DT {
            self.acc -= DT;
            self.tick(&mut events);
        }
        self.m.events = events;
        self.art.handle_events(&self.m);
        self.art.update(dt, &self.m);
        self.art.draw_sprites(&self.m);
        let mut cam = Camera2D { target: vec2(W / 2.0, H / 2.0), zoom: vec2(2.0 / W, 2.0 / H), ..Default::default() };
        cam.render_target = Some(self.rt.clone());
        set_camera(&cam);
        clear_background(BLACK);
        self.art.draw(&self.m);
    }

    fn tick(&mut self, events: &mut Vec<Event>) {
        if self.m.tick >= LOOP {
            self.m = Self::fresh();
            self.art.reset();
        }
        let t = self.m.tick;
        let m = &mut self.m;
        if THROWS.iter().any(|&at| t + TICK_RATE / 3 == at) && m.f[THROWER].holding.is_none() {
            let mut grenade = Item::new(ItemKind::Grenade, m.f[THROWER].hand());
            grenade.held_by = Some(THROWER);
            m.items.push(grenade);
            m.f[THROWER].holding = Some(ItemKind::Grenade);
        }
        let throw = THROWS.contains(&t);
        let thrower = Action { throw, target: throw.then(|| vec2(0.27, -1.0).normalize()), ..Action::default() };
        let vid = Action { special: t == READ_AT, ..Action::default() };
        let mut humans = [None; N_FIGHTERS];
        humans[THROWER] = Some(thrower);
        humans[VID] = Some(vid);
        let b = &self.brain;
        m.step_humans([b; N_FIGHTERS], humans);
        // No new items dropping in: just the ones on show.
        let spawned: Vec<Vec2> = m.events.iter().filter_map(|e| if let Event::Spawn { pos } = e { Some(*pos) } else { None }).collect();
        m.items.retain(|it| !spawned.contains(&it.pos));
        events.extend(m.events.iter().filter(|e| !matches!(e, Event::Spawn { .. })));
        // Nobody stays down: the dummy is back on its feet for the next go.
        m.f[DUMMY].hp = m.f[DUMMY].hp.max(30.0);
    }
}
