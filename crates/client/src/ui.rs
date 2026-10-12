//! A tiny immediate-mode UI: buttons, text fields, panels and text, drawn in screen pixels on
//! top of the arena.

use macroquad::prelude::*;

pub const INK: Color = Color::new(0.92, 0.92, 0.88, 1.0);
pub const DIM: Color = Color::new(0.60, 0.61, 0.64, 1.0);
pub const PANEL: Color = Color::new(0.05, 0.06, 0.08, 0.88);
pub const EDGE: Color = Color::new(0.32, 0.34, 0.40, 1.0);
pub const ACCENT: Color = Color::new(0.95, 0.75, 0.30, 1.0);
pub const BAD: Color = Color::new(0.95, 0.42, 0.36, 1.0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Name,
    Code,
    /// Shown as stars.
    Passphrase,
}

/// Menu navigation from a gamepad this frame (fresh presses only).
#[derive(Clone, Copy, Default, Debug)]
pub struct Nav {
    /// A d-pad / stick press: one of the four unit directions (y down).
    pub dir: Option<Vec2>,
    /// Press the focused control (Cross).
    pub activate: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slider {
    Master,
    Effects,
    Music,
}

pub struct Ui {
    /// UI scale: 1.0 at 720px tall.
    pub s: f32,
    mouse: Vec2,
    clicked: bool,
    pub focus: Option<Field>,
    typed: Vec<char>,
    backspace: bool,
    pub enter: bool,
    /// Something consumed this frame's click (so a click on a button doesn't also unfocus).
    hot: bool,
    /// A menu is open on top: everything drawn under it ignores the mouse.
    pub blocked: bool,
    /// The slider being dragged.
    dragging: Option<Slider>,
    /// A button was pressed this frame (for the click sound).
    pub pressed: bool,
    /// Gamepad navigation: show and move a focus between controls.
    pub pad_mode: bool,
    nav: Nav,
    /// Left/right on a focused slider this frame.
    adjust: f32,
    /// Controls that can take the focus, as drawn this frame and last frame (rect, is a slider).
    targets: Vec<(Rect, bool)>,
    prev_targets: Vec<(Rect, bool)>,
    /// The focused control, by where it was drawn.
    nav_focus: Option<Rect>,
}

fn same_rect(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() < 2.0 && (a.y - b.y).abs() < 2.0 && (a.w - b.w).abs() < 2.0 && (a.h - b.h).abs() < 2.0
}

impl Ui {
    pub fn new() -> Self {
        Self { s: 1.0, mouse: Vec2::ZERO, clicked: false, focus: None, typed: Vec::new(), backspace: false, enter: false, hot: false,
            blocked: false,
            dragging: None,
            pressed: false,
            pad_mode: false,
            nav: Nav::default(),
            adjust: 0.0,
            targets: Vec::new(),
            prev_targets: Vec::new(),
            nav_focus: None,
        }
    }

    pub fn begin(&mut self, nav: Nav, pad_mode: bool) {
        self.pad_mode = pad_mode;
        self.prev_targets = std::mem::take(&mut self.targets);
        self.navigate(nav);
        self.s = (screen_height() / 720.0).clamp(0.6, 2.5).min(screen_width() / 900.0).max(0.5);
        self.mouse = mouse_position().into();
        self.clicked = is_mouse_button_pressed(MouseButton::Left);
        self.hot = false;
        self.pressed = false;
        self.typed.clear();
        while let Some(c) = get_char_pressed() {
            self.typed.push(c);
        }
        self.backspace = is_key_pressed(KeyCode::Backspace);
        self.enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
    }

    /// Clicking empty space drops keyboard focus.
    pub fn end(&mut self) {
        if self.clicked && !self.hot {
            self.focus = None;
        }
    }

    pub fn px(&self, v: f32) -> f32 {
        (v * self.s).round()
    }

    pub fn text(&self, t: &str, x: f32, y: f32, size: f32, c: Color) {
        draw_text(t, x.round(), y.round(), self.px(size), c);
    }

    pub fn text_width(&self, t: &str, size: f32) -> f32 {
        measure_text(t, None, self.px(size) as u16, 1.0).width
    }

    /// Splits `t` into lines no wider than `width` (breaking at spaces).
    pub fn wrap(&self, t: &str, size: f32, width: f32) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for word in t.split_whitespace() {
            match lines.last_mut() {
                Some(line) if self.text_width(&format!("{line} {word}"), size) <= width => {
                    line.push(' ');
                    line.push_str(word);
                }
                _ => lines.push(word.to_string()),
            }
        }
        lines
    }

    /// `t` cut short with ".." if it's wider than `width`.
    pub fn fit(&self, t: &str, size: f32, width: f32) -> String {
        if self.text_width(t, size) <= width {
            return t.to_string();
        }
        let mut s: String = t.to_string();
        while !s.is_empty() && self.text_width(&format!("{s}.."), size) > width {
            s.pop();
        }
        format!("{s}..")
    }

    pub fn text_centered(&self, t: &str, cx: f32, y: f32, size: f32, c: Color) {
        self.text(t, cx - self.text_width(t, size) / 2.0, y, size, c);
    }

    pub fn panel(&self, r: Rect) {
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, EDGE);
    }

    fn hovered(&self, r: Rect) -> bool {
        !self.blocked && r.contains(self.mouse)
    }

    /// Moves the gamepad focus: to the nearest control in the pressed direction (judged on
    /// last frame's layout), or onto the first control if nothing has it. Left/right on a
    /// slider adjusts it instead.
    fn navigate(&mut self, nav: Nav) {
        self.nav = nav;
        self.adjust = 0.0;
        if !self.pad_mode {
            return;
        }
        let current = self.nav_focus.and_then(|f| self.prev_targets.iter().find(|(r, _)| same_rect(*r, f)).copied());
        let Some((cur, on_slider)) = current else {
            self.nav_focus = self.prev_targets.first().map(|t| t.0);
            return;
        };
        let Some(d) = nav.dir else { return };
        if on_slider && d.y == 0.0 {
            self.adjust = d.x;
            self.nav.dir = None;
            return;
        }
        let from = cur.center();
        let best = self
            .prev_targets
            .iter()
            .filter(|(r, _)| !same_rect(*r, cur))
            .filter_map(|(r, _)| {
                let v = r.center() - from;
                let along = v.dot(d);
                // Mostly in that direction; prefer close and well lined up.
                (along > 1.0).then(|| (along + 2.0 * (v - d * along).length(), *r))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, r)) = best {
            self.nav_focus = Some(r);
        }
    }

    /// Offers `r` to the gamepad focus; true if it has it.
    fn nav_target(&mut self, r: Rect, slider: bool) -> bool {
        if self.blocked {
            return false;
        }
        self.targets.push((r, slider));
        self.pad_mode && self.nav_focus.is_some_and(|f| same_rect(f, r))
    }

    fn draw_focus(&self, r: Rect) {
        let g = self.px(4.0);
        draw_rectangle_lines(r.x - g, r.y - g, r.w + g * 2.0, r.h + g * 2.0, 3.0, ACCENT);
    }

    /// A clickable area the caller draws itself (e.g. a character tile): whether it was clicked
    /// (or pressed with the gamepad), and whether the mouse is over it (or the gamepad is on it).
    pub fn area_hover(&mut self, r: Rect) -> (bool, bool) {
        let focused = self.nav_target(r, false);
        if focused {
            self.draw_focus(r);
        }
        let hovered = self.hovered(r);
        let clicked = (hovered && self.clicked) || (focused && self.nav.activate);
        if clicked {
            self.hot = true;
            self.pressed = true;
        }
        (clicked, hovered || focused)
    }

    /// A button; true when clicked.
    pub fn button(&mut self, r: Rect, label: &str, enabled: bool, color: Color) -> bool {
        let hover = enabled && self.hovered(r);
        let fill = if !enabled {
            Color::new(0.12, 0.12, 0.14, 0.9)
        } else if hover {
            Color::new(color.r * 0.55, color.g * 0.55, color.b * 0.55, 0.95)
        } else {
            Color::new(color.r * 0.32, color.g * 0.32, color.b * 0.32, 0.95)
        };
        draw_rectangle(r.x, r.y, r.w, r.h, fill);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if enabled { color } else { EDGE });
        let size = (r.h / self.s * 0.5).min(24.0);
        let tw = self.text_width(label, size);
        self.text(label, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + self.px(size) * 0.3, size, if enabled { INK } else { DIM });
        let focused = enabled && self.nav_target(r, false);
        if focused {
            self.draw_focus(r);
        }
        let clicked = (hover && self.clicked) || (focused && self.nav.activate);
        if clicked {
            self.hot = true;
            self.pressed = true;
        }
        clicked
    }

    /// A 0..1 slider: click or drag along it. Returns true while the value changes.
    pub fn slider(&mut self, id: Slider, r: Rect, value: &mut f32) -> bool {
        if self.clicked && self.hovered(Rect::new(r.x, r.y - r.h, r.w, r.h * 3.0)) {
            self.dragging = Some(id);
            self.hot = true;
        }
        if !is_mouse_button_down(MouseButton::Left) {
            self.dragging = None;
        }
        let mut changed = false;
        if self.dragging == Some(id) {
            let v = ((self.mouse.x - r.x) / r.w).clamp(0.0, 1.0);
            changed = v != *value;
            *value = v;
        }
        // The hit area is taller than the bar, so the focus outline sits around that.
        let focus_r = Rect::new(r.x, r.y - r.h, r.w, r.h * 3.0);
        if self.nav_target(focus_r, true) {
            self.draw_focus(focus_r);
            if self.adjust != 0.0 {
                let v = ((*value * 20.0).round() / 20.0 + self.adjust * 0.05).clamp(0.0, 1.0);
                changed |= v != *value;
                *value = v;
            }
        }
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.02, 0.02, 0.03, 0.95));
        draw_rectangle(r.x, r.y, r.w * *value, r.h, ACCENT);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, EDGE);
        let kx = r.x + r.w * *value;
        draw_rectangle(kx - self.px(4.0), r.y - self.px(5.0), self.px(8.0), r.h + self.px(10.0), INK);
        changed
    }

    /// A single-line text box. `filter` maps typed characters (None = rejected). Returns true
    /// when Enter is pressed in it.
    pub fn field(&mut self, id: Field, r: Rect, text: &mut String, placeholder: &str, max: usize, filter: impl Fn(char) -> Option<char>) -> bool {
        if self.clicked && self.hovered(r) {
            self.focus = Some(id);
            self.hot = true;
        }
        let focused = self.focus == Some(id);
        if focused {
            for &c in &self.typed {
                if let Some(c) = filter(c)
                    && text.chars().count() < max
                {
                    text.push(c);
                }
            }
            if self.backspace {
                text.pop();
            }
        }
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.02, 0.02, 0.03, 0.95));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if focused { ACCENT } else { EDGE });
        let size = (r.h / self.s * 0.5).min(24.0);
        let display = if id == Field::Passphrase { "*".repeat(text.chars().count()) } else { text.clone() };
        let (shown, c) = if text.is_empty() && !focused { (placeholder.to_string(), DIM) } else { (display.clone(), INK) };
        let x = r.x + self.px(10.0);
        let y = r.y + r.h / 2.0 + self.px(size) * 0.3;
        self.text(&shown, x, y, size, c);
        if focused && (get_time() * 2.0).fract() < 0.5 {
            let cx = x + self.text_width(&display, size) + 2.0;
            draw_rectangle(cx, r.y + r.h * 0.22, 2.0, r.h * 0.56, INK);
        }
        focused && self.enter
    }
}
