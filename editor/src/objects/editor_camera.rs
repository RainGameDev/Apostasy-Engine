use apostasy_core::{
    anyhow::Result,
    cgmath::{Vector3, Zero},
    objects::{
        components::transform::Transform, resources::input_manager::InputManager, world::World,
    },
    physics::velocity::Velocity,
    rendering::components::camera::EditorCamera,
    update,
};

#[update]
pub fn editor_camera_move(world: &mut World) -> Result<()> {
    let inputs = world.get_resource::<InputManager>().unwrap();
    let mouse_delta = inputs.mouse_delta;
    let direction = inputs.input_vector_2d("Left", "Right", "Backwards", "Forwards");

    if inputs.is_mousebind_active("RightMouseClick") {
        let camera = world.get_object_with_tag_mut::<EditorCamera>()?;
        let cam_transform = camera.get_component_mut::<Transform>()?;
        cam_transform.local_euler_angles.x -= mouse_delta.1 as f32;
        cam_transform.local_euler_angles.x = cam_transform.local_euler_angles.x.clamp(-89.0, 89.0);
        cam_transform.local_euler_angles.y -= mouse_delta.0 as f32;

        let cam_transform = camera.get_component::<Transform>()?.clone();
        let velocity = camera.get_component_mut::<Velocity>()?;

        let wish_dir = cam_transform.global_rotation * Vector3::new(direction.x, 0.0, direction.y);
        velocity.linear_velocity.x = wish_dir.x * 3.0;
        velocity.linear_velocity.y = wish_dir.y * 3.0;
        velocity.linear_velocity.z = wish_dir.z * 3.0;
    } else {
        let camera = world.get_object_with_tag_mut::<EditorCamera>()?;
        let velocity = camera.get_component_mut::<Velocity>()?;
        velocity.linear_velocity = Vector3::zero();
    }

    Ok(())
}
