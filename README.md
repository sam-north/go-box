# Go Box

Four humans locked in a box, free-for-all. Everyone starts **empty-handed**; spears, rocks, the
odd grenade and health lie around the arena. No fighter has any hand-written behaviour — each is a
small recurrent neural net, and one population evolves with a genetic algorithm. Nothing rewards
any particular weapon or style: the score is about outcomes, so whatever fighting style wins is
what evolves.

```
cargo run --release                     # watch the top 4 fight while training runs in the background
cargo run --release -- --fresh          # start evolution over from scratch
cargo run --release -- --headless 500   # train 500 generations without a window
cargo run --release -- --compare        # current champion vs. champions from 10k and 20k gens ago vs. an untrained brain
cargo run --release -- --play           # you vs. the current champion, 1v1
cargo run --release -- --watch          # watch/play without training (follows a run in another process)
cargo run --release -- --bench          # time the simulation (single thread)
cargo run --release -- --tournament a.json b.json ...   # pit saves' top brains against each other
```

Keys: `1-4` speed, `P` pause, `R` new match, `C` compare generations, `V` play the champion, `Tab`
show senses/hitboxes/blast radius, `S` save, `Esc` save + quit.

Playing (you are RED): `A/D` move, `Shift` run, `Space` jump, `R` melee (punch, or stab with a
spear), `E` grab, `Q` throw, hold `W`/`S` to aim higher/lower.

The champion of every 1000th generation is kept in `save/ffa_champions/` for compare mode (this
started at gen ~30,900, so the 10k/20k-ago seats fill in as training continues).

Only run one copy at a time: a headless run and the viewer would both write the same save.

## Rules

Everyone has the same body: walk, run, jump, gravity, 100 HP, knockback. One thing in hand at a
time; picking up is a deliberate **grab** (so choosing a spear over a rock is something to learn).

| Weapon | Damage | Headshot |
|---|---|---|
| Fists (empty hands, or holding a rock/grenade) | 8 | ×2 |
| Spear stab (holding a spear) | 16, longer reach | ×2 |
| Spear throw | 28, flies flat and fast | ×2 |
| Rock throw | 10 | **×3** — a rock in the right spot is devastating |
| Grenade | up to 45 in a radius, hurts everyone including the thrower | — |

A **headshot** is a hit on the top of the body. For melee that means striking from above — out of
a jump or down from a ledge. Thrown items land where they stop and can be picked up again. A
grenade's fuse (~1.5s) lights when thrown; a live one can be grabbed and thrown back.

Items: 2 spears and 4 rocks at the start; a new item appears every 5-8s (mostly rocks, some spears,
rarely a grenade — at most one grenade at a time).

**Health pickups** (glowing blue bulbs) restore 50 HP and are taken by touch when hurt. Final rules
are 1-2 per round; training starts with 5-6 and fades to that over 300 generations (a curriculum),
so brains discover them.

A match ends with the **last one standing**, or after 30s, when the fighter with the most health
wins. Only an exact tie is a draw.

## How it works

- **Senses:** 12 terrain raycasts; 12-sector "retinas" (line of sight only) for opponents, threats
  (flying items, live grenades), loose spears, loose rocks, loose grenades and health; what the
  nearest opponent is doing and holding; own body state, what's in hand, whether something is in
  reach, a held grenade's fuse; 4 memory values.
- **Brain:** 106 inputs → 32 tanh → 11 outputs (move, run, jump, attack, throw, aim, grab, 4 memory).
- **Evolution:** 96 brains. Each generation every brain plays at least 8 four-way matches; 20% of
  seats go to hall-of-fame champions from past generations. Elitism + tournament selection +
  uniform crossover + Gaussian mutation.
- **Fitness:** damage dealt − ½ (damage taken − health restored) + 60 per kill − 40 for dying,
  +80 for winning (plus a bonus for winning early). A "get close to someone" hint fades out over
  the first 300 generations.

## Watching it learn

The HUD shows a chart of **damage by weapon** (how they actually fight), plus headshot rate, heal
pickup rate, and knockouts (wins by last-one-standing vs. on health). Every generation is also
appended to `save/ffa.csv` for analysis; `--headless` prints the same numbers.

The full state saves to `save/ffa.json` every 3 minutes and on exit.

`save/` is local only (it's ~160MB and not committed). A few stable champions are committed in
`brains/` (see `brains/README.md`); a fresh clone without `save/` starts training from scratch.

## Hardware budget

Training uses at most half your logical CPUs minus one (7 of 16 threads on a 7800X3D), so it stays
under ~50% CPU. Population size is fixed, so memory stays small. The window renders a 320×180
image upscaled with vsync, which is negligible GPU load.

## Archive

The previous 2v2 humans-vs-creatures version is kept in `archive/tribes-2v2/`, with its save in
`save/state.json`.

## Tuning knobs

- `src/sim.rs` — physics, weapon damage, headshots, item spawns, heals, arena, senses, fitness.
- `src/evo.rs` — population size, matches per generation, mutation, hall of fame, curriculum.
- `src/brain.rs` — network size and memory.
- `src/render.rs` — all procedural art.

## Training against history, and a strength benchmark

After gen ~50k the champions stopped getting stronger (a tournament of champions from 31k to
138k was flat at ~25% each). Two changes followed:

- **Shared scenarios with archive opponents:** each generation, every brain plays the same 6
  scenarios (spawn seed + seat + three opponents: last generation's best, a recent champion, and a
  champion from anywhere in the archive). Scores are comparable, and old styles keep coming back,
  so the population can't drift in circles.
- **Benchmark:** every 250 generations the top 4 play 600 fixed-seed matches against a held-out
  panel of champions (gens 31k, 50k, 100k, 150k, 184k). Logged to `save/ffa_bench.csv` and shown
  as the "strength vs panel" chart in the viewer. 25% = as good as the panel; a rising line is
  real improvement. Baseline at the switch: 26.2%.

## Speed and training settings

`--bench` showed ~70% of the time went to the brains' math. A SIMD-friendly dot product, a fast
`tanh`, a cached ray table, a fast `atan2`, LTO and `target-cpu=native` (`.cargo/config.toml`)
took a tick from 10.8 to 3.6 µs: about 3x more training in the same 7-thread budget.

Settings were then compared head-to-head: three copies of the gen-36k save trained 12 minutes each,
then 4000 `--tournament` matches (25% = fair share):

| setting | gens trained | win rate |
|---|---|---|
| 8 matches per brain | 4.4k | 28.5% |
| starting point | 0 | 27.4% |
| population 192 | 4.0k | 24.3% |
| old default: 4 matches per brain | 7.4k | 18.2% |

With 4 matches, fitness was mostly luck and fighters drifted worse; 8 is now the default.
`GO_BOX_POP` / `GO_BOX_EVALS` override these for experiments, and `--minutes M` limits a
headless run.
