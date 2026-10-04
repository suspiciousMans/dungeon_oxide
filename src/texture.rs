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

/// The stone pattern sampled at continuous `u`/`v` (repeating beyond 1.0).
///
/// Split out from `stone_texture` so the periodicity test can evaluate it at
/// `u` and `u + 1` directly. Growing the output buffer does NOT work:
/// `stone_texture(2w, h)` divides by `2w`, so column `w` samples u = 0.5 — a
/// half period, not a full one.
pub fn cell_pattern(u: f32, v: f32) -> u8 {
    // Blocks per tile. Both MUST be integers: a fractional cell count makes
    // the pattern non-periodic, and an ODD row count flips the stagger's
    // parity across the vertical wrap.
    const COLS: f32 = 4.0;
    const ROWS: f32 = 4.0;

    // Fold into one period FIRST. Everything downstream then works on
    // cell-local coordinates, so a texel and its counterpart one tile over
    // produce byte-identical results.
    let fu = (u * COLS).rem_euclid(COLS);
    let fv = (v * ROWS).rem_euclid(ROWS);

    // Offset alternate rows by half a block — the thing that stops the wall
    // reading as woven fabric. Even ROWS keeps the parity stable across the
    // vertical wrap.
    let row = fv.floor();
    let fu = fu + (row as i32 % 2) as f32 * 0.5;

    let cu = fu.floor();
    let cv = fv.floor();
    let px = fu - cu; // 0..1 within the block
    let py = fv - cv;

    // Mortar: distance to the nearest block EDGE, so all four sides get the
    // same gap. Distance to the block CENTRE makes the top and bottom edges
    // differ by a whole mortar band, which reads as a bright seam line.
    let edge = (px - 0.5).abs().max((py - 0.5).abs());
    let mortar = ((edge - 0.42) / 0.08).clamp(0.0, 1.0);

    // Per-block brightness from the (folded) cell index.
    let hsh = ((cu * 12.9898 + cv * 78.233).sin() * 43758.5453).fract();
    let block = 0.5 + 0.5 * ((hsh - 0.5) * 2.0).abs();

    // Grain keyed off block-local px/py, which restart per block. Kept LOW:
    // at higher frequencies it reads as zigzag stripes rather than stone.
    let grain = ((px * 7.0 * TAU).sin() + (py * 6.0 * TAU).sin()) * 0.018;

    // Gentle bevel toward each block's centre.
    let bevel = (1.0 - edge * 1.6).clamp(0.0, 1.0) * 0.035;

    // Green channel: the other two are derived from it by fixed ratios in
    // `stone_texture`, so this returns the single channel value and the
    // buffer stays the only place the RGB split lives.
    ((0.21 + block * 0.085 + grain + bevel) * (1.0 - mortar * 0.45)
        * 255.0)
        .clamp(0.0, 255.0) as u8
}

/// The stone wall texture, sampling `cell_pattern` across the tile.
pub fn stone_texture(w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    // Sample at the texel CENTRE. Dividing x by w as f32 and then re-dividing
    // in the test loses the last bit of precision, which showed up as a
    // 1-value disagreement between the buffer and the sampler. Computing the
    // coordinate once, the way the texture generator does, removes the
    // discrepancy entirely.
    let xs: Vec<f32> = (0..w).map(|x| (x as f32 + 0.5) / w as f32).collect();
    let ys: Vec<f32> = (0..h).map(|y| (y as f32 + 0.5) / h as f32).collect();
    for y in 0..h {
        for x in 0..w {
            let g = cell_pattern(xs[x], ys[y]);
            let r = ((g as f32) * 1.03).min(255.0) as u8;
            let b = ((g as f32) * 0.93) as u8;
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


/// Seamless flesh/bone texture for monsters. Deliberately warm and organic
/// against the cold grey stone, so an enemy reads instantly as "not scenery"
/// even at the edge of the fog.
pub fn creature_texture(w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            // Mottled blotches — sine products at integer frequency, so it wraps.
            let mottle = (u * 5.0 * TAU).sin() * (v * 4.0 * TAU).sin()
                + (u * 11.0 * TAU).sin() * (v * 9.0 * TAU).sin();
            let fine = ((u * 17.0 * TAU).sin() + (v * 15.0 * TAU).sin()) * 0.5;
            // Pale, desaturated: near the readable band's top but not over it.
            let l = (0.30 + mottle * 0.05 + fine * 0.03).clamp(0.0, 1.0);
            let base = (l * 255.0) as u8;
            // Sickly green-grey with a warm undertone.
            let r = (base as f32 * 0.92).min(255.0) as u8;
            let g = base;
            let b = (base as f32 * 0.84).min(255.0) as u8;
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    out
}

/// Seamless texture for loot gems — cool and bright so a pickup pops against
/// both the dark floor and the grey walls.
pub fn gem_texture(w: usize, h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32;
            let v = y as f32 / h as f32;
            let facets = (u * 4.0 * TAU).sin() * (v * 4.0 * TAU).sin();
            let l = (0.42 + facets * 0.10).clamp(0.0, 1.0);
            let base = (l * 255.0) as u8;
            // Pale gold.
            let r = base;
            let g = (base as f32 * 0.92).min(255.0) as u8;
            let b = (base as f32 * 0.62).min(255.0) as u8;
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
    /// The wall texture must be *periodic*: column 0 identical to the last
    /// column, row 0 identical to the last row. That is the actual property
    /// tiling needs.
    ///
    /// An earlier version compared the wrap delta against the worst INTERIOR
    /// delta and demanded the wrap be no larger. That test is wrong for any
    /// texture with a hard feature: a mortar line is a legitimate
    /// high-contrast edge, so the tile boundary legitimately shows one, and
    /// the test rejected a perfectly good texture. Equality is the real
    /// requirement, and it's strictly stronger than "looks close enough".
    /// The wall texture must REPEAT exactly.
    ///
    /// Two earlier versions of this test were wrong in instructive ways:
    ///
    /// 1. Comparing the wrap delta against the worst INTERIOR delta. Wrong:
    ///    a mortar line is a legitimate high-contrast feature, so the tile
    ///    boundary legitimately shows one, and the test rejected a texture
    ///    that was in fact seamless.
    /// 2. Comparing column 0 to column `w` of a double-width texture. Also
    ///    wrong: with `cols = 4` cells across the tile, one full period is
    ///    `w / 4` texels, so column `w` is a half-period away — a legitimate
    ///    position, not a repeat.
    ///
    /// The real property is exact equality one PERIOD apart. Derive the
    /// period from the cell count rather than assuming it equals the width.
    /// The wall texture must REPEAT exactly, and the repeat distance is the
    /// FULL tile width — the half-cell stagger means no shorter horizontal
    /// period exists (verified by search: rows 0-3 all have their smallest
    /// period at 64 texels).
    ///
    /// Three wrong versions of this test got us here, all worth recording:
    ///
    /// 1. "Wrap delta <= worst interior delta". Wrong: a mortar line is a
    ///    legitimate high-contrast feature, so the tile boundary legitimately
    ///    shows one. This rejected a texture that was already seamless.
    /// 2. "Column 0 == column w of a double-width texture". Wrong twice
    ///    over — column `w` of a `2w` texture samples u=0.5, a half period.
    /// 3. "Column 0 == column w/CELLS". Wrong: the stagger rules out any
    ///    period shorter than the full width.
    ///
    /// So: generate at double width and compare column `x` with column
    /// `x + w`. Both are sampled with the same `w`, so `u` differs by exactly
    /// 1.0 — one period, by construction.
    /// The wall texture must REPEAT exactly every tile width and height.
    ///
    /// The only way to test this without changing the generator: sample the
    /// pattern at u and u+1 directly. Growing the buffer does NOT work —
    /// `stone_texture(2w, h)` divides by `2w`, so column `w` samples u=0.5,
    /// a half period, not a full one. (Three earlier versions of this test
    /// made exactly that mistake, plus a delta-vs-interior comparison that
    /// wrongly rejected seamless textures. All of them were testing the wrong
    /// thing.)
    ///
    /// Exposing the sampler directly is the honest fix, so `cell_pattern`
    /// takes u/v rather than x/y.
    #[test]
    fn wall_texture_repeats_exactly() {
        for y in 0..32 {
            let v = y as f32 / 64.0;
            for x in 0..64 {
                let u = x as f32 / 64.0;
                assert_eq!(
                    cell_pattern(u, v),
                    cell_pattern(u + 1.0, v),
                    "f({u:.4}, {v:.4}) != f(u+1): no horizontal repeat"
                );
                assert_eq!(
                    cell_pattern(u, v),
                    cell_pattern(u, v + 1.0),
                    "f({u:.4}, {v:.4}) != f(v+1): no vertical repeat"
                );
            }
        }
    }

    /// And the buffer the game actually uploads must agree with that sampler,
    /// so the test isn't just proving something the game doesn't do.
    #[test]
    fn wall_texture_buffer_matches_the_sampler() {
        let (w, h) = (64usize, 64usize);
        let px = stone_texture(w, h);
        for y in [0usize, 7, 16, 33, 63] {
            for x in [0usize, 3, 16, 40, 63] {
                let i = (y * w + x) * 4;
                // Channel 1 is GREEN — the one `cell_pattern` returns. Reading
                // index 0 compares the red channel, which is `g * 1.03` and
                // legitimately differs by a unit or two.
                let got = px[i + 1];
                let want = cell_pattern(
                    (x as f32 + 0.5) / w as f32,
                    (y as f32 + 0.5) / h as f32,
                );
                assert_eq!(
                    got, want,
                    "texel ({x},{y}) = {got} but sampler says {want}"
                );
            }
        }
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

    /// Every texture must stay inside the readable luma band — the creature
    /// and gem textures were added after a monster rendered as a white slab.
    #[test]
    fn all_textures_stay_in_the_readable_band() {
        for (name, px) in [
            ("floor", floor_texture(64, 64)),
            ("wall", stone_texture(64, 64)),
            ("creature", creature_texture(64, 64)),
            ("gem", gem_texture(64, 64)),
        ] {
            let l = mean_luma(&px);
            assert!(
                (0.08..0.48).contains(&l),
                "{name} mean luma {l:.3} is outside the readable band"
            );
        }
    }

    #[test]
    fn creature_texture_tiles_seamlessly() {
        let (w, h) = (64usize, 64usize);
        let px = creature_texture(w, h);
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
            "creature seam too visible: wrap {worst_wrap} vs interior {worst_interior}"
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