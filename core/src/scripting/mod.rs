pub mod component;
pub mod host;
pub mod lua;
pub mod runtime;

use anyhow::Result;
use apostasy_macros::{start, update};

use crate::ecs::world::World;

use self::runtime::{ScriptRuntime, discover_script_paths};

/// Builds the script runtime, discovers and loads .wasm files from res/scripts/, and runs on_start.
#[start(mode = "game", priority = 0)]
pub fn scripting_start(world: &mut World) -> Result<()> {
    let runtime = match ScriptRuntime::new() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("[scripting] failed to build runtime: {e}");
            return Ok(());
        }
    };

    for path in discover_script_paths() {
        if let Err(e) = runtime.load_module(&path) {
            tracing::error!("[scripting] failed to load {:?}: {e}", path);
        }
    }

    let world_ptr = world as *mut World;
    runtime.run_start(world_ptr);

    world.insert_resource(runtime);
    Ok(())
}

/// Hot-reloads changed modules and calls on_update in each loaded script.
#[update(mode = "game", priority = 0)]
pub fn scripting_update(world: &mut World) -> Result<()> {
    if !world.has_resource::<ScriptRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<ScriptRuntime>()?.clone();
    let world_ptr = world as *mut World;
    runtime.run_update(world_ptr);
    Ok(())
}
