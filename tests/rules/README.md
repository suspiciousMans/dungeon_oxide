# Dreamscape / dungeon_oxide rules tests

These tests exercise the **real** `scripts/dungeon.ox`: the file is imported
(`import "../../scripts/dungeon.ox"`), never copied.

## Run

```
oxidized test tests/rules/
```

Needs an `oxidized` build that has `oxidized test` **and** stand-ins for `fn native`
host functions. Without the stand-ins the rules file cannot be compiled outside the
engine (`unresolved import crate::__oxidized_natives`). Currently: 105 tests, all passing.

## What is covered

Every pure function in `dungeon.ox`: `monster_decision` (every branch and boundary),
`move_speed`, `strike_damage`, `strike_cooldown`, `clamp`/`clamp_i`,
`monster_count_for_depth`, `kind_pool_for_depth`, `health_on_descend`, `pickup_line`,
`floor_epigraph`, `player_attack_damage`, `attack_cooldown`, `attack_range`, `kill_reward`.

## Host-dependent rules (native mocks)

`test_dungeon_host_rules.ox` covers the functions that call the engine: `roll_loot`,
`incoming_damage`, `danger_level`. It uses `mock fn` (an `oxidized` feature: a test-only body
for a `fn native`) and drives the mocks through `shared_get` / `shared_set` cells, because a
`mock fn` has one body per file. Boundary cases build `r` from the same float expression as the
rule, so `<` vs `<=` is decided exactly.

## Mutation check

Single-operator and single-constant mutants of those three functions (48 total), each run
against both test files. Result: **47 killed, 1 survived**. The survivor
(`health_chance > 0.0` -> `>= 0.0`) is equivalent: the guard is redundant because `r >= 0`
can never be below a non-positive chance, and `health_chance` is never exactly `0.0` for an
integer depth. The first version of the mock tests (8 tests, one RNG value) killed only
16 of 48, which is why this suite exists.

Current totals: 105 tests (84 pure + 21 host), all passing.

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
