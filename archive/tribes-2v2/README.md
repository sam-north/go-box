# Box Arena

Two tribes locked in a box: **humans** and **creatures**, 2 vs 2. No fighter has any hand-written
behaviour. Each is a small recurrent neural net, and four populations **co-evolve** with a genetic
algorithm, so every improvement on one side pressures the other.

```
cargo run --release              # watch champions fight while training runs in the background
cargo run --release -- --fresh   # start evolution over from scratch
cargo run --release -- --headless 500   # train 500 generations without a window
```

Keys: `1-4` speed, `P` pause, `R` new match, `V` match with one of each kind, `Tab` show
senses/hitboxes, `S` save, `Esc` save + quit. Progress autosaves to `save/state.json` every 25
generations and on exit, and resumes on launch.

## Fighters

Every fighter has the same body: walk, run, jump, gravity, 100 HP, knockback. Each slot in a match
is the default kind, or (35% chance) its ranged variant.

| Kind | Tribe | Melee | Ranged |
|---|---|---|---|
| Spear (rust scarf) | human | spear, 14 dmg, long reach | — |
| Thrower (teal scarf) | human | punch, 10 dmg, short reach | rocks, **24 dmg** — must pick one up off the ground first |
| Biter (pink head) | creature | bite, 14 dmg, long reach | — |
| Spitter (acid-green head) | creature | claw, 10 dmg, short reach | acid, **24 dmg** — saliva refills over ~2s |

Ranged hits do much more damage than any melee, so ranged play is worth prioritising. Five rocks
spawn per match on the floor and platforms. Thrown rocks bounce, settle, and can be
picked up again; they can never leave the box.

**Health pickups** (glowing blue bulbs) are rare: 1 or 2 per round, appearing at random times
between 6s and 24s on the floor or a platform top. Touching one restores 50 HP (capped at 100); a
fighter at full health leaves it for someone else. Either tribe can take them, so grabbing one also
denies the enemy.

Heals are taught with a **curriculum**: when pickups are first introduced, rounds start with 5-6 so
brains discover them, fading to the final 1-2 per round over 300 generations. The viewer always
plays by the rules training is currently using.

**There are no draws** (barring an exact tie). A match ends when a tribe is wiped out, or after
30s, when the tribe with more total health left wins.

**Friendly fire is on.** Tribes are allied, but nothing stops a stray rock or swing from hitting
a teammate. Loyalty is learned: hurting an ally costs exactly what hurting an enemy earns, and the
whole tribe shares the win.

## How it works

- **Senses:** 12 terrain raycasts, plus five 12-sector "retinas" for enemies, allies, incoming
  projectiles, loose rocks, and health pickups (each only in line of sight); whether the nearest enemy is swinging,
  facing me, or ranged; own body state; holding a rock / ranged-ready; 4 memory values.
- **Brain:** 88 inputs → 28 tanh → 10 outputs (move, run, jump, melee, ranged, aim, 4 memory).
- **Evolution:** 4 populations of 64 (one per kind). Matches are drawn so every individual plays
  at least 3 per generation; 20% of slots are filled by hall-of-fame champions from past
  generations. Elitism + tournament selection + uniform crossover + Gaussian mutation.
- **Fitness:** enemy damage − ally damage − ½ (damage taken − health restored), ±60 per enemy/ally kill, −40 for dying,
  +80 if your tribe wins (plus a bonus for winning early by wipe-out), −20 if it loses.

## Watching lessons get learned

The bottom-left HUD panel tracks three behaviours across recent generations (10-gen smoothed):

- **Friendly fire**: damage to allies as a share of damage to enemies. Should fall.
- **Heals grabbed**: share of spawned pickups somebody took. Should rise.
- **Wins by wipe-out**: share of wins by eliminating the other tribe (the rest are won on health).

There is one score per fighter per match, so all of these are learned together; the panel shows
whether each one is actually improving. `--headless` prints the same numbers per generation.

## Save upgrades

When a feature adds new senses, existing saves are upgraded automatically on load (a copy is kept
as `save/state_pre_upgrade_gen<N>.json`). Brains keep everything they learned; the new senses
start disconnected and evolution wires them in.

## Hardware budget

Training uses at most half your logical CPUs minus one (7 of 16 threads on a 7800X3D), so it stays
under ~50% CPU. Populations are a fixed size, so memory stays at a few MB. The window renders a
320×180 image upscaled with vsync, which is negligible GPU load.

## Tuning knobs

- `src/sim.rs` — physics, damage/reach per kind, rocks and acid, arena layout, senses, fitness.
- `src/evo.rs` — population size, matches per generation, mutation rates, hall of fame.
- `src/brain.rs` — network size and memory.
- `src/render.rs` — all procedural art.
