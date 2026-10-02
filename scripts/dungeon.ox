# ============================================================================
# THE CATACOMBS OF RUST — game logic, written in oxidized.
#
# Everything in this file is GAMEPLAY: monster brains, loot tables, damage
# rules, difficulty pacing. The Rust host (src/main.rs) owns the engine —
# rendering, physics, input — and calls into these functions, passing in
# world state and getting decisions back.
#
# The `fn native` declarations below are implemented by the Rust host. The
# host pushes state in; we push decisions out.
# ============================================================================

# --- Host-provided engine functions -----------------------------------------

# Progress queries: what does the world look like right now?
fn native host_monster_count() -> Int;
fn native host_monster_alive(i: Int) -> Bool;
fn native host_monster_health(i: Int) -> Float;
fn native host_monster_distance(i: Int) -> Float;
fn native host_player_health() -> Float;
fn native host_room_number() -> Int;
fn native host_depth() -> Int;
fn native host_time() -> Float;

# Actions: tell the engine what to do.
fn native host_damage_monster(i: Int, amount: Float);
fn native host_damage_player(amount: Float);
fn native host_move_monster(i: Int, x: Float, z: Float);
fn native host_spawn_loot(x: Float, z: Float, kind: Int);
fn native host_play_tone(freq: Float);
fn native host_toast(text: String);
fn native host_rng() -> Float;

# ============================================================================
# MONSTER AI
# ============================================================================

# What a monster should do this frame. Encoded as an Int so the host can
# switch on it cheaply — this is called once per monster per frame.
#
#   0 = IDLE    wander near spawn
#   1 = PATROL  drift toward the player slowly
#   2 = CHASE   close in fast
#   3 = FLEE    run away, low health
#   4 = STRIKE  in range, attack

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

# How fast to move, per decision. Speed is multiplied by depth so the
# dungeon gets meaner.
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

# How much damage a strike deals, before the host applies armor.
fn strike_damage(kind: Int, depth: Int) -> Float {
    # Deliberately gentle: there is no player attack yet, so a monster that
    # lands 12 damage every 1.2s kills you in ~10s no matter how well you
    # play. Damage has to ramp instead of starting brutal.
    let base = 3.0 + kind as Float * 2.0 + depth as Float * 1.2
    return base
}

# How long a monster waits between strikes. Shared so the pacing rule stays
# in one place — the host just applies the cooldown it is told.
fn strike_cooldown(depth: Int) -> Float {
    let base = 2.0 - depth as Float * 0.12
    if base < 1.0 {
        return 1.0
    }
    return base
}

# ============================================================================
# LOOT
# ============================================================================

# What drops. Returns a loot kind:
#   0 = nothing, 1 = gold, 2 = health, 3 = weapon upgrade
#
# Rarer drops are gated behind depth so early floors stay readable.
fn roll_loot(depth: Int, monster_kind: Int) -> Int {
    let r = host_rng()

    # Nothing drops from the cowardly ones — makes them worth fleeing.
    if monster_kind == 2 {
        return 0
    }

    # Health is common early and rare later (you need it early).
    let health_chance = 0.4 - depth as Float * 0.03
    if health_chance > 0.0 && r < health_chance {
        return 2
    }

    # Gold is the bread and butter.
    if r < health_chance + 0.45 {
        return 1
    }

    # Upgrades are the reward for going deep.
    let upgrade_chance = 0.05 + depth as Float * 0.02
    if r < health_chance + 0.45 + upgrade_chance {
        return 3
    }
    return 0
}

# ============================================================================
# DAMAGE / SURVIVAL RULES
# ============================================================================

# The player's actual incoming damage: monster damage, reduced by armor,
# with a small random variance so no two hits feel identical.
fn incoming_damage(raw: Float, armor: Float) -> Float {
    # Armor caps at 0.75 so nothing is ever fully invulnerable.
    let reduced = raw * (1.0 - clamp(armor, 0.0, 0.75))
    let variance = 0.85 + host_rng() * 0.3
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

# ============================================================================
# ROOM / FLOOR GENERATION
# ============================================================================

# How many monsters a floor holds. Rises with depth but flattens so late
# floors are tense rather than impossible.
fn monster_count_for_depth(depth: Int) -> Int {
    # Depth 1 has few enough monsters to learn on, then it climbs. Capped so
    # a late floor is tense rather than a wall of bodies.
    let n = 2 + depth
    if n > 9 {
        return 9
    }
    return n
}

# Which monster kinds appear at a depth. Kinds unlock progressively, so
# floor 1 teaches you one thing at a time.
fn kind_pool_for_depth(depth: Int) -> Int {
    if depth < 2 {
        return 1
    }
    if depth < 5 {
        return 2
    }
    return 3
}

# The player's health when descending to a new floor. Partial heal — enough
# to keep going, not enough to make depth free.
fn health_on_descend(current: Float, depth: Int) -> Float {
    let healed = current + 15.0 - depth as Float * 2.0
    return clamp(healed, 1.0, 100.0)
}

# ============================================================================
# GAME FEEL
# ============================================================================

# A short line shown when you pick something up. Procedural, so it doesn't
# repeat — mixes a couple of word lists by index rather than randomly,
# giving variety without needing real randomness here.
# NOTE: each branch uses DISTINCT variable names on purpose. oxidized's
# transpiler leaks scope between sibling `if` blocks, so a re-`let` of the
# same name in a second `if` is emitted as an assignment (`bits = ...`)
# instead of a declaration, which doesn't compile. Sibling `if`/`else` is
# fine — it's two sequential `if`s that break.
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

# The dungeon's mood, shown on the floor banner. Deeper is worse.
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

# Danger rating 0-10, for the HUD. Drives the vignette and the music.
fn danger_level() -> Int {
    let alive = host_monster_count()
    let hp = host_player_health()
    if alive == 0 {
        return 0
    }
    # Low health reads as dangerous even when nothing is close.
    let hurt = 0
    if hp < 35.0 {
        hurt = 2
    }
    let base = alive / 2
    return clamp_i(base + hurt, 0, 10)
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