# TODO

## Cloud

Done: see `docs/DEPLOY.md` (Fly.io: Dockerfile, fly.toml, GitHub deploy workflow; SIGTERM,
`/healthz`, pings and idle timeout, origin allowlist, rate limits, per-address cap, room-creation
passphrase, pre-compressed files, built-in bot brain).

- [ ] Run the GitHub workflow once and confirm the tests pass on Linux too (the match fingerprint
      is meant to be identical across platforms).

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
