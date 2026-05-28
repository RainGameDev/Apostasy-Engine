use std::default;

use apostasy_core::{
    anyhow::Result,
    cgmath::{Vector3, Zero},
    init_core,
    objects::{Object, components::transform::Transform, world::World},
    packages::Packages,
    physics::{
        Gravity,
        collider::{Collider, ColliderShape},
        velocity::Velocity,
    },
    rendering::{
        RenderingBackend,
        components::{
            camera::{ActiveCamera, Camera, GameCamera},
            model_renderer::ModelRenderer,
        },
    },
    start,
};

fn main() {
    init_core(
        RenderingBackend::Vulkan,
        vec![Packages::Voxel, Packages::ItemSystem],
    )
    .unwrap();
}

#[start]
pub fn start(world: &mut World) -> Result<()> {
    // world.insert_resource(ChunkLoader::default());
    // world.insert_resource(ChunkGenQueue::default());
    // world.insert_resource(LoadingState::default());
    //
    let camera = Object::new()
        .add_component(Camera::default())
        .add_component(Transform {
            local_position: Vector3::new(0.0, 0.0, 10.0),
            ..Default::default()
        })
        .add_tag(ActiveCamera)
        .add_tag(GameCamera);

    world.add_object(camera);

    let floor = Object::new()
        .add_component(Transform::default())
        .add_component(ModelRenderer::default())
        .add_component(Collider::default());
    world.add_object(floor);

    let cube = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(0.0, 5.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::default());

    world.add_object(cube);

    let cube = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(0.0, 10.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::default());

    world.add_object(cube);

    let cube = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(0.0, 15.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::default());

    world.add_object(cube);

    let sphere = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(0.0, 18.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::from_path("sphere"))
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::new(
            ColliderShape::Sphere { radius: 1.0 },
            Vector3::zero(),
        ));

    world.add_object(sphere);

    Ok(())
}
