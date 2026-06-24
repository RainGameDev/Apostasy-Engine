pub mod component;
pub mod runtime;
pub mod world_api;

use anyhow::Result;
use apostasy_macros::{start, update};

use self::component::LuaComponentRegistry;
use self::runtime::{LuaRuntime, discover_lua_scripts};
use crate::ecs::world::World;

/// Builds the Lua runtime, registers the component registry, loads all scripts.
#[start(mode = "all", priority = 0)]
pub fn lua_scripting_start(world: &mut World) -> Result<()> {
    let runtime = match LuaRuntime::new() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("[lua] init failed: {e}");
            return Ok(());
        }
    };
    for path in discover_lua_scripts() {
        if let Err(e) = runtime.load_script(&path) {
            tracing::error!("[lua] load {:?} failed: {e}", path);
        }
    }
    world.insert_resource(LuaComponentRegistry::default());
    world.insert_resource(runtime.clone());
    runtime.run_event(world, "start");
    Ok(())
}

#[update(mode = "all", priority = 0)]
pub fn lua_scripting_update(world: &mut World) -> Result<()> {
    if !world.has_resource::<LuaRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<LuaRuntime>()?.clone();
    runtime.run_event(world, "update");
    Ok(())
}
