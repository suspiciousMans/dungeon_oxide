# Test Suite for Dungeon Oxidized Game Rules

## Overview

This directory contains a comprehensive test suite for **The Catacombs of Rust** game logic, implemented in the oxidized `.ox` language. The tests verify all pure game functions: monster AI, damage calculations, loot rules, difficulty scaling, and player combat mechanics.

## Files

- **`test_dungeon_rules.ox`** — Main test suite with 86 tests covering all pure functions
- **`pure_rules.ox`** — Extracted pure functions from `scripts/dungeon.ox` (no native dependencies)

## Running Tests

From the worktree root (`C:/Users/james/oxidized_wt/game`):

```bash
./game_oxidized.exe test tests/rules/test_dungeon_rules.ox
```

Or run all tests in the directory:

```bash
./game_oxidized.exe test tests/rules/
```

With output capture (see failing test prints):

```bash
./game_oxidized.exe test tests/rules/ --nocapture
```

Filter tests by name:

```bash
./game_oxidized.exe test tests/rules/ --filter monster_decision
```

## Test Coverage

### Monster AI (14 tests)
- Flee thresholds by kind (cowardly monsters at different health %s)
- Strike range logic (1.8 units at 15%+ health, 1.2 units always)
- Aggro range growth with health (6–12 unit dynamic range)
- Patrol and idle state transitions
- Distance-based decision tree

### Movement Speed (5 tests)
- Ramp scaling by depth (1.0 + depth × 0.08)
- Speed by decision type (idle, patrol, chase, flee)
- Depth-based difficulty progression

### Damage & Combat (16 tests)
- Strike damage formula (3.0 + kind×1.5 + depth×0.9)
- Strike cooldown capping at 1.0s
- Armor reduction (max 75% clamped)
- Variance in incoming damage (85–115% of reduced)
- Player attack scaling (10 + depth×2)
- Attack cooldown floor at 0.25s

### Game Generation (12 tests)
- Monster count per depth (2+depth, capped at 7)
- Kind pool unlocking (kind 1 at depth 0, kind 2 at depth 2, kind 3 at depth 5)
- Health restoration on descent (current + 34 - depth×1.5, clamped 55–100)

### Game Feel (9 tests)
- Pickup line generation (deterministic by depth/kind, indexed arrays)
- Floor epigraphs (mood escalation with depth)
- Flavor text variety without RNG

### Integration Tests (20 tests)
- Monster state machine (idle → patrol → chase → strike progression)
- Flee priority over strike range
- Difficulty scaling across depth (monsters stronger, player keeps pace)
- Armor stacking effects
- Edge cases (zero values, boundary conditions, min/max clamping)

### Limitations
- `roll_loot()` cannot be tested: calls `host_rng()` (engine-provided)
- `danger_level()` cannot be tested: calls native `host_monster_count()` and `host_player_health()`
- Tests of native-dependent functions are documented but not executable

## Architecture

The test suite uses **pure logic extracted from the real game rules**. Each test:

1. **Imports pure_rules.ox** (no native calls, so it compiles standalone)
2. **Tests one behavior** with a clear name: `test_<function>_<scenario>`
3. **Covers boundaries**: off-by-one, thresholds, edge cases
4. **Documents assumptions** in comments for complex formulas

## Known Issues

None identified in the current game rules. All tests document existing behavior accurately.

## Mutation Testing

To verify test quality, apply small bugs to `pure_rules.ox` and re-run. Each mutation should fail at least one test:

- Flip a comparison operator (`<` ↔ `>`)
- Off-by-one in bounds checks
- Wrong constant (e.g., 0.2 → 0.25)
- Swapped operands in math
- Removed conditional branch

See the main task report for mutation test results.
