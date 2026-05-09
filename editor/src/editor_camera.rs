use apostasy_core::{
    anyhow::Result,
    cgmath::{Vector3, num_traits::clamp},
    items::container::Container,
    objects::{
        Object,
        components::transform::Transform,
        resources::input_manager::InputManager,
        systems::DeltaTime,
        tags::{Player, skips_serilization::SkipsSerilization},
        world::World,
    },
    physics::velocity::Velocity,
    rendering::components::camera::{ActiveCamera, Camera, EditorCamera},
    start, update,
    voxels::voxel_raycast::voxel_raycast_system,
};

#[start]
pub fn start(world: &mut World) -> Result<()> {
    let cam = Object::new()
        .add_component(Velocity::default())
        .add_component(Camera::default())
        .add_component(Container::default())
        .add_component(Transform {
            local_position: Vector3::new(0.0, 18.0, 0.0),
            ..Default::default()
        })
        .add_tag(EditorCamera)
        .add_tag(ActiveCamera)
        .add_tag(Player)
        .add_tag(SkipsSerilization)
        .set_name("Camera".to_string());

    world.add_object(cam);
    Ok(())
}

#[update]
pub fn update(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;
    let inputs = world.get_resource_mut::<InputManager>()?;

    let mouse_delta = inputs.mouse_delta;
    let look_keyboard = inputs.input_vector_2d("LookRight", "LookLeft", "LookUp", "LookDown") * 5.0;
    let to_break = inputs.is_mousebind_active("Break");
    let to_place = inputs.is_mousebind_active("Place");
    let direction = inputs.input_vector_3d(
        "Right",
        "Left",
        "Upwards",
        "Downwards",
        "Backwards",
        "Forwards",
    );

    let camera = world.get_object_with_tag_mut::<EditorCamera>()?;
    let rotation = {
        let transform = camera.get_component::<Transform>()?;

        transform.global_rotation
    };

    let velocity = camera.get_component_mut::<Velocity>()?;

    velocity.linear_velocity = rotation * direction * delta * 5.0;

    let transform = camera.get_component_mut::<Transform>()?;
    transform.local_euler_angles.y -= mouse_delta.0 as f32 * 4.0;
    transform.local_euler_angles.x = clamp(
        transform.local_euler_angles.x - mouse_delta.1 as f32 * 4.0,
        -90.0,
        90.0,
    );

    transform.local_euler_angles.y -= look_keyboard.x as f32;
    transform.local_euler_angles.x = clamp(
        transform.local_euler_angles.x - look_keyboard.y as f32,
        -90.0,
        90.0,
    );

    if to_break {
        voxel_raycast_system(world, Some(0))?;
    }
    if to_place {
        voxel_raycast_system(world, Some(2))?;
    }

    Ok(())
}
