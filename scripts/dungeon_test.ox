# Test suite for dungeon game logic using mocks for native functions

import "dungeon.ox"

# ============================================================================
# MOCKS FOR NATIVE HOST FUNCTIONS
# ============================================================================

# Mock for host_rng: return fixed values for deterministic testing
mock fn host_rng() -> Float {
    return 0.5
}

# ============================================================================
# TESTS FOR roll_loot
# ============================================================================

# Test 1: host_rng returns 0.5, should roll the default gold (kind 1)
fn test_roll_loot_gold_at_depth_1() {
    let kind = roll_loot(1, 1)
    # At depth 1, kind 1 (warrior): health_chance = 0.4 - 0.03 = 0.37
    # 0.5 > 0.37, and 0.5 < 0.37 + 0.45 = 0.82, so should be gold (kind 1)
    assert(kind == 1)
}

# Test 2: Cowardly monsters (kind 2) always drop nothing
fn test_roll_loot_cowardly_drops_nothing() {
    let kind = roll_loot(5, 2)
    assert(kind == 0)
}

# ============================================================================
# TESTS FOR incoming_damage
# ============================================================================

# Test 3: With mocked host_rng returning 0.5 (variance = 0.85 + 0.5 * 0.3 = 1.0)
#         and armor = 0, incoming_damage should equal raw damage
fn test_incoming_damage_no_armor() {
    let raw = 10.0
    let armor = 0.0
    let damage = incoming_damage(raw, armor)
    # variance = 0.85 + 0.5 * 0.3 = 1.0
    # reduced = 10.0 * (1.0 - 0.0) = 10.0
    # damage = 10.0 * 1.0 = 10.0
    let expected = 10.0
    # Use a small tolerance for floating-point comparison
    assert(damage > expected - 0.1 && damage < expected + 0.1)
}

# Test 4: Armor reduces damage
fn test_incoming_damage_with_armor() {
    let raw = 10.0
    let armor = 0.5
    let damage = incoming_damage(raw, armor)
    # reduced = 10.0 * (1.0 - 0.5) = 5.0
    # variance = 0.85 + 0.5 * 0.3 = 1.0
    # damage = 5.0 * 1.0 = 5.0
    let expected = 5.0
    assert(damage > expected - 0.1 && damage < expected + 0.1)
}

# Test 5: Armor caps at 0.75 so full damage reduction is impossible
fn test_incoming_damage_armor_capped() {
    let raw = 10.0
    let armor = 0.9  # More than cap of 0.75
    let damage = incoming_damage(raw, armor)
    # reduced = 10.0 * (1.0 - 0.75) = 2.5 (armor clamped)
    # variance = 1.0
    # damage = 2.5 * 1.0 = 2.5
    let expected = 2.5
    assert(damage > expected - 0.1 && damage < expected + 0.1)
}

# ============================================================================
# TESTS FOR monster_decision
# ============================================================================

# Test 6: Healthy monster at range should patrol
fn test_monster_patrol_when_far() {
    let decision = monster_decision(1, 0.8, 10.0)
    # kind=1, hp_frac=0.8, distance=10.0
    # Not cowardly thresholds
    # Not in strike range (distance >= 1.8, or distance < 1.2)
    # Aggro range = 6.0 + 0.8 * 6.0 = 10.8 > 10.0, so in aggro range
    # decision should be CHASE (2)
    assert(decision == 2)
}

# Test 7: Cowardly monster at low HP should flee
fn test_monster_flee_when_hurt() {
    let decision = monster_decision(2, 0.3, 5.0)
    # kind=2, hp_frac=0.3 < 0.35 (coward threshold)
    # decision should be FLEE (3)
    assert(decision == 3)
}

# Test 8: Monster in strike range should attack
fn test_monster_strike_in_range() {
    let decision = monster_decision(1, 0.5, 1.5)
    # kind=1, hp_frac=0.5 > 0.15
    # distance=1.5 < 1.8 and hp_frac > 0.15
    # decision should be STRIKE (4)
    assert(decision == 4)
}
