# go-box architecture review

*Reviewed 2026-10-12 at commit `0f93515`. I only read the code: nothing was built or run.*

## Executive summary

1. The core is in good shape. The sim is deterministic and tick-based (`TICK_RATE`, `secs()`), the server is authoritative with prediction and interpolation, `Special` is a strategy trait, and the classic-match fingerprint test (`sim.rs` `CLASSIC_HASH`) is a strong regression net. Keep all of these.
2. The biggest future-change risk is the wire format. `MatchView` serializes sim structs (`Fighter`, `Item`, `Event`, `Bomb`, `Dog`) directly, so any sim field change is a protocol change. The only guard is a hand-bumped `PROTOCOL_VERSION`, which has gone 1 to 6 in six commits.
3. A character's data is spread over about 9 places (sim enum and impl, sim `Event`s, `Match` vecs and `senses`, render colour, look and effects, the `Sfx` enum and match, client interpolation, a trainer cast list, a test's `% 4`, README).
4. The arena and items are hard-wired `const`s (`SOLIDS`, `PLATFORMS`, spawn `zones`, `one_hot` sized 3). A 4th `ItemKind` would **panic or corrupt the brain inputs**.
5. Training leftovers are mixed into the game: `fitness`, `close_ticks`, `dmg_by_source`, `duel`, `timed_out`, the ES optimizer, `target-cpu=native`, `archive/`, and stats sent to clients 30 times a second.
6. Deployment gaps: no SIGTERM handling (`docker stop` waits 10s and then kills the server), no health endpoint, no WebSocket ping or idle timeout (a ghost lobby player holds a seat forever), no Origin check, no per-connection rate limit, no cache headers or compression, and no Dockerfile.
7. Determinism has two portability traps. `rand` 0.8 `SmallRng` is a different algorithm on wasm32 than on x86-64, and `CLASSIC_HASH` depends on platform `sin`/`cos`, so it may fail on Linux CI.
8. `client/src/main.rs` (903 lines) mixes app state, network handling, five screens, settings persistence and test-client automation. Split it before adding screens.
9. Test gaps: client prediction and interpolation (`game.rs`) have no tests, the room logic is only tested end-to-end with real timers, and nothing pins the wire format.
10. Top 3: pin the wire format with a golden-bytes test and slim the snapshot (S); a deployment-hardening bundle with a Dockerfile (M); archive the trainer and delete its sim surface (M).

---

## 1. Extensibility seams

### What each kind of change costs today

| Change | Touch points today | Verdict |
|---|---|---|
| **New character** | `character.rs`: enum variant, `ALL`, `name()`, `special()`, a `Special` impl and its consts. If the special has an entity: a new `Vec` on `Match`, `MatchView`, `view()`/`from_view()`, `step_effects`, threat marking in `Match::senses` (sim.rs ~598-603) and the client `game.rs` lerp. New `Event` variants. `render/lib.rs`: `character_color`, `look`, `handle_events`, a `draw_*`. `sound.rs`: `Sfx` variants, the hand-sized `Sfx::ALL: [Sfx; 30]`, `synth`, and the `Event::Special` match on `Character` (line ~420). Also a `PROTOCOL_VERSION` bump, the README table, the trainer `--cast` list, and the `% 4` / `[0u32; 4]` in `bots_use_every_special_and_matches_finish`. | Scattered. The compiler's exhaustive matches catch about half of these. The rest (the `ALL` arrays, test, README, cast list, version bump) are silent. |
| **New item / weapon** | `ItemKind`, `throw_speed`, `bounce` in `step_item`, the damage match in `step_items` (`_ =>` falls through to rock), `melee_reach`/`attack_box`, the stab check in `step_humans`, `spawn_items` probabilities (`GRENADE_CHANCE + 0.35`), `Source`, `dmg_by_source: [f32; 5]`, `one_hot() -> [f32; 3]`, `IN_ITEMS + kind.index() * N_SECTORS`, `draw_item`, the `Clatter` sound match. | **Dangerous.** `one_hot` indexes `v[3]` and panics. The retina write lands in `IN_HEAL`. Brains are frozen at 3 item kinds. |
| **New map** | `SOLIDS`/`PLATFORMS` consts are used by sim collision, `terrain_ray`, `random_surface_spot_between`, `fly`, `run_dog`, render `is_stone`/`is_metal`/`make_background` (a 320x180 layout hand-placed for this map), and `Match::new` `zones = [20,135,250,365]`. | Hard. Every consumer reads globals. |
| **New game mode** | End-of-match logic is inline in `step_humans` (last standing / `MAX_TICKS` / solo). There is no respawn path, and `heals: RangeInclusive` is the only per-match knob. | No seam. |
| **Music** | `Sounds.master`/`effects` exist and the settings screen has a "Music: none yet" row. | Easy: add a `music` volume and a looped `Sound`. |
| **New setting** | Three `PREF_*` consts and hand-parsing in `main()` (lines 283-289), plus save calls inside `settings_screen` and the `M` key handler. | Fiddly but small. |
| **Rebindable controls** | Bindings are consts and inline `KeyCode` lists in `input.rs` (`latch`, `take`, `settings_pressed`), plus four hand-written copies of the help text: the `input.rs` module doc, `KEYS_HELP`/`PAD_HELP`, `special_key()` ("Shift"/"Triangle"), and the README table. They have already drifted: the code also binds Up arrow and d-pad-up to jump, and the README doesn't say so. | Not possible without a refactor. |
| **New UI screen** | Add a function in `main.rs` and an arm in the `match (&app.room, app.net.is_some())` dispatch, and thread `App` through it. | Fine at this size, but `main.rs` is the bottleneck (section 3). |

### Proposed structures (small, Rust-idiomatic, no ECS)

ECS would be overkill: there are 4 fighters and a handful of entities. Keep enums and exhaustive matches as the "registry". Add a test or derive for every hand-maintained `ALL` array so that adding a variant fails loudly.

**a) A data table for items, which also protects the brain layout** (`sim.rs`):
```rust
pub struct ItemDef { throw_speed: f32, bounce: f32, gravity: f32, thrown: Option<(f32 /*dmg*/, f32 /*headshot mult*/, Source)>,
                     melee: Option<(f32, f32 /*reach*/, Source)>, fuse: Option<u32>, spawn_weight: f32, max_alive: usize,
                     sensed_as: Sensed /* Spear | Rock | Grenade: frozen by the brains */ }
impl ItemKind { pub const fn def(self) -> &'static ItemDef { match self { … } } }
```
`one_hot` and the retina use `def().sensed_as`, so a new item becomes "looks like a rock to bots" and never changes `N_IN`. `spawn_items` picks by `spawn_weight`. Because the values are identical, this refactor must leave `CLASSIC_HASH` unchanged, which is a free correctness check.

**b) An arena value instead of globals.** `pub struct Arena { solids: &'static [Aabb], platforms: &'static [Aabb], spawn_x: [f32; N_FIGHTERS] }`, with `pub static ARENAS: &[Arena]`. `Match` holds `arena: &'static Arena`, or an `ArenaId` in `MatchView`/`MatchStart`. Keep `W`/`H` fixed: the render target, the camera and the brains' `RAY_MAX`/`VIS_MAX` normalisation all assume 480x270, and varying only the walls and platforms is 90% of the value. `Art::new(&Arena)` builds the textures. `make_background` needs to become layout-aware or neutral.

**c) One "effects" list instead of a `Vec` per special.** Replace `bombs: Vec<Bomb>, dogs: Vec<Dog>` with `effects: Vec<Effect>`, where `enum Effect { Bomb(Bomb), Dog(Dog) }` has `step()`, `threat_pos()` (used by `senses`), `pos_mut()` (used by `game.rs` interpolation) and `owner()`. A new special entity then means one enum variant plus its arms, and `MatchView`/`from_view`/`senses`/lerp never change again. (Interpolation already guards on equal length. Also compare variants, as it does for items.)

**d) Per-crate character tables.** Move the per-character presentation data next to each other:
- sim: `Special` gains `fn blurb(&self) -> &'static str` (README/lobby text).
- render: `pub struct CharacterArt { color, look_fn, special_sfx: Option<Sfx> }` with `fn art(c) -> &'static CharacterArt`. This kills the `Character` match in `sound.rs::events` and duplicate colour logic.
- Add one test per crate: `for c in Character::ALL { art(c); c.special()… }`. Also add a test that `Character::ALL.len()` equals the variant count (use a `match` with no wildcard in a `const fn count()`), and do the same for `Sfx::ALL`. Then fix `% 4` in the character test to `% Character::ALL.len()`.

**e) `MatchConfig` for modes and tuning.** `Match::new(rng, &MatchConfig { max_ticks, heals: RangeInclusive<usize>, items: ItemRules, mode: Mode })`, where `enum Mode { LastStanding, /* later: Stocks(u8), Timed */ }` and `fn outcome(&self, m: &Match) -> Option<Option<usize>>` replaces the inline block in `step_humans`. Mode then becomes a lobby choice exactly like `Difficulty` (protocol enum, `RoomInfo` field, leader button).

**f) `Settings` and `Bindings` on the client** (new `client/src/settings.rs`):
```rust
#[derive(Serialize, Deserialize, Default)] #[serde(default)]
struct Settings { master: f32, effects: f32, music: f32, muted: bool, bindings: Bindings }
enum Act { Left, Right, Jump, Hand, Melee, Special, Menu }
enum Bind { Key(u16), Mouse(u8), Pad(u32) }          // KeyCode isn't serde: store a code
struct Bindings(Vec<(Act, Vec<Bind>)>);
impl Bindings { fn down(&self, a: Act, …) -> bool; fn label(&self, a: Act, pad: bool) -> String }
```
Store the whole thing as one JSON blob under a single localStorage key via `platform::save_pref`. `KEYS_HELP`, `PAD_HELP` and `special_key()` are then generated from `label()`, and the README points at the settings screen instead of repeating the table. A rebinding screen becomes a list of rows plus "press a key", which is easy in the existing immediate-mode UI. Note that `serde_json` would then become a real client dependency.

---

## 2. Configuration and dependency boundaries

### Inject or hard-wire

| Thing | Today | Recommendation |
|---|---|---|
| Physics and weapon tuning (`const`s in `sim.rs` and `character.rs`) | consts | **Keep as consts.** The brains are tuned to them and the hash test guards them. Group gameplay knobs that a mode might vary into `MatchConfig` (above) and leave physics alone. |
| RNG | `Match::new(rng)` takes one. Good. Rooms use `SmallRng::from_entropy()`. | Log the room's match seed at `info`. With the per-tick human `Action`s (already in `Player.inputs`), you can **replay any reported bug offline**: deterministic sim plus seed plus inputs. Add an optional `GO_BOX_RECORD` that dumps `(seed, cast, Vec<[Option<Action>;4]>)`. |
| RNG algorithm | `rand::rngs::SmallRng` | In rand 0.8, `SmallRng` is Xoshiro256++ on 64-bit targets and **Xoshiro128++ on wasm32**, and it is documented as not portable across rand versions. Today only the attract mode runs the sim in the browser, so nothing breaks. Before anything client-side needs to match the server (replays, spectating from seed, lockstep), switch the sim to an explicit `rand_xoshiro::Xoshiro256PlusPlus` or `rand_pcg::Pcg32`. That is one deliberate `CLASSIC_HASH` re-baseline. |
| Brains | Read from `GO_BOX_BRAINS` dir by `Brains::FILES` in `server/lib.rs`. The client embeds `gen_219000.json` via `include_str!`. | Embed in the server too (`include_str!` the three difficulty files in a `const` table `[(Difficulty, &str)]`). That deletes `GO_BOX_BRAINS`, a runtime failure mode, and one Docker `COPY`. |
| Clocks | Sim uses ticks (good). The room loop uses `Instant` plus a 4ms `interval`. The client uses macroquad `get_time()`. | Fine. To unit-test rooms without sockets or sleeps, have `Room` hold `Arc<Brains>` rather than `Arc<Hub>` (it only uses `hub.brains`), so `Room::handle`/`step` can be driven synchronously in tests. |
| Platform layer | `client/platform.rs` and `net.rs` `cfg`-split `imp` modules, plus `web/net.js`. | **Good, leave it.** One nit: the stringly `kind: i32` multiplexing in `go_text` (0 = URL, 1 = storage, 2 = share/copy) deserves named consts on both sides. |
| Server config | `Config::from_env` in `server/lib.rs`. Protocol consts `DEFAULT_PORT`/`WS_PATH`. | **Keep env-only** (12-factor; no config file). Make `Config` the single place for every deploy knob and keep the table in the `main.rs` doc comment as the only documentation. New fields are listed below. |

### Local vs. cloud: what has to change

Keep the defaults safe for local use (`127.0.0.1`), and make the cloud differences env vars only.

| Concern | Today | Needed for the cloud | Where |
|---|---|---|---|
| Bind | `GO_BOX_BIND`, default `127.0.0.1:7878` | `0.0.0.0:8080` set in the Dockerfile `ENV` only | unchanged |
| TLS / wss | `net.js` already picks `wss:` on https pages | Terminate TLS in front: Caddy on a small VPS (automatic certificates), or the platform's edge (Fly.io and similar). Don't put TLS in the binary. | none |
| Shutdown | `tokio::signal::ctrl_c()` only | Also handle `SIGTERM` (`tokio::signal::unix::signal(SignalKind::terminate())` under `cfg(unix)`). Today `docker stop` waits 10s, then the server is killed. | `server/main.rs` |
| Health | none | `GET /healthz` returns `200 {rooms, connections}` | `server/lib.rs` router |
| Liveness | No pings. After `Hello` there is no read timeout. | Server sends a WS Ping every 15s and drops the connection after about 45s of silence. Otherwise a dead mobile tab keeps a lobby seat and keeps the room alive forever (the lobby client sends nothing). | `conn.rs` `next_msg` loop |
| Origin | not checked | `GO_BOX_ORIGINS` allowlist, compared to the `Origin` header in `ws_handler` (empty means allow all, for local use) | `lib.rs` |
| Rate limits | Global caps `max_rooms`/`max_connections` only. One client can fill all 64 connections. `SetCharacter`/`SetReady` spam is re-broadcast to 4 players. Room codes (24^4 ≈ 332k) can be brute-forced. | Per-connection token bucket: Inputs about 120/s, other messages about 10/s, close on excess. Close the connection after about 5 failed `JoinRoom`s. Per-IP connection cap via `ConnectInfo<SocketAddr>`, or `X-Forwarded-For` behind a proxy (`GO_BOX_TRUST_PROXY=1`). | `conn.rs`, `lib.rs` |
| Static files | `ServeDir` with no compression or cache policy. The wasm is 929 KB. | `tower-http` `compression-br`/`gzip` features (roughly a third of the size), and `Cache-Control: no-cache` on `index.html`/`.wasm`/`.js` so that "page out of date, reload" actually fetches the new build | `lib.rs` |
| Logging | `tracing` with `RUST_LOG` | Fine. Optionally `GO_BOX_LOG=json`. Add a match seed and duration line. | `main.rs` |
| Build | `scripts/build-web.{sh,ps1}`, plus `cargo run -p go-box-server` | Multi-stage `Dockerfile`: `rust:1-slim` with the `wasm32-unknown-unknown` target builds the server and the wasm, then a `debian:bookworm-slim` (or distroless cc) image gets `go-box-server` plus `web/` (and `brains/` until they're embedded). Optionally embed `web/` with `include_bytes!` or `rust-embed` for a single-file binary, keeping `GO_BOX_WEB` as a dev override. | new `Dockerfile` |
| Sizing | — | 1-4 matches is tiny: per tick, 4 brains times about 8.4k MACs. The smallest shared-CPU VM or about a $5/month VPS is plenty. Pick a region near the players. Latency matters far more than CPU. | — |

Also note `.cargo/config.toml` `target-cpu=native` for `x86_64-pc-windows-msvc`. A server built on your PC won't run on an older CPU. It's harmless for Linux and Docker builds and only helped training (section 4).

---

## 3. Architecture risks and coupling

1. **The wire format is the sim's memory layout.** `Snapshot { view: MatchView }` carries the full `Fighter`, including `memory`, `action`, and 12 stat fields such as `dmg_by_source`, `close_ticks`, `self_dmg`, `healed` and `hits`. The client never reads these: the scoreboard comes from `Results`, which the server computes. The only reader is `rooms.rs` (`f[0].action.mv`). Risks: (a) forgetting a `PROTOCOL_VERSION` bump means `decode` returns `None`, messages are silently dropped and the client freezes; (b) bandwidth.
   - **S fix:** add `#[serde(skip)]` to the client-irrelevant `Fighter` fields (`memory`, `action`, and all stats except `hp`/`died_at`), plus `Bomb::pull` and `Dog::bite_cd`. Change the test to assert on `ack`/position.
   - **S fix:** add a protocol test `wire_format_is_pinned` that encodes a fixed-seed `Snapshot` and every `ClientMsg` variant, hashes the bytes, and asserts against a const with the message "bump PROTOCOL_VERSION, then update this hash". That turns the manual bump into a compiler-like checklist.
   - **L, later:** a dedicated `FighterView` in `protocol`. Only worth it if you ever need to evolve sim and protocol independently.

2. **The sim doubles as a training environment.** `Match` contains `fitness`, `close_ticks` (an O(n²) distance loop every server tick), `dmg_by_source` with a comment that it's "the trainer's saved stats format", `duel`, `timed_out`, `heals_spawned`, and `Source::ALL` excluding `Special`. The real constraint is that **senses, physics and the `Action` decode are frozen by the brains**. Changing them degrades bots silently. `CLASSIC_HASH` catches changes but not quality. If the trainer goes (section 4), keep a ~60-line `sim/examples/tournament.rs` (bots vs. bots over N seeds, win shares per brain). It is the only way to check that a physics tweak didn't make Hard play like Easy.

3. **Determinism assumptions.** The sim is single-threaded with ordered iteration, no `HashMap`, and seeded RNG. Good. Two traps:
   - `SmallRng` differs on wasm32 (above).
   - `CLASSIC_HASH` runs through `f32::sin`/`cos` (`Fighter::aim_dir`, `ray_dir`) and `powf` (`pull_strength`). These call the platform libm, so the test may produce a different hash on Linux than on Windows. Check this once your first Linux CI or Docker test runs. The fix is the `libm` crate for those calls (another deliberate re-baseline).
   - Client prediction (`predict` = `control` + `collide`) uses only `+ - * /`, so it is safe in wasm.

4. **`client/src/main.rs` (903 lines)** holds `Options` and test automation, `Attract`, `autopilot`, `App` with network handling, five screens, settings persistence, and the HUD. Split it into `app.rs` (App plus `update_net`), `screens/{home,lobby,settings,hud}.rs`, `attract.rs` and `testclient.rs` (Options, autopilot, screenshots). Do it first. It is mechanical and low-risk, and every later UI change gets cheaper.

5. **Immediate-mode UI (`ui.rs`)**: fine for this game, keep it. One fragility: gamepad focus is identified by `Rect` (`same_rect`, 2px tolerance), so any layout shift (a notice appearing, a lobby row added) drops focus. If that bites, give `area`/`button` an explicit `id: u64` (hash of the label plus a section) and match focus on the id.

6. **Room and connection coupling**: `Room` holds `Arc<Hub>` just for brains, the room's message handling only runs inside the async `run` loop, and tests are end-to-end with 5s timeouts. Inject `Arc<Brains>` and a seed, and add sync unit tests for `handle`/`step` (leader handover, mid-match leave becoming a bot, seat fills, input backlog trimming in `INPUT_BACKLOG_MAX`).

7. **Test coverage gaps** that make refactors risky:
   - `client/game.rs`: no tests at all. It is pure logic apart from macroquad's `Vec2`, which is glam. Add: reconciliation replays un-acked inputs; `offset` snaps above `SNAP_DISTANCE`; events are played exactly once across `frame` calls; interpolation with differing item counts.
   - `input.rs` binding behaviour (latching across frames with no tick).
   - Wire format (item 1).
   - The hand-sized `ALL` arrays (section 1d).
   - render and sound don't need tests.

8. **Vendored JS**: `web/gl.js` and `web/audio.js` must match `miniquad 0.4.11` and `quad-snd 0.2.8` (gl.js checks plugin and crate versions at load). Write those versions in a header comment of each file, and note in the README that a `macroquad` bump means re-copying them.

---

## 4. Deletion candidates

Confidence: **H** = I verified it is unused (grep). **M** = I verified the usage, but something you value may depend on it.

| Candidate | Confidence | Evidence / how to verify |
|---|---|---|
| `archive/tribes-2v2/` (whole dir) and the README "Archive" section | H | Excluded from the workspace (`exclude = ["archive"]`); nothing references it. Git history keeps it. `git rm -r archive` |
| `serde_json` in `crates/client/Cargo.toml` and `crates/server/Cargo.toml` | H | No `serde_json` in either crate's sources (only sim, trainer). Remove the dependency and run `cargo check` |
| `Sounds::loaded()` (`sound.rs`) | H | No callers. It is `pub`, so the compiler won't warn |
| `.gitignore` lines `/target-exp` and `!/assets/**/*.png` | H | Neither `target-exp` nor `assets/` exists |
| `Fighter.action`/`memory`/stats **on the wire** (`#[serde(skip)]`) | H | The client reads none of them. Only `rooms.rs:116` reads `action.mv`. About 100 bytes per fighter per snapshot |
| **The whole `crates/trainer`** (move to `archive/` or delete) | M | You said no more training. It costs `rayon`, the ES optimizer (`GO_BOX_ALGO=es`, undocumented), the env vars `GO_BOX_POP`/`GO_BOX_EVALS`/`GO_BOX_THREADS`/`GO_BOX_SAVE`, the flags `--duel`/`--duels`/`--tournament`/`--bench`/`--cast`, a third input scheme (`player_action`), and about 130 README lines. You lose the offline viewer and the benchmarking tools; the replacement is `sim/examples/tournament.rs` (section 3, item 2). Verify: delete it, set `default-members = ["crates/server"]`, run `cargo check --workspace` and `cargo test` |
| …which then frees in **sim**: `Match::fitness`, `close_ticks` and the `CLOSE_DIST` loop, `self_dmg`, `dmg_by_source`/`Source::ALL`/`Source::index`/`Source::name` (and the `dmg_special` split), `healed`/`heals_taken`/`heals_spawned`, `Match::duel`, `timed_out`, `Brain::random`, `gauss` (keep it in `#[cfg(test)]`), and the `heals: RangeInclusive` param (it becomes `MatchConfig`) | H once the trainer is gone | Each was grepped: the only users are the trainer and the sim tests. `CLASSIC_HASH` must stay unchanged, since none of them affect state that is hashed (`close_ticks` is not hashed) |
| …in **render**: `NAMES`, and `Art::draw`'s `debug` param and branch | H once the trainer is gone | The client always passes `false` (`main.rs:407`) |
| …in **`.cargo/config.toml`**: the `x86_64-pc-windows-msvc` `target-cpu=native` section | H once the trainer is gone | It only sped up training, and it makes Windows binaries non-portable. **Keep** the wasm32 `--allow-undefined` section (audio.js needs it) |
| v1 brain graft (`SavedBrain`, `V1_*`, the `From` impl, `graft_tests`) | M, after a one-off conversion | **Not dead today.** `gen_031000`, `gen_100000`, `gen_150000` and `gen_184000` are v1-sized (3,787 weights; `gen_219000` has 8,395). Easy and Normal are v1 brains. Re-save those four through `Brain::from_json` and serialize them once (graft is exact), then delete about 60 lines. `CLASSIC_HASH` must not change |
| `GO_BOX_BRAINS` (and the runtime `Brains::load`) | M | Delete if the brains get embedded (section 2) |
| `brains/gen_100000.json`, `gen_184000.json` | keep | Used only by tests (`Brain::committed()` asserts 5; the hash lineup uses all 5). Removing them forces a re-baseline for no gain |
| Native test-client automation: `Options.{autopilot, code_file, shots, shot_every, start_when, quit_after, settings, character}`, `autopilot()`, the automation block in `main()` | M | Nothing in the repo invokes these flags. They look like agent or screenshot tooling. Keep them if you drive the native client from scripts or agents. If you keep them, move them into `testclient.rs` |
| `scripts/build-web.ps1` + `.sh` | low value | Seven lines each. Keep until the Dockerfile builds the wasm, then optionally drop the `.ps1` |
| Local `save/` (159 MB, untracked) | your call | Not in git. Only the trainer reads it |

---

## 5. Prioritized changes

| # | Change | Effort | Payoff | Risk |
|---|---|---|---|---|
| 1 | **Pin the wire format**: golden-bytes test in `protocol`, plus `#[serde(skip)]` on client-irrelevant `Fighter`/`Bomb`/`Dog` fields | S | High: no more silent version skew; snapshots shrink by about a third | Low |
| 2 | **Cloud hardening bundle**: SIGTERM, `/healthz`, ping and idle timeout, Origin allowlist, per-connection rate limits and failed-join cap, compression and `Cache-Control`; all in `Config` | M | High: the prerequisite for any public host | Low (localhost defaults unchanged) |
| 3 | **Dockerfile** (multi-stage, builds server and wasm) plus brains embedded via `include_str!` | S–M | High: one artifact, cheap host | Low |
| 4 | **Archive the trainer** and delete its sim, render and config surface, `archive/`, and the unused deps. Add `sim/examples/tournament.rs` | M | Medium-high: about 1.5k fewer lines to reason about, and the sim reads as a game | Low (guarded by `CLASSIC_HASH`) |
| 5 | **Split `client/main.rs`** into `app`, `screens/*`, `attract` and `testclient` | M | High for all future UI work | Low (mechanical) |
| 6 | **`Settings` + `Bindings`** (one localStorage blob; help text generated; enables a rebinding screen and a music slider) | M | Medium-high | Low |
| 7 | **`ItemDef` table with `sensed_as`** | M | Medium: new weapons become data, and the brain layout is protected from a latent panic | Low (hash must stay identical) |
| 8 | **`Effect` enum** replacing `bombs`/`dogs`, plus per-crate character tables and the `ALL`-completeness tests | M | Medium: a new character becomes about 4 places, all compiler-checked | Low–medium (protocol bump) |
| 9 | **Client and room tests**: `game.rs` reconciliation and interpolation; sync `Room` tests with injected `Brains` and seed; seed logging and optional input recording for replays | M | Medium-high: makes items 5–8 and 10 safe | Low |
| 10 | **`Arena` value plus `MatchConfig`/`Mode`** (maps and modes as lobby options) | M–L | Medium (only when you want content) | Medium: touches every collision site. Do it as a pure refactor first (hash unchanged), then add content |
| later | Portable RNG (`rand_xoshiro`) and `libm` trig in the sim, re-baselining `CLASSIC_HASH` once | S | Needed before any client-side replay or spectate-from-seed | Low |

**Leave as-is:** the tick and `secs()` time model, `Special` as a strategy trait, server authority with prediction and reconciliation (`game.rs` is clean), `sanitize()` on inputs, the `Hello` version handshake, the `cfg`-split platform layer, postcard, the immediate-mode UI, and the `CLASSIC_HASH` fingerprint test.
