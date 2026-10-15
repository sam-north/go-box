//! The few things that differ between the browser and a native window: the page URL, saved
//! settings, the clipboard. In the browser they go through `web/net.js`.

#[cfg(target_arch = "wasm32")]
mod imp {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn go_text(kind: i32, key: *const u8, key_len: u32, out: *mut u8, cap: u32) -> u32;
        fn go_text_set(kind: i32, key: *const u8, key_len: u32, val: *const u8, val_len: u32);
    }

    fn get(kind: i32, key: &str) -> String {
        let mut buf = vec![0u8; 1024];
        let n = unsafe { go_text(kind, key.as_ptr(), key.len() as u32, buf.as_mut_ptr(), buf.len() as u32) } as usize;
        buf.truncate(n);
        String::from_utf8(buf).unwrap_or_default()
    }

    fn set(kind: i32, key: &str, val: &str) {
        unsafe { go_text_set(kind, key.as_ptr(), key.len() as u32, val.as_ptr(), val.len() as u32) }
    }

    /// The room code in the page's `?room=` link, if any.
    pub fn url_room() -> Option<String> {
        Some(get(0, "room")).filter(|r| !r.is_empty())
    }

    /// Keeps `?room=CODE` in the address bar while in a room, so the link can be copied or the
    /// page reloaded.
    pub fn set_url_room(code: Option<&str>) {
        set(0, "room", code.unwrap_or(""));
    }

    pub fn load_name() -> String {
        get(1, "go-box-name")
    }

    pub fn save_name(name: &str) {
        set(1, "go-box-name", name);
    }

    /// A saved setting (localStorage), "" if never set.
    pub fn load_pref(key: &str) -> String {
        get(1, key)
    }

    pub fn save_pref(key: &str, val: &str) {
        set(1, key, val);
    }

    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn go_pad(out: *mut f32) -> i32;
    }

    /// The first connected gamepad (the browser's Gamepad API, standard mapping).
    pub fn gamepad() -> super::Pad {
        let mut v = [0.0f32; 5];
        if unsafe { go_pad(v.as_mut_ptr()) } == 0 {
            return super::Pad::default();
        }
        super::Pad { connected: true, left: macroquad::math::vec2(v[0], v[1]), right: macroquad::math::vec2(v[2], v[3]), buttons: v[4] as u32 }
    }

    pub fn share_link(code: &str) -> String {
        get(2, code)
    }

    pub fn copy(text: &str) {
        set(2, "", text);
    }

    /// Reloads the page (to pick up a newer build).
    pub fn reload() {
        set(4, "", "");
    }

    /// Whatever was pasted into the page since last asked (the browser's paste event).
    pub fn take_paste() -> Option<String> {
        Some(get(3, "")).filter(|s| !s.is_empty())
    }

    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn go_now_ms() -> f64;
        fn go_key_ms() -> f64;
    }

    /// The browser's clock, in ms.
    pub fn now_ms() -> f64 {
        unsafe { go_now_ms() }
    }

    /// When the last key went down, on the same clock (the event's own timestamp), if any has.
    pub fn last_key_ms() -> Option<f64> {
        Some(unsafe { go_key_ms() }).filter(|&t| t >= 0.0)
    }

    /// Debug mode (see `debug.rs`): `?debug=1` in the URL turns it on and remembers that in this
    /// browser, `?debug=0` turns it off again.
    pub fn debug_mode() -> bool {
        match get(0, "debug").as_str() {
            "1" => set(1, "go-box-debug", "1"),
            "0" => set(1, "go-box-debug", ""),
            _ => {}
        }
        get(1, "go-box-debug") == "1"
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use crate::testclient::Options;

    pub fn url_room() -> Option<String> {
        Options::get().join.clone()
    }

    pub fn set_url_room(_code: Option<&str>) {}

    pub fn load_name() -> String {
        Options::get().name.clone().unwrap_or_default()
    }

    pub fn save_name(_name: &str) {}

    /// Settings aren't saved natively (the native client is a test client), except that
    /// `--mute` starts it silent.
    pub fn load_pref(key: &str) -> String {
        if key == "go-box-muted" && Options::get().mute { "1".into() } else { String::new() }
    }

    pub fn save_pref(_key: &str, _val: &str) {}

    /// No gamepad support in the native test client.
    pub fn gamepad() -> super::Pad {
        super::Pad::default()
    }

    pub fn share_link(code: &str) -> String {
        format!("{}/?room={code}", Options::get().server.trim_end_matches(go_box_protocol::WS_PATH).replace("ws://", "http://").replace("wss://", "https://"))
    }

    pub fn copy(text: &str) {
        macroquad::miniquad::window::clipboard_set(text);
    }

    pub fn reload() {}

    pub fn now_ms() -> f64 {
        macroquad::time::get_time() * 1000.0
    }

    /// (Natively there's no event timestamp to read.)
    pub fn last_key_ms() -> Option<f64> {
        None
    }

    /// Debug mode: `--debug`, or GOBOX_DEBUG set.
    pub fn debug_mode() -> bool {
        Options::get().debug || std::env::var_os("GOBOX_DEBUG").is_some()
    }

    /// The clipboard, when Ctrl+V was just pressed.
    pub fn take_paste() -> Option<String> {
        use macroquad::input::{KeyCode, is_key_down, is_key_pressed};
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if ctrl && is_key_pressed(KeyCode::V) { macroquad::miniquad::window::clipboard_get() } else { None }
    }
}

pub use imp::*;

/// A gamepad's state this frame: sticks in -1..1 (y down), and a bitmask of held buttons by
/// their standard-mapping number.
#[derive(Clone, Copy, Default, Debug)]
pub struct Pad {
    pub connected: bool,
    pub left: macroquad::math::Vec2,
    pub right: macroquad::math::Vec2,
    pub buttons: u32,
}
