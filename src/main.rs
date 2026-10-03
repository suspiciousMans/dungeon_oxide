//! The Catacombs of Rust — a first-person dungeon crawl.
//!
//! The split between the two languages is deliberate:
//!
//! - This file owns the *engine*: window, rendering, input, physics, the
//!   world, and the player.
//! - `scripts/dungeon.ox` owns the game's *rules*: monster brains, loot
//!   tables, damage, difficulty pacing. It's written in oxidized and
//!   transpiled to Rust by `build.rs` at compile time.
//!
//! The `.ox` side reaches the engine through `fn native` declarations
//! implemented in `natives.rs`.

mod dungeon;
mod natives;

// The generated oxidized code resolves its `fn native` calls through the
// crate-root path `crate::__oxidized_natives`, so re-export it there.
pub use natives::__oxidized_natives;
mod rng;
mod texture;

// Generated from scripts/*.ox by build.rs. Each .ox file becomes a module.
include!(concat!(env!("OUT_DIR"), "/ox_generated.rs"));

use std::path::{Path, PathBuf};
use std::sync::Arc;

use engine::app::{App, Context, Game};
use engine::audio::AudioContext;
use engine::camera::FirstPersonCamera;
use engine::ecs::{Entity, MeshRenderer, Transform, World};
use engine::mesh::{primitives, GpuMesh};
use engine::profile::RenderParams;
use engine::hud::HudState;
use engine::renderer::{PostParams, Renderer};
use engine::shader::ShaderVariantCache;
use engine::texture::{GpuTexture, TextureFilter};
use glam::{Mat4, Vec3};
// glow's Context methods live on this trait; without it in scope every
// `gl.enable(...)`/`gl.clear(...)` call is a method-not-found error.
use glow::HasContext;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;

use engine::ui::{draw_hud, egui, EguiState};

use natives::Bridge;
use texture::stone_texture;

/// Eye offset above the player's feet.
const EYE_HEIGHT: f32 = 1.5;
const WALK_SPEED: f32 = 4.2;
const PLAYER_RADIUS: f32 = 0.45;
const MOUSE_SENSITIVITY: f32 = 0.0025;
/// How often the .ox brain re-runs. Decisions at 10 Hz are plenty and keep
/// the cross-language calls cheap.
const BRAIN_HZ: f32 = 10.0;
/// Below this health fraction the HUD starts flashing.
const LOW_HEALTH_FRAC: f32 = 0.35;

/// Fixed-size point-light arrays declared in mesh.vert.
const MAX_POINT_LIGHTS: usize = 4;

struct Monster {
    entity: Entity,
    kind: i32,
    health: f32,
    max_health: f32,
    position: Vec3,
    home: Vec3,
    decision: i32,
    attack_cooldown: f32,
}

struct Loot {
    entity: Entity,
    kind: i32,
    position: Vec3,
}

/// Uniform locations for the mesh shader. Resolved once per frame; setting
/// only some of them is what renders geometry pure black.
struct MeshUniforms {
    model: Option<glow::UniformLocation>,
    view: Option<glow::UniformLocation>,
    proj: Option<glow::UniformLocation>,
    light_dir: Option<glow::UniformLocation>,
    ambient_color: Option<glow::UniformLocation>,
    lighting_mode: Option<glow::UniformLocation>,
    vertex_snap_amount: Option<glow::UniformLocation>,
    fog_start: Option<glow::UniformLocation>,
    fog_end: Option<glow::UniformLocation>,
    fog_color: Option<glow::UniformLocation>,
    point_light_pos: Vec<Option<glow::UniformLocation>>,
    point_light_color: Vec<Option<glow::UniformLocation>>,
    point_light_intensity: Vec<Option<glow::UniformLocation>>,
    point_light_range: Vec<Option<glow::UniformLocation>>,
    point_light_count: Option<glow::UniformLocation>,
}

impl MeshUniforms {
    fn resolve(gl: &glow::Context, program: glow::Program) -> Self {
        unsafe {
            let loc = |n: &str| gl.get_uniform_location(program, n);
            MeshUniforms {
                model: loc("uModel"),
                view: loc("uView"),
                proj: loc("uProj"),
                light_dir: loc("uLightDir"),
                ambient_color: loc("uAmbientColor"),
                lighting_mode: loc("uLightingMode"),
                vertex_snap_amount: loc("uVertexSnapAmount"),
                fog_start: loc("uFogStart"),
                fog_end: loc("uFogEnd"),
                fog_color: loc("uFogColor"),
                point_light_pos: (0..MAX_POINT_LIGHTS)
                    .map(|i| loc(&format!("uPointLightPos[{i}]")))
                    .collect(),
                point_light_color: (0..MAX_POINT_LIGHTS)
                    .map(|i| loc(&format!("uPointLightColor[{i}]")))
                    .collect(),
                point_light_intensity: (0..MAX_POINT_LIGHTS)
                    .map(|i| loc(&format!("uPointLightIntensity[{i}]")))
                    .collect(),
                point_light_range: (0..MAX_POINT_LIGHTS)
                    .map(|i| loc(&format!("uPointLightRange[{i}]")))
                    .collect(),
                point_light_count: loc("uPointLightCount"),
            }
        }
    }
}

struct DungeonGame {
    world: World,
    renderer: Option<Renderer>,
    shader_cache: Option<ShaderVariantCache>,
    params: RenderParams,
    camera: FirstPersonCamera,

    renderer_gl: Option<Arc<glow::Context>>,
    cube: Option<Arc<GpuMesh>>,
    white: Option<Arc<GpuTexture>>,

    floor: dungeon::Floor,
    depth: i32,
    monsters: Vec<Monster>,
    loot: Vec<Loot>,

    player_entity: Entity,
    player_position: Vec3,
    player_velocity: Vec3,
    player_health: f32,
    player_armor: f32,
    gold: i32,

    time: f32,
    brain_accumulator: f32,
    dead: bool,
    won: bool,
    run_time: f32,

    banner: Option<(String, f32)>,

    autopilot: bool,
    seed: u64,
    last_hud_log: f32,
    ui: Option<EguiState>,
    hud: Option<HudState>,
    last_bite_toast: f32,
    attack_cooldown: f32,
    attack_cd_cache: f32,
    audio: Option<AudioContext>,
    stone: Option<Arc<GpuTexture>>,
}

impl DungeonGame {
    fn new(seed: u64, autopilot: bool) -> Self {
        DungeonGame {
            world: World::new(),
            renderer: None,
            shader_cache: None,
            params: RenderParams::default(),
            camera: FirstPersonCamera::new(),
            renderer_gl: None,
            cube: None,
            white: None,
            floor: dungeon::Floor {
                depth: 0,
                width: 0,
                height: 0,
                walkable: vec![],
                rooms: vec![],
                walls: vec![],
                floor_blocks: vec![],
                spawn: (0, 0),
                stairs: (0, 0),
                monster_spawns: vec![],
            },
            depth: 0,
            monsters: Vec::new(),
            loot: Vec::new(),
            player_entity: Entity::DANGLING,
            player_position: Vec3::ZERO,
            player_velocity: Vec3::ZERO,
            player_health: 100.0,
            player_armor: 0.0,
            gold: 0,
            time: 0.0,
            brain_accumulator: 0.0,
            dead: false,
            won: false,
            run_time: 0.0,
            banner: None,
            autopilot,
            seed,
            last_hud_log: 0.0,
            ui: None,
            hud: None,
            last_bite_toast: -10.0,
            attack_cooldown: 0.0,
            attack_cd_cache: 0.6,
            audio: None,
            stone: None,
        }
    }

    fn toast(&mut self, text: String) {
        log::info!("toast: {text}");
        if let Some(hud) = self.hud.as_mut() {
            hud.show_toast(&text, 3.5);
        }
    }

    /// The player's eye position — the camera sits here every frame.
    fn eye(&self) -> Vec3 {
        self.player_position + Vec3::new(0.0, EYE_HEIGHT, 0.0)
    }

    /// Builds a new floor at `depth`: clears the world, generates geometry,
    /// spawns the player, and asks the .ox side how many monsters to place.
    fn descend(&mut self, depth: i32) -> anyhow::Result<()> {
        rng::seed(self.seed ^ (depth as u64).wrapping_mul(0x9E3779B97F4A7C15));
        let floor = dungeon::generate(depth);

        self.world.clear();
        self.monsters.clear();
        self.loot.clear();

        let cube = Arc::clone(self.cube.as_ref().expect("mesh set up in init"));
        let white = Arc::clone(self.white.as_ref().expect("texture set up in init"));

        // Floor slab.
        for block in &floor.floor_blocks {
            self.world.spawn((
                Transform {
                    position: Vec3::new(block.x, block.y, block.z),
                    rotation: glam::Quat::IDENTITY,
                    scale: Vec3::new(block.sx, block.sy, block.sz),
                },
                MeshRenderer {
                    mesh: Arc::clone(&cube),
                    texture: Some(Arc::clone(&white)),
                },
            ));
        }

        // Walls. Textured, so they read distinctly from the floor slab —
        // the white fallback is what made the whole dungeon one flat grey.
        let stone = Arc::clone(self.stone.as_ref().expect("texture set up in init"));
        for block in &floor.walls {
            self.world.spawn((
                Transform {
                    position: Vec3::new(block.x, block.y, block.z),
                    rotation: glam::Quat::IDENTITY,
                    scale: Vec3::new(block.sx, block.sy, block.sz),
                },
                MeshRenderer {
                    mesh: Arc::clone(&cube),
                    texture: Some(Arc::clone(&stone)),
                },
            ));
        }

        let spawn = Vec3::new(
            dungeon::world_x(floor.spawn.0),
            0.0,
            dungeon::world_z(floor.spawn.1),
        );
        self.player_position = spawn;
        self.player_velocity = Vec3::ZERO;
        // The player entity exists so `render` has a Transform to follow, but
        // it must NOT be visible: a full-size cube at the camera's own position
        // fills the screen with a grey slab. Zero scale makes it inert while
        // keeping the entity (and its position) available.
        self.player_entity = self.world.spawn((
            Transform {
                position: spawn,
                rotation: glam::Quat::IDENTITY,
                scale: Vec3::ZERO,
            },
            MeshRenderer {
                mesh: Arc::clone(&cube),
                texture: Some(Arc::clone(&white)),
            },
        ));

        // Monster count comes from the .ox brain — the pacing rule lives in
        // oxidized, not here.
        // Every .ox call goes through an installed bridge, even the pure
        // ones — a native must never be reachable without one.
        let monster_count = self.ask(|_| ox_modules::dungeon::monster_count_for_depth(depth as i64))
            .max(1) as usize;
        let kind_pool = self
            .ask(|_| ox_modules::dungeon::kind_pool_for_depth(depth as i64))
            .max(1) as usize;

        let spawn_points: Vec<(i32, i32)> = floor.monster_spawns.clone();
        for (i, cell) in spawn_points.iter().take(monster_count).enumerate() {
            let kind = (i % kind_pool.max(1)) as i32;
            let pos = Vec3::new(dungeon::world_x(cell.0), 0.9, dungeon::world_z(cell.1));
            let health = 30.0 + kind as f32 * 15.0 + depth as f32 * 5.0;
            let entity = self.world.spawn((
                // Scale is the world size: the cube primitive spans -0.5..0.5,
                // so Vec3::ONE would render a 2x2x2 block — bigger than the
                // player and, next to the camera, a grey slab across the view.
                Transform {
                    position: pos,
                    rotation: glam::Quat::IDENTITY,
                    scale: Vec3::splat(MONSTER_SIZE),
                },
                MeshRenderer {
                    mesh: Arc::clone(&cube),
                    texture: Some(Arc::clone(&white)),
                },
            ));
            self.monsters.push(Monster {
                entity,
                kind,
                health,
                max_health: health,
                position: pos,
                home: pos,
                decision: 0,
                attack_cooldown: 0.0,
            });
        }

        // Face the stairs at spawn. yaw=0 looks toward -Z, which on most
        // floors is a wall — spawning nose-first into stone is a bad first
        // frame even though the renderer is fine.
        let to_stairs = Vec3::new(
            dungeon::world_x(floor.stairs.0) - spawn.x,
            0.0,
            dungeon::world_z(floor.stairs.1) - spawn.z,
        );
        if to_stairs.length() > 0.01 {
            self.camera.yaw = to_stairs.x.atan2(-to_stairs.z);
        }

        self.floor = floor;
        self.depth = depth;

        // Partial heal on descending, decided by the .ox side.
        let current_health = self.player_health;
        self.player_health = self.ask(|_| {
            ox_modules::dungeon::health_on_descend(current_health as f64, depth as i64) as f32
        });

        let epigraph = self.ask(|_| ox_modules::dungeon::floor_epigraph(depth as i64));
        self.sfx(440.0, 0.3);
        self.banner = Some((epigraph, 4.0));
        log::info!(
            "descended to depth {depth}: {} rooms, {} monsters, spawn {:?} stairs {:?}",
            self.floor.rooms.len(),
            self.monsters.len(),
            self.floor.spawn,
            self.floor.stairs
        );
        Ok(())
    }

    /// Runs `f` with a bridge describing the CURRENT game state installed.
    /// `f` receives the bridge so a native-reading helper can be written
    /// without another wrapper.
    fn ask<R>(&mut self, f: impl FnOnce(&mut Bridge) -> R) -> R {
        let views: Vec<MonsterView> = self
            .monsters
            .iter()
            .map(|m| MonsterView::of(m, self.player_position))
            .collect();
        let mut bridge = Bridge::snapshot(
            self.depth,
            self.player_health,
            self.player_armor,
            &views,
            self.player_position,
        );
        let out = natives::with_bridge(&mut bridge, f);
        // Natives that act (rather than merely read) leave actions behind;
        // drain them so a pure query can't accidentally do damage.
        bridge.actions.clear();
        out
    }
}


/// The subset of a monster the bridge needs — a plain view so `natives`
/// doesn't have to know the game's `Monster` (which owns an Entity).
pub struct MonsterView {
    pub alive: bool,
    pub health: f32,
    pub max_health: f32,
    pub kind: i32,
    pub position: Vec3,
    pub distance: f32,
}

impl MonsterView {
    fn of(m: &Monster, player: Vec3) -> Self {
        MonsterView {
            alive: m.health > 0.0,
            health: m.health,
            max_health: m.max_health,
            kind: m.kind,
            position: m.position,
            distance: player.distance(m.position),
        }
    }

    /// Health as a 0..1 fraction — what the .ox brain reasons about, so a
    /// monster's max health must not change how timid it is.
    fn health_frac(&self) -> f64 {
        if self.max_health <= 0.0 {
            1.0
        } else {
            (self.health / self.max_health) as f64
        }
    }
}

impl DungeonGame {
    /// Mouse look. SDL reports y positive-downward, so dy is negated to
    /// make looking up pitch the camera up.
    fn update_look(&mut self, ctx: &Context) {
        let (dx, dy) = ctx.input.mouse_delta();
        if dx != 0 || dy != 0 {
            self.camera
                .look(dx as f32 * MOUSE_SENSITIVITY, -dy as f32 * MOUSE_SENSITIVITY);
        }
    }

    /// Movement direction from WASD, or from the autopilot when running
    /// headless. Movement is flattened to the horizontal plane so looking
    /// up doesn't slow you down.
    fn move_dir(&mut self, ctx: &Context) -> Vec3 {
        if self.autopilot {
            // Headless play has to fight, not just walk. Head for the nearest
            // living monster while it's further away than a comfortable hit,
            // then keep navigating to the stairs once the area is clear.
            let monsters: Vec<(Vec3, f32)> = self
                .monsters
                .iter()
                .map(|m| (m.position, m.health))
                .collect();
            let range = self.ask(|_| ox_modules::dungeon::attack_range() as f32);
            if let Some(i) = nearest_monster_in_range(self.player_position, &monsters, range * 1.4) {
                // Only chase while the target is in the same walkable
                // neighbourhood. Steering straight-line at a FLEEING monster
                // is how the autopilot deadlocked: the monster backed into a
                // dead end and the player pressed against the wall forever,
                // never reaching the stairs. Give up on anything the BFS
                // can't route to and go back to descending.
                if self.reachable_cells().contains(&self.cell_of(self.monsters[i].position)) {
                    let dir = self.monsters[i].position - self.player_position;
                    return Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
                }
            }
            return self.autopilot_dir();
        }
        let forward = self.camera.forward();
        let right = self.camera.right();
        let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

        let mut dir = Vec3::ZERO;
        if ctx.input.is_key_down(Keycode::W) {
            dir += forward_flat;
        }
        if ctx.input.is_key_down(Keycode::S) {
            dir -= forward_flat;
        }
        if ctx.input.is_key_down(Keycode::D) {
            dir += right_flat;
        }
        if ctx.input.is_key_down(Keycode::A) {
            dir -= right_flat;
        }
        dir.normalize_or_zero()
    }

    /// Headless navigation: walk toward the stairs. Straight-line steering
    /// would jam into walls on any non-empty floor, so this follows the
    /// generator's own route — a BFS path over walkable cells, recomputed
    /// when the target cell changes.
    fn autopilot_dir(&mut self) -> Vec3 {
        let goal = self.floor.stairs;
        let here = (
            (self.player_position.x / dungeon::CELL).round() as i32,
            (self.player_position.z / dungeon::CELL).round() as i32,
        );
        if here == goal {
            return Vec3::ZERO;
        }
        let next = self.route_step(here, goal);
        let target = Vec3::new(
            dungeon::world_x(next.0),
            0.0,
            dungeon::world_z(next.1),
        );
        let dir = target - self.player_position;
        Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero()
    }

    /// One BFS step from `from` toward `goal` over walkable cells. Falls
    /// back to straight-line if the grid says they're unreachable (which
    /// the generator's own tests say can't happen, but a stale grid could).
    /// Grid cell containing a world position.
    fn cell_of(&self, pos: Vec3) -> (i32, i32) {
        (
            (pos.x / dungeon::CELL).round() as i32,
            (pos.z / dungeon::CELL).round() as i32,
        )
    }

    /// Every walkable cell reachable from where the player stands. Cached per
    /// frame-sized call because the autopilot consults it every frame and the
    /// BFS is cheap on a grid this small.
    fn reachable_cells(&self) -> std::collections::HashSet<(i32, i32)> {
        crate::dungeon::reachable(&self.floor, self.cell_of(self.player_position))
            .into_iter()
            .collect()
    }

    fn route_step(&self, from: (i32, i32), goal: (i32, i32)) -> (i32, i32) {
        use std::collections::VecDeque;
        let w = self.floor.width as usize;
        let h = self.floor.height as usize;
        let in_bounds = |x: i32, y: i32| x >= 0 && y >= 0 && x < self.floor.width && y < self.floor.height;

        let mut prev = vec![(i32::MIN, i32::MIN); w * h];
        let mut seen = vec![false; w * h];
        let mut queue = VecDeque::new();
        seen[from.1 as usize * w + from.0 as usize] = true;
        queue.push_back(from);

        while let Some((cx, cy)) = queue.pop_front() {
            if (cx, cy) == goal {
                // Walk the parent chain back to the first step.
                let mut cur = (cx, cy);
                while prev[cur.1 as usize * w + cur.0 as usize] != (i32::MIN, i32::MIN)
                    && prev[cur.1 as usize * w + cur.0 as usize] != from
                {
                    cur = prev[cur.1 as usize * w + cur.0 as usize];
                }
                return cur;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if !in_bounds(nx, ny) || seen[ny as usize * w + nx as usize] {
                    continue;
                }
                if !self.floor.walkable[ny as usize][nx as usize] {
                    continue;
                }
                seen[ny as usize * w + nx as usize] = true;
                prev[ny as usize * w + nx as usize] = (cx, cy);
                queue.push_back((nx, ny));
            }
        }
        goal
    }

    /// Integrates movement and resolves collision against the wall grid.
    fn update_movement(&mut self, ctx: &Context, dt: f32) {
        let dir = self.move_dir(ctx);
        let target = dir * WALK_SPEED;

        // Ease horizontal velocity toward the target rather than snapping,
        // which stops the player feeling like they're on ice.
        let accel = 24.0;
        self.player_velocity.x += (target.x - self.player_velocity.x) * (accel * dt).min(1.0);
        self.player_velocity.z += (target.z - self.player_velocity.z) * (accel * dt).min(1.0);

        let mut next = self.player_position + self.player_velocity * dt;

        // Remember what we WANT to do before the corrections destroy it —
        // both corrections zero velocity, so by the end of the block there is
        // no direction left to reason about.
        let wish = self.player_velocity;

        // Axis-separated so sliding along a wall works instead of sticking.
        if self.blocked(next) {
            next.x = self.player_position.x;
            self.player_velocity.x = 0.0;
        }
        if self.blocked(next) {
            next.z = self.player_position.z;
            self.player_velocity.z = 0.0;
        }

        // Both axes blocked (a corner, or a wall directly ahead): slide along
        // whichever single axis is actually free. Without this the autopilot
        // pressed into a wall forever — it could SEE past the wall, and
        // `blocked_+x` was false the whole time, but a diagonal wish vector
        // meant neither axis ever moved.
        let stuck = (next - self.player_position).length() < 1e-4;
        if stuck && (wish.x != 0.0 || wish.z != 0.0) {
            let step = 0.05;
            let free_x = !self.blocked(Vec3::new(
                self.player_position.x + wish.x.signum() * step,
                0.0,
                self.player_position.z,
            ));
            let free_z = !self.blocked(Vec3::new(
                self.player_position.x,
                0.0,
                self.player_position.z + wish.z.signum() * step,
            ));
            if free_x {
                self.player_velocity.x = wish.x.signum() * WALK_SPEED;
                self.player_velocity.z = 0.0;
                next.x = self.player_position.x + wish.x.signum() * step;
            } else if free_z {
                self.player_velocity.z = wish.z.signum() * WALK_SPEED;
                self.player_velocity.x = 0.0;
                next.z = self.player_position.z + wish.z.signum() * step;
            }
        }
        self.player_position = next;

        // Keep the ECS transform in sync — that's what render reads. Scale
        // stays ZERO so the player is never drawn (see `descend`).
        if let Ok(mut t) = self.world.get::<&mut Transform>(self.player_entity) {
            t.position = self.player_position;
            t.scale = Vec3::ZERO;
        }
    }

    /// Whether a candidate position overlaps a wall cell. The player is a
    /// circle, so test the cell under its edge rather than its centre —
    /// otherwise you can half-embed yourself in a wall.
    fn blocked(&self, pos: Vec3) -> bool {
        let r = PLAYER_RADIUS;
        for (ox, oz) in [(-r, -r), (r, -r), (-r, r), (r, r)] {
            let gx = ((pos.x + ox) / dungeon::CELL).round() as i32;
            let gy = ((pos.z + oz) / dungeon::CELL).round() as i32;
            if gx < 0 || gy < 0 || gx >= self.floor.width || gy >= self.floor.height {
                return true;
            }
            if !self.floor.walkable[gy as usize][gx as usize] {
                return true;
            }
        }
        false
    }

    /// Runs the .ox brain at BRAIN_HZ: asks each monster what to do, and
    /// asks for its speed. Then executes the movement decisions.
    fn update_brain(&mut self, dt: f32) {
        self.brain_accumulator += dt;
        if self.brain_accumulator < 1.0 / BRAIN_HZ {
            return;
        }
        self.brain_accumulator = 0.0;

        let views: Vec<MonsterView> = self
            .monsters
            .iter()
            .map(|m| MonsterView::of(m, self.player_position))
            .collect();
        let mut bridge = Bridge::snapshot(
            self.depth,
            self.player_health,
            self.player_armor,
            &views,
            self.player_position,
        );

        // The decision function lives in oxidized; the host only carries it out.
        let decisions: Vec<i32> = natives::with_bridge(&mut bridge, |_| {
            views.iter()
                .map(|v| {
                    ox_modules::dungeon::monster_decision(
                        v.kind as i64,
                        v.health_frac(),
                        v.distance as f64,
                    ) as i32
                })
                .collect()
        });

        for (i, decision) in decisions.iter().enumerate() {
            if i < self.monsters.len() {
                self.monsters[i].decision = *decision;
            }
        }
        // A decision is a query, not an act — nothing it queued should fire.
        bridge.actions.clear();;
    }

    /// A short procedural sound effect. `play_tone` needs no asset files,
    /// which matters because this repo ships none.
    fn sfx(&self, freq: f32, secs: f32) {
        if let Some(audio) = &self.audio {
            audio.play_tone(freq, secs);
        }
    }

    /// True when the player asked to swing this frame. Left mouse button, or
    /// the autopilot, which fights on its own.
    fn wants_attack(&self, ctx: &Context) -> bool {
        self.autopilot || ctx.input.is_button_down(MouseButton::Left)
    }

    /// Resolve a player swing: pick a target, roll damage from `.ox`, apply
    /// it, and pay out gold on a kill.
    ///
    /// This is the ONLY place a monster dies, so loot drops exactly once per
    /// kill — `update_monsters` deliberately does not handle death.
    fn update_combat(&mut self, ctx: &Context, dt: f32) {
        self.attack_cooldown -= dt;
        if self.dead || self.won || !self.wants_attack(ctx) || self.attack_cooldown > 0.0 {
            return;
        }

        let depth = self.depth;
        // The cooldown is a `.ox` rule but only changes on descend, so cache
        // it rather than asking across the boundary on every swing.
        self.attack_cd_cache =
            self.ask(|_| ox_modules::dungeon::attack_cooldown(depth as i64) as f32);
        self.attack_cooldown = self.attack_cd_cache;

        let monsters: Vec<(Vec3, f32)> = self
            .monsters
            .iter()
            .map(|m| (m.position, m.health))
            .collect();
        let range = self.ask(|_| ox_modules::dungeon::attack_range() as f32);
        let Some(index) = nearest_monster_in_range(self.player_position, &monsters, range) else {
            return;
        };

        let damage =
            self.ask(|_| ox_modules::dungeon::player_attack_damage(depth as i64) as f32);
        self.monsters[index].health -= damage;

        if self.monsters[index].health <= 0.0 {
            let kind = self.monsters[index].kind;
            let reward =
                self.ask(|_| ox_modules::dungeon::kill_reward(depth as i64, kind as i64));
            self.gold += reward as i32;
            self.sfx(90.0, 0.25);
            self.toast(format!("Slain. +{reward} gold."));
            self.drop_loot(index);
            if let Ok(mut t) = self.world.get::<&mut Transform>(self.monsters[index].entity) {
                t.scale = Vec3::ZERO;   // despawn visually
            }
        } else {
            let remaining = self.monsters[index].health;
            self.sfx(150.0, 0.08);
            self.toast(format!("Hit for {damage:.0} ({remaining:.0} left)."));
        }
    }

    fn update_monsters(&mut self, dt: f32) {
        for i in 0..self.monsters.len() {
            if self.monsters[i].health <= 0.0 {
                continue;
            }
            self.monsters[i].attack_cooldown -= dt;
            let decision = self.monsters[i].decision;
            let depth = self.depth;
            let speed =
                self.ask(|_| ox_modules::dungeon::move_speed(decision as i64, depth as i64) as f32);
            let target = match decision {
                2 => Some(self.player_position),   // chase
                1 => Some(self.player_position),   // patrol: drift closer
                3 => Some(self.monsters[i].home),  // flee home
                4 => None,                         // strike: don't move
                _ => {
                    // Idle drift, so nothing stands perfectly still.
                    let (dx, dz) = rng::flat_dir();
                    Some(self.monsters[i].position + Vec3::new(dx, 0.0, dz) * 0.5)
                }
            };
            if let Some(target) = target {
                let to = target - self.monsters[i].position;
                let dist = Vec3::new(to.x, 0.0, to.z).length();
                if dist > 0.01 {
                    let dir = Vec3::new(to.x, 0.0, to.z).normalize_or_zero();
                    let mut next = self.monsters[i].position + dir * speed * dt;
                    // Monsters use the same wall test as the player.
                    if self.blocked(next) {
                        next = self.monsters[i].position;
                    }
                    self.monsters[i].position = next;
                    let yaw = dir.x.atan2(-dir.z);
                    if let Ok(mut t) = self.world.get::<&mut Transform>(self.monsters[i].entity) {
                        t.position = next;
                        t.rotation = glam::Quat::from_rotation_y(yaw);
                    }
                }
            }

            // NOTE: a monster that reaches 0 health here was killed by a
            // swing in update_combat, which runs AFTER this function — so
            // death (loot + despawn) is handled in exactly one place.

            // In range and off cooldown: strike. The damage number comes
            // from oxidized.
            let to_player =
                (self.player_position - self.monsters[i].position).length();
            if to_player < 2.0 && self.monsters[i].attack_cooldown <= 0.0 {
                // The strike cadence is a pacing rule, so it lives in the
                // .ox brain rather than here.
                let depth = self.depth;
                self.monsters[i].attack_cooldown = self.ask(|_| {
                    ox_modules::dungeon::strike_cooldown(depth as i64) as f32
                });
                let (raw, dealt) = {
                    let kind = self.monsters[i].kind;
                    let armor = self.player_armor;
                    let mut b = Bridge::snapshot(
                        self.depth,
                        self.player_health,
                        armor,
                        &[],
                        self.player_position,
                    );
                    natives::with_bridge(&mut b, |_| {
                        let raw = ox_modules::dungeon::strike_damage(kind as i64, self.depth as i64) as f32;
                        let dealt = ox_modules::dungeon::incoming_damage(raw as f64, armor as f64);
                        (raw, dealt as f32)
                    })
                };
                let _ = raw;
                self.player_health -= dealt;
                // Five monsters biting at once would otherwise stack five
                // toasts on one frame and paper the screen.
                if self.time - self.last_bite_toast > 1.5 {
                    self.last_bite_toast = self.time;
                    self.sfx(70.0, 0.15);
                    let noun = ["thing", "shade", "husk"][(self.monsters[i].kind as usize).min(2)];
                    self.toast(format!("A {noun} bites for {dealt:.0}."));
                }
                if self.player_health <= 0.0 {
                    self.player_health = 0.0;
                    self.dead = true;
                    log::info!("player died at depth {}", self.depth);
                }
            }
        }
    }

    /// Rolls loot for a dead monster. The drop table is oxidized's.
    fn drop_loot(&mut self, index: usize) {
        let kind = self.monsters[index].kind;
        let depth = self.depth;
        let loot_kind = self.ask(|_| ox_modules::dungeon::roll_loot(depth as i64, kind as i64));
        if loot_kind == 0 {
            return;
        }
        // Drop at the player's feet, not the corpse: loot across a room is
        // loot the player never walks over, which made armor effectively
        // unreachable (1 pickup per run in testing).
        let pos = self.player_position;
        let cube = Arc::clone(self.cube.as_ref().expect("mesh set up in init"));
        let white = Arc::clone(self.white.as_ref().expect("texture set up in init"));
        let entity = self.world.spawn((
            Transform {
                position: pos,
                rotation: glam::Quat::IDENTITY,
                scale: Vec3::splat(LOOT_SIZE),
            },
            MeshRenderer {
                mesh: cube,
                texture: Some(white),
            },
        ));
        self.loot.push(Loot {
            entity,
            kind: loot_kind as i32,
            position: pos,
        });
        let line = self.ask(|_| ox_modules::dungeon::pickup_line(loot_kind as i64, depth as i64));
        self.toast(format!("Found: {line}"));
    }

    fn check_pickups(&mut self) {
        let mut collected = Vec::new();
        for (i, l) in self.loot.iter().enumerate() {
            if (l.position - self.player_position).length() < 1.2 {
                collected.push(i);
            }
        }
        for i in collected.into_iter().rev() {
            let l = self.loot.swap_remove(i);
            match l.kind {
                1 => self.gold += 1 + self.depth,
                2 => self.player_health = (self.player_health + 25.0).min(100.0),
                // Armor is the only real defense against a swarm; 0.08 took
                // ten pickups to matter, so it never mattered.
                3 => {
                    self.player_armor = (self.player_armor + 0.15).min(0.75);
                    self.toast(format!("Armor up ({:.0}%).", self.player_armor * 100.0));
                }
                _ => {}
            }
            self.world.despawn(l.entity);
            log::info!("picked up loot kind {}", l.kind);
        }
    }

    /// Descend when standing on the stairs. Returns false if the floor
    /// wasn't rebuilt, so the caller knows to skip the rest of the frame.
    fn check_stairs(&mut self) {
        let goal = Vec3::new(
            dungeon::world_x(self.floor.stairs.0),
            0.0,
            dungeon::world_z(self.floor.stairs.1),
        );
        if (goal - self.player_position).length() > 1.5 {
            return;
        }
        if self.depth >= MAX_DEPTH {
            self.won = true;
            log::info!("escaped at depth {} with {} gold", self.depth, self.gold);
            return;
        }
        let next = self.depth + 1;
        if let Err(e) = self.descend(next) {
            log::error!("descend to {next} failed: {e}");
        }
    }
}


impl DungeonGame {
    /// HUD: health/armor bars, depth, gold, monster count, toasts, and an
    /// end-of-run panel. Runs AFTER `present`, because egui paints to the
    /// default framebuffer.
    fn draw_hud(&mut self, ctx: &mut Context) {
        let drawable_size = ctx.drawable_size();
        let Some(ui) = self.ui.as_mut() else { return };
        let Some(hud) = self.hud.as_mut() else { return };

        let alive = self.monsters.iter().filter(|m| m.health > 0.0).count();
        hud.set_bar("Health", self.player_health / 100.0);
        hud.set_bar("Armor", self.player_armor);
        hud.title = Some(format!(
            "Depth {}  |  {} gold  |  {} monsters left",
            self.depth, self.gold, alive
        ));

        // Snapshot what the closure needs so it never borrows `self`.
        let dead = self.dead;
        let won = self.won;
        let depth = self.depth;
        let gold = self.gold;
        let run_time = self.run_time;
        let banner = self.banner.clone();
        let low = self.player_health < 35.0 && !dead;

        let style = self.params.hud;
        let output = ui.run(drawable_size, |c| {
            draw_hud(c, hud, &style);

            if let Some((text, _)) = &banner {
                center_window(c, "banner", text);
            }
            if low {
                // A red edge vignette when hurt, drawn as four border
                // bands rather than a rounded shape.
                let rect = c.screen_rect();
                // Alpha premultiplied in GAMMA space: egui's
                // from_rgba_unmultiplied converts in linear space, which
                // turns a subtle 28% wash into a near-opaque red slab.
                let band = egui::Rect::from_min_size(
                    rect.min,
                    egui::vec2(rect.width(), 70.0),
                );
                c.layer_painter(egui::LayerId::background()).rect_filled(
                    band,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(48, 6, 6, 40),
                );
            }
            if dead {
                center_window(
                    c,
                    "over",
                    &format!("You died on depth {depth}\n\n{gold} gold  |  {run_time:.0}s"),
                );
            } else if won {
                center_window(
                    c,
                    "over",
                    &format!("You escaped\n\n{gold} gold  |  {run_time:.0}s"),
                );
            }
        });
        ui.paint(drawable_size, output);
    }
}

/// Where `assets/` and `profiles/` live: next to the executable if present,
/// else the crate root in debug builds, else the exe's directory. Never the
/// current working directory — running the game from anywhere but the repo
/// root used to fail with a GLSL "version directive must be first statement"
/// error, because the bare relative paths silently missed and a fallback
/// shader carrying its own `#version` line got the cache's prefix prepended.
fn resolve_asset_root() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));

    if exe_dir.join("assets").is_dir() {
        return exe_dir;
    }
    if cfg!(debug_assertions) {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        if manifest.join("assets").is_dir() {
            return manifest;
        }
    }
    exe_dir
}

/// Index of the nearest living monster within `range` of `origin`, or `None`.
/// Pure, so it's unit-testable without a window or an ECS world. Dead
/// monsters (health <= 0) are skipped — a corpse must never eat a swing.
fn nearest_monster_in_range(
    origin: Vec3,
    monsters: &[(Vec3, f32)],
    range: f32,
) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, (pos, health)) in monsters.iter().enumerate() {
        if *health <= 0.0 {
            continue;
        }
        let d = origin.distance(*pos);
        if d > range {
            continue;
        }
        match best {
            Some((_, bd)) if bd <= d => {}
            _ => best = Some((i, d)),
        }
    }
    best.map(|(i, _)| i)
}

/// A centered egui window — used for the floor banner and the end-of-run
/// panel. Named so repeated frames reuse one window rather than stacking.
fn center_window(c: &egui::Context, id: &str, text: &str) {
    egui::Window::new(id)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(c, |ui| { ui.label(text); });
}

const MAX_DEPTH: i32 = 8;
/// World size of a monster, in units. The cube primitive spans -0.5..0.5, so
/// this is literally the edge length.
const MONSTER_SIZE: f32 = 0.9;
/// Loot cubes are small on purpose — they're meant to be spotted, not blocking.
const LOOT_SIZE: f32 = 0.45;

impl Game for DungeonGame {
    fn init(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        unsafe {
            ctx.gl().enable(glow::DEPTH_TEST);
        }
        let gl = ctx.gl_arc();
        self.renderer_gl = Some(Arc::clone(&gl));

        let cube_mesh = primitives::cube();
        let cube = GpuMesh::upload(&gl, &cube_mesh)?;
        self.cube = Some(Arc::new(cube));

        // The frag shader samples uTex unconditionally, so every mesh needs
        // a bound texture — a 1x1 white one stands in when there's no art.
        let white = GpuTexture::from_rgba8(&gl, &[255, 255, 255, 255], 1, 1, TextureFilter::Nearest)?;
        self.white = Some(Arc::new(white));

        // Assets are resolved relative to the executable (or, in debug,
        // the crate root) — NOT the current working directory. Bare
        // "assets/..." paths silently worked from the repo root and crashed
        // everywhere else.
        let asset_root = resolve_asset_root();
        let profiles_dir = asset_root.join("profiles");
        let _ = std::fs::create_dir_all(&profiles_dir);
        self.params = match engine::profile::load_dir(&profiles_dir) {
            Ok(list) if !list.is_empty() => {
                log::info!("using shader profile {:?}", list[0].name);
                list[0].render
            }
            other => {
                if let Err(e) = &other {
                    log::warn!("no shader profiles in {profiles_dir:?} ({e}); using defaults");
                }
                RenderParams::default()
            }
        };

        let post_src =
            std::fs::read_to_string(asset_root.join("assets/shaders/post_composite.frag"))
            .unwrap_or_else(|_| engine::renderer::DEFAULT_FRAGMENT_SRC.to_string());
        self.renderer = Some(Renderer::new(
            &gl,
            ctx.drawable_size(),
            self.params.resolution_scale,
            &post_src,
        )?);

        // ShaderVariantCache needs its sources up front, not lazily.
        let vert = std::fs::read_to_string(asset_root.join("assets/shaders/mesh.vert"))
            .unwrap_or_else(|_| FALLBACK_VERT.to_string());
        let frag = std::fs::read_to_string(asset_root.join("assets/shaders/mesh.frag"))
            .unwrap_or_else(|_| FALLBACK_FRAG.to_string());
        self.shader_cache = Some(ShaderVariantCache::new(vert, frag));

        // A seamless stone texture for the walls. Bilinear, not Nearest: a
        // 64px pattern nearest-filtered shimmers into static at range.
        let stone = GpuTexture::from_rgba8(
            &gl,
            &stone_texture(64, 64),
            64,
            64,
            TextureFilter::Bilinear,
        )?;
        self.stone = Some(Arc::new(stone));

        // Audio is optional: a machine with no output device must still play.
        self.audio = match AudioContext::new() {
            Ok(audio) => Some(audio),
            Err(err) => {
                log::warn!("no audio output available, sounds will be silent: {err}");
                None
            }
        };

        // egui backs the HUD. Its painter leaves DEPTH_TEST off and
        // BLEND/SCISSOR on, which is why render() re-enables depth right
        // after begin_scene.
        self.ui = Some(EguiState::new(Arc::clone(&gl))?);
        self.hud = Some(HudState::default());

        rng::seed(self.seed);
        self.descend(1)?;
        log::info!("init complete: seed {}", self.seed);
        Ok(())
    }

    fn handle_event(&mut self, ctx: &mut Context, event: &Event) {
        if let Event::KeyDown { keycode, .. } = *event {
            match keycode {
                Some(Keycode::Escape) => ctx.should_quit = true,
                _ => {}
            }
        }
    }

    fn update(&mut self, ctx: &mut Context, dt: f32) -> anyhow::Result<()> {
        let dt = dt.min(0.05); // a hitch must not teleport anyone through a wall
        self.time += dt;
        self.run_time += dt;

        if let Some(hud) = self.hud.as_mut() {
            hud.tick(dt);
        }
        if let Some((_, t)) = self.banner.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.banner = None;
            }
        }

        if self.dead || self.won {
            if self.autopilot {
                log::info!(
                    "run ended after {:.1}s: {} at depth {}, gold {}",
                    self.run_time,
                    if self.dead { "died" } else { "escaped" },
                    self.depth,
                    self.gold
                );
                ctx.should_quit = true;
            }
            return Ok(());
        }

        self.update_look(ctx);
        self.update_movement(ctx, dt);
        self.update_brain(dt);
        self.update_combat(ctx, dt);
        self.update_monsters(dt);
        self.check_pickups();
        self.check_stairs();
        Ok(())
    }

    fn render(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let drawable_size = ctx.drawable_size();
        let aspect = ctx.aspect_ratio();
        let gl = ctx.gl();
        // Compute everything that reads `self` BEFORE taking the mutable
        // renderer borrow, or the two overlap.
        let eye = self.eye();
        let view = self.camera.view_matrix(eye);
        let proj = self.camera.projection_matrix(aspect);
        let params = self.params;

        let renderer = self.renderer.as_mut().expect("renderer set up in init");
        renderer.resize_if_needed(gl, drawable_size)?;
        renderer.begin_scene(gl);

        // egui's painter leaves DEPTH_TEST off and BLEND/SCISSOR on; reset
        // them or the world draws without depth sorting from frame 2 on.
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.disable(glow::BLEND);
            gl.disable(glow::SCISSOR_TEST);
            gl.enable(glow::CULL_FACE);
            gl.cull_face(glow::BACK);
            if params.backface_culling {
                gl.enable(glow::CULL_FACE);
            } else {
                gl.disable(glow::CULL_FACE);
            }
            gl.clear_color(
                params.fog_color[0],
                params.fog_color[1],
                params.fog_color[2],
                1.0,
            );
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
        }

        let cache = self.shader_cache.as_mut().expect("shader cache set up in init");
        let flags = if params.affine_texture_mapping {
            engine::shader::AFFINE_UV_BIT
        } else {
            0
        };
        let program = cache.get_or_compile(gl, flags)?;
        let uniforms = MeshUniforms::resolve(gl, program);

        unsafe {
            gl.use_program(Some(program));
            gl.uniform_matrix_4_f32_slice(uniforms.view.as_ref(), false, &view.to_cols_array());
            gl.uniform_matrix_4_f32_slice(uniforms.proj.as_ref(), false, &proj.to_cols_array());

            let light_dir = Vec3::from(params.light_dir).normalize_or_zero();
            gl.uniform_3_f32(
                uniforms.light_dir.as_ref(),
                light_dir.x,
                light_dir.y,
                light_dir.z,
            );
            gl.uniform_3_f32(
                uniforms.ambient_color.as_ref(),
                params.ambient_color[0],
                params.ambient_color[1],
                params.ambient_color[2],
            );
            gl.uniform_1_i32(
                uniforms.lighting_mode.as_ref(),
                match params.lighting_mode {
                    engine::profile::LightingMode::Unlit => 0,
                    engine::profile::LightingMode::VertexLit => 1,
                },
            );
            gl.uniform_1_f32(uniforms.vertex_snap_amount.as_ref(), params.vertex_snap_amount);
            gl.uniform_1_f32(uniforms.fog_start.as_ref(), params.fog_start);
            gl.uniform_1_f32(uniforms.fog_end.as_ref(), params.fog_end);
            gl.uniform_3_f32(
                uniforms.fog_color.as_ref(),
                params.fog_color[0],
                params.fog_color[1],
                params.fog_color[2],
            );
            gl.uniform_1_i32(uniforms.point_light_count.as_ref(), 0);
        }

        for (_entity, (transform, mesh_renderer)) in
            self.world.query::<(&Transform, &MeshRenderer)>().iter()
        {
            let model = Mat4::from_scale_rotation_translation(
                transform.scale,
                transform.rotation,
                transform.position,
            );
            unsafe {
                gl.uniform_matrix_4_f32_slice(uniforms.model.as_ref(), false, &model.to_cols_array());
            }
            // A player torch, carried in the point-light slot, so a dark
            // dungeon is still readable.
            if let Some(tex) = &mesh_renderer.texture {
                tex.bind(gl, 0);
            } else if let Some(white) = &self.white {
                white.bind(gl, 0);
            }
            mesh_renderer.mesh.draw(gl);
        }

        renderer.present(
            gl,
            drawable_size,
            &PostParams {
                color_levels: params.color_levels as f32,
                dither_strength: params.dither_strength,
                tint_color: [1.0, 1.0, 1.0],
                tint_strength: 0.0,
            },
        );

        self.draw_hud(ctx);
        Ok(())
    }
}

/// Minimal shaders, used only if the files under assets/ are missing so the
/// game still boots from a fresh clone instead of dying in `init`.
const FALLBACK_VERT: &str = r#"
#version 300 es
in vec3 aPosition;
in vec3 aNormal;
uniform mat4 uModel;
uniform mat4 uView;
uniform mat4 uProj;
out vec3 vNormal;
void main() {
    vNormal = aNormal;
    gl_Position = uProj * uView * uModel * vec4(aPosition, 1.0);
}
"#;

const FALLBACK_FRAG: &str = r#"
#version 300 es
precision highp float;
in vec3 vNormal;
uniform vec3 uAmbientColor;
uniform vec3 uFogColor;
uniform float uFogStart;
uniform float uFogEnd;
out vec4 fragColor;
void main() {
    float d = length(vNormal) * 0.5 + 0.5;
    vec3 base = mix(vec3(0.18, 0.17, 0.21), vec3(0.55, 0.53, 0.58), d);
    fragColor = vec4(base * uAmbientColor, 1.0);
}
"#;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let autopilot = std::env::var("DUNGEON_AUTOPILOT").is_ok();
    let seed = std::env::var("DUNGEON_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(1)
        });

    App::run(
        "The Catacombs of Rust",
        1280,
        720,
        DungeonGame::new(seed, autopilot),
    )
}

#[cfg(test)]
mod ox_rules_tests {
    use super::ox_modules::dungeon as ox;
    use super::*;

    // --- Task 1.1: the attack rules, which don't exist yet ---

    #[test]
    fn attack_damage_scales_with_depth() {
        let shallow = ox::player_attack_damage(1);
        let deep = ox::player_attack_damage(8);
        assert!(
            deep > shallow,
            "deeper floors must hit harder: {deep} !> {shallow}"
        );
    }

    #[test]
    fn attack_damage_is_never_zero() {
        for depth in 1..=MAX_DEPTH {
            assert!(
                ox::player_attack_damage(depth as i64) > 0.0,
                "depth {depth} dealt zero damage"
            );
        }
    }

    #[test]
    fn attack_cooldown_never_reaches_zero() {
        // A zero cooldown would be a fire rate of infinity.
        for depth in 1..=MAX_DEPTH {
            assert!(
                ox::attack_cooldown(depth as i64) > 0.0,
                "depth {depth} cooldown was 0"
            );
        }
    }

    #[test]
    fn attack_cooldown_shrinks_with_depth() {
        assert!(ox::attack_cooldown(8) < ox::attack_cooldown(1));
    }

    #[test]
    fn attack_range_is_reachable_and_not_pointless() {
        let r = ox::attack_range();
        assert!(r >= 1.5, "melee range {r} is too short to be usable");
        assert!(r <= 4.0, "attack range {r} is longer than a dungeon corridor");
    }

    #[test]
    fn kill_reward_grows_with_depth() {
        assert!(ox::kill_reward(8, 0) > ox::kill_reward(1, 0));
    }

    // --- Task 1.5: melee targeting, which doesn't exist yet either ---

    #[test]
    fn nearest_monster_in_range_is_found() {
        let monsters = [
            (Vec3::new(0.5, 0.0, 0.0), 0.0f32),  // dead, and closest of all
            (Vec3::new(1.0, 0.0, 0.0), 50.0f32), // alive, nearest living
            (Vec3::new(2.0, 0.0, 0.0), 50.0f32), // alive, farther
        ];
        let hit = nearest_monster_in_range(Vec3::ZERO, &monsters, 2.5);
        assert_eq!(hit, Some(1), "should hit the nearest ALIVE monster");
    }

    #[test]
    fn no_monster_in_range_returns_none() {
        let monsters = [(Vec3::new(10.0, 0.0, 0.0), 50.0f32)];
        assert_eq!(nearest_monster_in_range(Vec3::ZERO, &monsters, 2.5), None);
    }

    #[test]
    fn dead_monsters_are_never_targeted() {
        let monsters = [(Vec3::new(1.0, 0.0, 0.0), 0.0f32)];
        assert_eq!(nearest_monster_in_range(Vec3::ZERO, &monsters, 5.0), None);
    }

    #[test]
    fn nearer_monster_wins_regardless_of_order() {
        let monsters = [
            (Vec3::new(4.0, 0.0, 0.0), 50.0f32),
            (Vec3::new(1.0, 0.0, 0.0), 50.0f32),
            (Vec3::new(2.0, 0.0, 0.0), 50.0f32),
        ];
        assert_eq!(nearest_monster_in_range(Vec3::ZERO, &monsters, 5.0), Some(1));
    }
}
