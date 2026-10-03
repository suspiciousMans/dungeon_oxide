//! Procedural textures. Pure functions producing RGBA8, so they're testable
//! without a GL context — which is the only practical way to catch a seam
//! before it's on screen.

/// Seamless stone-ish texture.
///
/// Every term is a periodic function of `x`/`y` across the tile, so the
/// pattern wraps with no visible edge. A texture built from non-periodic
/// noise (or from a domain warp that isn't itself periodic) leaves a bright
/// line where the tile repeats, which is exactly what the test below
/// catches.
const TAU: f32 = std::f32::consts::TAU;

pub fn stone_texture(w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            // Integer-CYCLE sines are periodic over the tile — and "integer
            // cycle" means a multiple of TAU, not a plain integer. `sin(u * 8.0)`
            // completes only ~1.3 cycles across the tile and leaves a hard
            // seam (the seam test caught exactly that: wrap delta 66 vs
            // interior 28).
            let n = (u * 8.0 * TAU).sin() * (v * 6.0 * TAU).sin()
                + (u * 13.0 * TAU).sin() * (v * 11.0 * TAU).sin();
            let grain = ((u * 40.0 * TAU).sin() + (v * 37.0 * TAU).sin()) * 0.15;
            let l = 0.42 + n * 0.06 + grain;
            let base = (l.clamp(0.0, 1.0) * 255.0) as u8;
            let r = (base as f32 * 0.98) as u8;
            let g = base;
            let b = (base as f32 * 1.06).min(255.0) as u8;
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiling texture must not have a visible seam. Compare the
    /// wrap-around neighbour delta (last column/row -> first) against the
    /// worst interior neighbour delta; if the wrap delta is materially
    /// larger, the pattern doesn't tile.
    #[test]
    fn generated_texture_tiles_seamlessly() {
        let (w, h) = (64usize, 64usize);
        let px = stone_texture(w, h);
        assert_eq!(px.len(), w * h * 4, "texture must be RGBA8");

        let delta = |a: u8, b: u8| (a as i32 - b as i32).abs();
        let at = |x: usize, y: usize| {
            let i = (y % h * w + x % w) * 4;
            px[i]
        };

        let mut worst_wrap = 0i32;
        let mut worst_interior = 0i32;
        for y in 0..h {
            for x in 0..w {
                worst_wrap = worst_wrap.max(delta(at(x, y), at(0, y)));
                worst_wrap = worst_wrap.max(delta(at(x, y), at(x, 0)));
                if x + 1 < w {
                    worst_interior = worst_interior.max(delta(at(x, y), at(x + 1, y)));
                }
                if y + 1 < h {
                    worst_interior = worst_interior.max(delta(at(x, y), at(x, y + 1)));
                }
            }
        }
        assert!(
            worst_wrap <= worst_interior + 8,
            "seam too visible: wrap delta {worst_wrap} vs interior {worst_interior}"
        );
    }

    /// Alpha must be opaque everywhere — the engine's blend state treats a
    /// transparent texel as a hole in the wall.
    #[test]
    fn texture_is_fully_opaque() {
        let (w, h) = (16usize, 16usize);
        let px = stone_texture(w, h);
        for (i, chunk) in px.chunks_exact(4).enumerate() {
            assert_eq!(chunk[3], 255, "pixel {i} has alpha {}", chunk[3]);
        }
    }

    /// Deterministic: the same call must produce the same bytes, or a
    /// screenshot comparison is meaningless.
    #[test]
    fn texture_is_deterministic() {
        assert_eq!(stone_texture(8, 8), stone_texture(8, 8));
    }

    /// A flat texture would satisfy the seam test trivially, so prove there's
    /// actual variation.
    #[test]
    fn texture_has_visible_variation() {
        let px = stone_texture(32, 32);
        let first = px[0];
        let max_delta = px
            .chunks_exact(4)
            .map(|c| (c[0] as i32 - first as i32).abs())
            .max()
            .unwrap();
        assert!(max_delta > 12, "texture is nearly flat (max delta {max_delta})");
    }
}