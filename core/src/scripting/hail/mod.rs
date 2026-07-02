use anyhow::Result;
use apostasy_macros::{start, update};

pub mod components;
pub mod logging;
pub mod query;
pub mod resources;
pub mod world;

use crate::{
    ecs::World,
    scripting::hail::runtime::{HailRuntime, discover_hail_scripts},
};

pub mod resolver;
pub mod runtime;

#[start(mode = "all", priority = 100)]
pub fn hail_scripting_load(world: &mut World) -> Result<()> {
    let runtime = HailRuntime::new();
    for path in discover_hail_scripts() {
        if let Some(program) = runtime.load_script(&path) {
            // Run top-level code once at load ("start" semantics).
            if let Err(e) = hail::Executor::default().run(&program) {
                crate::log_error!("[hail] runtime error in {:?}: {e:?}", path);
            }
        }
    }
    world.insert_resource(runtime);
    Ok(())
}

/// Fires each script's `start(world)` once. Game mode only; runs after the
/// loader thanks to its lower priority.
#[start(mode = "game", priority = 0)]
pub fn hail_scripting_start(world: &mut World) -> Result<()> {
    if !world.has_resource::<HailRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<HailRuntime>()?.clone();
    runtime.run_event(world, "start");
    Ok(())
}

/// Fires each script's `update(world)`. Game mode only.
#[update(mode = "game", priority = 0)]
pub fn hail_scripting_update(world: &mut World) -> Result<()> {
    if !world.has_resource::<HailRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<HailRuntime>()?.clone();
    runtime.run_event(world, "update");
    Ok(())
}
