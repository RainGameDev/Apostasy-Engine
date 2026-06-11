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

use crate::systems::history::EditorCommand;
use crate::ui::cell_panel::CellSearchState;

#[start(mode = "editor")]
pub fn init(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>().unwrap();

    inputs.register_default_keybind(
        "ControlModifier",
        KeyBind::new(PhysicalKey::Code(KeyCode::ControlLeft), KeyAction::Hold),
    );
    inputs.register_default_keybind(
        "Copy",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyC), KeyAction::Press),
    );
    inputs.register_default_keybind(
        "Paste",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyV), KeyAction::Press),
    );
    inputs.register_default_keybind(
        "Duplicate",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyD), KeyAction::Press),
    );
    Ok(())
}

#[update(mode = "editor")]
pub fn copy_paste_objects(world: &mut World) -> Result<()> {
    if world.get_resource::<CellSearchState>().is_err() {
        world.insert_resource(CellSearchState::default());
    }

    let inputs = world.get_resource::<InputManager>().unwrap();
    let ctrl = inputs.is_keybind_active("ControlModifier");
    let do_copy = ctrl && inputs.is_keybind_active("Copy");
    let do_paste = ctrl && inputs.is_keybind_active("Paste");
    let do_duplicate = ctrl && inputs.is_keybind_active("Duplicate");

    let clipboard = world.get_resource::<CellSearchState>()?.copied_obj.clone();
    let selected_id = world.get_resource::<CellSearchState>()?.selected_obj;

    let selected_obj: Option<Object> = selected_id
        .and_then(|id| world.get_object(id).cloned());

    if do_copy {
        if let Some(obj) = selected_obj.clone() {
            world.get_resource_mut::<CellSearchState>()?.copied_obj = Some(obj);
        }
    }

    if do_paste {
        if let Some(copied) = clipboard {
            let mut cmd = Box::new(crate::systems::history::AddObjectCmd::new(copied, None));
            cmd.execute(world)?;
            world.get_resource_mut::<crate::systems::history::History>()?.push(cmd);
        }
    } else if do_duplicate {
        if let Some(obj) = selected_obj {
            let mut cmd = Box::new(crate::systems::history::AddObjectCmd::new(obj, None));
            cmd.execute(world)?;
            world.get_resource_mut::<crate::systems::history::History>()?.push(cmd);
        }
    }

    Ok(())
}
