# TODO

## Cloud (deferred: do before hosting publicly)

From the architecture review (`docs/architecture-review.md`, section 2). Local defaults stay as
they are (`127.0.0.1` only); every cloud difference should be an env var in `server::Config`.

- [ ] **Shutdown**: handle SIGTERM as well as Ctrl+C (`docker stop` otherwise waits 10s and kills).
- [ ] **Health**: `GET /healthz` returning 200 with room/connection counts.
- [ ] **Liveness**: WebSocket ping every ~15s; drop a connection after ~45s of silence, so a dead
      tab doesn't hold a lobby seat or keep a room alive forever.
- [ ] **Origin allowlist**: `GO_BOX_ORIGINS`, checked in `ws_handler` (empty = allow all, local).
- [ ] **Rate limits**: per-connection token bucket (inputs ~120/s, other messages ~10/s); close
      after ~5 failed `JoinRoom`s (room codes are brute-forceable); per-IP connection cap
      (`X-Forwarded-For` behind a proxy, `GO_BOX_TRUST_PROXY=1`).
- [ ] **Static files**: brotli/gzip compression for the wasm (~930 KB); `Cache-Control: no-cache`
      on `index.html`, `.wasm`, `.js` so a new build is always picked up.
- [ ] **Dockerfile**: multi-stage (Rust + wasm32 target builds server and wasm; slim runtime image
      with the server and `web/`); `GO_BOX_BIND=0.0.0.0:8080` set only there.
- [ ] **Embed** the bot brains (and optionally `web/`) in the server binary: one artifact, no
      `GO_BOX_BRAINS`.
- [ ] **TLS**: terminate in front (Caddy on a small VPS, or the platform edge); `net.js` already
      switches to `wss:` on https pages.
- [ ] **Hosting**: smallest shared-CPU VM / ~$5 VPS near the players is plenty for 1-4 matches.

## Later

- [ ] Chat messages / emoji reactions (new `ClientMsg`/`ServerMsg` variants; see the protocol
      crate's notes on adding message kinds).
- [ ] More languages: add `crates/client/text/<lang>.txt` files with the same keys as `en-US.txt`.

## From the architecture review, not done yet

- [ ] Gamepad menu focus is matched by where a control is drawn; give controls ids if layout
      shifts start dropping the focus.
- [ ] Note in `web/gl.js` / `web/audio.js` which miniquad / quad-snd versions they're copied from
      (re-copy them when bumping macroquad).
- [ ] A game-mode picker once there's a second `Mode`.
