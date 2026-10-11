# Dreamscape / dungeon_oxide rules tests

These tests exercise the **real** `scripts/dungeon.ox`: the file is imported
(`import "../../scripts/dungeon.ox"`), never copied.

## Run

```
oxidized test tests/rules/
```

Needs an `oxidized` build that has `oxidized test` **and** stand-ins for `fn native`
host functions. Without the stand-ins the rules file cannot be compiled outside the
engine (`unresolved import crate::__oxidized_natives`). Currently: 84 tests, all passing.

## What is covered

Every pure function in `dungeon.ox`: `monster_decision` (every branch and boundary),
`move_speed`, `strike_damage`, `strike_cooldown`, `clamp`/`clamp_i`,
`monster_count_for_depth`, `kind_pool_for_depth`, `health_on_descend`, `pickup_line`,
`floor_epigraph`, `player_attack_damage`, `attack_cooldown`, `attack_range`, `kill_reward`.

## What is NOT covered (needs native mocking)

Functions that call the host engine cannot run under `oxidized test`: each `fn native`
gets a stand-in that panics with
`native function ... is implemented by the host engine and is not available under oxidized test`.

- `incoming_damage` (uses `host_rng()`)
- `roll_loot` (uses `host_rng()`)
- `danger_level` (reads host state)

Their tests were removed rather than run against a made-up copy. They become testable
once natives can be mocked.

## Mutation check

17 hand-made bugs were applied one at a time to a temporary copy of `dungeon.ox`
(flipped comparisons, off-by-ones, wrong constants, swapped operands, removed guards)
and the suite was run against each:

- 16 killed.
- 1 survived: `n > capped` -> `n >= capped` in `monster_count_for_depth`. It is an
  **equivalent mutant** (at `n == capped` both branches return 7), so no test can
  tell the two apart.
- 2 mutants survived at first (the `hp_frac > 0.15` guard on the 1.8 attack range, and
  the 1.2 point-blank boundary for wounded monsters); four tests were added to kill them.
