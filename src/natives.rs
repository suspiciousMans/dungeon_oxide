//! The engine half of the oxidized boundary.
//!
//! Everything in `__oxidized_natives` here is called FROM `scripts/*.ox`.
//! Those files declare matching `fn native` signatures; the Rust generated
//! from them resolves each call through `crate::__oxidized_natives`.
//!
//! Natives are free functions (that's what a native is), so they can't take
//! `&mut Game`. Instead the game installs a pointer to a per-frame [`Bridge`]
//! for the duration of the brain update, and the natives read world state
//! from it and push [`Action`]s into it. `Game::update` drains those actions
//! afterwards. This is the standard "context global" shape — the alternative
//! (a thread-local `RefCell`) needs aliasing gymnastics to hand out `&mut`
//! from a shared slot, which is a footgun for no benefit.

use std::cell::Cell;

use crate::rng;

/// Per-frame world state the .ox brain can see, plus the actions it wants
/// performed. Rebuilt by `Game` each frame, before the brain runs.
pub struct Bridge {
    pub frame: u64,
    pub time: f32,
    pub depth: i32,
    pub room_number: i32,

    pub player_health: f32,
    pub player_armor: f32,

    // Monster state, parallel arrays — index-stable for the whole frame so
    // an index passed back in an Action refers to the same monster.
    pub monster_count: usize,
    pub monster_alive: Vec<bool>,
    pub monster_health: Vec<f32>,
    pub monster_distance: Vec<f32>,
    pub monster_kind: Vec<i32>,

    /// Accumulated by natives during the brain update, drained by the game.
    pub actions: Vec<Action>,
}

impl Bridge {
    /// Builds a bridge describing the current game state. `monster_distance`
    /// is filled in by the caller, which knows the player's position.
    pub fn snapshot(
        depth: i32,
        player_health: f32,
        player_armor: f32,
        monsters: &[crate::MonsterView],
        player_position: glam::Vec3,
    ) -> Bridge {
        let n = monsters.len();
        Bridge {
            frame: 0,
            time: 0.0,
            depth,
            room_number: depth,
            player_health,
            player_armor,
            monster_count: n,
            monster_alive: monsters.iter().map(|m| m.alive).collect(),
            monster_health: monsters.iter().map(|m| m.health).collect(),
            monster_distance: monsters
                .iter()
                .map(|m| player_position.distance(m.position))
                .collect(),
            monster_kind: monsters.iter().map(|m| m.kind).collect(),
            actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    DamageMonster { index: usize, amount: f32 },
    DamagePlayer { amount: f32 },
    MoveMonster { index: usize, x: f32, z: f32 },
    SpawnLoot { x: f32, z: f32, kind: i32 },
    PlayTone { freq: f32 },
    Toast { text: String },
}

thread_local! {
    /// Valid only inside `with_bridge`. Null otherwise.
    static ACTIVE: Cell<*mut Bridge> = const { Cell::new(std::ptr::null_mut()) };
}

/// Installs `bridge` for the duration of `f`, then leaves the (mutated)
/// bridge in place for the caller to drain.
///
/// # Safety
/// The game loop is single-threaded and the returned pointer never escapes
/// `f`, so no other code can observe the bridge while it's installed.
pub fn with_bridge<R>(bridge: &mut Bridge, f: impl FnOnce(&mut Bridge) -> R) -> R {
    ACTIVE.with(|slot| {
        let ptr: *mut Bridge = bridge;
        slot.set(ptr);
        let out = f(unsafe { &mut *ptr });
        slot.set(std::ptr::null_mut());
        out
    })
}

/// The installed bridge, or a panic explaining the misuse. Called by every
/// native.
fn bridge() -> &'static mut Bridge {
    let ptr = ACTIVE.with(|s| s.get());
    assert!(
        !ptr.is_null(),
        "an oxidized native was called outside `with_bridge` — natives are \
         only valid during the game's brain update"
    );
    unsafe { &mut *ptr }
}

/// The module generated oxidized code imports its natives from. This name
/// is the contract documented in oxidized's README.
pub mod __oxidized_natives {
    use super::*;

    pub fn host_monster_count() -> i64 {
        bridge().monster_count as i64
    }

    pub fn host_monster_alive(i: i64) -> bool {
        let b = bridge();
        i >= 0 && (i as usize) < b.monster_alive.len() && b.monster_alive[i as usize]
    }

    pub fn host_monster_health(i: i64) -> f64 {
        let b = bridge();
        match b.monster_health.get(i.max(0) as usize) {
            Some(v) => *v as f64,
            None => 0.0,
        }
    }

    pub fn host_monster_distance(i: i64) -> f64 {
        let b = bridge();
        match b.monster_distance.get(i.max(0) as usize) {
            Some(v) => *v as f64,
            None => 0.0,
        }
    }

    pub fn host_player_health() -> f64 {
        bridge().player_health as f64
    }

    pub fn host_room_number() -> i64 {
        bridge().room_number as i64
    }

    pub fn host_depth() -> i64 {
        bridge().depth as i64
    }

    pub fn host_time() -> f64 {
        bridge().time as f64
    }

    pub fn host_damage_monster(i: i64, amount: f64) {
        bridge()
            .actions
            .push(Action::DamageMonster { index: i.max(0) as usize, amount: amount as f32 });
    }

    pub fn host_damage_player(amount: f64) {
        bridge().actions.push(Action::DamagePlayer { amount: amount as f32 });
    }

    pub fn host_move_monster(i: i64, x: f64, z: f64) {
        bridge()
            .actions
            .push(Action::MoveMonster { index: i.max(0) as usize, x: x as f32, z: z as f32 });
    }

    pub fn host_spawn_loot(x: f64, z: f64, kind: i64) {
        bridge().actions.push(Action::SpawnLoot { x: x as f32, z: z as f32, kind: kind as i32 });
    }

    pub fn host_play_tone(freq: f64) {
        bridge().actions.push(Action::PlayTone { freq: freq as f32 });
    }

    pub fn host_toast(text: String) {
        bridge().actions.push(Action::Toast { text });
    }

    /// The host's RNG, so every random decision the .ox side makes draws
    /// from one seeded stream and a given seed replays identically.
    pub fn host_rng() -> f64 {
        rng::next_f64()
    }
}