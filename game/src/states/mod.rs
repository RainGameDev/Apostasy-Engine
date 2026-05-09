use apostasy_core::{
    anyhow::Result,
    objects::{
        resources::{
            cursor_manager::{CursorLockMode, CursorManager},
            input_manager::InputManager,
            window_manager::WindowManager,
        },
        world::World,
    },
    update,
};
use apostasy_macros::Resource;

#[derive(Resource, Clone)]
pub struct HasInitGeneration;
#[derive(Resource, Clone)]
pub struct GetNewSeed;

#[derive(Resource, Clone)]
pub struct IsPaused;

#[update]
pub fn pause(world: &mut World) -> Result<()> {
    let inputs = world.get_resource::<InputManager>()?;

    if inputs.is_keybind_active("Pause") {
        if world.get_resource::<IsPaused>().is_ok() {
            world.remove_resource::<IsPaused>();
        } else {
            world.insert_resource(IsPaused);
        }
    }

    Ok(())
}

#[update]
pub fn paused_update(world: &mut World) -> Result<()> {
    let is_paused = world.get_resource::<IsPaused>().is_ok();
    {
        let cursor_manager = world.get_resource_mut::<CursorManager>()?;

        if !is_paused {
            cursor_manager.set_mode(CursorLockMode::LockedHidden);
        } else {
            cursor_manager.set_mode(CursorLockMode::NoneVisible);
        }
    }

    {
        let cursor_manager = world.get_resource::<CursorManager>()?.clone();
        let window_manager = world.get_resource_mut::<WindowManager>()?;
        cursor_manager.update_cursor(window_manager);
    }

    Ok(())
}
