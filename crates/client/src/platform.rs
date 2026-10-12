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

    pub fn share_link(code: &str) -> String {
        get(2, code)
    }

    pub fn copy(text: &str) {
        set(2, "", text);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use crate::Options;

    pub fn url_room() -> Option<String> {
        Options::get().join.clone()
    }

    pub fn set_url_room(_code: Option<&str>) {}

    pub fn load_name() -> String {
        Options::get().name.clone().unwrap_or_default()
    }

    pub fn save_name(_name: &str) {}

    pub fn share_link(code: &str) -> String {
        format!("{}/?room={code}", Options::get().server.trim_end_matches("/ws").replace("ws://", "http://").replace("wss://", "https://"))
    }

    pub fn copy(text: &str) {
        macroquad::miniquad::window::clipboard_set(text);
    }
}

pub use imp::*;
