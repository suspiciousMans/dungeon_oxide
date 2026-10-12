import "dungeon.ox"

mock fn host_rng() -> Float {
    return 0.5
}

# This test should FAIL because it has wrong expectation
fn test_wrong_assertion() {
    let kind = roll_loot(1, 1)
    assert(kind == 2)  # Wrong: should be 1, not 2
}
