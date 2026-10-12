import "dungeon.ox"

# No mock - should panic when it tries to call host_rng
fn test_no_mock_should_panic() {
    let kind = roll_loot(1, 1)
    assert(kind >= 0)
}
