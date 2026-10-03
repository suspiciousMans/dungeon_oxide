//! Procedural dungeon generation — pure data, no GL, no engine types.
//!
//! A floor is a grid of cells, each carved or solid, generated as a
//! sequence of rooms joined by corridors. Because this module has no
//! rendering and no physics in it, a level can be generated and *proven*
//! in a unit test without opening a window — which matters, because the
//! alternative (finding out a floor is unreachable by playing it) is a much
//! slower feedback loop.

use crate::rng;
use std::collections::VecDeque;

/// One solid block to spawn in the world. Axis-aligned by necessity: the
/// engine's colliders are AABB-only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Block {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub sx: f32,
    pub sy: f32,
    pub sz: f32,
}

impl Block {
    pub fn new(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) -> Self {
        Block { x, y, z, sx, sy, sz }
    }
}

#[derive(Debug, Clone)]
pub struct Room {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Room {
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// A generated floor: the walkable grid, the wall blocks, where the player
/// starts, and where the stairs down are.
#[derive(Debug, Clone)]
pub struct Floor {
    pub depth: i32,
    pub width: i32,
    pub height: i32,
    /// `walkable[y][x]` — true where the player can stand.
    pub walkable: Vec<Vec<bool>>,
    pub rooms: Vec<Room>,
    pub walls: Vec<Block>,
    pub floor_blocks: Vec<Block>,
    pub spawn: (i32, i32),
    pub stairs: (i32, i32),
    pub monster_spawns: Vec<(i32, i32)>,
}

pub const CELL: f32 = 3.0;
pub const WALL_HEIGHT: f32 = 3.5;

/// World X for a grid column's center.
pub fn world_x(gx: i32) -> f32 {
    gx as f32 * CELL
}
/// World Z for a grid row's center.
pub fn world_z(gy: i32) -> f32 {
    gy as f32 * CELL
}

/// Generates floor `depth`. Deterministic given the global RNG state.
pub fn generate(depth: i32) -> Floor {
    let (width, height) = (grid_size(depth), grid_size(depth));
    let mut walkable = vec![vec![false; width as usize]; height as usize];
    let mut rooms: Vec<Room> = Vec::new();

    // How many rooms to carve. Grows gently with depth, then plateaus — a
    // huge floor isn't harder, just slower.
    let room_target = 6 + (depth as usize).min(8);

    // Rooms are placed on a jittered lattice rather than by rejection
    // sampling. Rejection sampling fails to fill a floor when rooms are big
    // relative to the grid: on an 11x11 grid, three 6-wide rooms can't
    // coexist without overlap, so every attempt is rejected and you get a
    // one-room floor (where spawn == stairs and there are no monsters).
    // A lattice always fits: rooms never overlap by construction, and the
    // jitter keeps them from looking gridded.
    let cols = 3usize;
    let rows = ((room_target + cols - 1) / cols).max(2);

    let cell_w = (width - 2) / cols as i32;
    let cell_h = (height - 2) / rows as i32;
    let mut placed = 0usize;

    'outer: for cy in 0..rows {
        for cx in 0..cols {
            if placed >= room_target {
                break 'outer;
            }
            // Room fits inside its lattice cell, with a little slack.
            let max_w = (cell_w - 1).max(2);
            let max_h = (cell_h - 1).max(2);
            let w = rng::range_i32(2, max_w.min(5));
            let h = rng::range_i32(2, max_h.min(5));
            if w < 2 || h < 2 {
                continue 'outer;
            }
            // Jitter within the cell so floors don't look gridded, but stay
            // inside the lattice cell so rooms can never overlap.
            let slack_x = (cell_w - w).max(0);
            let slack_y = (cell_h - h).max(0);
            let x = 1 + cx as i32 * cell_w + rng::range_i32(0, slack_x);
            let y = 1 + cy as i32 * cell_h + rng::range_i32(0, slack_y);
            if x + w >= width - 1 || y + h >= height - 1 {
                continue 'outer;
            }

            let room = Room { x, y, w, h };
            if rooms.iter().any(|r| rooms_overlap(r, &room)) {
                continue 'outer;
            }
            for gy in y..y + h {
                for gx in x..x + w {
                    walkable[gy as usize][gx as usize] = true;
                }
            }
            rooms.push(room);
            placed += 1;
        }
    }

    // Connect rooms in sequence with L-shaped corridors. Connecting each room
    // to the *next* (not to the nearest) keeps the whole floor connected as
    // a chain, which is what makes it a dungeon rather than islands.
    for i in 1..rooms.len() {
        let (ax, ay) = rooms[i - 1].center();
        let (bx, by) = rooms[i].center();
        carve_corridor(&mut walkable, ax, ay, bx, by);
    }

    let spawn = rooms
        .first()
        .map(|r| r.center())
        .unwrap_or((width / 2, height / 2));
    let stairs = rooms
        .last()
        .map(|r| r.center())
        .unwrap_or((width / 2, height / 2));

    let mut monster_spawns = Vec::new();
    for room in rooms.iter().skip(1) {
        let (cx, cy) = room.center();
        monster_spawns.push((cx, cy));
    }

    let (walls, floor_blocks) = build_blocks(&walkable, width, height);

    Floor {
        depth,
        width,
        height,
        walkable,
        rooms,
        walls,
        floor_blocks,
        spawn,
        stairs,
        monster_spawns,
    }
}

fn grid_size(depth: i32) -> i32 {
    // Grows to a plateau: past ~14 the fog and the player's torch range make
    // bigger floors unreadable rather than more interesting.
    9 + (depth as i32).min(5)
}

fn rooms_overlap(a: &Room, b: &Room) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// L-shaped corridor: horizontal first, then vertical.
fn carve_corridor(grid: &mut [Vec<bool>], ax: i32, ay: i32, bx: i32, by: i32) {
    let (x0, x1) = (ax.min(bx), ax.max(bx));
    for gx in x0..=x1 {
        grid[ay as usize][gx as usize] = true;
    }
    let (y0, y1) = (ay.min(by), ay.max(by));
    for gy in y0..=y1 {
        grid[gy as usize][bx as usize] = true;
    }
}

fn build_blocks(
    walkable: &[Vec<bool>],
    width: i32,
    height: i32,
) -> (Vec<Block>, Vec<Block>) {
    let mut walls = Vec::new();
    let mut floors = Vec::new();

    // One big floor slab under everything — cheaper than per-cell tiles.
    //
    // It must be centred on the GRID's actual extent, not the origin. Grid
    // cell `i` sits at world position `i * CELL`, so the grid spans
    // `0 .. width*CELL`. Centring on the origin made the slab cover
    // `-15..15` while the cells ran `0..27`, so the floor's edge jutted into
    // view as a grey slab and half the dungeon had no floor under it at all.
    // Cells are centred on `i * CELL`, so the grid's full extent runs from
    // `-CELL/2` to `(width - 1) * CELL + CELL/2` — a half-cell margin on each
    // side. Without it the outermost cells hang over the slab's edge.
    let span_x = CELL * (width as f32 + 1.0);
    let span_z = CELL * (height as f32 + 1.0);
    floors.push(Block::new(span_x / 2.0 - CELL / 2.0, -0.5, span_z / 2.0 - CELL / 2.0, span_x, 1.0, span_z));

    for gy in 0..height {
        for gx in 0..width {
            if !walkable[gy as usize][gx as usize] {
                walls.push(Block::new(world_x(gx), WALL_HEIGHT / 2.0, world_z(gy), CELL, WALL_HEIGHT, CELL));
            }
        }
    }
    (walls, floors)
}

/// Breadth-first flood fill from `start` over walkable cells.
pub fn reachable(floor: &Floor, start: (i32, i32)) -> Vec<(i32, i32)> {
    let mut seen = vec![vec![false; floor.width as usize]; floor.height as usize];
    let mut queue = VecDeque::new();
    if !floor.walkable[start.1 as usize][start.0 as usize] {
        return Vec::new();
    }
    seen[start.1 as usize][start.0 as usize] = true;
    queue.push_back(start);

    let mut out = vec![start];
    while let Some((cx, cy)) = queue.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (cx + dx, cy + dy);
            if nx < 0 || ny < 0 || nx >= floor.width || ny >= floor.height {
                continue;
            }
            if seen[ny as usize][nx as usize] || !floor.walkable[ny as usize][nx as usize] {
                continue;
            }
            seen[ny as usize][nx as usize] = true;
            queue.push_back((nx, ny));
            out.push((nx, ny));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every floor at every depth must be fully connected: if the stairs
    /// aren't reachable from the spawn, the floor is unwinnable.
    #[test]
    fn stairs_are_always_reachable() {
        for depth in 1..=12 {
            rng::seed(1000 + depth as u64);
            let floor = generate(depth);
            let cells = reachable(&floor, floor.spawn);
            assert!(
                cells.contains(&floor.stairs),
                "depth {depth}: stairs {:?} unreachable from spawn {:?}",
                floor.stairs,
                floor.spawn
            );
        }
    }

    /// The generator is driven entirely by the RNG, so it must actually vary
    /// — a constant layout would mean the seed isn't reaching the generator.
    #[test]
    fn layouts_vary_between_seeds() {
        rng::seed(1);
        let a = generate(3);
        rng::seed(2);
        let b = generate(3);
        assert_ne!(a.walkable, b.walkable, "different seeds produced identical floors");
    }

    #[test]
    fn same_seed_same_floor() {
        rng::seed(5);
        let a = generate(4);
        rng::seed(5);
        let b = generate(4);
        assert_eq!(a.walkable, b.walkable);
        assert_eq!(a.spawn, b.spawn);
        assert_eq!(a.stairs, b.stairs);
    }

    /// Spawn and stairs must be on walkable cells, and the player must not
    /// start inside a wall.
    #[test]
    fn spawn_and_stairs_are_walkable() {
        for depth in 1..=10 {
            rng::seed(2000 + depth as u64);
            let floor = generate(depth);
            assert!(floor.walkable[floor.spawn.1 as usize][floor.spawn.0 as usize]);
            assert!(floor.walkable[floor.stairs.1 as usize][floor.stairs.0 as usize]);
            assert_ne!(floor.spawn, floor.stairs, "floor {depth}: spawned on the stairs");
        }
    }

    /// The border must be solid, or the player can walk off the edge of the
    /// world into the void.
    #[test]
    fn border_is_always_solid() {
        for depth in 1..=10 {
            rng::seed(3000 + depth as u64);
            let floor = generate(depth);
            for gx in 0..floor.width {
                assert!(!floor.walkable[0][gx as usize], "top border open at {gx}");
                assert!(
                    !floor.walkable[(floor.height - 1) as usize][gx as usize],
                    "bottom border open at {gx}"
                );
            }
            for gy in 0..floor.height {
                assert!(!floor.walkable[gy as usize][0], "left border open at {gy}");
                assert!(
                    !floor.walkable[gy as usize][(floor.width - 1) as usize],
                    "right border open at {gy}"
                );
            }
        }
    }

    /// The floor slab must cover the whole grid. It used to be centred on the
    /// world origin while the grid runs from `0..width*CELL`, so its edge
    /// jutted into view and half the dungeon had no floor.
    #[test]
    fn floor_slab_covers_every_cell() {
        for depth in 1..=10 {
            rng::seed(6000 + depth as u64);
            let floor = generate(depth);
            let slab = floor.floor_blocks[0];
            let min_x = world_x(0) - CELL / 2.0;
            let max_x = world_x(floor.width - 1) + CELL / 2.0;
            let min_z = world_z(0) - CELL / 2.0;
            let max_z = world_z(floor.height - 1) + CELL / 2.0;
            assert!(
                slab.x - slab.sx / 2.0 <= min_x + 1e-3,
                "depth {depth}: floor starts at {} but the grid starts at {min_x}",
                slab.x - slab.sx / 2.0
            );
            assert!(
                slab.x + slab.sx / 2.0 >= max_x - 1e-3,
                "depth {depth}: floor ends at {} but the grid ends at {max_x}",
                slab.x + slab.sx / 2.0
            );
            assert!(
                slab.z - slab.sz / 2.0 <= min_z + 1e-3,
                "depth {depth}: floor starts at {} but the grid starts at {min_z}",
                slab.z - slab.sz / 2.0
            );
            assert!(
                slab.z + slab.sz / 2.0 >= max_z - 1e-3,
                "depth {depth}: floor ends at {} but the grid ends at {max_z}",
                slab.z + slab.sz / 2.0
            );
        }
    }

    /// Every wall block must correspond to a non-walkable cell, and every
    /// non-walkable cell must have a wall — otherwise there are invisible
    /// walls, or holes you can see through.
    #[test]
    fn walls_match_the_grid_exactly() {
        for depth in 1..=10 {
            rng::seed(4000 + depth as u64);
            let floor = generate(depth);
            let expected: usize = (0..floor.height)
                .flat_map(|y| (0..floor.width).map(move |x| (x, y)))
                .filter(|(x, y)| !floor.walkable[*y as usize][*x as usize])
                .count();
            assert_eq!(
                floor.walls.len(),
                expected,
                "depth {depth}: wall count doesn't match non-walkable cells"
            );
        }
    }

    /// Monsters must not spawn on top of the player.
    #[test]
    fn monster_spawns_avoid_the_player_spawn() {
        for depth in 1..=10 {
            rng::seed(5000 + depth as u64);
            let floor = generate(depth);
            for spawn in &floor.monster_spawns {
                assert_ne!(*spawn, floor.spawn, "depth {depth}: monster spawned on the player");
            }
        }
    }
}