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
            let selected_obj = world.get_object(selected_obj).unwrap().clone();
            if !selected_obj.has_component::<Transform>() {
                return Ok(());
            }
            let selected_transform = selected_obj.get_component::<Transform>()?;
            let editor_camera = world.get_object_with_tag_mut::<EditorCamera>()?;
            let transform = editor_camera.get_component_mut::<Transform>()?;

            transform.local_position = selected_transform.global_position
                - (transform.global_rotation * Vector3::new(0.0, 0.0, -10.0));
            transform.global_position = transform.local_position;

            transform.look_at(selected_transform.global_position);
        }

        if world.has_resource::<IsObjectFocused>() && middle_click {
            let selected_obj = world.get_object(selected_obj).unwrap().clone();
            let selected_transform = selected_obj.get_component::<Transform>()?;

            let editor_camera = world.get_object_with_tag_mut::<EditorCamera>()?;
            let transform = editor_camera.get_component_mut::<Transform>()?;

            let pivot = selected_transform.global_position;
            let sensitivity = 0.3_f32;

            transform.rotate_around(pivot, transform::UP, delta.0 as f32 * sensitivity);

            // local_rotation is updated by the yaw rotate_around above, so calculate_right() is current
            let right = transform.calculate_right();
            transform.rotate_around(pivot, right, delta.1 as f32 * sensitivity);

            transform.look_at(pivot);
        }
    }

    Ok(())
}
