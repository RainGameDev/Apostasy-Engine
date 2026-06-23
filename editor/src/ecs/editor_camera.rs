use apostasy_core::{
    anyhow::Result,
    cgmath::{Vector3, num_traits::clamp},
    ecs::{
        components::transform::Transform, resources::input_manager::InputManager,
        systems::DeltaTime, world::World,
    },
    rendering::components::camera::EditorCamera,
    update,
};
use apostasy_macros::Resource;

use crate::{
    systems::entity_focus::IsEntityFocused,
    terrain::TerrainToolState,
    ui::{preferences_panel::EditorPreferences, viewport_panel::ViewportInfo},
};

#[derive(Resource, Clone)]
pub struct EditorCameraSettings {
    pub move_speed: f32,
}

impl Default for EditorCameraSettings {
    fn default() -> Self {
        Self { move_speed: 5.0 }
    }
}

#[update(mode = "editor", priority = 2)]
pub fn editor_camera_move(world: &mut World) -> Result<()> {
    if !world.has_resource::<ViewportInfo>() {
        world.insert_resource(ViewportInfo::default());
    }
    let viewport_info = world.get_resource::<ViewportInfo>()?.clone();

    if !world.has_resource::<EditorCameraSettings>() {
        world.insert_resource(EditorCameraSettings::default());
    }
    let editor_camera_settings = world.get_resource::<EditorCameraSettings>()?.clone();

    if !world.has_resource::<TerrainToolState>() {
        world.insert_resource(TerrainToolState::default());
    }
    let state = world.get_resource::<TerrainToolState>()?.clone();

    let is_entity_focused = world.has_resource::<IsEntityFocused>();

    // inputs
    let inputs = world.get_resource::<InputManager>().unwrap();
    let is_looking = inputs.is_mousebind_active("RightMouseClick") && !state.active
        || inputs.is_mousebind_active("MiddleMouseClick");
    let is_middle_mouse = inputs.is_mousebind_active("MiddleMouseClick");
    let mouse_delta = inputs.mouse_delta;
    let scroll_delta = inputs.scroll_delta;
    let direction = inputs.input_vector_2d("Left", "Right", "Backwards", "Forwards");
    let delta = world.get_resource::<DeltaTime>()?.0;

    // camera
    let Ok(cam_id) = world.get_entity_with_tag::<EditorCamera>() else {
        return Ok(());
    };

    {
        let cam_transform = world.get_component_mut::<Transform>(cam_id)
            .ok_or_else(|| anyhow::anyhow!("EditorCamera has no Transform"))?;

        if is_middle_mouse {
            if !is_entity_focused {
                cam_transform.local_euler_angles.x -= mouse_delta.1 as f32;
                cam_transform.local_euler_angles.x =
                    cam_transform.local_euler_angles.x.clamp(-89.0, 89.0);
                cam_transform.local_euler_angles.y -= mouse_delta.0 as f32;
            }

            let current_transform = cam_transform.clone();
            let wish_dir =
                current_transform.global_rotation * Vector3::new(scroll_delta.0, 0.0, scroll_delta.1);
            cam_transform.local_position -= wish_dir * 3000.0 * delta;
        }

        if !viewport_info.is_hovered || !is_looking {
            return Ok(());
        }

        cam_transform.local_euler_angles.x -= mouse_delta.1 as f32;
        cam_transform.local_euler_angles.x = cam_transform.local_euler_angles.x.clamp(-89.0, 89.0);
        cam_transform.local_euler_angles.y -= mouse_delta.0 as f32;

        let current_transform = cam_transform.clone();
        let wish_dir = current_transform.global_rotation * Vector3::new(direction.x, 0.0, direction.y);
        cam_transform.local_position += wish_dir * editor_camera_settings.move_speed * delta;
    }

    world.remove_resource::<IsEntityFocused>();

    if is_looking && scroll_delta.1 != 0.0 {
        let new_speed = clamp(
            world.get_resource::<EditorCameraSettings>()?.move_speed + scroll_delta.1 * 5.0,
            1.0,
            256.0,
        );
        world.get_resource_mut::<EditorCameraSettings>()?.move_speed = new_speed;
        EditorPreferences::save_camera_speed(new_speed);
    }

    Ok(())
}
