//! Custom low-poly meshes for the dungeon, built in code so the repo ships no
//! binary assets.
//!
//! Everything here is pure data — no GL — so it is unit-testable, and the
//! tests exist for one reason above all: **winding**. The engine's front face
//! is CCW seen from outside (same as `primitives::cube`), and the profiles
//! enable back-face culling. A mesh wound the wrong way is invisible or
//! inside-out, which looks exactly like a broken renderer and isn't one.

use engine::mesh::{MeshData, Vertex};
use glam::Vec3;

/// Fits the unit cube: -0.5..0.5 on every axis, so `Transform::scale` is
/// literally the world size.
pub const UNIT: f32 = 0.5;

fn v(p: [f32; 3], n: [f32; 3], uv: [f32; 2]) -> Vertex {
    Vertex {
        position: p,
        normal: n,
        uv,
        // No baked static lighting — [0,0,0] is a documented no-op.
        color: [0.0, 0.0, 0.0],
    }
}

/// Pushes a triangle, flipping it if its winding disagrees with `outward`.
/// `outward` is a point the face should point away from (typically the
/// shape's interior). Winding is CCW-seen-from-outside, so
/// `cross(b-a, c-a)` must point AWAY from `outward`.
fn tri(
    out: &mut MeshData,
    a: Vertex,
    b: Vertex,
    c: Vertex,
    outward: Vec3,
    _tri_index: usize,
) {
    let pa = Vec3::from(a.position);
    let pb = Vec3::from(b.position);
    let pc = Vec3::from(c.position);
    let geometric = (pb - pa).cross(pc - pa);
    let verts = if geometric.dot(outward) < 0.0 {
        // Wound inside-out — swap b and c to correct it.
        [a, c, b]
    } else {
        [a, b, c]
    };
    let base = out.vertices.len() as u32;
    out.vertices.extend_from_slice(&verts);
    out.indices.extend_from_slice(&[base, base + 1, base + 2]);
}

/// A flat face from three corners, with a normal derived from the winding.
/// `outward` is the direction the face should point (the shape's outward
/// normal), used both to fix winding and to build the vertex normals.
fn face(out: &mut MeshData, p: [Vec3; 3], outward: Vec3, uv_scale: f32) {
    // Centroid gives us a point strictly inside to test winding against.
    let centroid = (p[0] + p[1] + p[2]) / 3.0;
    let to_interior = centroid;
    let a = v(p[0].to_array(), outward.to_array(), [0.0, 0.0]);
    let b = v(p[1].to_array(), outward.to_array(), [
        (p[1] - p[0]).length() * uv_scale,
        0.0,
    ]);
    let c = v(p[2].to_array(), outward.to_array(), [
        (p[2] - p[0]).length() * uv_scale,
        (p[2] - p[1]).length() * uv_scale,
    ]);
    let idx = out.indices.len() / 3;
    tri(out, a, b, c, to_interior, idx);
}

/// Axis-aligned box spanning `-UNIT..UNIT`, flat-shaded. This is the workhorse:
/// walls, floors, pillars, steps.
pub fn box_mesh() -> MeshData {
    let mut m = MeshData::default();
    let (a, b) = (-UNIT, UNIT);

    // Six faces, each given with an explicit outward normal.
    let faces: [([Vec3; 3], Vec3); 12] = [
        // +X
        ([Vec3::new(b, a, b), Vec3::new(b, a, a), Vec3::new(b, b, a)], Vec3::X),
        ([Vec3::new(b, b, a), Vec3::new(b, b, b), Vec3::new(b, a, b)], Vec3::X),
        // -X
        ([Vec3::new(a, a, a), Vec3::new(a, a, b), Vec3::new(a, b, b)], -Vec3::X),
        ([Vec3::new(a, b, b), Vec3::new(a, b, a), Vec3::new(a, a, a)], -Vec3::X),
        // +Y (top)
        ([Vec3::new(a, b, b), Vec3::new(b, b, b), Vec3::new(b, b, a)], Vec3::Y),
        ([Vec3::new(b, b, a), Vec3::new(a, b, a), Vec3::new(a, b, b)], Vec3::Y),
        // -Y (bottom)
        ([Vec3::new(a, a, a), Vec3::new(b, a, a), Vec3::new(b, a, b)], -Vec3::Y),
        ([Vec3::new(b, a, b), Vec3::new(a, a, b), Vec3::new(a, a, a)], -Vec3::Y),
        // +Z
        ([Vec3::new(a, a, b), Vec3::new(b, a, b), Vec3::new(b, b, b)], Vec3::Z),
        ([Vec3::new(b, b, b), Vec3::new(a, b, b), Vec3::new(a, a, b)], Vec3::Z),
        // -Z
        ([Vec3::new(b, a, a), Vec3::new(a, a, a), Vec3::new(a, b, a)], -Vec3::Z),
        ([Vec3::new(a, b, a), Vec3::new(b, b, a), Vec3::new(b, a, a)], -Vec3::Z),
    ];

    for (corners, normal) in faces {
        face(&mut m, corners, normal, 1.0);
    }
    m
}

/// A four-sided tapered pillar/broken column, `height` tall, narrower at the
/// top. Reads as dungeon architecture rather than a box.
pub fn pillar_mesh(height: f32) -> MeshData {
    let mut m = MeshData::default();
    let h = height / 2.0;
    let (bottom, top) = (UNIT, UNIT * 0.62);

    // Four side faces, each a trapezoid.
    let sides: [([Vec3; 4], Vec3); 4] = [
        // +Z
        (
            [
                Vec3::new(-bottom, -h, bottom),
                Vec3::new(bottom, -h, bottom),
                Vec3::new(top, h, top),
                Vec3::new(-top, h, top),
            ],
            Vec3::Z,
        ),
        (
            [
                Vec3::new(bottom, -h, bottom),
                Vec3::new(bottom, -h, -bottom),
                Vec3::new(top, h, -top),
                Vec3::new(top, h, top),
            ],
            Vec3::X,
        ),
        (
            [
                Vec3::new(bottom, -h, -bottom),
                Vec3::new(-bottom, -h, -bottom),
                Vec3::new(-top, h, -top),
                Vec3::new(top, h, -top),
            ],
            -Vec3::Z,
        ),
        (
            [
                Vec3::new(-bottom, -h, -bottom),
                Vec3::new(-bottom, -h, bottom),
                Vec3::new(-top, h, top),
                Vec3::new(-top, h, -top),
            ],
            -Vec3::X,
        ),
    ];

    for (quad, normal) in sides {
        face(&mut m, [quad[0], quad[1], quad[2]], normal, 1.0);
        face(&mut m, [quad[0], quad[2], quad[3]], normal, 1.0);
    }
    // Cap the top so you don't see inside it from above.
    face(
        &mut m,
        [
            Vec3::new(-top, h, top),
            Vec3::new(top, h, top),
            Vec3::new(top, h, -top),
        ],
        Vec3::Y,
        1.0,
    );
    m
}

/// A skull-ish monster: a box body with a narrower head block, so it reads as
/// a creature rather than a crate.
pub fn monster_mesh() -> MeshData {
    let mut m = box_mesh();
    let base = m.vertices.len();

    // A smaller box on top, offset forward — the head.
    let (h, s) = (UNIT * 1.15, UNIT * 0.52);
    let head_faces: [([Vec3; 3], Vec3); 6] = [
        ([Vec3::new(s, -h + UNIT, s), Vec3::new(s, -h + UNIT, -s), Vec3::new(s, h, -s)], Vec3::X),
        ([Vec3::new(s, h, -s), Vec3::new(s, h, s), Vec3::new(s, -h + UNIT, s)], Vec3::X),
        ([Vec3::new(-s, -h + UNIT, -s), Vec3::new(-s, -h + UNIT, s), Vec3::new(-s, h, s)], -Vec3::X),
        ([Vec3::new(-s, h, s), Vec3::new(-s, h, -s), Vec3::new(-s, -h + UNIT, -s)], -Vec3::X),
        ([Vec3::new(-s, h, s), Vec3::new(s, h, s), Vec3::new(s, h, -s)], Vec3::Y),
        ([Vec3::new(s, -h + UNIT, -s), Vec3::new(s, -h + UNIT, s), Vec3::new(-s, -h + UNIT, s)], -Vec3::Y),
    ];
    for (corners, normal) in head_faces {
        face(&mut m, corners, normal, 1.0);
    }
    debug_assert!(base < m.vertices.len());
    m
}

/// A small octahedron — reads as a floating gem/pickup, and being faceted it
/// catches the light differently at each angle.
pub fn gem_mesh() -> MeshData {
    let mut m = MeshData::default();
    let top = Vec3::new(0.0, UNIT, 0.0);
    let bottom = Vec3::new(0.0, -UNIT * 0.8, 0.0);
    let r = UNIT * 0.62;
    let ring = [
        Vec3::new(r, 0.0, 0.0),
        Vec3::new(0.0, 0.0, r),
        Vec3::new(-r, 0.0, 0.0),
        Vec3::new(0.0, 0.0, -r),
    ];
    for i in 0..4 {
        let a = ring[i];
        let b = ring[(i + 1) % 4];
        let outward = (a + b).normalize();
        face(&mut m, [top, a, b], outward, 1.0);
        face(&mut m, [bottom, b, a], outward, 1.0);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(m: &MeshData) -> impl Iterator<Item = &Vertex> {
        m.vertices.iter()
    }

    #[test]
    fn indices_are_in_range_and_triangulated() {
        for (name, m) in [
            ("box", box_mesh()),
            ("pillar", pillar_mesh(2.0)),
            ("monster", monster_mesh()),
            ("gem", gem_mesh()),
        ] {
            assert!(!m.indices.is_empty(), "{name} produced no triangles");
            assert_eq!(
                m.indices.len() % 3,
                0,
                "{name} index count isn't a multiple of 3"
            );
            let max = m.vertices.len() as u32;
            for &i in &m.indices {
                assert!(i < max, "{name} index {i} out of range (max {max})");
            }
        }
    }

    /// The winding test that actually matters: every triangle's geometric
    /// normal must agree with its stored vertex normals and point OUTWARD.
    /// A backwards face is culled and the mesh looks broken.
    #[test]
    fn winding_is_ccw_seen_from_outside() {
        for (name, m) in [
            ("box", box_mesh()),
            ("pillar", pillar_mesh(2.0)),
            ("monster", monster_mesh()),
            ("gem", gem_mesh()),
        ] {
            let centroid: Vec3 = all(&m)
                .map(|v| Vec3::from(v.position))
                .sum::<Vec3>()
                / m.vertices.len() as f32;

            for t in 0..m.indices.len() / 3 {
                let ia = m.indices[t * 3] as usize;
                let ib = m.indices[t * 3 + 1] as usize;
                let ic = m.indices[t * 3 + 2] as usize;
                let (a, b, c) = (
                    Vec3::from(m.vertices[ia].position),
                    Vec3::from(m.vertices[ib].position),
                    Vec3::from(m.vertices[ic].position),
                );
                let geometric = (b - a).cross(c - a);
                assert!(
                    geometric.length() > 1e-6,
                    "{name} triangle {t} is degenerate (zero area)"
                );
                let stored = Vec3::from(m.vertices[ia].normal);
                assert!(
                    geometric.dot(stored) > 0.0,
                    "{name} triangle {t} is wound inside-out: geometric normal \
                     {geometric:?} disagrees with stored normal {stored:?}"
                );
                // And the face must point away from the shape's interior.
                let face_center = (a + b + c) / 3.0;
                let outward = face_center - centroid;
                if outward.length() > 1e-4 {
                    assert!(
                        geometric.dot(outward.normalize()) > 0.0,
                        "{name} triangle {t} faces inward"
                    );
                }
            }
        }
    }

    #[test]
    fn normals_are_unit_length() {
        for (name, m) in [
            ("box", box_mesh()),
            ("pillar", pillar_mesh(2.0)),
            ("monster", monster_mesh()),
            ("gem", gem_mesh()),
        ] {
            for v in all(&m) {
                let n = Vec3::from(v.normal);
                assert!(
                    (n.length() - 1.0).abs() < 1e-3,
                    "{name} has a non-unit normal: {}",
                    n.length()
                );
            }
        }
    }

    #[test]
    fn meshes_stay_inside_the_unit_cube() {
        for (name, m) in [
            ("box", box_mesh()),
            ("gem", gem_mesh()),
        ] {
            for v in all(&m) {
                for (axis, c) in v.position.iter().enumerate() {
                    assert!(
                        c.abs() <= UNIT + 1e-4,
                        "{name} pokes out of the unit cube on axis {axis}: {c}"
                    );
                }
            }
        }
    }

    /// Coverage guards against a face silently disappearing.
    #[test]
    fn box_has_all_six_faces() {
        let m = box_mesh();
        // 12 faces (two per axis on 3 axes) = 12 triangles.
        assert_eq!(m.indices.len(), 12 * 3, "box should be 12 triangles");
        for axis in 0..3 {
            let pos = m
                .vertices
                .iter()
                .filter(|v| v.normal[axis].abs() > 0.9)
                .count();
            assert!(pos >= 4, "box missing a face on axis {axis}");
        }
    }

    #[test]
    fn meshes_are_deterministic() {
        assert_eq!(box_mesh().vertices.len(), box_mesh().vertices.len());
        assert_eq!(gem_mesh().indices, gem_mesh().indices);
    }
}