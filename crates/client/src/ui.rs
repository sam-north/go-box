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
    /// A button was pressed this frame (for the click sound).
    pub pressed: bool,
}

impl Ui {
    pub fn new() -> Self {
        Self { s: 1.0, mouse: Vec2::ZERO, clicked: false, focus: None, typed: Vec::new(), backspace: false, enter: false, hot: false, pressed: false }
    }

    pub fn begin(&mut self) {
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

    pub fn text_centered(&self, t: &str, cx: f32, y: f32, size: f32, c: Color) {
        self.text(t, cx - self.text_width(t, size) / 2.0, y, size, c);
    }

    pub fn panel(&self, r: Rect) {
        draw_rectangle(r.x, r.y, r.w, r.h, PANEL);
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, EDGE);
    }

    fn hovered(&self, r: Rect) -> bool {
        r.contains(self.mouse)
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
        let clicked = hover && self.clicked;
        if clicked {
            self.hot = true;
            self.pressed = true;
        }
        clicked
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
        let (shown, c) = if text.is_empty() && !focused { (placeholder.to_string(), DIM) } else { (text.clone(), INK) };
        let x = r.x + self.px(10.0);
        let y = r.y + r.h / 2.0 + self.px(size) * 0.3;
        self.text(&shown, x, y, size, c);
        if focused && (get_time() * 2.0).fract() < 0.5 {
            let cx = x + self.text_width(text, size) + 2.0;
            draw_rectangle(cx, r.y + r.h * 0.22, 2.0, r.h * 0.56, INK);
        }
        focused && self.enter
    }
}
