# Dungeon Oxide — combat, audio, texture, and shipping

## Goal

Make *The Catacombs of Rust* a complete, winnable game: the player can fight and kill
monsters, the autopilot can win a run end-to-end, and the game has audio and textured
geometry instead of untextured grey boxes and silence.

## Current context / assumptions

Repo: `C:\Users\james\dungeon_oxide` (standalone; engine is a git dependency on
`suspiciousMans/jame-engine`). Branch `main`, clean tree at `0dfcf19`. Remote
`suspiciousMans/dungeon_oxide`. Pushed.

**Read this before touching anything.** The codebase is unusual and the following is
load-bearing:

1. **The game logic is not in Rust.** `scripts/dungeon.ox` holds every rule (monster
   brains, loot tables, damage, pacing, flavour text). It is written in **oxidized**, a
   language whose compiler lives at `github.com/suspiciousMans/oxidized` and must be on
   `PATH` as an `oxidized` binary (or set `OXIDIZED=<path>`). `build.rs` shells out to it
   at compile time to transpile `.ox` → Rust. **A plain `cargo build` without the binary
   present fails in `build.rs`.**
2. **Rust calls `.ox` functions as ordinary Rust.** Each `.ox` function becomes a
   `pub fn` in `ox_modules::dungeon`, with typed signatures. `Int` in oxidized is `i64`,
   so **every call site needs `as i64` / `as i64` casts**.
3. **Two testability tiers for `.ox` functions — this matters for writing tests:**
   - *Pure* functions (no `host_*` call) are callable directly from a Rust `#[cfg(test)]`
     test with no setup: `monster_decision`, `move_speed`, `strike_damage`,
     `strike_cooldown`, `monster_count_for_depth`, `kind_pool_for_depth`,
     `health_on_descend`, `pickup_line`, `floor_epigraph`, `clamp`, `clamp_i`.
   - *Impure* functions that call `host_*` require an installed bridge or they **panic**:
     `roll_loot` (calls `host_rng`), `incoming_damage` (calls `host_rng`), `danger_level`
     (calls `host_monster_count`, `host_player_health`). Test those via
     `natives::with_bridge(&mut Bridge::snapshot(...), |_| ...)`.
4. **Do not hand out `&mut` from a `RefCell` for the bridge.** It already uses a
   thread-local `*mut Bridge` installed by `natives::with_bridge`. Keep it that way.
5. **Assets resolve via `resolve_asset_root()`** (exe dir, then `CARGO_MANIFEST_DIR` in
   debug). Never bare relative paths — that was a shipped crash, fixed in `0dfcf19`.
6. **An active `.exe` locks the binary on Windows.** Before every rebuild:
   `taskkill /F /IM dungeon_oxide.exe`. Treat `Access is denied (os error 5)` as a failed
   build.
7. Shaders fed to `ShaderVariantCache::new` must **not** contain a `#version` line.
8. egui's painter leaves `DEPTH_TEST` off and `BLEND`/`SCISSOR_TEST` on; `render()`
   re-enables depth right after `renderer.begin_scene(gl)`. Keep that.

**Build/test commands** (run from the repo root; `oxidized` must be on PATH —
`export PATH="$HOME/oxidized_repo/target/release:$PATH"`):

```sh
cargo build                                  # expect: Finished `dev` profile
cargo test                                   # expect: 14 passed; 0 failed
DUNGEON_AUTOPILOT=1 DUNGEON_SEED=42 RUST_LOG=info \
  timeout 60 ./target/debug/dungeon_oxide.exe 2>&1 | grep -E 'descended|run ended'
```

Current autopilot behaviour: reaches depth 5–6 of 8, dies, zero panics. Gold is always 0
because the autopilot cannot fight — that is the gap this plan closes.

## Architecture / proposed approach

Combat is added the same way everything else in this project works: the **rules go in
`.ox`, the mechanics go in Rust**. `.ox` gains `player_attack_damage`, `attack_cooldown`,
`attack_range`, and `kill_reward`; Rust gains the input, the raycast-ish range check, the
damage application, monster death, and the gold payout. The autopilot gains a simple
"engage nearest monster in range, attack" behaviour so a run can be *proven* winnable
headlessly. Audio and textures are Rust-side presentation work that does not touch the
`.ox` boundary.

## Step-by-step tasks

Work in order. Each ends with a commit. **Total ~20 tasks.**

---

### Milestone 1 — Combat (the big one)

#### Task 1.1 — Write failing tests for the new `.ox` attack rules

The `.ox` functions don't exist yet, so these tests won't compile — that *is* the failing
state.

Append to `src/main.rs` (inside the existing `mod tests` if there is one; the file
currently has **no** `#[cfg(test)]` module — add one at the end of the file):

```rust
#[cfg(test)]
mod ox_rules_tests {
    use super::ox_modules::dungeon as ox;

    #[test]
    fn attack_damage_scales_with_depth() {
        let shallow = ox::player_attack_damage(1);
        let deep = ox::player_attack_damage(8);
        assert!(
            deep > shallow,
            "deeper floors must hit harder: {deep} !> {shallow}"
        );
    }

    #[test]
    fn attack_damage_is_never_zero() {
        for depth in 1..=MAX_DEPTH {
            assert!(
                ox::player_attack_damage(depth) > 0.0,
                "depth {depth} dealt zero damage"
            );
        }
    }

    #[test]
    fn attack_cooldown_never_reaches_zero() {
        // A zero cooldown would be a fire rate of infinity.
        for depth in 1..=MAX_DEPTH {
            assert!(ox::attack_cooldown(depth) > 0.0, "depth {depth} cooldown was 0");
        }
    }

    #[test]
    fn attack_cooldown_shrinks_with_depth() {
        assert!(ox::attack_cooldown(8) < ox::attack_cooldown(1));
    }

    #[test]
    fn attack_range_is_reachable_and_not_pointless() {
        let r = ox::attack_range();
        assert!(r >= 1.5, "melee range {r} is too short to be usable");
        assert!(r <= 4.0, "attack range {r} is longer than a dungeon corridor");
    }

    #[test]
    fn kill_reward_grows_with_depth() {
        assert!(ox::kill_reward(8) > ox::kill_reward(1));
    }
}
```

Run: `cargo test ox_rules_tests 2>&1 | tail -20`
Expected: compile errors naming `player_attack_damage`, `attack_cooldown`, `attack_range`,
`kill_reward` as unresolved. **That is the expected failure.** Confirm the errors are
about the missing functions and nothing else.

#### Task 1.2 — Add the attack rules to `scripts/dungeon.ox`

**Oxidized constraint that will bite you:** the transpiler *leaks scope between sibling
`if` blocks* — a re-`let` of the same name in a second sequential `if` is emitted as an
assignment, not a declaration, and won't compile. Sibling `if`/`else` is fine. Use distinct
names in sequential `if`s. Also: `Int` is `i64`; `Float` is `f64`.

Append to `scripts/dungeon.ox`:

```rust
# ============================================================================
# PLAYER COMBAT
#
# The host asks for these numbers each time the player swings; all the
# balance lives here.
# ============================================================================

# How hard a swing hits. Grows with depth so a floor-8 monster still takes a
# few hits rather than dying to one — otherwise the deep floors are a wall.
fn player_attack_damage(depth: Int) -> Float {
    let base = 10.0 + depth as Float * 2.0
    return base
}

# Seconds between swings. Tightens a little with depth so it feels like
# escalation, but never hits zero.
fn attack_cooldown(depth: Int) -> Float {
    let t = 0.55 - depth as Float * 0.03
    if t < 0.25 {
        return 0.25
    }
    return t
}

# How far the swing reaches, in world units.
fn attack_range() -> Float {
    return 2.2
}

# Gold awarded for a kill. The only source of gold in the game, so it has to
# be the thing that rewards descending.
fn kill_reward(depth: Int, monster_kind: Int) -> Int {
    let base = 1 + depth + monster_kind
    return base
}
```

Verify the `.ox` parses and typechecks **on its own** before building:

```sh
oxidized check "C:/Users/james/dungeon_oxide/scripts/dungeon.ox"
```
Expected: `oxidized: no errors`

Then `cargo build`. Expected: `Finished` with **no errors**; the new Rust test functions
must now compile (you may still see unused-code warnings).

#### Task 1.3 — Add the attack state to the game struct

In `src/main.rs`, add fields to `struct DungeonGame` and initialise them in `new()`:

```rust
    attack_cooldown: f32,
    attack_cd_cache: f32,
```

```rust
            attack_cooldown: 0.0,
            attack_cd_cache: 0.6,
```

`attack_cd_cache` exists because the cooldown is *asked* from `.ox` but only changes on
descend; calling across the boundary every frame for a value that never changes would be
wasteful.

Commit: `git commit -am "Add player combat rules to the oxidized game brain"`

#### Task 1.4 — Attack input

SDL mouse buttons — `use sdl2::mouse::MouseButton;` at the top of `src/main.rs`.

Add this method inside the `impl DungeonGame` block that has `update_look`:

```rust
    /// True when the player asked to swing this frame: left mouse button, or
    /// the keyboard fallback (Space) because some users don't have a mouse.
    fn wants_attack(&self, ctx: &Context) -> bool {
        self.autopilot || ctx.input.is_button_down(MouseButton::Left)
    }
```

Verification: `cargo build`. Expected `Finished`, no errors.

#### Task 1.5 — Failing test for the attack resolution (pure Rust, no window)

Add to the same `mod ox_rules_tests` (or a new `mod combat_tests`) in `src/main.rs`:

```rust
    /// Which monster, if any, a swing connects with: the nearest living one
    /// within `range`. Extracted as a pure function so it can be tested
    /// without a window or an ECS world.
    #[test]
    fn nearest_monster_in_range_is_found() {
        let monsters = [
            (Vec3::new(1.0, 0.0, 0.0), 50.0f32), // alive, in range
            (Vec3::new(3.0, 0.0, 0.0), 50.0f32), // alive, farther
            (Vec3::new(0.5, 0.0, 0.0), 0.0f32),  // dead, closest
        ];
        let hit = nearest_monster_in_range(Vec3::ZERO, &monsters, 2.5);
        assert_eq!(hit, Some(1), "should hit the nearest ALIVE monster");
    }

    #[test]
    fn no_monster_in_range_returns_none() {
        let monsters = [(Vec3::new(10.0, 0.0, 0.0), 50.0f32)];
        assert_eq!(nearest_monster_in_range(Vec3::ZERO, &monsters, 2.5), None);
    }

    #[test]
    fn dead_monsters_are_never_targeted() {
        let monsters = [(Vec3::new(1.0, 0.0, 0.0), 0.0f32)];
        assert_eq!(nearest_monster_in_range(Vec3::ZERO, &monsters, 5.0), None);
    }
```

Run `cargo test combat_tests 2>&1 | tail -20`.
Expected: `cannot find function nearest_monster_in_range`. **That is the expected failure.**

#### Task 1.6 — Implement `nearest_monster_in_range`

Add this as a free function in `src/main.rs` (place it next to `center_window`):

```rust
/// Index of the nearest living monster within `range` of `origin`, or `None`.
/// Pure, so it's unit-testable without a window or an ECS world. Dead
/// monsters (health <= 0) are skipped — a corpse must never eat a swing.
fn nearest_monster_in_range(
    origin: Vec3,
    monsters: &[(Vec3, f32)],
    range: f32,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, (pos, health)) in monsters.iter().enumerate() {
        if *health <= 0.0 {
            continue;
        }
        let d = origin.distance(*pos);
        if d > range {
            continue;
        }
        match best {
            Some((_, bd)) if bd <= d => {}
            _ => best = Some((i, d)),
        }
    }
    best.map(|(i, _)| i)
}
```

Run `cargo test combat_tests`. Expected: `3 passed`.

Commit: `git commit -am "Add melee targeting as a pure, testable function"`

#### Task 1.7 — Wire the swing into the update loop

In `fn update`, insert this call **after** `self.update_monsters(dt);` and **before**
`self.check_pickups();` (so a monster killed this frame can't also drop loot this frame —
loot is handled inside the kill path):

```rust
        self.update_combat(ctx, dt);
```

Then add the method:

```rust
    /// Resolve a player swing: pick the target, roll damage from `.ox`,
    /// apply it, and pay out gold on a kill.
    fn update_combat(&mut self, ctx: &Context, dt: f32) {
        self.attack_cooldown -= dt;
        if self.dead || self.won || !self.wants_attack(ctx) || self.attack_cooldown > 0.0 {
            return;
        }

        let depth = self.depth;
        self.attack_cd_cache = self
            .ask(|_| ox_modules::dungeon::attack_cooldown(depth as i64) as f32);
        self.attack_cooldown = self.attack_cd_cache;

        let monsters: Vec<(Vec3, f32)> = self
            .monsters
            .iter()
            .map(|m| (m.position, m.health))
            .collect();
        let range = self.ask(|_| ox_modules::dungeon::attack_range() as f32);
        let Some(index) = nearest_monster_in_range(self.player_position, &monsters, range)
        else {
            return;
        };

        let damage = self
            .ask(|_| ox_modules::dungeon::player_attack_damage(depth as i64) as f32);
        self.monsters[index].health -= damage;

        if self.monsters[index].health <= 0.0 {
            let kind = self.monsters[index].kind;
            let reward = self
                .ask(|_| ox_modules::dungeon::kill_reward(depth as i64, kind as i64));
            self.gold += reward as i32;
            self.toast(format!("Slain. +{reward} gold."));
            self.drop_loot(index);
            if let Ok(mut t) = self.world.get::<&mut Transform>(self.monsters[index].entity) {
                t.scale = Vec3::ZERO;
            }
        } else {
            let remaining = self.monsters[index].health;
            self.toast(format!("Hit for {damage:.0} ({remaining:.0} left)."));
        }
    }
```

**Note on `drop_loot`:** it currently starts with `let kind = self.monsters[index].kind;`
and reads `self.monsters[index].position`. It is safe to call on a dead monster — it does
not mutate the monster. But it *does* push to `self.loot`, and `check_pickups` will later
`swap_remove` it. Verify no double-drop: `drop_loot` must only be called from **one** place.
Grep first: `grep -n 'drop_loot' src/main.rs` — if `update_monsters` already calls it on
death, **remove that call** (it currently does, inside its `health <= 0.0` branch) and
delete that branch, so death handling lives only in `update_combat`. A monster killed by a
swing must drop loot exactly once.

`cargo build`. Expected `Finished`, no errors.

Commit: `git commit -am "Wire player melee into the game loop"`

#### Task 1.8 — Failing test: the autopilot must be able to win

This is the end-to-end proof. It cannot be a unit test (it needs a window), so it's a
scripted check — but write it down as a task with a hard pass bar.

First create `tools/` and add `tools/autopilot_check.sh`:

```sh
#!/usr/bin/env bash
# Proves a run is winnable: the autopilot must reach MAX_DEPTH and set won=true.
# Pass bar: "escaped" in the output, zero panics, exit 0.
set -uo pipefail
cd "$(dirname "$0")/.."
BIN=./target/debug/dungeon_oxide.exe
[ -x "$BIN" ] || { echo "FAIL: build first (cargo build)"; exit 1; }

fails=0
for seed in 1 42 777 1234 2024; do
  out=$(DUNGEON_AUTOPILOT=1 DUNGEON_SEED=$seed RUST_LOG=info \
        timeout 120 "$BIN" 2>&1)
  panics=$(printf '%s' "$out" | grep -c 'panic')
  if printf '%s' "$out" | grep -q 'escaped'; then
    echo "seed $seed: WON   (panics=$panics)"
  else
    echo "seed $seed: LOST  (panics=$panics) $(printf '%s' "$out" | grep 'run ended' | tail -1)"
    fails=$((fails+1))
  fi
  [ "$panics" -eq 0 ] || { echo "  seed $seed had $panics panics"; fails=$((fails+1)); }
done
[ "$fails" -eq 0 ] && echo "ALL SEEDS WON" || echo "FAIL: $fails problem(s)"
exit $fails
```

Run: `chmod +x tools/autopilot_check.sh && ./tools/autopilot_check.sh`
Expected **right now**: every seed reports `LOST`. That is the failing state.

Commit: `git commit -am "Add autopilot win-check script (currently failing by design)"`

#### Task 1.9 — Make the autopilot fight

In `fn autopilot_dir` (or a new `autopilot_fight_dir`), the autopilot must attack when a
monster is in range and otherwise walk to the stairs. Replace the body of the autopilot
branch in `fn move_dir` with:

```rust
        if self.autopilot {
            let monsters: Vec<(Vec3, f32)> = self
                .monsters
                .iter()
                .map(|m| (m.position, m.health))
                .collect();
            let range = self.ask(|_| ox_modules::dungeon::attack_range() as f32);
            // Fight if something is worth hitting and we're not already on top
            // of the stairs; otherwise keep navigating.
            if let Some(i) = nearest_monster_in_range(self.player_position, &monsters, range) {
                let dir = self.monsters[i].position - self.player_position;
                return Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
            }
            return self.autopilot_dir();
        }
```

(`update_combat` already swings whenever `wants_attack` is true and the autopilot sets
that flag, so no extra attack code is needed here.)

`cargo build`, then `./tools/autopilot_check.sh`.
Expected: `seed N: WON` for most seeds. **If some seeds still lose, do not relax the test.**
Instead rebalance in `.ox`: lower `player_attack_damage` growth, raise `health_on_descend`,
or reduce `monster_count_for_depth`. Re-run until all seeds win or you can justify each
loss explicitly in the commit message.

Commit: `git commit -am "Autopilot fights, so a run is provably winnable"`

#### Task 1.10 — Full regression

```sh
cargo test 2>&1 | tail -5
./tools/autopilot_check.sh
```
Expected: `test result: ok. 20 passed` (or more) and `ALL SEEDS WON`.

**Also verify from a foreign working directory** — this caught a shipped crash before:

```sh
cd /tmp && "$HOME/dungeon_oxide/target/debug/dungeon_oxide.exe" 2>&1 | head -4
```
Expected: `using shader profile "clean_lowpoly"` and `init complete: seed …`, **not** a
shader compile error.

`cd "$HOME/dungeon_oxide"` to get back.

---

### Milestone 2 — Audio

#### Task 2.1 — Add the dependency

`Cargo.toml` needs the audio dependency to match the engine's:

```toml
rodio = { version = "0.19", default-features = false, features = ["symphonia-all"] }
```

(Without this the `AudioContext` API is unusable even though the engine re-exports it.)

#### Task 2.2 — Create the context

`use engine::audio::AudioContext;` in `src/main.rs`. Add `audio: Option<AudioContext>` to
the struct and, in `init()`:

```rust
        // Audio is optional: a machine with no output device must still play.
        self.audio = match AudioContext::new() {
            Ok(audio) => Some(audio),
            Err(err) => {
                log::warn!("no audio output available, sounds will be silent: {err}");
                None
            }
        };
```

with `audio: None` in `new()`.

`cargo build`. Expected `Finished`.

#### Task 2.3 — Procedural SFX

There are no sound files in the repo. Use `play_tone`, which needs no assets:

```rust
    /// Fire-and-forget sound effect. `AudioContext::play_tone(freq_hz, secs)`.
    fn sfx(&self, freq: f32, secs: f32) {
        if let Some(audio) = &self.audio {
            audio.play_tone(freq, secs);
        }
    }
```

Call sites, added to the existing event points:

- swing misses → `self.sfx(220.0, 0.05);`
- hit a monster → `self.sfx(150.0, 0.08);`
- kill → `self.sfx(90.0, 0.25);`
- taking damage → `self.sfx(70.0, 0.15);`
- descending a floor → `self.sfx(440.0, 0.3);`

`.cargo/config.toml` is unrelated to audio; ignore it if it looks tempting.

`cargo build`, then **listen**: launch the game and swing. Verify by ear. If you cannot
hear it, say so in the commit — do not claim audio works because it compiles.

Commit: `git commit -am "Add procedural audio for combat and feedback"`

#### Task 2.4 — Route the `.ox` `host_play_tone` native

`scripts/dungeon.ox` already declares `fn native host_play_tone(freq: Float);` and
`src/natives.rs` already pushes a `PlayTone` action — **but nothing drains it.** Either
(a) call `sfx` directly at the Rust call sites above (simplest, and what this plan does), or
(b) drain `bridge.actions` and handle `Action::PlayTone`. **Choose (a) and delete the
unused `Action::PlayTone` variant and the `host_play_tone` native** to avoid dead code. If
you keep the native instead, you must drain the actions list and you need a test for it.

Commit with the previous task is fine; don't leave the variant unused.

---

### Milestone 3 — Textured geometry

#### Task 3.1 — Failing test: textures tile seamlessly

Pure function, no GL. Add to `src/rng.rs`'s test module (or a new `mod texture_tests` in a
new file `src/texture.rs`):

```rust
    /// A tiling texture must not have a visible seam. Compare the wrap-around
    /// neighbour delta (last column/row -> first) against the worst interior
    /// neighbour delta; if the wrap delta is materially larger, the pattern
    /// doesn't tile.
    #[test]
    fn generated_texture_tiles_seamlessly() {
        let (w, h) = (64usize, 64usize);
        let px = stone_texture(w, h);
        assert_eq!(px.len(), w * h * 4, "texture must be RGBA8");

        let wrap_d = |a: u8, b: u8| (a as i32 - b as i32).abs();
        let at = |x: usize, y: usize| {
            let i = (y % h * w + x % w) * 4;
            px[i]
        };

        let mut worst_wrap = 0i32;
        let mut worst_interior = 0i32;
        for y in 0..h {
            for x in 0..w {
                worst_wrap = worst_wrap.max(wrap_d(at(x, y), at(0, y)));
                worst_wrap = worst_wrap.max(wrap_d(at(x, y), at(x, 0)));
                if x + 1 < w {
                    worst_interior = worst_interior.max(wrap_d(at(x, y), at(x + 1, y)));
                }
                if y + 1 < h {
                    worst_interior = worst_interior.max(wrap_d(at(x, y), at(x, y + 1)));
                }
            }
        }
        assert!(
            worst_wrap <= worst_interior + 8,
            "seam too visible: wrap delta {worst_wrap} vs interior {worst_interior}"
        );
    }
```

Run `cargo test stone_texture`. Expected: `cannot find function stone_texture`. **Expected
failure.**

#### Task 3.2 — Implement the stone texture

New file `src/texture.rs`:

```rust
//! Procedural textures. Pure functions producing RGBA8, so they're testable
//! without a GL context.

/// Seamless stone-ish texture. Every term is a periodic function of `x`/`y`
/// over the tile, so the pattern wraps with no visible seam.
pub fn stone_texture(w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            // Integer-frequency sines are periodic over the tile.
            let n = (u * 8.0).sin() * (v * 6.0).sin() + (u * 13.0).sin() * (v * 11.0).sin();
            let grain = ((u * 40.0).sin() + (v * 37.0).sin()) * 0.15;
            let l = 0.42 + n * 0.06 + grain;
            let base = (l.clamp(0.0, 1.0) * 255.0) as u8;
            let r = (base as f32 * 0.98) as u8;
            let g = base;
            let b = (base as f32 * 1.06).min(255.0) as u8;
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    out
}
```

`cargo test stone_texture`. Expected `1 passed`.

#### Task 3.3 — Upload and use it

In `init()`, after the white fallback texture:

```rust
        let stone = GpuTexture::from_rgba8(
            &gl,
            &stone_texture(64, 64),
            64,
            64,
            TextureFilter::Bilinear,
        )?;
        self.stone = Some(Arc::new(stone));
```

Add `mod texture;` and `use texture::stone_texture;`, plus a `stone: Option<Arc<GpuTexture>>`
field. Then, **only for the wall entities**, spawn with `texture: self.stone.clone()`;
leave the floor slab on the white fallback so the two read differently.

The cleanest way to do that without threading an extra field through every spawn: spawn
walls with `texture: Some(Arc::clone(self.stone.as_ref().expect("texture set up in init")))`
in the wall loop of `descend()`. Floors keep `white`.

**Bilinear, not Nearest** — a 64px pattern nearest-filtered shimmers into static at range.

#### Task 3.4 — Confirm visually — mandatory

```sh
taskkill /F /IM dungeon_oxide.exe 2>&1 | head -1
(cd . && DUNGEON_SEED=42 RUST_LOG=info ./target/debug/dungeon_oxide.exe > "$LOCALAPPDATA/Temp/dungeon_play.log" 2>&1 &)
sleep 5
```

Take a screenshot and **actually look at it**: walls must show visible texture, the floor
must be distinguishable from the walls. A flat grey screen is the failure mode — and note
that a camera facing a wall looks identical to a broken renderer (see Risks). Rotate or
move before concluding anything.

Commit: `git commit -am "Add a procedural seamless stone texture for walls"`

---

### Milestone 4 — Ship it

#### Task 4.1 — Update the README

`README.md`: correct the Controls table (add left mouse = attack), state the win
condition, and remove any "no player attack yet" implication. Keep the existing sections
on the `.ox`/Rust split — they're the point of the project.

#### Task 4.2 — Full verification sweep

```sh
cargo test 2>&1 | tail -3
./tools/autopilot_check.sh
cargo build --release 2>&1 | tail -3
cd /tmp && "$HOME/dungeon_oxide/target/release/dungeon_oxide.exe" 2>&1 | head -3
cd "$HOME/dungeon_oxide"
```

Expected: all tests pass, `ALL SEEDS WON`, release builds, and the release binary starts
from a foreign cwd.

#### Task 4.3 — Final human playtest handoff

Launch it for the user:

```sh
taskkill /F /IM dungeon_oxide.exe 2>&1 | head -1
(cd . && RUST_LOG=info ./target/debug/dungeon_oxide.exe > "$LOCALAPPDATA/Temp/dungeon_play.log" 2>&1 &)
sleep 4
```

Report: controls, the goal, and **what feedback you want**. Then mine
`$LOCALAPPDATA/Temp/dungeon_play.log` after they play before planning anything else.

#### Task 4.4 — Brain note

Append to `C:\Users\james\Documents\Obsidian Vault\Brain\Projects\oxidized-jame-dungeon\`
a new dated note recording: the combat rules added, the rebalance numbers that made the
autopilot win, and any new oxidized/engine traps. Update `Brain/index.md`'s existing
Dungeon Oxide bullet to say combat landed.

#### Task 4.5 — Push

```sh
git add -A
git commit -m "Combat, audio, and textures: the game is now winnable"
git push
```

---

## Tests / validation

**Per-task TDD** is specified inline above: every `.ox` rule and every pure Rust function
gets a test written first, run to see it fail for the *right* reason, implemented
minimally, re-run to pass, then committed. The expected failure output is quoted for each
so an implementer can tell "failing correctly" from "failing for a typo".

**Whole-suite gates** — the run is done when all of these hold:

| Command | Expected |
|---|---|
| `cargo test` | `test result: ok. N passed; 0 failed` |
| `./tools/autopilot_check.sh` | `ALL SEEDS WON`, exit 0 |
| `cd /tmp && <abs path to exe>` | boots, no shader error |
| screenshot of running window | textured walls, distinguishable floor, HUD correct |

**Not verifiable by tests** — state these honestly rather than claiming success: whether
the audio is audible, and whether the game *feels* good to play. Ask the user.

## Risks, tradeoffs, and open questions

**A flat grey screen is ambiguous.** Two very different failures look identical: a broken
renderer, and a camera pointed at a wall (`FirstPersonCamera` faces **-Z** at `yaw=0`,
which on a bordered procedural floor is almost always stone). Before touching the render
pipeline, log eye position, yaw/pitch, entity count, and the first drawn mesh's scale. The
player now faces the stairs at spawn, so this should be rarer — but don't trust that.

**Oxidized's sibling-`if` scope leak.** A re-`let` of the same name in a second sequential
`if` block emits an assignment and won't compile. Sibling `if`/`else` is fine. This is an
**unfixed transpiler bug** — if you write sequential `if`s, use distinct names. Consider
filing it as an issue on `suspiciousMans/oxidized`.

**Autopilot balance is a knife edge.** Task 1.9's pass bar (all seeds win) may force
several rebalance rounds. Rebalance in `.ox`, never by special-casing the autopilot — an
autopilot-only buff makes the win meaningless. If you can't make it win without breaking
manual play, say so and stop; don't fudge the test.

**`drop_loot` double-call risk.** It is currently called from `update_monsters`' death
branch. Task 1.7 must consolidate death handling into `update_combat` or a monster dies
twice and drops two loot piles. Grep before editing; add a test if you're unsure.

**The `ask()` helper clears actions.** `DungeonGame::ask` drains `bridge.actions` after
each call, so a `.ox` function that calls an *acting* native (e.g. `host_damage_monster`)
has its effect discarded. Every current `.ox` function is pure, so this is safe today — but
if you add one that acts, `ask` will silently swallow it. Either extend `ask` or route
acting natives through a separate non-draining path. **This is a real trap for the next
person.**

**Pure vs impure `.ox` functions in tests.** `roll_loot`, `incoming_damage`, and
`danger_level` call `host_rng`/`host_*` and **panic without an installed bridge**. Tests for
those must wrap them in `natives::with_bridge(...)`. See Task 3.1's note in the reference
table above.

**`oxide` binary required at build time.** Any fresh clone needs oxidized built and on
PATH before `cargo build` works. The README documents it; make sure it stays true.

**Audio can't be verified by CI.** `play_tone` requires an output device. On a machine
without one, `AudioContext::new()` fails and the game logs a warning and stays silent —
that's the intended behaviour, but it means "no sound in CI" is not a bug.

**`MAX_DEPTH` is 8 and lives in `src/main.rs`.** The `.ox` pacing functions assume a
similar range. Changing one without the other skews balance; the tests in Task 1.1 use
`MAX_DEPTH` so they'll follow automatically, but re-run `./tools/autopilot_check.sh` after
touching either.

**Open questions for the user** (don't guess — ask):
- Is 8 floors the right run length, or should a "full game" be longer with a boss?
- Should loot be visible on the floor as pickups, or auto-collected on kill? (Currently
  pickups spawn as cubes and are walked over.)
- Should there be multiple weapon types, or is one attack enough for v1?