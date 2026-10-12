# Go Box

Four fighters locked in a 480×270 box with four tiers of ledges, free-for-all. Everyone starts **empty-handed**; spears, rocks, the
odd grenade and health lie around the arena. The bots aren't scripted: each is a small recurrent
neural net evolved with a genetic algorithm (see [The bots](#the-bots)). Play it with friends in the
browser; any empty seat is filled by a trained bot.

## Playing online

```
./scripts/build-web.sh                  # build the browser client into web/ (or scripts/build-web.ps1)
cargo run --release -p go-box-server    # serve it on http://127.0.0.1:7878 (this machine only)
```

Open http://127.0.0.1:7878, pick a name and **Create a room**. Share the invite link (or the
4-letter code); up to four people can join, and bots take the empty seats. The room leader picks
the map and starts matches; after each match an overlay shows the scoreboard (kills,
headshots, damage, when each fighter went down), everyone can switch character, the others can
mark themselves ready, and the leader hits **Play again**. Someone who leaves mid-match is
replaced by a bot; someone who joins mid-match watches until the next one.

Controls: you always run; **Esc** (Options on a controller) opens Settings, where the **Controls**
page shows every binding and lets you change it, separately for keyboard/mouse and controller (PS4,
PS5 or Xbox pads in Chrome, Edge or Firefox). By default: `A`/`D` move, `W`/`Space` jump, the mouse
aims, left click grabs (empty-handed) or throws (holding something), right click attacks toward the
cursor, `Shift` uses your special, `M` mutes. Settings are saved in the browser.

The browser only reports a controller after you press a button on it. In menus (lobby, scoreboard,
settings) the d-pad or left stick moves a highlight and Cross presses it.

The leader also picks the **map** (Tiers or Bridges). In the lobby the leader sets each seat without a player to **Bot** or **None**, so a match can have
anywhere from 1 to 4 fighters (alone, it runs until the clock or you run out).

The server only listens on `127.0.0.1` unless told otherwise (`GO_BOX_BIND`); see
`crates/server/src/main.rs` for its settings. To put it online for friends, see **`docs/DEPLOY.md`** (Fly.io).


### Characters

Everyone has the same body; each character adds one special on a cooldown.

| Character | Special | Cooldown |
|---|---|---|
| Classic | none — the hooded fighter the brains were trained as | — |
| Sam | **Gravity Bomb**: lobs a bomb ~90px ahead; it goes off when it lands or touches someone and yanks everyone on the map toward it for a quarter second (anyone within 76px right into the middle, the rest less the farther away they are), then explodes for up to 34 | 30s |
| Austin | **Pop-Up**: teleports in the direction he's facing, at least 160px ahead (a third of the arena) when there's room, else anywhere ahead; prefers spots 40px from everyone | 15s |
| Tyler | **Release the Hounds**: three dogs run the floors for 5s, biting anyone but Tyler for 15 (each dog at most twice a second per victim) | 30s |

Bots playing a character use its special by a simple rule (the brains never trained with
specials): Sam works out where his bomb would land and throws only if it would catch two people,
finish one off, or turn a fight he's losing; Austin pops away from incoming projectiles, grenades
about to blow, or a crowd when hurt; Tyler releases the hounds at someone on his level ahead. Tuning lives at the top of `crates/sim/src/character.rs`.

**Adding a map** is a variant in `ArenaId` with its platforms (`crates/sim/src/arena.rs`) and its
name in the text file; check the bots cope with it using `--example tournament 400 <map id>`.
A match's rules (map, length, heals, how it's won) are a `MatchConfig`; a new way to win is a
variant of `Mode`.

**Adding a character** (the goal is 20+): a variant in `Character` and its row in
`Character::def` (`crates/sim/src/character.rs`, with a new `Special` if it has one), its row in
`art` (`crates/render/src/lib.rs`: colours, hair, special sound), and its name in
`crates/client/text/en-US.txt`. If its special leaves something in the arena, that's a variant of
`Effect`; the compiler then lists what to fill in (how it steps, is drawn and is seen by bots).

### Sound

Every sound is synthesised in code at startup (`crates/render/src/sound.rs`): no audio files.
Hits sound different by weapon, headshots ding, grenades tick faster as the fuse burns, and each
special has its own. Your own jumps and swings play the instant you press the key. **M** toggles
sound (remembered by the browser).

### Text and languages

Everything players read is in `crates/client/text/en-US.txt` (`key = text`). Another language is
a translated copy of that file registered in `crates/client/src/text.rs`; a test checks the code
and the English file agree (no missing or unused keys). The server never sends display text: it
sends ids (characters, refusals) that the client looks up.

### How the netcode works

The server runs the only real match (60 ticks/s) and sends snapshots 30 times a second. Your own
fighter moves the instant you press a key (the client predicts it, then reconciles with the
server); everyone else is drawn 100ms in the past, smoothly interpolated. Hits and damage are
always decided by the server.

## Layout

- `crates/sim` — rules, physics, characters and specials, the bots' brains. Shared by everything.
- `crates/render` — procedural pixel art and sound effects.
- `crates/protocol` — the messages between server and client.
- `crates/server` — rooms, lobby, matches; serves the browser client (`cargo run --release`).
- `crates/client` — the game client (wasm in the browser; also runs natively for testing).
- `brains/` — the trained champions the bots use (see `brains/README.md`).
- `web/` — the page that hosts the client (`gl.js` and `audio.js` are copied from the miniquad
  and quad-snd crates).
- `docs/` — the architecture review and the TODO list.

## Rules

Everyone has the same body: run, jump, gravity, 100 HP, knockback. One thing in hand at a time;
picking up is a deliberate **grab**.

| Weapon | Damage | Headshot |
|---|---|---|
| Fists (empty hands, or holding a rock/grenade) | 8 | ×2 |
| Spear stab (holding a spear) | 16, longer reach | ×2 |
| Spear throw | 28, flies flat and fast | ×2 |
| Rock throw | 10 | **×3** — a rock in the right spot is devastating |
| Grenade | up to 85 in a 34px radius (30 at the edge), hurts everyone including the thrower | — |

A **headshot** is a hit on the top of the body. For melee that means striking from above — out of
a jump or down from a ledge. Thrown items land where they stop and can be picked up again. A
grenade's fuse (1s) lights when thrown; a live one can be grabbed and thrown back. A fighter who dies
holding a grenade drops it armed: it goes off half a second later.

Items: 2 spears and 4 rocks at the start; a new item appears every 5-8s (rocks, spears, and
grenades — at most two grenades at a time). **Health pickups** (glowing blue bulbs) restore 50 HP
and are taken by touch when hurt; 1-2 appear per match.

A match ends with the **last one standing**, or after 45s, when the fighter with the most health
wins. Only an exact tie is a draw.

## The bots

Each bot is a small recurrent neural net (119 inputs → 64 tanh → 11 outputs) that was evolved with
a genetic algorithm over ~220k generations; the trainer has since been retired (it's in the git
history). The bots see the arena through 12 terrain raycasts and 12-sector "retinas" for
opponents, threats, items and health, plus their own state and a few memory values.

Their senses and physics are what they were trained on, so changes to movement, senses or the
brain's inputs and outputs make them play worse. `cargo run --release -p go-box-sim --example
tournament [n] [map]` plays the committed brains against each other: run it before and after a gameplay
change to check the bots still play as before.
