# Go Box

Four fighters locked in a 480×270 box with four tiers of ledges, free-for-all. Everyone starts **empty-handed**; spears, rocks,
grenades and health lie around the arena. The bots aren't scripted: each is a small recurrent
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
cursor, `Shift` uses your special, `S` dashes, `M` mutes. Settings are saved in the browser. In the
air against a wall, press jump to kick off it (and hold toward a wall to slide down it slowly).

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
| Sam | **Glue Gun**: fires a glob of glue (like Noita's glue gunners) the way he aims. Whoever it hits takes 4 and is stuck for 5s: moving at a third of normal speed, tethered to the nearest surface by an elastic strand that snaps if stretched past 48px. A glob that hits terrain leaves a blob, and anyone who runs into a blob or a strand is stuck too | 8s |
| Austin | **Pop-Up**: teleports to the mouse cursor (out of walls, onto tables). With a controller, and for bots, it goes the way he's facing, at least 160px ahead (a third of the arena) when there's room, else anywhere ahead; prefers spots 40px from everyone | 6s |
| Tyler | **Release the Hounds**: three dogs run the floors for 5s, biting anyone but Tyler for 15 (each dog at most twice a second per victim), and Tyler runs 35% faster while they're out | 30s |
| Seth | **Mini Me**: two smaller (70%) clones hop out in front of him for 15s. They fight like bots (grabbing and throwing too), die to one hit, and their kills count as his; they vanish if he goes down | 30s |
| Corwin | **Cat Herder**: twenty cats pour out and run the floors for 5s; each scratch is only 2 (once a second per cat per victim) but they come as a swarm. Sushi within 100px distracts them: they crowd round it (hopping up onto a table for it) and scratch nobody | 30s |
| Justin | **Sushi Table**: a table appears in the air in front of him (over his head, if he's facing a wall) and stays right there as a platform, with three pieces of sushi on top. Solid from the sides and on top but never from below, so it can't pin anyone; sushi throws like a rock (10) and splats on whatever it hits. Lasts 15s, one at a time | 10s |
| Ryan | **Yanking Your Chain**: throws a chain up to 110px the way he aims; the first foe it hooks takes 6 and is yanked to just in front of him, dazed for 0.4s | 8s |
| Jake | **Into the Phone**: turns into light and hides inside his phone for 5s, which drops from his head; meanwhile he can't act or be hurt, and anyone can grab and throw the phone (it hits like a rock). Then he pops back out standing on wherever it went, and the phone stays as an ordinary item. Every phone stays all match: while inside, left/right hops him to the next phone that way (any Jake's, if it's free), marked by a chevron, and he comes out of whichever he's in | 20s |
| Robert | **Forked Lightning**: a forked bolt strikes every opponent in the arena at once (no line of sight needed): 30 up close, falling off to 5 at 300px and beyond | 25s |
| Kyle | **Organize**: pins every opponent exactly where they are (even in mid-air) for 3s, with a glow at their feet; they can still swing and throw, just not move or jump | 20s |
| Vid | **The Chosen One**: goes inside the opponent nearest the mouse cursor (for bots, nearest him) and drives their body for 3s, while it's drawn as a silhouette of falling green code; he can't be hurt meanwhile, then steps back out beside them | 20s |

Bots playing a character use its special by a simple rule (the brains never trained with
specials), and never during the protected start: Sam fires the glue at someone ahead of him, near his level; Austin pops away from incoming projectiles, grenades
about to blow, or a crowd when hurt; Tyler releases the hounds at someone on his level ahead; Seth splits when a foe is close; Corwin lets the cats go at someone on his level ahead; Justin
serves a table when he has nothing to throw; Ryan throws the chain at someone level with him, in reach; Jake hides in his phone when Austin would
pop away; Robert casts when the bolts would do 30 all told or finish someone; Kyle organizes when someone's
within arm's reach. Tuning lives at the top of `crates/sim/src/character.rs`.

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
| Rocket | up to 60 in a 26px radius (24 at the edge), homing; hurts everyone including the shooter | — |

A **headshot** is a hit on the top of the body. For melee that means striking from above — out of
a jump or down from a ledge. Thrown items land where they stop and can be picked up again. A
grenade's fuse (1s) lights when thrown; a live one can be grabbed and thrown back. A fighter who dies
holding a grenade drops it armed: it goes off half a second later.

Items: 5 grenades, 2 spears and one rock at the start (the only rock all match, a nod to how the game
began); a new item appears every 2-4s: a grenade (28%, while there are fewer than ten), a Molotov
(27%), a rocket launcher (17%, at most eight a match), a giant sword (12%), a spear (9%) or an
air-strike radio (7%, at most four). A **rocket launcher** fires one homing rocket (it curves
toward the nearest opponent and explodes for up to 60), then it's spent. The **giant sword** slashes
in a wide arc (22, reach 19) and flies tip first when thrown (32). A **Molotov** shatters on whatever
it hits and sets the surface below burning for 4s (5 every half second to anyone standing in it, the
thrower included). The **air-strike radio** calls in a jet that crosses the top of the arena once,
dropping 5-7 bombs (up to 55 in a 46px radius) at random points. **Health pickups** (glowing blue bulbs) restore 50 HP
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
