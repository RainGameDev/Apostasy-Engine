use apostasy_core::{
    anyhow::Result,
    objects::{Object, components::transform::Transform, world::World},
    physics::velocity::Velocity,
    rendering::components::camera::{ActiveCamera, Camera},
    start,
};

#[start]
pub fn editor_camera_init(world: &mut World) -> Result<()> {
    let editor_camera = Object::new()
        .add_component(Transform::default())
        .add_component(Camera::default())
        .add_component(Velocity::default())
        .add_tag(ActiveCamera);

    world.add_object(editor_camera);

    Ok(())
}
