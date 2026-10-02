//! One seeded RNG for the whole game.
//!
//! Everything random draws from here — level generation AND the loot rolls
//! that happen inside the .ox brain (via `host_rng`). That's deliberate: a
//! single stream means `DUNGEON_SEED=1234` replays the exact same run,
//! including which monsters dropped what.
//!
//! A hash-based integer RNG rather than `rand`'s StdRng: no dependency, no
//! version-specific reproducibility caveats, and it's trivially portable to
//! a `#[test]` that needs the same sequence.

use std::cell::Cell;

thread_local! {
    static STATE: Cell<u64> = const { Cell::new(0x9E3779B97F4A7C15) };
}

/// Reseeds the stream. Called once at startup from the seed (env var or
/// clock).
pub fn seed(s: u64) {
    // Avoid the fixed point at 0, which would make every run identical.
    STATE.with(|c| c.set(s ^ 0x9E3779B97F4A7C15 | 1));
}

/// splitmix64 — one round of it per draw. Fast, passes the usual
/// statistical smoke tests, and (unlike a `Wrapping*` LCG) doesn't have the
/// low-bit structure that shows up as visible grid artifacts in a
/// grid-based dungeon.
fn next_u64() -> u64 {
    STATE.with(|c| {
        let mut z = c.get().wrapping_add(0x9E3779B97F4A7C15);
        c.set(z);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    })
}

/// Uniform in `[0, 1)`.
pub fn next_f64() -> f64 {
    // 53 significant bits, the full mantissa of an f64.
    (next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// Uniform in `[0, n)`. Panics if `n == 0` — a zero-width range is always a
/// caller bug (see the `rng.gen_range` pitfall in the Jame skill).
pub fn below(n: usize) -> usize {
    assert!(n > 0, "rng::below(0) — zero-width range");
    (next_f64() * n as f64) as usize
}

/// Uniform float in `[lo, hi)`.
pub fn range_f32(lo: f32, hi: f32) -> f32 {
    lo + (next_f64() as f32) * (hi - lo)
}

/// Uniform int in `[lo, hi]` inclusive.
pub fn range_i32(lo: i32, hi: i32) -> i32 {
    if hi <= lo {
        return lo;
    }
    lo + below((hi - lo + 1) as usize) as i32
}

/// A random unit-ish direction on the x/z plane, for idle monster drift.
pub fn flat_dir() -> (f32, f32) {
    let a = range_f32(0.0, std::f32::consts::TAU);
    (a.cos(), a.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        seed(42);
        let a: Vec<u64> = (0..8).map(|_| next_u64()).collect();
        seed(42);
        let b: Vec<u64> = (0..8).map(|_| next_u64()).collect();
        assert_eq!(a, b, "same seed must replay the same sequence");
    }

    #[test]
    fn different_seeds_differ() {
        seed(1);
        let a: Vec<u64> = (0..8).map(|_| next_u64()).collect();
        seed(2);
        let b: Vec<u64> = (0..8).map(|_| next_u64()).collect();
        assert_ne!(a, b);
    }

    #[test]
    fn next_f64_in_unit_range() {
        seed(7);
        for _ in 0..10_000 {
            let v = next_f64();
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn distribution_is_not_obviously_biased() {
        // A crude chi-square-ish check: 10 buckets over 100k draws. Catches
        // a broken RNG (all draws in one bucket) without being flaky.
        seed(99);
        let mut buckets = [0usize; 10];
        for _ in 0..100_000 {
            buckets[(next_f64() * 10.0) as usize] += 1;
        }
        let expected = 10_000.0;
        for (i, b) in buckets.iter().enumerate() {
            let dev = (*b as f64 - expected).abs() / expected;
            assert!(dev < 0.05, "bucket {i} deviates {dev:.3} (count {b})");
        }
    }

    #[test]
    fn below_covers_whole_range() {
        seed(3);
        let mut seen = [false; 5];
        for _ in 0..1000 {
            seen[below(5)] = true;
        }
        assert!(seen.iter().all(|s| *s), "below(5) never hit every value");
    }

    #[test]
    fn below_zero_panics() {
        // A zero-width range is a caller bug and must fail loudly.
        let r = std::panic::catch_unwind(|| below(0));
        assert!(r.is_err(), "below(0) should panic");
    }

    #[test]
    fn range_i32_is_inclusive_and_ordered() {
        seed(11);
        for _ in 0..1000 {
            let v = range_i32(3, 5);
            assert!((3..=5).contains(&v), "out of range: {v}");
        }
        assert_eq!(range_i32(5, 5), 5);
        assert_eq!(range_i32(9, 2), 9, "inverted range should return lo");
    }
}