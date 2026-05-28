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
            local_position: Vector3::new(0.0, 2.0, 20.0),
            ..Default::default()
        })
        .add_tag(ActiveCamera)
        .add_tag(GameCamera);

    let camera = world.add_object(camera);

    let floor = Object::new()
        .add_component(Transform {
            local_scale: Vector3::new(15.0, 1.0, 15.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::static_object())
        .add_component(Collider::new_static(
            ColliderShape::Cuboid {
                size: Vector3::new(1.0, 1.0, 1.0),
            },
            Vector3::zero(),
        ));
    world.add_object(floor);

    let cube = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(4.0, 10.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::default());

    world.add_object(cube);

    let cube = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(-4.0, 15.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::default());

    world.add_object(cube);

    let sphere = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(0.0, 8.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::from_path("sphere"))
        .add_component(Velocity::default_sphere())
        .add_component(Gravity::default())
        .add_component(Collider::new(
            ColliderShape::Sphere { radius: 1.0 },
            Vector3::zero(),
        ));

    let sphere = world.add_object(sphere);
    world.set_parent(camera, Some(sphere));

    Ok(())
}
