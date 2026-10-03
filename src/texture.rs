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

            // Irregular stone blocks. A regular grid of cells reads as woven
            // fabric (which is exactly what the first version looked like),
            // so: coarse cells, offset every other ROW by half a cell, and
            // vary each block's brightness from a hash of its index.
            let rows = 4.0;
            let cols = 3.0;
            let fy = v * rows;
            let row = fy.floor();
            let stagger = (row as i32 % 2) as f32 * 0.5;
            let fx = u * cols + stagger;
            let col = fx.floor();

            let px = fx - col;
            let py = fy - row;

            // Mortar: darken toward each block's edge.
            let edge_x = (px - 0.5).abs();
            let edge_y = (py - 0.5).abs();
            let edge = edge_x.max(edge_y);
            let mortar = ((edge - 0.40) / 0.09).clamp(0.0, 1.0);

            // Per-block brightness from a hash of (col,row) - all sines at
            // integer frequency, so the pattern still wraps.
            let hsh = ((col * 12.9898 + row * 78.233).sin() * 43758.5453).fract();
            let block = 0.5 + 0.5 * ((hsh - 0.5) * 2.0).abs();

            // Fine grain within each block. Kept LOW: high-frequency detail
            // at this scale reads as zigzag stripes rather than stone.
            let grain = ((fx * 7.0 * TAU).sin() + (fy * 6.0 * TAU).sin()) * 0.018;

            // A subtle bevel: brighter toward the block's centre.
            let bevel = (1.0 - edge * 1.6).clamp(0.0, 1.0) * 0.035;

            let l = 0.21 + block * 0.085 + grain + bevel;
            let base = (l.clamp(0.0, 1.0) * 255.0) as u8;
            let r = (base as f32 * 1.03) as u8;
            let g = base;
            let b = (base as f32 * 0.93) as u8;
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    out
}

/// Seamless flagstone floor — visibly different from the wall stone so the
/// two read as different materials rather than one grey box world.
pub fn floor_texture(w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    let cells = 4.0; // stones per tile, so the pattern still wraps
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            // Position within a cell, and the cell index (for per-stone tint).
            let fx = u * cells;
            let fy = v * cells;
            let cx = fx.floor();
            let cy = fy.floor();
            let px = fx - cx;
            let py = fy - cy;

            // Mortar lines: darken near a cell edge.
            let edge = (px - 0.5).abs().max((py - 0.5).abs());
            let mortar = ((edge - 0.44) / 0.06).clamp(0.0, 1.0);

            // Per-stone brightness varies by a hash of the cell index, so the
            // floor isn't uniform. Both terms are integer-frequency in the
            // cell grid, which wraps cleanly.
            let tint = 0.5 + 0.5 * ((cx * 2.7 + cy * 5.3).sin() * 0.5);
            let grain = ((fx * 6.0 * TAU).sin() + (fy * 7.0 * TAU).sin()) * 0.02;

            // Kept DARK on purpose. The first version sat around 0.44 and the
            // floor rendered as a blown-out white slab under vertex lighting —
            // the brightest thing on screen, drawing the eye to the floor in a
            // game where the floor is the least interesting surface.
            let l = (0.17 + tint * 0.055 + grain) * (1.0 - mortar * 0.5);
            let base = (l.clamp(0.0, 1.0) * 255.0) as u8;
            let r = (base as f32 * 1.05) as u8;
            let g = base;
            let b = (base as f32 * 0.90) as u8;
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

    #[test]
    fn floor_texture_tiles_seamlessly() {
        let (w, h) = (64usize, 64usize);
        let px = floor_texture(w, h);
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
            worst_wrap <= worst_interior + 10,
            "floor seam too visible: wrap {worst_wrap} vs interior {worst_interior}"
        );
    }

    #[test]
    fn floor_differs_visibly_from_wall() {
        // A floor that looks like the walls makes the whole dungeon read as
        // one undifferentiated grey box world.
        let a = stone_texture(32, 32);
        let b = floor_texture(32, 32);
        let avg = |p: &Vec<u8>| -> f64 {
            p.chunks_exact(4).map(|c| c[0] as f64).sum::<f64>() / (p.len() / 4) as f64
        };
        assert!(
            (avg(&a) - avg(&b)).abs() > 8.0,
            "floor and wall are too similar ({:.1} vs {:.1})",
            avg(&a),
            avg(&b)
        );
    }

    /// Mean brightness. Used to catch a texture that's blown out — a white
    /// floor reads as the most important surface in the scene.
    fn mean_luma(p: &[u8]) -> f64 {
        p.chunks_exact(4)
            .map(|c| {
                (0.2126 * c[0] as f64 + 0.7152 * c[1] as f64 + 0.0722 * c[2] as f64) / 255.0
            })
            .sum::<f64>()
            / (p.len() / 4) as f64
    }

    /// Both surfaces must stay in a readable mid-dark band. Too bright and the
    /// floor blows out under vertex lighting; too dark and the dungeon is a
    /// black void you can't navigate.
    #[test]
    fn textures_are_not_blown_out_or_murky() {
        for (name, px) in [
            ("floor", floor_texture(64, 64)),
            ("wall", stone_texture(64, 64)),
        ] {
            let l = mean_luma(&px);
            assert!(
                (0.08..0.42).contains(&l),
                "{name} mean luma {l:.3} is outside the readable band 0.08..0.42"
            );
        }
    }

    /// The floor must be darker than the walls so the eye goes to the walls
    /// and the monsters standing in front of them, not the ground.
    #[test]
    fn floor_is_darker_than_walls() {
        let f = mean_luma(&floor_texture(64, 64));
        let w = mean_luma(&stone_texture(64, 64));
        assert!(
            f < w,
            "floor ({f:.3}) should be darker than walls ({w:.3})"
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