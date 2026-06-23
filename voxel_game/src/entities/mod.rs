use apostasy_core::{
    Component,
    anyhow::Result,
    cgmath::{self, InnerSpace, Vector3, Zero},
    ecs::{components::transform::Transform, world::World},
    physics::{Gravity, collider::Collider, raycast::Direction, velocity::Velocity},
    rand::{RngExt, rng},
    serde_yaml::Value,
    start, update,
    voxels::voxel_raycast::voxel_raycast,
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
    let ids: Vec<_> = world.get_entities_with_component::<PassiveAI>();

    for id in ids {
        let (has_location, location, global_position) = {
            let (hl, loc) = match world.get_component::<PassiveAI>(id) {
                Some(c) => (c.has_location, c.location),
                None => continue,
            };
            let global_pos = match world.get_component::<Transform>(id) {
                Some(t) => t.global_position,
                None => continue,
            };
            (hl, loc, global_pos)
        };

        if has_location {
            if (global_position - location).magnitude2() < 3.0 {
                if let Some(ai) = world.get_component_mut::<PassiveAI>(id) {
                    ai.has_location = false;
                }
            } else {
                let diff = location - global_position;
                let dir = if diff.magnitude2() > 0.0 {
                    diff.normalize()
                } else {
                    cgmath::Vector3::unit_x()
                };
                if let Some(vel) = world.get_component_mut::<Velocity>(id) {
                    vel.linear_velocity.x = dir.x * 1.0;
                    vel.linear_velocity.z = dir.z * 1.0;
                }
            }
        } else {
            let location_transform = {
                let transform = match world.get_component::<Transform>(id) {
                    Some(t) => t,
                    None => continue,
                };
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

            if let Some(hit) = voxel_raycast(world, &location_transform, 1000.0, Direction::Down) {
                if let Some(ai) = world.get_component_mut::<PassiveAI>(id) {
                    ai.has_location = true;
                    ai.location = Vector3::new(
                        hit.voxel_pos.x as f32,
                        hit.voxel_pos.y as f32,
                        hit.voxel_pos.z as f32,
                    );
                }
            }
        }
    }
    Ok(())
}

#[start]
pub fn entity_init(world: &mut World) -> Result<()> {
    let sheep_id = world.spawn().id();
    world.add_component(sheep_id, PassiveAI::default());
    world.add_component(sheep_id, Transform::default());
    world.add_component(sheep_id, Gravity::default());
    world.add_component(sheep_id, Collider::default());
    world.add_component(sheep_id, Velocity::default());
    world.add_tag::<NeedsSpawnPoint>(sheep_id);

    Ok(())
}
