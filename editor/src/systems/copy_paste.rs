use anyhow::Result;
use apostasy_core::{
    objects::{
        Object,
        resources::input_manager::{InputManager, KeyAction, KeyBind},
        world::World,
    },
    start, update,
    winit::keyboard::{KeyCode, PhysicalKey},
};

use crate::ui::cell_panel::CellSearchState;

#[start(mode = "editor")]
pub fn init(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>().unwrap();

    inputs.register_keybind(
        "ControlModifier",
        KeyBind::new(PhysicalKey::Code(KeyCode::ControlLeft), KeyAction::Hold),
    )?;
    inputs.register_keybind(
        "Paste",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyV), KeyAction::Press),
    )?;
    inputs.register_keybind(
        "Copy",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyC), KeyAction::Press),
    )?;
    Ok(())
}

#[update(mode = "editor")]
pub fn copy_paste_objects(world: &mut World) -> Result<()> {
    if world.get_resource::<CellSearchState>().is_err() {
        world.insert_resource(CellSearchState::default());
    }
    let mut object_to_copy: Option<Object> = None;
    let inputs = world.get_resource::<InputManager>().unwrap();

    let copy = world.get_resource::<CellSearchState>()?.copied_obj.clone();
    let selected_object = world.get_resource::<CellSearchState>()?.clicked_obj.clone();

    if inputs.is_keybind_active("ControlModifier") && inputs.is_keybind_active("Copy") {
        if let Some(selected) = selected_object {
            object_to_copy = Some(world.get_object(selected).unwrap().clone());
        }
    }
    if inputs.is_keybind_active("ControlModifier") && inputs.is_keybind_active("Paste") {
        if let Some(copied) = copy {
            world.add_object(copied);
        }
    }

    if object_to_copy.is_some() {
        world.get_resource_mut::<CellSearchState>()?.copied_obj = object_to_copy;
    }
    Ok(())
}
