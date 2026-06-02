use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    objects::systems::DeltaTime,
    objects::{
        components::transform::Transform, resources::input_manager::InputManager, world::World,
    },
    rendering::components::camera::EditorCamera,
    update,
};

use crate::ui::viewport_panel::ViewportInfo;

#[update(mode = "editor")]
pub fn editor_camera_move(world: &mut World) -> Result<()> {
    if !world.has_resource::<ViewportInfo>() {
        world.insert_resource(ViewportInfo::default());
    }
    let viewport_info = world.get_resource::<ViewportInfo>()?;

    let inputs = world.get_resource::<InputManager>().unwrap();
    let is_looking = inputs.is_mousebind_active("RightMouseClick");

    if !viewport_info.is_hovered || !is_looking {
        return Ok(());
    }

    let mouse_delta = inputs.mouse_delta;
    let direction = inputs.input_vector_2d("Left", "Right", "Backwards", "Forwards");
    let delta = world.get_resource::<DeltaTime>()?.0;
    let camera = world.get_object_with_tag_mut::<EditorCamera>()?;
    let cam_transform = camera.get_component_mut::<Transform>()?;

    cam_transform.local_euler_angles.x -= mouse_delta.1 as f32;
    cam_transform.local_euler_angles.x = cam_transform.local_euler_angles.x.clamp(-89.0, 89.0);
    cam_transform.local_euler_angles.y -= mouse_delta.0 as f32;

    let current_transform = cam_transform.clone();
    let wish_dir = current_transform.global_rotation * Vector3::new(direction.x, 0.0, direction.y);
    cam_transform.local_position += wish_dir * 300.0 * delta;
    dbg!(&cam_transform.local_position);

    Ok(())
}
