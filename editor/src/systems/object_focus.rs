use anyhow::Result;
use apostasy_core::{
    cgmath::Vector3,
    ecs::{
        components::transform::{self, Transform},
        resources::input_manager::InputManager,
        world::World,
    },
    rendering::components::camera::EditorCamera,
    ui::ui_context::EguiContext,
    update,
};
use apostasy_macros::Resource;

#[derive(Resource, Clone)]
pub struct IsObjectFocused;

use crate::ui::cell_panel::CellSearchState;

#[update(mode = "editor")]
pub fn object_focus(world: &mut World) -> Result<()> {
    let inputs = world.get_resource::<InputManager>().unwrap();
    let delta = inputs.mouse_delta;
    let middle_click = inputs.is_mousebind_active("MiddleMouseClick");
    if !world.has_resource::<CellSearchState>() {
        return Ok(());
    }
    if let Some(selected_obj) = world
        .get_resource::<CellSearchState>()
        .unwrap()
        .selected_obj
    {
        if let Ok(ctx) = world.get_resource::<EguiContext>() {
            if ctx.0.egui_wants_keyboard_input() {
                return Ok(());
            }
        }
        if inputs.is_keybind_active("FocusObject") {
            world.insert_resource(IsObjectFocused);
            if !world.has_component::<Transform>(selected_obj) {
                return Ok(());
            }
            let selected_global_pos = world.get_component::<Transform>(selected_obj)
                .map(|t| t.global_position);
            if let Some(sel_pos) = selected_global_pos {
                let cam_id = world.get_entity_with_tag::<EditorCamera>()?;
                let transform = world.get_component_mut::<Transform>(cam_id)
                    .ok_or_else(|| anyhow::anyhow!("EditorCamera has no Transform"))?;
                transform.local_position = sel_pos
                    - (transform.global_rotation * Vector3::new(0.0, 0.0, -10.0));
                transform.global_position = transform.local_position;
                transform.look_at(sel_pos);
            }
        }

        if world.has_resource::<IsObjectFocused>() && middle_click {
            let sel_pos = world.get_component::<Transform>(selected_obj)
                .map(|t| t.global_position);
            if let Some(pivot) = sel_pos {
                let cam_id = world.get_entity_with_tag::<EditorCamera>()?;
                let transform = world.get_component_mut::<Transform>(cam_id)
                    .ok_or_else(|| anyhow::anyhow!("EditorCamera has no Transform"))?;
                let sensitivity = 0.3_f32;
                transform.rotate_around(pivot, transform::UP, delta.0 as f32 * sensitivity);
                let right = transform.calculate_right();
                transform.rotate_around(pivot, right, delta.1 as f32 * sensitivity);
                transform.look_at(pivot);
            }
        }
    }

    Ok(())
}
