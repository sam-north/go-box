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

/// The text-editing keys pressed this frame.
#[derive(Clone, Copy, Default, Debug)]
struct Edit {
    left: bool,
    right: bool,
    home: bool,
    end: bool,
    delete: bool,
    select_all: bool,
    /// Extending the selection (Shift held).
    shift: bool,
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
    /// The focused text box's cursor and the other end of its selection (char indices; equal
    /// when nothing is selected).
    cursor: usize,
    anchor: usize,
    /// The focused box's first visible character, when its text is too long to fit.
    scroll: usize,
    /// Dragging out a selection in this box.
    text_drag: Option<Field>,
    /// When and in which box the last click landed (two quick ones select everything).
    last_click: (f64, Option<Field>),
    /// Editing keys this frame (Backspace, Delete and the arrows repeat while held).
    edit: Edit,
    repeats: Vec<(KeyCode, f64)>,
    /// Tab has moved the focus (shown as a ring, like the gamepad's), and did so this frame.
    tab_mode: bool,
    tab_moved: bool,
    /// Text pasted this frame, and whether copy (Ctrl+C) or cut (Ctrl+X) was pressed.
    pasted: Option<String>,
    copy: bool,
    cut: bool,
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
        Self { s: 1.0, mouse: Vec2::ZERO, clicked: false, focus: None, typed: Vec::new(), cursor: 0, anchor: 0, scroll: 0, text_drag: None, last_click: (0.0, None), edit: Edit::default(), repeats: Vec::new(), tab_mode: false, tab_moved: false, pasted: None, copy: false, cut: false, backspace: false, enter: false, hot: false,
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

    /// `menus`: a menu is up (Tab navigates only then, never in play).
    pub fn begin(&mut self, nav: Nav, pad_mode: bool, menus: bool) {
        self.pad_mode = pad_mode;
        self.prev_targets = std::mem::take(&mut self.targets);
        self.navigate(nav);
        self.s = (screen_height() / 720.0).clamp(0.6, 2.5).min(screen_width() / 900.0).max(0.5);
        self.mouse = mouse_position().into();
        self.clicked = is_mouse_button_pressed(MouseButton::Left);
        self.hot = false;
        self.pressed = false;
        self.typed.clear();
        // (Ctrl or Cmd shortcuts aren't typing.)
        let ctrl = [KeyCode::LeftControl, KeyCode::RightControl, KeyCode::LeftSuper, KeyCode::RightSuper].into_iter().any(is_key_down);
        while let Some(c) = get_char_pressed() {
            if !ctrl {
                self.typed.push(c);
            }
        }
        self.pasted = crate::platform::take_paste();
        self.copy = ctrl && is_key_pressed(KeyCode::C);
        self.cut = ctrl && is_key_pressed(KeyCode::X);
        self.backspace = self.pressed_or_repeat(KeyCode::Backspace);
        self.enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        self.edit = Edit {
            left: self.pressed_or_repeat(KeyCode::Left),
            right: self.pressed_or_repeat(KeyCode::Right),
            home: is_key_pressed(KeyCode::Home),
            end: is_key_pressed(KeyCode::End),
            delete: self.pressed_or_repeat(KeyCode::Delete),
            select_all: ctrl && is_key_pressed(KeyCode::A),
            shift,
        };
        // Tab / Shift+Tab: the next / previous control on screen (as drawn last frame, so only
        // what's visible). Enter presses a focused button.
        self.tab_moved = false;
        self.tab_mode &= menus;
        if menus && is_key_pressed(KeyCode::Tab) && !self.prev_targets.is_empty() {
            let n = self.prev_targets.len();
            let at = self.nav_focus.and_then(|f| self.prev_targets.iter().position(|(r, _)| same_rect(*r, f)));
            let next = match at {
                Some(i) if shift => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
                None if shift => n - 1,
                None => 0,
            };
            self.nav_focus = Some(self.prev_targets[next].0);
            self.tab_mode = true;
            self.tab_moved = true;
            self.focus = None;
        }
        if self.tab_mode && self.enter {
            self.nav.activate = true;
        }
    }

    /// A key just pressed, or held long enough to repeat (macroquad doesn't report repeats).
    fn pressed_or_repeat(&mut self, k: KeyCode) -> bool {
        let now = get_time();
        if is_key_pressed(k) {
            self.repeats.retain(|(c, _)| *c != k);
            self.repeats.push((k, now + 0.45));
            return true;
        }
        if !is_key_down(k) {
            self.repeats.retain(|(c, _)| *c != k);
            return false;
        }
        match self.repeats.iter_mut().find(|(c, _)| *c == k) {
            Some(r) if now >= r.1 => {
                r.1 = now + 0.035;
                true
            }
            _ => false,
        }
    }

    /// Clicking empty space drops keyboard focus; any click ends Tab navigation.
    pub fn end(&mut self) {
        if self.clicked {
            self.tab_mode = false;
            if !self.hot {
                self.focus = None;
            }
        }
        if !is_mouse_button_down(MouseButton::Left) {
            self.text_drag = None;
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
        (self.pad_mode || self.tab_mode) && self.nav_focus.is_some_and(|f| same_rect(f, r))
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
            // Tab carries on from here.
            self.nav_focus = Some(r);
        }
        clicked
    }

    /// A low-key button (e.g. Exit): just an outline and dim text, filling with `color` only
    /// while hovered or focused. True when clicked.
    pub fn ghost_button(&mut self, r: Rect, label: &str, color: Color) -> bool {
        let focused = self.nav_target(r, false);
        let lit = self.hovered(r) || focused;
        let fill = if lit { Color::new(color.r * 0.55, color.g * 0.55, color.b * 0.55, 0.95) } else { Color::new(0.0, 0.0, 0.0, 0.15) };
        draw_rectangle(r.x, r.y, r.w, r.h, fill);
        let edge = if lit { color } else { Color::new(color.r, color.g, color.b, 0.45) };
        draw_rectangle_lines(r.x, r.y, r.w, r.h, if lit { 2.0 } else { 1.0 }, edge);
        let size = (r.h / self.s * 0.5).min(24.0);
        let tw = self.text_width(label, size);
        self.text(label, r.x + (r.w - tw) / 2.0, r.y + r.h / 2.0 + self.px(size) * 0.3, size, if lit { INK } else { DIM });
        if focused {
            self.draw_focus(r);
        }
        let clicked = (self.hovered(r) && self.clicked) || (focused && self.nav.activate);
        if clicked {
            self.hot = true;
            self.pressed = true;
            self.nav_focus = Some(r);
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

    /// A single-line text box, edited like any other: click to place the cursor, drag (or
    /// Shift+arrows, Shift+Home/End) to select, double-click or Ctrl/Cmd+A for everything;
    /// typing and pasting replace the selection; Ctrl+C / Ctrl+X copy or cut the selection (the
    /// whole box if nothing's selected; never the passphrase). Tab moves on. `filter` maps typed
    /// (or pasted) characters (None = rejected). Returns true when Enter is pressed in it.
    pub fn field(&mut self, id: Field, r: Rect, text: &mut String, placeholder: &str, max: usize, filter: impl Fn(char) -> Option<char>) -> bool {
        let size = (r.h / self.s * 0.5).min(24.0);
        let x = r.x + self.px(10.0);
        let mask = |t: &str| if id == Field::Passphrase { "*".repeat(t.chars().count()) } else { t.to_string() };

        // Tab landed here: focus it with everything selected, as browsers do.
        let nav = self.nav_target(r, false);
        if nav && self.tab_moved {
            self.focus = Some(id);
            self.scroll = 0;
            self.anchor = 0;
            self.cursor = text.chars().count();
        }
        if self.clicked && self.hovered(r) {
            let now = get_time();
            let double = self.last_click.1 == Some(id) && now - self.last_click.0 < 0.35;
            self.last_click = (now, Some(id));
            // (A box that wasn't focused is showing its text from the start.)
            if self.focus != Some(id) {
                self.scroll = 0;
            }
            self.focus = Some(id);
            self.nav_focus = Some(r);
            self.hot = true;
            if double {
                self.anchor = 0;
                self.cursor = text.chars().count();
            } else {
                self.cursor = self.index_at(&mask(text), self.scroll, x, size);
                if !self.edit.shift {
                    self.anchor = self.cursor;
                }
                self.text_drag = Some(id);
            }
        }
        let focused = self.focus == Some(id);
        if focused {
            let len = text.chars().count();
            self.cursor = self.cursor.min(len);
            self.anchor = self.anchor.min(len);
            if self.text_drag == Some(id) && is_mouse_button_down(MouseButton::Left) && !self.clicked {
                self.cursor = self.index_at(&mask(text), self.scroll, x, size);
            }
            self.edit_text(id, text, max, &filter);
        }

        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.02, 0.02, 0.03, 0.95));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, if focused { ACCENT } else { EDGE });
        if nav && !focused {
            self.draw_focus(r);
        }
        // Only what fits inside the box is drawn: scrolled along to keep the cursor in view when
        // focused, from the start otherwise.
        let chars: Vec<char> = mask(text).chars().collect();
        let inner = r.w - self.px(20.0);
        let span = |ui: &Ui, a: usize, b: usize| ui.text_width(&chars[a..b].iter().collect::<String>(), size);
        let mut from = 0;
        if focused {
            from = self.scroll.min(self.cursor);
            while from < self.cursor && span(self, from, self.cursor) > inner {
                from += 1;
            }
            // Scroll back as far as still fits (after deleting, say).
            while from > 0 && span(self, from - 1, chars.len()) <= inner && span(self, from - 1, self.cursor) <= inner {
                from -= 1;
            }
            self.scroll = from;
        }
        let mut to = chars.len();
        while to > from && span(self, from, to) > inner {
            to -= 1;
        }
        let y = r.y + r.h / 2.0 + self.px(size) * 0.3;
        let x_of = |ui: &Ui, i: usize| x + span(ui, from, i.clamp(from, to));
        if focused && self.cursor != self.anchor {
            let (a, b) = (self.cursor.min(self.anchor), self.cursor.max(self.anchor));
            let (xa, xb) = (x_of(self, a), x_of(self, b));
            draw_rectangle(xa, r.y + r.h * 0.2, xb - xa, r.h * 0.6, Color::new(ACCENT.r, ACCENT.g, ACCENT.b, 0.35));
        }
        if text.is_empty() && !focused {
            self.text(&self.fit(placeholder, size, inner), x, y, size, DIM);
        } else {
            self.text(&chars[from..to].iter().collect::<String>(), x, y, size, INK);
        }
        if focused && (get_time() * 2.0).fract() < 0.5 {
            let cx = x_of(self, self.cursor) + 1.0;
            draw_rectangle(cx, r.y + r.h * 0.22, 2.0, r.h * 0.56, INK);
        }
        focused && self.enter
    }

    /// Where in `shown` (drawn from `x`, scrolled to start at character `from`) the mouse is:
    /// the nearest gap between characters.
    fn index_at(&self, shown: &str, from: usize, x: f32, size: f32) -> usize {
        let chars: Vec<char> = shown.chars().collect();
        let from = from.min(chars.len());
        let gap = |i: usize| (x + self.text_width(&chars[from..i].iter().collect::<String>(), size) - self.mouse.x).abs();
        (from..=chars.len()).min_by(|&a, &b| gap(a).total_cmp(&gap(b))).unwrap_or(from)
    }

    /// This frame's keys and typing, applied to the focused box's text, cursor and selection.
    fn edit_text(&mut self, id: Field, text: &mut String, max: usize, filter: &impl Fn(char) -> Option<char>) {
        let byte = |t: &str, i: usize| t.char_indices().nth(i).map_or(t.len(), |(b, _)| b);
        let delete = |t: &mut String, a: usize, b: usize| {
            let (a, b) = (byte(t, a), byte(t, b));
            t.replace_range(a..b, "");
        };
        let len = text.chars().count();
        let (lo, hi) = (self.cursor.min(self.anchor), self.cursor.max(self.anchor));
        let selected = lo != hi;
        let e = self.edit;
        if e.select_all {
            self.anchor = 0;
            self.cursor = len;
            return;
        }
        // Copy / cut: the selection, or the whole box if nothing's selected.
        if (self.copy || self.cut) && id != Field::Passphrase && !text.is_empty() {
            let (a, b) = if selected { (lo, hi) } else { (0, len) };
            crate::platform::copy(&text[byte(text, a)..byte(text, b)]);
            if self.cut {
                delete(text, a, b);
                self.cursor = a;
                self.anchor = a;
            }
            return;
        }
        // Moving the cursor (Shift extends the selection; without it, a selection collapses).
        let moved = if e.left {
            Some(if selected && !e.shift { lo } else { self.cursor.saturating_sub(1) })
        } else if e.right {
            Some(if selected && !e.shift { hi } else { (self.cursor + 1).min(len) })
        } else if e.home {
            Some(0)
        } else if e.end {
            Some(len)
        } else {
            None
        };
        if let Some(c) = moved {
            self.cursor = c;
            if !e.shift {
                self.anchor = c;
            }
            return;
        }
        // Deleting.
        if self.backspace || e.delete {
            if selected {
                delete(text, lo, hi);
                self.cursor = lo;
            } else if self.backspace && self.cursor > 0 {
                delete(text, self.cursor - 1, self.cursor);
                self.cursor -= 1;
            } else if e.delete && self.cursor < len {
                delete(text, self.cursor, self.cursor + 1);
            }
            self.anchor = self.cursor;
            return;
        }
        // Typing and pasting replace the selection.
        let mut pasted = self.pasted.as_deref().unwrap_or("");
        // A whole invite link pasted into the room code box: just its code.
        if id == Field::Code
            && let Some((_, code)) = pasted.split_once("room=")
        {
            pasted = code.split('&').next().unwrap_or("");
        }
        let incoming: Vec<char> = self.typed.iter().copied().chain(pasted.chars()).filter_map(filter).collect();
        if incoming.is_empty() {
            return;
        }
        if selected {
            delete(text, lo, hi);
            self.cursor = lo;
        }
        for c in incoming {
            if text.chars().count() >= max {
                break;
            }
            let at = byte(text, self.cursor);
            text.insert(at, c);
            self.cursor += 1;
        }
        self.anchor = self.cursor;
    }
}
