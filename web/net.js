// Bridge between the Rust client (wasm) and the browser: the WebSocket to the server, the
// page URL (?room=CODE), localStorage (your name) and the clipboard. Registered as a miniquad
// plugin, so these become imports of the wasm module.
"use strict";
(function () {
  let ws = null;
  let state = 0; // 0 connecting, 1 open, 2 closed
  const inbox = [];
  const mem = (ptr, len) => new Uint8Array(wasm_memory.buffer, ptr, len);
  const str = (ptr, len) => new TextDecoder().decode(mem(ptr, len));
  const put = (s, ptr, cap) => {
    const bytes = new TextEncoder().encode(s).slice(0, cap);
    mem(ptr, bytes.length).set(bytes);
    return bytes.length;
  };

  miniquad_add_plugin({
    name: "go_box_net",
    version: "1",
    register_plugin(importObject) {
      const env = importObject.env;
      env.go_ws_open = () => {
        const proto = location.protocol === "https:" ? "wss:" : "ws:";
        state = 0;
        ws = new WebSocket(proto + "//" + location.host + "/ws");
        ws.binaryType = "arraybuffer";
        ws.onopen = () => { state = 1; };
        ws.onclose = ws.onerror = () => { state = 2; };
        ws.onmessage = (e) => { if (e.data instanceof ArrayBuffer) inbox.push(new Uint8Array(e.data)); };
      };
      env.go_ws_state = () => state;
      env.go_ws_send = (ptr, len) => { if (state === 1) ws.send(mem(ptr, len).slice()); };
      env.go_ws_next_len = () => (inbox.length ? inbox[0].length : -1);
      env.go_ws_take = (ptr) => { const m = inbox.shift(); mem(ptr, m.length).set(m); };

      // kind 0: URL query parameter `key`; 1: localStorage `key`; 2: share link for room `key`.
      env.go_text = (kind, kp, kl, out, cap) => {
        const key = str(kp, kl);
        let v = "";
        try {
          if (kind === 0) v = new URLSearchParams(location.search).get(key) || "";
          else if (kind === 1) v = localStorage.getItem(key) || "";
          else if (kind === 2) { const u = new URL(location.href); u.search = ""; u.hash = ""; u.searchParams.set("room", key); v = u.toString(); }
        } catch (e) { /* storage blocked, etc. */ }
        return put(v, out, cap);
      };
      // kind 0: set (or clear, if empty) URL query parameter; 1: localStorage; 2: copy `val`.
      env.go_text_set = (kind, kp, kl, vp, vl) => {
        const key = str(kp, kl), val = str(vp, vl);
        try {
          if (kind === 0) {
            const u = new URL(location.href);
            if (val) u.searchParams.set(key, val); else u.searchParams.delete(key);
            history.replaceState(null, "", u);
          } else if (kind === 1) localStorage.setItem(key, val);
          else if (kind === 2) navigator.clipboard.writeText(val);
        } catch (e) { /* not allowed here */ }
      };
    },
  });
})();
