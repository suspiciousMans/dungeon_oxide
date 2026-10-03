# The Catacombs of Rust

A first-person dungeon crawler where the **game rules are written in oxidized** and the
engine is Rust.

You descend through procedurally generated floors of the Jame engine, fighting your way
past monsters whose behaviour, loot drops, damage numbers, and difficulty pacing are all
decided in a `.ox` file — no Rust in the game logic at all.

## The point of this project

Two languages, one strict boundary:

| | Owns |
|---|---|
| `src/*.rs` | engine — window, rendering, input, physics, the world, the player |
| `scripts/dungeon.ox` | rules — monster brains, loot tables, damage, pacing, flavour text |

The two talk through a small, explicit FFI. `scripts/dungeon.ox` *declares* the engine
functions it needs:

```rust
fn native host_monster_count() -> Int;
fn native host_monster_health(i: Int) -> Float;
fn native host_damage_player(amount: Float);
```

and `src/natives.rs` *implements* them. `build.rs` transpiles the `.ox` to Rust at compile
time, so there's no runtime dependency on the `oxidized` binary — just a normal `cargo build`.

Because the boundary is explicit, all the interesting rules are readable in one file:

```rust
fn monster_decision(kind: Int, hp_frac: Float, distance: Float) -> Int {
    if kind == 2 && hp_frac < 0.35 { return 3 }   // cowards flee when hurt
    if distance < 1.8 && hp_frac > 0.15 { return 4 }  // in range: strike
    if distance < 6.0 + hp_frac * 6.0 { return 2 }    // healthy monsters notice you sooner
    return 0                                        // otherwise idle
}

fn player_attack_damage(depth: Int) -> Float {
    let base = 10.0 + depth as Float * 2.0
    return base
}
```

Your damage, the monsters' damage, their aggression and cowardice, what drops and how
often, how hard a descent is to survive — all of it is in `scripts/dungeon.ox`. Nothing in
that file knows what a GPU is.

## Building and playing

Requires Rust, and the `oxidized` binary on `PATH` (or `OXIDIZED=/path/to/oxidized`).

```sh
cargo build
./target/debug/dungeon_oxide.exe
```

| Input | Action |
|---|---|
| Mouse | Look |
| `W` `A` `S` `D` | Move |
| `Esc` | Quit |

Walk onto the stairs to descend. Collect loot for gold, health, and armor.

### Environment variables

| Var | Effect |
|---|---|
| `DUNGEON_SEED=<n>` | Fixed seed — the same seed replays the same dungeon, including loot drops |
| `DUNGEON_AUTOPILOT=1` | Headless run: navigates to the stairs by itself and logs the result |

## Verifying it

```sh
cargo test                                    # generator + RNG invariants
DUNGEON_AUTOPILOT=1 DUNGEON_SEED=42 \
  RUST_LOG=info ./target/debug/dungeon_oxide.exe 2>&1 | grep -E 'descended|run ended'
```

The autopilot is the end-to-end check: it drives real movement and real collision through
BFS navigation to the stairs, so a floor the generator can't solve shows up as a run that
stops descending.

## Layout

```
build.rs              transpiles scripts/*.ox → Rust at compile time
scripts/dungeon.ox    the game's rules
src/main.rs           engine: loop, rendering, input, the world
src/natives.rs        the engine half of the FFI (implements `fn native`)
src/dungeon.rs        procedural floor generation (pure data, unit-tested)
src/rng.rs            one seeded stream for the whole run
src/texture.rs        seamless stone + flagstone (pure data, unit-tested)
src/mesh.rs           custom low-poly meshes: box, pillar, monster, gem
src/hud.rs            HUD layout as pure, unit-tested functions
tools/                autopilot_check.sh — the winnability gate
```

## Notes on the two halves

**`src/natives.rs`** uses a thread-local pointer installed for the duration of the brain
update. Natives are free functions, so they can't take `&mut Game`; the pointer is the
smallest thing that works and keeps the `.ox` side free of Rust-specific concepts.

**`src/dungeon.rs`** places rooms on a jittered lattice rather than by rejection sampling.
Rejection sampling can't fill small floors — on an 11×11 grid three 6-wide rooms can never
coexist without overlap, so every attempt is rejected and you get a one-room floor where
the spawn and the stairs are the same tile. A lattice always fits. There's a test for it.

## Credits

- [jame-engine](https://github.com/suspiciousMans/jame-engine) — the engine
- [oxidized](https://github.com/suspiciousMans/oxidized) — the game-logic language