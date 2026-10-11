# ============================================================================
# PURE FUNCTIONS FROM DUNGEON.OX (extracted for testing)
# ============================================================================
# This file contains only the pure logic functions from dungeon.ox that
# don't depend on native functions. It's used by tests to verify game rules.
# ============================================================================

# =============================================================================
# MONSTER AI
# =============================================================================

fn monster_decision(kind: Int, hp_frac: Float, distance: Float) -> Int {
    # Some monsters are cowards by nature, others only when hurt.
    if kind == 2 && hp_frac < 0.35 {
        return 3
    }
    if kind == 1 && hp_frac < 0.2 {
        return 3
    }

    # In range: strike. The window scales with how healthy it is — a
    # wounded monster is slower to commit.
    if distance < 1.8 && hp_frac > 0.15 {
        return 4
    }
    if distance < 1.2 {
        return 4
    }

    # Aggro range grows with the monster's own health: a healthy monster
    # notices you from further away.
    let aggro = 6.0 + hp_frac * 6.0
    if distance < aggro {
        return 2
    }

    # Slowly closing the gap even out of aggro, so nothing is ever static.
    if distance < 14.0 {
        return 1
    }
    return 0
}

fn move_speed(decision: Int, depth: Int) -> Float {
    let ramp = 1.0 + depth as Float * 0.08
    if decision == 2 {
        return 2.6 * ramp
    }
    if decision == 3 {
        return 3.4 * ramp
    }
    if decision == 1 {
        return 1.1 * ramp
    }
    return 0.4 * ramp
}

fn strike_damage(kind: Int, depth: Int) -> Float {
    let base = 3.0 + kind as Float * 1.5 + depth as Float * 0.9
    return base
}

fn strike_cooldown(depth: Int) -> Float {
    let base = 2.0 - depth as Float * 0.12
    if base < 1.0 {
        return 1.0
    }
    return base
}

# =============================================================================
# DAMAGE / SURVIVAL RULES
# =============================================================================

fn incoming_damage(raw: Float, armor: Float) -> Float {
    let reduced = raw * (1.0 - clamp(armor, 0.0, 0.75))
    let variance = 0.85 + 0.5 * 0.3  # Using fixed 0.5 instead of host_rng()
    return reduced * variance
}

fn clamp(v: Float, lo: Float, hi: Float) -> Float {
    if v < lo {
        return lo
    }
    if v > hi {
        return hi
    }
    return v
}

# =============================================================================
# ROOM / FLOOR GENERATION
# =============================================================================

fn monster_count_for_depth(depth: Int) -> Int {
    let n = 2 + depth
    let capped = 7
    if n > capped {
        return capped
    }
    return n
}

fn kind_pool_for_depth(depth: Int) -> Int {
    if depth < 2 {
        return 1
    }
    if depth < 5 {
        return 2
    }
    return 3
}

fn health_on_descend(current: Float, depth: Int) -> Float {
    let healed = current + 34.0 - depth as Float * 1.5
    return clamp(healed, 55.0, 100.0)
}

# =============================================================================
# GAME FEEL
# =============================================================================

fn pickup_line(kind: Int, depth: Int) -> String {
    if kind == 1 {
        let coin_adj = ["Cold", "Bright", "Old", "Worm-eaten", "Gilded", "Tarnished"]
        let coin_noun = ["coin", "coin", "coin", "doubloon", "shilling", "ingot"]
        let ca = coin_adj[(depth + kind) % 6]
        let cn = coin_noun[(depth * 2 + kind) % 6]
        return ca + " " + cn
    }
    if kind == 2 {
        let brew_adj = ["Warm", "Sticky", "Bitter", "Cold", "Sweet", "Wobbly"]
        let brew_noun = ["potion", "phial", "draught", "flask", "tonic", "brew"]
        let ba = brew_adj[(depth * 3) % 6]
        let bn = brew_noun[(depth + 2) % 6]
        return ba + " " + bn
    }
    if kind == 3 {
        return "Something sharper"
    }
    return "Nothing worth taking"
}

fn floor_epigraph(depth: Int) -> String {
    if depth <= 1 {
        return "The stones remember a name."
    }
    if depth <= 3 {
        return "Something below is counting."
    }
    if depth <= 6 {
        return "It stopped pretending to be stone."
    }
    return "It has been waiting for you specifically."
}

fn clamp_i(v: Int, lo: Int, hi: Int) -> Int {
    if v < lo {
        return lo
    }
    if v > hi {
        return hi
    }
    return v
}

# =============================================================================
# PLAYER COMBAT
# =============================================================================

fn player_attack_damage(depth: Int) -> Float {
    let base = 10.0 + depth as Float * 2.0
    return base
}

fn attack_cooldown(depth: Int) -> Float {
    let t = 0.55 - depth as Float * 0.03
    if t < 0.25 {
        return 0.25
    }
    return t
}

fn attack_range() -> Float {
    return 2.2
}

fn kill_reward(depth: Int, monster_kind: Int) -> Int {
    let base = 1 + depth + monster_kind
    return base
}
