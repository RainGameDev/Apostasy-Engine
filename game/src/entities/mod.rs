use apostasy_core::{
    Component,
    anyhow::Result,
    cgmath::{self, InnerSpace, Vector3, Zero},
    log,
    objects::{Object, components::transform::Transform, world::World},
    physics::{Gravity, collider::Collider, velocity::Velocity},
    rand::{RngExt, rng},
    rendering::components::model_renderer::ModelRenderer,
    serde_yaml::Value,
    start, update,
    voxels::voxel_raycast::{Direction, voxel_raycast},
};

use crate::entities::spawn_point::NeedsSpawnPoint;

pub mod loading_gate;
pub mod player;
pub mod spawn_point;

#[derive(Component, Debug, Clone)]
pub struct PassiveAI {
    pub has_location: bool,
    pub location: Vector3<f32>,
}

impl Default for PassiveAI {
    fn default() -> Self {
        Self {
            has_location: false,
            location: Vector3::zero(),
        }
    }
}

impl PassiveAI {
    pub fn deserialize(&mut self, _value: &Value) -> Result<()> {
        Ok(())
    }
}

#[update]
pub fn entity_process(world: &mut World) -> Result<()> {
    let objects: Vec<_> = world
        .get_objects_with_component_with_ids::<PassiveAI>()
        .iter()
        .map(|o| o.0)
        .collect();

    for id in objects {
        let (has_location, location, global_position) = {
            let object = world.get_object(id).unwrap();
            let passive_ai = object.get_component::<PassiveAI>()?;
            let transform = object.get_component::<Transform>()?;
            (
                passive_ai.has_location,
                passive_ai.location,
                transform.global_position,
            )
        };

        if has_location {
            if (global_position - location).magnitude2() < 3.0 {
                let object = world.get_object_mut(id).unwrap();
                object.get_component_mut::<PassiveAI>()?.has_location = false;
            } else {
                let diff = location - global_position;
                let dir = if diff.magnitude2() > 0.0 {
                    diff.normalize()
                } else {
                    cgmath::Vector3::unit_x()
                };
                let object = world.get_object_mut(id).unwrap();
                object.get_component_mut::<Velocity>()?.linear_velocity.x = dir.x * 1.0;
                object.get_component_mut::<Velocity>()?.linear_velocity.z = dir.z * 1.0;
            }
        } else {
            let location_transform = {
                let object = world.get_object(id).unwrap();
                let transform = object.get_component::<Transform>()?;
                let mut rng = rng();

                let random_position_x = rng.random_range(
                    transform.global_position.x - 16.0..=transform.global_position.x + 16.0,
                );
                let random_position_z = rng.random_range(
                    transform.global_position.z - 16.0..=transform.global_position.z + 16.0,
                );

                Transform {
                    local_position: Vector3::new(random_position_x, 256.0, random_position_z),
                    global_position: Vector3::new(random_position_x, 256.0, random_position_z),
                    ..Default::default()
                }
            };

            if let Ok(hit) = voxel_raycast(world, &location_transform, 1000.0, Direction::Down) {
                let object = world.get_object_mut(id).unwrap();
                let passive_ai = object.get_component_mut::<PassiveAI>()?;
                passive_ai.has_location = true;
                passive_ai.location = Vector3::new(
                    hit.voxel_pos.x as f32,
                    hit.voxel_pos.y as f32,
                    hit.voxel_pos.z as f32,
                );
            }
        }
    }
    Ok(())
}

#[start]
pub fn entity_init(world: &mut World) -> Result<()> {
    let sheep = Object::new()
        .add_component(PassiveAI::default())
        .add_component(Transform::default())
        .add_component(Gravity::default())
        .add_component(Collider::default())
        .add_component(Velocity::default())
        // .add_component(ModelRenderer::from_path("model.glb"))
        .add_tag(NeedsSpawnPoint);

    world.add_object(sheep);

    Ok(())
}
