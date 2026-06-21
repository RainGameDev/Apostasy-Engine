use std::sync::Arc;

use anyhow::Result;
use apostasy_macros::{Component, Inspect, fixed_update, update};

use crate::{
    assets::asset_manager::AssetManager,
    ecs::{cell::ObjectId, components::transform::Transform, world::World},
    physics::{collider::{Collider, ColliderShape}, velocity::Velocity},
    rendering::shared::model::Bvh,
};

pub mod collider;
pub mod collision_system;
pub mod raycast;
pub mod velocity;

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

    // Collect unresolved mesh colliders — can't hold object borrow while reading resources
    let unresolved: Vec<ObjectId> = world
        .get_objects_with_component_with_ids::<Collider>()
        .into_iter()
        .filter_map(|(id, obj)| {
            let col = obj.get_component::<Collider>().ok()?;
            if let ColliderShape::Mesh { bvh, model_path, .. } = &col.shape {
                if bvh.nodes.is_empty() && !model_path.is_empty() {
                    return Some(id);
                }
            }
            None
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
        .unwrap()
        .paths
        .clone();

    for id in unresolved {
        let (model_path, scale) = {
            let obj = world.get_object(id).unwrap();
            let col = obj.get_component::<Collider>().unwrap();
            let t = obj.get_component::<Transform>().unwrap();
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

                if let Some(obj) = world.get_object_mut(id) {
                    if let Ok(col) = obj.get_component_mut::<Collider>() {
                        if let ColliderShape::Mesh { bvh: b, triangles: t, .. } = &mut col.shape {
                            *b = world_bvh;
                            *t = triangles;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[fixed_update(priority = 10)]
pub fn apply_gravity(world: &mut World, delta: f32) -> Result<()> {
    for object in world.get_objects_with_component_mut::<Velocity>() {
        if let Ok(gravity) = object.get_component::<Gravity>().cloned() {
            let velocity = object.get_component_mut::<Velocity>()?;
            if velocity.is_grounded {
                if velocity.linear_velocity.y < 0.0 {
                    velocity.linear_velocity.y = 0.0;
                }
            } else {
                let gravity = gravity.strength;
                let mut fall_accel = gravity;
                if velocity.linear_velocity.y < 0.0 {
                    fall_accel *= 1.8;
                }
                velocity.linear_velocity.y -= fall_accel * delta;
                velocity.linear_velocity.y = velocity.linear_velocity.y.max(-50.0);
            }
        }
    }
    Ok(())
}
