use anyhow::Result;
use apostasy_core::{
    ecs::{
        cell::EntityBlob,
        resources::input_manager::{InputManager, KeyAction, KeyBind},
        world::World,
    },
    start, ui::ui_context::EguiContext, update,
    winit::keyboard::{KeyCode, PhysicalKey},
};

use crate::systems::history::EditorCommand;
use crate::ui::cell_panel::CellSearchState;

#[start(mode = "editor")]
pub fn init(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>().unwrap();

    inputs.register_default_keybind(
        "Copy",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyC), KeyAction::Press).with_ctrl(),
    );
    inputs.register_default_keybind(
        "Paste",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyV), KeyAction::Press).with_ctrl(),
    );
    inputs.register_default_keybind(
        "Duplicate",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyD), KeyAction::Press).with_ctrl(),
    );
    inputs.register_default_keybind(
        "Delete",
        KeyBind::new(PhysicalKey::Code(KeyCode::Delete), KeyAction::Press),
    );
    inputs.register_default_keybind(
        "GizmoTranslate",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyG), KeyAction::Press).with_context("viewport"),
    );
    inputs.register_default_keybind(
        "GizmoRotate",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyR), KeyAction::Press).with_context("viewport"),
    );
    inputs.register_default_keybind(
        "GizmoScale",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyS), KeyAction::Press).with_context("viewport"),
    );
    inputs.register_default_keybind(
        "Save",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyS), KeyAction::Press).with_shift(),
    );
    inputs.register_default_keybind(
        "SaveAs",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyS), KeyAction::Press)
            .with_shift()
            .with_ctrl(),
    );
    inputs.register_default_keybind(
        "ReloadShaders",
        KeyBind::new(PhysicalKey::Code(KeyCode::F1), KeyAction::Press),
    );
    inputs.register_default_keybind(
        "SnapModifier",
        KeyBind::new(PhysicalKey::Code(KeyCode::ShiftLeft), KeyAction::Hold),
    );
    inputs.register_default_keybind(
        "ShiftModifier",
        KeyBind::new(PhysicalKey::Code(KeyCode::ShiftLeft), KeyAction::Hold),
    );
    Ok(())
}

#[update(mode = "editor")]
pub fn copy_paste_entities(world: &mut World) -> Result<()> {
    if world.get_resource::<CellSearchState>().is_err() {
        world.insert_resource(CellSearchState::default());
    }

    let wants_keyboard = world
        .get_resource::<EguiContext>()
        .map(|ctx| ctx.0.egui_wants_keyboard_input())
        .unwrap_or(false);
    let inputs = world.get_resource::<InputManager>().unwrap();
    let do_copy = !wants_keyboard && inputs.is_keybind_active("Copy");
    let do_paste = !wants_keyboard && inputs.is_keybind_active("Paste");
    let do_duplicate = !wants_keyboard && inputs.is_keybind_active("Duplicate");

    let clipboard: Option<EntityBlob> = world.get_resource::<CellSearchState>()?.copied_entity.clone();
    let selected_id = world.get_resource::<CellSearchState>()?.selected_entity;
    let selected_blob: Option<EntityBlob> = selected_id.and_then(|id| world.capture_entity(id));

    if do_copy {
        if let Some(blob) = selected_blob.clone() {
            world.get_resource_mut::<CellSearchState>()?.copied_entity = Some(blob);
        }
    }

    if do_paste {
        if let Some(copied) = clipboard {
            let mut cmd = Box::new(crate::systems::history::AddEntityCmd::new(copied, None));
            cmd.execute(world)?;
            world
                .get_resource_mut::<crate::systems::history::History>()?
                .push(cmd);
        }
    } else if do_duplicate {
        if let Some(blob) = selected_blob {
            let mut cmd = Box::new(crate::systems::history::AddEntityCmd::new(blob, None));
            cmd.execute(world)?;
            world
                .get_resource_mut::<crate::systems::history::History>()?
                .push(cmd);
        }
    }

    Ok(())
}
