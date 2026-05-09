use anyhow::Result;
use apostasy_macros::update;
use cgmath::{Vector3, Zero};

use crate::{
    objects::{
        components::transform::Transform, scene::ObjectId, systems::DeltaTime, world::World,
    },
    physics::{collider::Collider, velocity::Velocity},
    voxels::{voxel::VoxelRegistry, voxel_components::is_solid::IsSolid},
};

#[derive(Default)]
pub struct CollisionFlags {
    pub grounded: bool,
    pub hit_ceil: bool,
    pub hit_wall: bool,
}

pub struct ColliderData {
    id: ObjectId,
    position: Vector3<f32>,
    half_extents: Vector3<f32>,
}

#[update]
pub fn voxel_collision_system(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    let collider_data: Vec<ColliderData> = world
        .get_objects_with_component_with_ids::<Collider>()
        .iter()
        .filter_map(|(id, obj)| {
            let half_extents = obj.get_component::<Collider>().ok()?.half_extents;
            let transform = obj.get_component::<Transform>().ok()?;
            let scale = transform.global_scale;
            let position = transform.global_position;
            Some(ColliderData {
                id: *id,
                position,
                half_extents: Vector3::new(
                    half_extents.x * scale.x,
                    half_extents.y * scale.y,
                    half_extents.z * scale.z,
                ),
            })
        })
        .collect();

    let registry = world.get_resource::<VoxelRegistry>()?.clone();

    for data in collider_data {
        let velocity_snapshot = world
            .get_object(data.id)
            .and_then(|o| o.get_component::<Velocity>().ok())
            .map(|v| v.linear_velocity)
            .unwrap_or(Vector3::zero());

        let current_pos = data.position + velocity_snapshot * delta;

        if let Some(obj) = world.get_object_mut(data.id) {
            if let Ok(t) = obj.get_component_mut::<Transform>() {
                t.local_position = current_pos;
                t.global_position = current_pos;
            }
        }

        let min = current_pos - data.half_extents;
        let max = current_pos + data.half_extents;

        let min_vox = Vector3::new(
            min.x.floor() as i32 - 1,
            min.y.floor() as i32 - 1,
            min.z.floor() as i32 - 1,
        );
        let max_vox = Vector3::new(
            max.x.ceil() as i32 + 1,
            max.y.ceil() as i32 + 1,
            max.z.ceil() as i32 + 1,
        );

        let mut total_correction = Vector3::zero();
        let mut grounded = false;

        for vx in min_vox.x..max_vox.x {
            for vy in min_vox.y..max_vox.y {
                for vz in min_vox.z..max_vox.z {
                    // get the voxel info
                    let voxel_id = match world.get_voxel(vx, vy, vz) {
                        Some(id) if id != 0 => id,
                        _ => continue,
                    };
                    let def = match registry.get_def(voxel_id) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };

                    // if its not a solid voxel return as it doesn't have collision
                    if !def.has_component::<IsSolid>() {
                        continue;
                    }

                    // calculate the extents of the object and voxel
                    let pos = current_pos + total_correction;
                    let cur_min = pos - data.half_extents;
                    let cur_max = pos + data.half_extents;

                    let vox_min = Vector3::new(vx as f32, vy as f32, vz as f32);
                    let vox_max = vox_min + Vector3::new(1.0, 1.0, 1.0);

                    // detects overlap between the object and voxels collider
                    let overlap_x = (cur_max.x.min(vox_max.x) - cur_min.x.max(vox_min.x)).max(0.0);
                    let overlap_y = (cur_max.y.min(vox_max.y) - cur_min.y.max(vox_min.y)).max(0.0);
                    let overlap_z = (cur_max.z.min(vox_max.z) - cur_min.z.max(vox_min.z)).max(0.0);

                    // detect if theres no overlap
                    if overlap_x <= 0.0 || overlap_y <= 0.0 || overlap_z <= 0.0 {
                        continue;
                    }

                    let vox_center = vox_min + Vector3::new(0.5, 0.5, 0.5);

                    // collision correction maths, determines how far to push the object out of the
                    // collider
                    if overlap_y <= overlap_x && overlap_y <= overlap_z {
                        if pos.y > vox_center.y {
                            total_correction.y += overlap_y;
                            let feet = pos.y - data.half_extents.y;

                            // grounded detection 1
                            if (feet - vox_max.y).abs() < 0.2 {
                                grounded = true;
                            }
                        } else {
                            total_correction.y -= overlap_y;
                        }
                    } else if overlap_x <= overlap_z {
                        if pos.x > vox_center.x {
                            total_correction.x += overlap_x;
                        } else {
                            total_correction.x -= overlap_x;
                        }
                    } else {
                        if pos.z > vox_center.z {
                            total_correction.z += overlap_z;
                        } else {
                            total_correction.z -= overlap_z;
                        }
                    }
                }
            }
        }

        // Calculations for determing where the "feet" of an entity are
        let feet_pos = current_pos + total_correction;
        let feet_y = feet_pos.y - data.half_extents.y;
        let foot_vox_y = feet_y.floor() as i32 - 1;

        let foot_min_x = (feet_pos.x - data.half_extents.x).floor() as i32;
        let foot_max_x = (feet_pos.x + data.half_extents.x).ceil() as i32;
        let foot_min_z = (feet_pos.z - data.half_extents.z).floor() as i32;
        let foot_max_z = (feet_pos.z + data.half_extents.z).ceil() as i32;

        // detect if the feet are on the ground
        for fx in foot_min_x..foot_max_x {
            for fz in foot_min_z..foot_max_z {
                if let Some(voxel_id) = world.get_voxel(fx, foot_vox_y, fz) {
                    if voxel_id == 0 {
                        continue;
                    }
                    if let Ok(def) = registry.get_def(voxel_id) {
                        if def.has_component::<IsSolid>() {
                            let vox_top = foot_vox_y as f32 + 1.0;
                            if (feet_y - vox_top).abs() < 0.1 {
                                // if the feet are on the ground then the entity is grounded
                                grounded = true;
                            }
                        }
                    }
                }
            }
        }

        // resolve the colisions pushing the entity out of the ground/walls
        if let Some(obj) = world.get_object_mut(data.id) {
            if total_correction != Vector3::zero() {
                if let Ok(t) = obj.get_component_mut::<Transform>() {
                    t.local_position += total_correction;
                    t.global_position += total_correction;
                }
            }
            if let Ok(v) = obj.get_component_mut::<Velocity>() {
                v.is_grounded = grounded;
                if total_correction.y > 0.0 && v.linear_velocity.y < 0.0 {
                    v.linear_velocity.y = 0.0;
                    v.is_grounded = true;
                }
                if total_correction.y < 0.0 && v.linear_velocity.y > 0.0 {
                    v.linear_velocity.y = 0.0;
                }
                if total_correction.x.abs() > 0.0 {
                    v.linear_velocity.x = 0.0;
                }
                if total_correction.z.abs() > 0.0 {
                    v.linear_velocity.z = 0.0;
                }
            }
        }
    }

    Ok(())
}

pub fn resolve_object_collisions(
    position: &mut Vector3<f32>,
    delta: &mut Vector3<f32>,
    half_extents: Vector3<f32>,
    self_id: ObjectId,
    other_colliders: &[(ObjectId, Vector3<f32>, Vector3<f32>)],
) -> CollisionFlags {
    let mut flags = CollisionFlags::default();

    let axes: [usize; 3] = [1, 0, 2];

    for axis in axes {
        let axis_vel = match axis {
            0 => delta.x,
            1 => delta.y,
            _ => delta.z,
        };

        if axis_vel.abs() < 1e-6 {
            continue;
        }

        let axis_delta = match axis {
            0 => Vector3::new(delta.x, 0.0, 0.0),
            1 => Vector3::new(0.0, delta.y, 0.0),
            _ => Vector3::new(0.0, 0.0, delta.z),
        };

        let candidate = *position + axis_delta;
        let a_min = candidate - half_extents;
        let a_max = candidate + half_extents;

        let mut best_overlap = 0.0f32;

        for (id, other_pos, other_half) in other_colliders {
            if *id == self_id {
                continue;
            }

            let b_min = other_pos - other_half;
            let b_max = other_pos + other_half;

            let overlap_x = a_min.x < b_max.x && a_max.x > b_min.x;
            let overlap_y = a_min.y < b_max.y && a_max.y > b_min.y;
            let overlap_z = a_min.z < b_max.z && a_max.z > b_min.z;

            if !overlap_x || !overlap_y || !overlap_z {
                continue;
            }

            let (ca_min, ca_max, cb_min, cb_max) = match axis {
                0 => (a_min.x, a_max.x, b_min.x, b_max.x),
                1 => (a_min.y, a_max.y, b_min.y, b_max.y),
                _ => (a_min.z, a_max.z, b_min.z, b_max.z),
            };

            let overlap = if axis_vel > 0.0 {
                cb_min - ca_max
            } else {
                cb_max - ca_min
            };

            let is_penetrating =
                (axis_vel > 0.0 && overlap < 0.0) || (axis_vel < 0.0 && overlap > 0.0);

            if !is_penetrating {
                continue;
            }

            if overlap.abs() > best_overlap.abs() {
                best_overlap = overlap;
            }
        }

        if best_overlap.abs() < 1e-6 {
            *position += axis_delta;
            continue;
        }

        let correction = if axis_vel > 0.0 {
            best_overlap.max(-axis_vel.abs())
        } else {
            best_overlap.min(axis_vel.abs())
        };

        match axis {
            0 => {
                position.x += axis_delta.x + correction;
                flags.hit_wall = true;
            }
            1 => {
                position.y += axis_delta.y + correction;
                delta.y = 0.0;
                if axis_vel < 0.0 {
                    flags.grounded = true;
                } else {
                    flags.hit_ceil = true;
                }
            }
            _ => {
                position.z += axis_delta.z + correction;
                flags.hit_wall = true;
            }
        }
    }

    flags
}

// #[update]
pub fn resolve_object_collisions_system(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    let collider_snapshot: Vec<(ObjectId, Vector3<f32>, Vector3<f32>)> = world
        .get_objects_with_component_with_ids::<Collider>()
        .iter()
        .filter_map(|(id, obj)| {
            let transform = obj.get_component::<Transform>().ok()?;
            let collider = obj.get_component::<Collider>().ok()?;
            let scaled = Vector3::new(
                collider.half_extents.x * transform.global_scale.x,
                collider.half_extents.y * transform.global_scale.y,
                collider.half_extents.z * transform.global_scale.z,
            );
            Some((id.clone(), transform.global_position, scaled))
        })
        .collect();

    let mut objects = world.get_objects_with_component_mut::<Collider>();
    for (i, object) in objects.iter_mut().enumerate() {
        let Ok(collider) = object.get_component::<Collider>() else {
            continue;
        };
        let Ok(transform) = object.get_component::<Transform>() else {
            continue;
        };
        let Ok(velocity) = object.get_component::<Velocity>() else {
            continue;
        };

        let scaled_half = Vector3::new(
            collider.half_extents.x * transform.global_scale.x,
            collider.half_extents.y * transform.global_scale.y,
            collider.half_extents.z * transform.global_scale.z,
        );

        let self_id = collider_snapshot[i].0;
        let mut position = transform.global_position;
        let mut frame_delta = velocity.linear_velocity * delta;

        let flags = resolve_object_collisions(
            &mut position,
            &mut frame_delta,
            scaled_half,
            self_id,
            &collider_snapshot,
        );

        if flags.hit_wall || flags.grounded || flags.hit_ceil {
            object.get_component_mut::<Transform>()?.global_position = position;
            object.get_component_mut::<Transform>()?.local_position = position;
        }

        let vel = object.get_component_mut::<Velocity>()?;
        if flags.grounded {
            vel.is_grounded = true;
        }
    }

    Ok(())
}
