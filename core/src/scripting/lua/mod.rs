pub mod component;
pub mod query;
pub mod resource;
pub mod runtime;
pub mod ui_api;
pub mod world_api;

use anyhow::Result;
use apostasy_macros::{fixed_update, late_update, prerender, start, update};

use self::component::LuaComponentRegistry;
use self::resource::LuaResources;
use self::runtime::{LuaRuntime, discover_lua_scripts};
use crate::ecs::world::World;

/// Loads scripts and applies their `register_component` declarations in BOTH
/// editor and game mode. Runs only top-level script code — no `start`/`update`,
/// so the editor learns component schemas without executing gameplay logic.
#[start(mode = "all", priority = 100)]
pub fn lua_scripting_load(world: &mut World) -> Result<()> {
    let runtime = match LuaRuntime::new() {
        Ok(r) => r,
        Err(e) => {
            crate::log_error!("[lua] init failed: {e}");
            return Ok(());
        }
    };
    for path in discover_lua_scripts() {
        if let Err(e) = runtime.load_script(&path) {
            crate::log_error!("[lua] load {:?} failed: {e}", path);
        }
    }
    world.insert_resource(LuaComponentRegistry::default());
    world.insert_resource(LuaResources::default());
    runtime.apply_registrations(world);
    world.insert_resource(runtime);
    Ok(())
}

/// Fires each script's `start(world)`. Game mode only — never runs in the editor.
#[start(mode = "game", priority = 0)]
pub fn lua_scripting_start(world: &mut World) -> Result<()> {
    if !world.has_resource::<LuaRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<LuaRuntime>()?.clone();
    runtime.run_event(world, "start");
    Ok(())
}

/// Fires each script's `prerender(world)`, before the frame is drawn. Game mode only.
#[prerender(mode = "game", priority = 0)]
pub fn lua_scripting_prerender(world: &mut World) -> Result<()> {
    if !world.has_resource::<LuaRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<LuaRuntime>()?.clone();
    runtime.run_event(world, "prerender");
    Ok(())
}

/// Hot-reloads changed scripts and fires `update(world)`. Game mode only.
#[update(mode = "game", priority = 0)]
pub fn lua_scripting_update(world: &mut World) -> Result<()> {
    if !world.has_resource::<LuaRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<LuaRuntime>()?.clone();
    runtime.hot_reload();
    runtime.apply_registrations(world); // pick up any newly-declared components
    runtime.run_event(world, "update");
    Ok(())
}

/// Fires each script's `fixed_update(world, delta)` at the fixed timestep (20 Hz).
/// Game mode only.
#[fixed_update(mode = "game", priority = 0)]
pub fn lua_scripting_fixed_update(world: &mut World, delta: f32) -> Result<()> {
    if !world.has_resource::<LuaRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<LuaRuntime>()?.clone();
    runtime.run_fixed_event(world, "fixed_update", delta);
    Ok(())
}

/// Fires each script's `late_update(world)` at the end of the frame. Game mode only.
#[late_update(mode = "game", priority = 0)]
pub fn lua_scripting_late_update(world: &mut World) -> Result<()> {
    if !world.has_resource::<LuaRuntime>() {
        return Ok(());
    }
    let runtime = world.get_resource::<LuaRuntime>()?.clone();
    runtime.run_event(world, "late_update");
    Ok(())
}
