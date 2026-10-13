# Tests for the host-dependent rules in dungeon.ox: roll_loot, incoming_damage,
# danger_level. The natives are mocked; each mock reads a shared cell, so one
# file can drive many values (a `mock fn` has one body per file).
#
# Boundary cases use the SAME float expression as the rule under test, so
# `<` vs `<=` mutants are decided exactly, not by luck of rounding.

import "../../scripts/dungeon.ox"

mock fn host_rng() -> Float {
    return shared_get("rng")
}

mock fn host_monster_count() -> Int {
    return shared_get("count")
}

mock fn host_player_health() -> Float {
    return shared_get("hp")
}

fn near(a: Float, b: Float) -> Bool {
    let d = a - b
    if d < 0.0 {
        d = 0.0 - d
    }
    return d < 0.000001
}

fn loot(depth: Int, kind: Int, r: Float) -> Int {
    shared_set("rng", r)
    return roll_loot(depth, kind)
}

fn dmg(raw: Float, armor: Float, r: Float) -> Float {
    shared_set("rng", r)
    return incoming_damage(raw, armor)
}

fn danger(count: Int, hp: Float) -> Int {
    shared_set("count", count)
    shared_set("hp", hp)
    return danger_level()
}

# ---------------------------------------------------------------- roll_loot
# depth 1: health < 0.37, gold < 0.82, upgrade < 1.02 (always)

fn test_loot_health_band_at_depth_1() {
    assert_eq(loot(1, 0, 0.0), 2)
    assert_eq(loot(1, 0, 0.35), 2)
}

fn test_loot_gold_band_at_depth_1() {
    assert_eq(loot(1, 0, 0.40), 1)
    assert_eq(loot(1, 0, 0.80), 1)
}

fn test_loot_upgrade_band_at_depth_1() {
    assert_eq(loot(1, 0, 0.84), 3)
    assert_eq(loot(1, 0, 0.99), 3)
}

fn test_loot_cowards_drop_nothing_whatever_the_roll() {
    assert_eq(loot(1, 2, 0.0), 0)
    assert_eq(loot(1, 2, 0.5), 0)
    assert_eq(loot(9, 2, 0.99), 0)
}

fn test_loot_health_boundary_is_exclusive() {
    # r exactly equal to health_chance is NOT health.
    let r = 0.4 - 1 as Float * 0.03
    assert_eq(loot(1, 0, r), 1)
    let r5 = 0.4 - 5 as Float * 0.03
    assert_eq(loot(5, 0, r5), 1)
}

fn test_loot_gold_boundary_is_exclusive() {
    # r exactly equal to health_chance + 0.45 is NOT gold.
    let hc = 0.4 - 3 as Float * 0.03
    let r = hc + 0.45
    assert_eq(loot(3, 0, r), 3)
}

fn test_loot_chance_shrinks_with_depth() {
    # r = 0.30: health at depth 1 (0.37) and 3 (0.31), not at depth 5 (0.25).
    assert_eq(loot(1, 0, 0.30), 2)
    assert_eq(loot(3, 0, 0.30), 2)
    assert_eq(loot(5, 0, 0.30), 1)
}

fn test_loot_health_still_possible_when_chance_is_tiny() {
    # depth 12: health_chance = 0.04
    assert_eq(loot(12, 0, 0.01), 2)
    assert_eq(loot(12, 0, 0.05), 1)
}

# depth 10: health < 0.10, gold < 0.55, upgrade < 0.93, else nothing
fn test_loot_nothing_band_exists_deep_down() {
    assert_eq(loot(10, 0, 0.935), 0)
    assert_eq(loot(10, 0, 0.99), 0)
}

fn test_loot_upgrade_band_at_depth_10() {
    assert_eq(loot(10, 0, 0.56), 3)
    assert_eq(loot(10, 0, 0.90), 3)
}

fn test_loot_upgrade_boundary_is_exclusive() {
    let hc = 0.4 - 10 as Float * 0.03
    let uc = 0.18 + 10 as Float * 0.02
    let r = hc + 0.45 + uc
    assert_eq(loot(10, 0, r), 0)
}

fn test_loot_upgrade_chance_grows_with_depth() {
    # depth 6: 0.22 + 0.45 + 0.30 = 0.97 -> 0.96 still upgrade
    # depth 11: 0.07 + 0.45 + 0.40 = 0.92 -> 0.95 is nothing
    assert_eq(loot(6, 0, 0.96), 3)
    assert_eq(loot(11, 0, 0.95), 0)
}

# ----------------------------------------------------------- incoming_damage

fn test_damage_variance_range() {
    assert(near(dmg(10.0, 0.0, 0.0), 8.5))
    assert(near(dmg(10.0, 0.0, 0.5), 10.0))
    assert(near(dmg(10.0, 0.0, 1.0), 11.5))
}

fn test_damage_armor_reduces_before_variance() {
    assert(near(dmg(10.0, 0.5, 0.0), 4.25))
    assert(near(dmg(20.0, 0.25, 1.0), 17.25))
}

fn test_damage_armor_is_capped_at_three_quarters() {
    assert(near(dmg(10.0, 0.75, 0.5), 2.5))
    assert(near(dmg(10.0, 5.0, 0.5), 2.5))
}

fn test_damage_negative_armor_is_ignored() {
    assert(near(dmg(10.0, -1.0, 0.5), 10.0))
}

# -------------------------------------------------------------- danger_level

fn test_danger_zero_when_nothing_alive_even_if_hurt() {
    assert_eq(danger(0, 100.0), 0)
    assert_eq(danger(0, 10.0), 0)
}

fn test_danger_is_half_the_monster_count() {
    assert_eq(danger(1, 100.0), 0)
    assert_eq(danger(2, 100.0), 1)
    assert_eq(danger(4, 100.0), 2)
    assert_eq(danger(5, 100.0), 2)
    assert_eq(danger(6, 100.0), 3)
}

fn test_danger_low_health_adds_two() {
    assert_eq(danger(2, 10.0), 3)
    assert_eq(danger(4, 34.9), 4)
    assert_eq(danger(1, 20.0), 2)
}

fn test_danger_health_threshold_is_exclusive() {
    assert_eq(danger(2, 35.0), 1)
    assert_eq(danger(2, 35.02), 1)
    assert_eq(danger(2, 34.98), 3)
}

fn test_danger_is_clamped_to_ten() {
    assert_eq(danger(30, 100.0), 10)
    assert_eq(danger(20, 100.0), 10)
    assert_eq(danger(19, 100.0), 9)
    assert_eq(danger(20, 5.0), 10)
}
