pub mod component;
pub mod runtime;

use anyhow::Result;
use apostasy_macros::start;

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
    world.insert_resource(runtime);
    Ok(())
}
