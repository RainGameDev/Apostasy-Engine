use std::sync::Arc;

use anyhow::Result;
use apostasy_macros::{Component, Inspect, Resource, fixed_update, update};

use crate::{
    assets::asset_manager::AssetManager,
    ecs::{cell::EntityId, components::transform::Transform, tags::Player, world::World},
    physics::{collider::{Collider, ColliderShape}, velocity::Velocity},
    rendering::shared::model::Bvh,
};

pub mod collider;
pub mod collision_system;
pub mod raycast;
pub mod velocity;

/// When `true`, all colliders are ignored and physics (other than the
/// player's own velocity) stops processing, letting the player free-float
/// through the world. Toggled by the `tcl` console command.
#[derive(Resource, Clone, Default)]
pub struct Noclip(pub bool);

/// When `true`, every `Collider` gets a wireframe debug visual showing its shape.
/// Toggled by the `trc` console command.
#[derive(Resource, Clone, Default)]
pub struct ColliderRenderDebug(pub bool);

#[derive(Component, Inspect, Clone, Debug)]
pub struct Gravity {
    pub strength: f32,
}

impl Default for Gravity {
    fn default() -> Self {
        Self { strength: 9.81 }
    }
}

impl Gravity {
    pub fn serialize(&self) -> Option<serde_yaml::Value> {
        let mut map = serde_yaml::Mapping::new();
        map.insert("type".into(), "Gravity".into());
        map.insert("strength".into(), (self.strength as f64).into());
        Some(serde_yaml::Value::Mapping(map))
    }

    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        if let Some(v) = value.get("strength").and_then(|v| v.as_f64()) {
            self.strength = v as f32;
        }
        Ok(())
    }
}
#[update(priority = 1, mode = "all")]
pub fn resolve_mesh_colliders(world: &mut World) -> Result<()> {
    if !world.has_resource::<AssetManager>() {
        return Ok(());
    }

    // Collect unresolved mesh colliders — can't hold entity borrow while reading resources
    let unresolved: Vec<EntityId> = world
        .get_entities_with_component::<Collider>()
        .into_iter()
        .filter(|&id| {
            if let Some(col) = world.get_component::<Collider>(id) {
                if let ColliderShape::Mesh { bvh, model_path, .. } = &col.shape {
                    return bvh.nodes.is_empty() && !model_path.is_empty();
                }
            }
            false
        })
        .collect();

    if unresolved.is_empty() {
        return Ok(());
    }

    let registry = world
        .get_resource::<AssetManager>()?
        .model_loader
        .registry
        .read()
        .paths
        .clone();

    for id in unresolved {
        let (model_path, scale) = {
            let col = match world.get_component::<Collider>(id) {
                Some(c) => c,
                None => continue,
            };
            let t = match world.get_component::<Transform>(id) {
                Some(t) => t,
                None => continue,
            };
            let path = match &col.shape {
                ColliderShape::Mesh { model_path, .. } => model_path.clone(),
                _ => continue,
            };
            (path, t.global_scale)
        };

        if let Some(gpu_model) = registry.get(&model_path) {
            if let Some(local_bvh) = &gpu_model.collision_bvh {
                // Bake only scale into the BVH — position/rotation are applied at query time
                let scaled_triangles: Vec<[cgmath::Vector3<f32>; 3]> = local_bvh
                    .triangles
                    .iter()
                    .map(|tri| {
                        tri.map(|v| {
                            cgmath::Vector3::new(v.x * scale.x, v.y * scale.y, v.z * scale.z)
                        })
                    })
                    .collect();

                let world_bvh = Arc::new(Bvh::build(scaled_triangles.clone()));
                let triangles = Arc::new(scaled_triangles);

                if let Some(col) = world.get_component_mut::<Collider>(id) {
                    if let ColliderShape::Mesh { bvh: b, triangles: t, .. } = &mut col.shape {
                        *b = world_bvh;
                        *t = triangles;
                    }
                }
            }
        }
    }

    Ok(())
}

#[fixed_update(priority = 10)]
pub fn apply_gravity(world: &mut World, delta: f32) -> Result<()> {
    let noclip = world.get_resource::<Noclip>().map(|n| n.0).unwrap_or(false);
    let ids = world.get_entities_with_component::<Velocity>();
    for id in ids {
        if noclip && !world.has_tag::<Player>(id) {
            continue;
        }
        let gravity_strength = world.get_component::<Gravity>(id).map(|g| g.strength);
        if let Some(gravity) = gravity_strength {
            if let Some(velocity) = world.get_component_mut::<Velocity>(id) {
                if velocity.is_grounded {
                    if velocity.linear_velocity.y < 0.0 {
                        velocity.linear_velocity.y = 0.0;
                    }
                } else {
                    let mut fall_accel = gravity;
                    if velocity.linear_velocity.y < 0.0 {
                        fall_accel *= 1.8;
                    }
                    velocity.linear_velocity.y -= fall_accel * delta;
                    velocity.linear_velocity.y = velocity.linear_velocity.y.max(-50.0);
                }
            }
        }
    }
    Ok(())
}
