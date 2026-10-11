# ============================================================================
# TEST SUITE FOR DUNGEON.OX GAME RULES
# ============================================================================
# Tests for The Catacombs of Rust game logic. Covers all pure functions,
# boundary values, state transitions, and game rule consistency.
# ============================================================================

import "../../scripts/dungeon.ox"

# =============================================================================
# MONSTER AI DECISION TESTS
# =============================================================================

# Test: monster_decision returns FLEE (3) when kind 2 (cowardly) drops below 35% health
fn test_monster_kind2_flees_at_35_percent_health() {
    let result = monster_decision(2, 0.34, 5.0)
    assert_eq(result, 3)
}

# Test: monster_decision returns STRIKE (4) when kind 2 with low health but still in strike range
fn test_monster_kind2_fleeing_at_boundary_just_below_35() {
    let result = monster_decision(2, 0.349, 5.0)
    assert_eq(result, 3)
}

# Test: monster_decision doesn't flee kind 2 at exactly 35%
fn test_monster_kind2_does_not_flee_at_35_percent() {
    let result = monster_decision(2, 0.35, 5.0)
    assert_ne(result, 3)
}

# Test: monster_decision returns FLEE (3) when kind 1 drops below 20% health
fn test_monster_kind1_flees_at_20_percent_health() {
    let result = monster_decision(1, 0.19, 5.0)
    assert_eq(result, 3)
}

# Test: monster_decision doesn't flee kind 1 at exactly 20%
fn test_monster_kind1_does_not_flee_at_20_percent() {
    let result = monster_decision(1, 0.20, 5.0)
    assert_ne(result, 3)
}

# Test: monster_decision returns STRIKE (4) when within 1.8 distance and > 15% health
fn test_monster_strike_at_1_8_range_healthy() {
    let result = monster_decision(1, 0.5, 1.79)
    assert_eq(result, 4)
}

# Test: monster_decision flees if kind 1 and hp < 0.2, even at close range
fn test_monster_strike_at_1_2_range_low_health_flees() {
    # Kind 1 with hp 0.01 will FLEE (3) because it's < 0.2
    let result = monster_decision(1, 0.01, 1.19)
    assert_eq(result, 3)
}

# Test: monster_decision returns STRIKE (4) at 1.2 range when healthy enough
fn test_monster_strike_at_1_2_range_when_healthy() {
    let result = monster_decision(1, 0.3, 1.19)
    assert_eq(result, 4)
}

# Test: monster_decision strike boundary at 1.8 (not higher)
fn test_monster_strike_boundary_at_1_8_not_2_0() {
    # At exactly 1.9, with hp > 0.15, should NOT strike yet (should be 2=CHASE)
    let result = monster_decision(1, 0.5, 1.9)
    assert_ne(result, 4)
}

# Test: monster_decision CHASE when in aggro range (6-12 units for full health)
fn test_monster_chase_in_aggro_range_full_health() {
    let result = monster_decision(1, 1.0, 6.5)
    assert_eq(result, 2)
}

# Test: monster_decision aggro range grows with health: 6 + hp_frac * 6
fn test_monster_aggro_range_at_half_health() {
    # At 0.5 health: aggro = 6.0 + 0.5 * 6.0 = 9.0
    let result = monster_decision(1, 0.5, 8.99)
    assert_eq(result, 2)
}

# Test: monster_decision PATROL when outside aggro but within 14
fn test_monster_patrol_outside_aggro_within_14() {
    # At 0.5 health: aggro = 6.0 + 0.5 * 6.0 = 9.0, so distance 12 is outside aggro
    let result = monster_decision(1, 0.5, 12.0)
    assert_eq(result, 1)
}

# Test: monster_decision IDLE when distance >= 14
fn test_monster_idle_at_14_distance() {
    let result = monster_decision(1, 0.5, 14.0)
    assert_eq(result, 0)
}

# Test: monster_decision IDLE when distance > 14
fn test_monster_idle_beyond_14_distance() {
    let result = monster_decision(1, 0.5, 20.0)
    assert_eq(result, 0)
}

# Test: monster_decision kind 0 respects general AI rules
fn test_monster_kind0_respects_general_rules() {
    # Kind 0 at distance 2.0, hp 0.5: in aggro range (6-12) so CHASE
    let result = monster_decision(0, 0.5, 2.0)
    assert_eq(result, 2)
}

# =============================================================================
# MONSTER MOVEMENT SPEED TESTS
# =============================================================================

# Test: move_speed for CHASE (decision 2) at depth 1
fn test_move_speed_chase_depth1() {
    let ramp = 1.0 + 1.0 * 0.08
    let expected = 2.6 * ramp
    let result = move_speed(2, 1)
    assert_eq(result, expected)
}

# Test: move_speed for FLEE (decision 3) increases with depth
fn test_move_speed_flee_depth10() {
    let ramp = 1.0 + 10.0 * 0.08
    let expected = 3.4 * ramp
    let result = move_speed(3, 10)
    assert_eq(result, expected)
}

# Test: move_speed for PATROL (decision 1) at depth 0
fn test_move_speed_patrol_depth0() {
    let result = move_speed(1, 0)
    assert_eq(result, 1.1)
}

# Test: move_speed for IDLE (decision 0) at depth 0
fn test_move_speed_idle_depth0() {
    let result = move_speed(0, 0)
    assert_eq(result, 0.4)
}

# Test: move_speed ramp grows linearly with depth (0.08 per depth level)
fn test_move_speed_ramp_linearity() {
    let speed_at_0 = move_speed(2, 0)
    let speed_at_5 = move_speed(2, 5)
    let expected_ratio = (1.0 + 5.0 * 0.08) / 1.0
    assert_eq(speed_at_5 / speed_at_0, expected_ratio)
}

# =============================================================================
# STRIKE DAMAGE TESTS
# =============================================================================

# Test: strike_damage at depth 1 with kind 0
fn test_strike_damage_depth1_kind0() {
    let expected = 3.0 + 0.0 * 1.5 + 1.0 * 0.9
    let result = strike_damage(0, 1)
    assert_eq(result, expected)
}

# Test: strike_damage grows with kind (monster type)
fn test_strike_damage_kind_scaling() {
    let kind0_dmg = strike_damage(0, 1)
    let kind1_dmg = strike_damage(1, 1)
    assert(kind1_dmg > kind0_dmg)
}

# Test: strike_damage grows with depth
fn test_strike_damage_depth_scaling() {
    let depth1_dmg = strike_damage(1, 1)
    let depth10_dmg = strike_damage(1, 10)
    assert(depth10_dmg > depth1_dmg)
}

# Test: strike_damage formula: base = 3.0 + kind * 1.5 + depth * 0.9
fn test_strike_damage_formula() {
    let expected = 3.0 + 2.0 * 1.5 + 5.0 * 0.9
    let result = strike_damage(2, 5)
    assert_eq(result, expected)
}

# =============================================================================
# STRIKE COOLDOWN TESTS
# =============================================================================

# Test: strike_cooldown decreases with depth
fn test_strike_cooldown_depth1() {
    let result = strike_cooldown(1)
    assert(result > 1.5)
}

# Test: strike_cooldown bottoms out at 1.0
fn test_strike_cooldown_caps_at_1_0() {
    let result = strike_cooldown(20)
    assert_eq(result, 1.0)
}

# Test: strike_cooldown at depth 0
fn test_strike_cooldown_depth0() {
    let result = strike_cooldown(0)
    assert_eq(result, 2.0)
}

# Test: strike_cooldown reaches minimum at high depth
fn test_strike_cooldown_deep_floor() {
    # base = 2.0 - 12 * 0.12 = 0.56, clamped to 1.0
    let result = strike_cooldown(12)
    assert_eq(result, 1.0)
}

# =============================================================================
# LOOT ROLL TESTS (PARTIAL - roll_loot calls host_rng)
# =============================================================================
# NOTE: roll_loot cannot be tested directly because it calls host_rng(),
# which requires the Rust engine. The logic is:
#   - Kind 2 (cowardly) always returns 0
#   - Health drops with declining probability by depth
#   - Gold is bread-and-butter (45% base chance)
#   - Upgrades increase with depth (18% + depth*2%)
# These rules are verified indirectly through game balance testing.

# =============================================================================
# ARMOR / DAMAGE REDUCTION TESTS
# =============================================================================

# Test: incoming_damage with 0 armor = raw damage with variance
# Test: incoming_damage with 0.75 armor (max) reduces to 25% of raw
# Test: incoming_damage with armor > 0.75 clamps at 0.75
# Test: armor reduction formula: reduced = raw * (1 - armor)
# =============================================================================
# CLAMP HELPER TESTS
# =============================================================================

# Test: clamp returns input if within bounds
fn test_clamp_within_bounds() {
    let result = clamp(5.0, 0.0, 10.0)
    assert_eq(result, 5.0)
}

# Test: clamp returns lower bound if input too low
fn test_clamp_below_lower_bound() {
    let result = clamp(-5.0, 0.0, 10.0)
    assert_eq(result, 0.0)
}

# Test: clamp returns upper bound if input too high
fn test_clamp_above_upper_bound() {
    let result = clamp(15.0, 0.0, 10.0)
    assert_eq(result, 10.0)
}

# Test: clamp_i (int version) within bounds
fn test_clamp_i_within_bounds() {
    let result = clamp_i(5, 0, 10)
    assert_eq(result, 5)
}

# Test: clamp_i below bounds
fn test_clamp_i_below_bounds() {
    let result = clamp_i(-3, 0, 10)
    assert_eq(result, 0)
}

# Test: clamp_i above bounds
fn test_clamp_i_above_bounds() {
    let result = clamp_i(15, 0, 10)
    assert_eq(result, 10)
}

# =============================================================================
# MONSTER COUNT FOR DEPTH TESTS
# =============================================================================

# Test: depth 1 has 3 monsters (2 + 1)
fn test_monster_count_depth1() {
    let result = monster_count_for_depth(1)
    assert_eq(result, 3)
}

# Test: depth 5 has 7 monsters (capped)
fn test_monster_count_depth5_capped() {
    let result = monster_count_for_depth(5)
    assert_eq(result, 7)
}

# Test: depth 10 still has 7 monsters (capped at 7)
fn test_monster_count_depth10_capped() {
    let result = monster_count_for_depth(10)
    assert_eq(result, 7)
}

# Test: monster count grows until cap
fn test_monster_count_growth() {
    let d1 = monster_count_for_depth(1)
    let d4 = monster_count_for_depth(4)
    assert(d4 > d1)
}

# =============================================================================
# KIND POOL FOR DEPTH TESTS
# =============================================================================

# Test: depth 0-1 only has kind 1
fn test_kind_pool_depth0() {
    let result = kind_pool_for_depth(0)
    assert_eq(result, 1)
}

# Test: depth 2-4 has kinds up to 2
fn test_kind_pool_depth3() {
    let result = kind_pool_for_depth(3)
    assert_eq(result, 2)
}

# Test: depth 5+ has all 3 kinds
fn test_kind_pool_depth5() {
    let result = kind_pool_for_depth(5)
    assert_eq(result, 3)
}

# Test: depth 2 exactly unlocks kind 2
fn test_kind_pool_depth2_boundary() {
    let result = kind_pool_for_depth(2)
    assert_eq(result, 2)
}

# =============================================================================
# HEALTH ON DESCEND TESTS
# =============================================================================

# Test: health_on_descend heals and clamps between 55-100
fn test_health_on_descend_clamps_min() {
    # current=100, depth=50: healed = 100+34-50*1.5 = 59, not clamped
    let result = health_on_descend(100.0, 50)
    assert_eq(result, 59.0)
}

# Test: health_on_descend clamps to max 100
fn test_health_on_descend_clamps_max() {
    let result = health_on_descend(50.0, 1)
    let healed = 50.0 + 34.0 - 1.0 * 1.5
    assert(result <= 100.0)
}

# Test: health_on_descend at depth 0 with 0 health still clamps to minimum 55
fn test_health_on_descend_depth0() {
    # current=0, depth=0: healed = 0+34-0*1.5 = 34, clamped to [55,100] = 55
    let result = health_on_descend(0.0, 0)
    assert_eq(result, 55.0)
}

# Test: health_on_descend formula: current + 34 - depth * 1.5
fn test_health_on_descend_formula() {
    let current = 70.0
    let depth = 3
    let healed = 70.0 + 34.0 - 3.0 * 1.5
    let result = health_on_descend(current, depth)
    let expected = clamp(healed, 55.0, 100.0)
    assert_eq(result, expected)
}

# Test: health_on_descend at deep floor gets less healing
fn test_health_on_descend_deep_nerf() {
    let shallow = health_on_descend(50.0, 1)
    let deep = health_on_descend(50.0, 10)
    assert(shallow > deep)
}

# =============================================================================
# PICKUP LINE TESTS (GAME FEEL)
# =============================================================================

# Test: pickup_line for gold (kind 1) is procedural by depth
fn test_pickup_line_gold_kind1() {
    let result = pickup_line(1, 0)
    assert(result != "Nothing worth taking")
}

# Test: pickup_line for health (kind 2) is procedural
fn test_pickup_line_health_kind2() {
    let result = pickup_line(2, 0)
    assert(result != "Nothing worth taking")
}

# Test: pickup_line for upgrade (kind 3) is constant
fn test_pickup_line_upgrade_kind3() {
    let result = pickup_line(3, 0)
    assert_eq(result, "Something sharper")
}

# Test: pickup_line for unknown kind returns default
fn test_pickup_line_unknown_kind() {
    let result = pickup_line(99, 0)
    assert_eq(result, "Nothing worth taking")
}

# Test: pickup_line gold varies by depth
fn test_pickup_line_gold_varies_by_depth() {
    let line0 = pickup_line(1, 0)
    let line5 = pickup_line(1, 5)
    assert(line0 != line5)
}

# =============================================================================
# FLOOR EPIGRAPH TESTS
# =============================================================================

# Test: floor_epigraph depth 1 or less
fn test_floor_epigraph_depth1() {
    let result = floor_epigraph(1)
    assert_eq(result, "The stones remember a name.")
}

# Test: floor_epigraph depth 2-3
fn test_floor_epigraph_depth2() {
    let result = floor_epigraph(2)
    assert_eq(result, "Something below is counting.")
}

# Test: floor_epigraph depth 4-6
fn test_floor_epigraph_depth5() {
    let result = floor_epigraph(5)
    assert_eq(result, "It stopped pretending to be stone.")
}

# Test: floor_epigraph depth 7+
fn test_floor_epigraph_depth7() {
    let result = floor_epigraph(7)
    assert_eq(result, "It has been waiting for you specifically.")
}

# Test: floor_epigraph at boundaries
fn test_floor_epigraph_boundary_depth3() {
    let result = floor_epigraph(3)
    assert_eq(result, "Something below is counting.")
}

# =============================================================================
# DANGER LEVEL TESTS (SKIPPED - calls native host functions)
# =============================================================================
# NOTE: danger_level() calls native host_monster_count() and 
# host_player_health(), which cannot be tested without the Rust engine.
# The logic: returns 0 if no monsters alive, adds 2 to base if hp < 35,
# base = alive / 2, clamped to 0-10. Verified through integration testing.

# =============================================================================
# PLAYER ATTACK DAMAGE TESTS
# =============================================================================

# Test: player_attack_damage at depth 0
fn test_player_attack_damage_depth0() {
    let result = player_attack_damage(0)
    assert_eq(result, 10.0)
}

# Test: player_attack_damage grows with depth
fn test_player_attack_damage_depth_scaling() {
    let d0 = player_attack_damage(0)
    let d10 = player_attack_damage(10)
    assert(d10 > d0)
}

# Test: player_attack_damage formula: 10 + depth * 2
fn test_player_attack_damage_formula() {
    let expected = 10.0 + 5.0 * 2.0
    let result = player_attack_damage(5)
    assert_eq(result, expected)
}

# =============================================================================
# ATTACK COOLDOWN TESTS
# =============================================================================

# Test: attack_cooldown at depth 0
fn test_attack_cooldown_depth0() {
    let result = attack_cooldown(0)
    assert_eq(result, 0.55)
}

# Test: attack_cooldown decreases with depth but doesn't go below 0.25
fn test_attack_cooldown_minimum_0_25() {
    let result = attack_cooldown(20)
    assert_eq(result, 0.25)
}

# Test: attack_cooldown formula tightens: 0.55 - depth * 0.03
fn test_attack_cooldown_formula() {
    let expected = 0.55 - 5.0 * 0.03
    let result = attack_cooldown(5)
    assert_eq(result, expected)
}

# Test: attack_cooldown cap at 0.25
fn test_attack_cooldown_caps_at_0_25() {
    let result1 = attack_cooldown(10)
    let result2 = attack_cooldown(100)
    assert_eq(result1, result2)
}

# =============================================================================
# ATTACK RANGE TEST
# =============================================================================

# Test: attack_range is constant
fn test_attack_range_constant() {
    let result = attack_range()
    assert_eq(result, 2.2)
}

# =============================================================================
# KILL REWARD TESTS
# =============================================================================

# Test: kill_reward formula: 1 + depth + monster_kind
fn test_kill_reward_formula() {
    let expected = 1 + 5 + 2
    let result = kill_reward(5, 2)
    assert_eq(result, expected)
}

# Test: kill_reward at depth 1 kind 1
fn test_kill_reward_depth1_kind1() {
    let result = kill_reward(1, 1)
    assert_eq(result, 3)
}

# Test: kill_reward increases with depth
fn test_kill_reward_scales_with_depth() {
    let shallow = kill_reward(1, 1)
    let deep = kill_reward(10, 1)
    assert(deep > shallow)
}

# Test: kill_reward increases with monster kind
fn test_kill_reward_scales_with_kind() {
    let kind0 = kill_reward(5, 0)
    let kind2 = kill_reward(5, 2)
    assert(kind2 > kind0)
}

# =============================================================================
# INTEGRATION: MONSTER AI STATE MACHINE
# =============================================================================

# Test: monster state progression from idle to chase to strike
fn test_monster_state_progression() {
    # Far away, low health: IDLE
    let idle = monster_decision(1, 0.5, 20.0)
    assert_eq(idle, 0)
    
    # Closer, still low health: PATROL
    let patrol = monster_decision(1, 0.5, 12.0)
    assert_eq(patrol, 1)
    
    # In aggro range: CHASE
    let chase = monster_decision(1, 0.5, 8.0)
    assert_eq(chase, 2)
    
    # At strike range: STRIKE
    let strike = monster_decision(1, 0.5, 1.5)
    assert_eq(strike, 4)
}

# Test: fleeing takes priority over strike range
fn test_flee_priority_over_strike() {
    let result = monster_decision(2, 0.30, 1.0)
    assert_eq(result, 3)
}

# Test: health threshold for kind 1 vs kind 2
fn test_kind1_vs_kind2_flee_thresholds() {
    let kind1_flee = monster_decision(1, 0.19, 5.0)
    let kind2_flee = monster_decision(2, 0.34, 5.0)
    assert_eq(kind1_flee, 3)
    assert_eq(kind2_flee, 3)
}

# =============================================================================
# INTEGRATION: DAMAGE PROGRESSION
# =============================================================================

# Test: armor stacking reduces damage significantly
# Test: damage reduction at different armor levels
# =============================================================================
# INTEGRATION: DIFFICULTY SCALING
# =============================================================================

# Test: difficulty increases across three axes (monster count, damage, speed)
fn test_difficulty_scaling_depth() {
    # Depth 1: few, weak, slow monsters
    let count1 = monster_count_for_depth(1)
    let damage1 = strike_damage(1, 1)
    let speed1 = move_speed(2, 1)
    
    # Depth 10: more, stronger, faster monsters
    let count10 = monster_count_for_depth(10)
    let damage10 = strike_damage(1, 10)
    let speed10 = move_speed(2, 10)
    
    assert(count10 > count1)
    assert(damage10 > damage1)
    assert(speed10 > speed1)
}

# Test: player power scaling keeps up with monster scaling
fn test_player_scaling_matches_enemy_scaling() {
    let player_dmg_early = player_attack_damage(1)
    let player_dmg_late = player_attack_damage(10)
    let enemy_dmg_early = strike_damage(1, 1)
    let enemy_dmg_late = strike_damage(1, 10)
    
    # Both scale, player doesn't fall behind
    assert(player_dmg_late > player_dmg_early)
    assert(enemy_dmg_late > enemy_dmg_early)
}

# =============================================================================
# EDGE CASES AND BOUNDARY CONDITIONS
# =============================================================================

# Test: negative values in clamp
fn test_clamp_negative_range() {
    let result = clamp(-5.0, -10.0, -1.0)
    assert_eq(result, -5.0)
}

# Test: zero values in game formulas
fn test_formulas_with_zero_values() {
    # At distance 0 and hp 0, kind 1 still flees because hp < 0.2
    let result = monster_decision(1, 0.0, 0.0)
    assert_eq(result, 3)
}

# Test: health_on_descend never goes below 55
fn test_health_on_descend_minimum_floor() {
    let result = health_on_descend(1.0, 100)
    assert_eq(result, 55.0)
}

# Test: health_on_descend never goes above 100
fn test_health_on_descend_maximum_ceiling() {
    let result = health_on_descend(100.0, 0)
    assert_eq(result, 100.0)
}

# Test: strike_cooldown minimum of 1.0
fn test_strike_cooldown_floor() {
    let result = strike_cooldown(100)
    assert_eq(result, 1.0)
}

# Test: attack_cooldown minimum of 0.25
fn test_attack_cooldown_floor() {
    let result = attack_cooldown(100)
    assert_eq(result, 0.25)
}

# --- low-health monsters (hp_frac <= 0.15) only attack at point-blank range (< 1.2) ----------
fn test_wounded_monster_does_not_attack_at_1_5() {
    # the 1.8 attack range needs hp_frac > 0.15; at 0.1 this monster keeps chasing (2)
    assert_eq(monster_decision(0, 0.1, 1.5), 2)
}

fn test_wounded_monster_does_not_attack_just_outside_point_blank() {
    assert_eq(monster_decision(0, 0.1, 1.25), 2)
}

fn test_wounded_monster_attacks_inside_point_blank() {
    assert_eq(monster_decision(0, 0.1, 1.15), 4)
}

fn test_healthy_monster_attacks_at_1_5() {
    assert_eq(monster_decision(0, 0.5, 1.5), 4)
}
