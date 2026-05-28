use anyhow::Result;
use cgmath::{Vector3, Zero};

use crate::{
    objects::{
        components::transform::Transform, scene::ObjectId, systems::DeltaTime, world::World,
    },
    physics::{collider::Collider, velocity::Velocity},
    voxels::{voxel::VoxelRegistry, voxel_components::is_solid::IsSolid},
};

pub fn voxel_collision_system(world: &mut World) -> Result<()> {
    if !world.has_resource::<VoxelRegistry>() {
        return Ok(());
    }

    let registry = world.get_resource::<VoxelRegistry>()?.clone();
    let delta = world.get_resource::<DeltaTime>()?.0;

    // Snapshot all collidable objects up front to avoid borrow conflicts
    struct CollidableSnapshot {
        id: ObjectId,
        position: Vector3<f32>,
        half_extents: Vector3<f32>,
        linear_velocity: Vector3<f32>,
        process: bool,
    }

    let snapshots: Vec<CollidableSnapshot> = world
        .get_objects_with_component_with_ids::<Collider>()
        .into_iter()
        .filter_map(|(id, obj)| {
            let transform = obj.get_component::<Transform>().ok()?;
            let collider = obj.get_component::<Collider>().ok()?;
            let velocity = obj.get_component::<Velocity>().ok()?;
            let scale = transform.global_scale;
            let half = collider.half_extents();
            Some(CollidableSnapshot {
                id,
                position: transform.global_position,
                half_extents: Vector3::new(half.x * scale.x, half.y * scale.y, half.z * scale.z),
                linear_velocity: velocity.linear_velocity,
                process: velocity.process,
            })
        })
        .collect();

    for snap in snapshots {
        if !snap.process {
            continue;
        }

        // The velocity system already moves the object; we just need the
        // post-move position to run collision resolution against voxels.
        let current_pos = snap.position;
        let half = snap.half_extents;

        let min = current_pos - half;
        let max = current_pos + half;

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

        let mut total_correction: Vector3<f32> = Vector3::zero();
        let mut grounded = false;

        for vx in min_vox.x..max_vox.x {
            for vy in min_vox.y..max_vox.y {
                for vz in min_vox.z..max_vox.z {
                    let voxel_id = match world.get_voxel(vx, vy, vz) {
                        Some(id) if id != 0 => id,
                        _ => continue,
                    };
                    let def = match registry.get_def(voxel_id) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    if !def.has_component::<IsSolid>() {
                        continue;
                    }

                    // Use accumulated correction so each voxel sees the already-corrected position
                    let pos = current_pos + total_correction;
                    let cur_min = pos - half;
                    let cur_max = pos + half;

                    let vox_min = Vector3::new(vx as f32, vy as f32, vz as f32);
                    let vox_max = vox_min + Vector3::new(1.0, 1.0, 1.0);

                    let overlap_x = (cur_max.x.min(vox_max.x) - cur_min.x.max(vox_min.x)).max(0.0);
                    let overlap_y = (cur_max.y.min(vox_max.y) - cur_min.y.max(vox_min.y)).max(0.0);
                    let overlap_z = (cur_max.z.min(vox_max.z) - cur_min.z.max(vox_min.z)).max(0.0);

                    if overlap_x <= 0.0 || overlap_y <= 0.0 || overlap_z <= 0.0 {
                        continue;
                    }

                    let vox_center = vox_min + Vector3::new(0.5, 0.5, 0.5);

                    if overlap_y <= overlap_x && overlap_y <= overlap_z {
                        if pos.y > vox_center.y {
                            total_correction.y += overlap_y;
                            let feet = pos.y - half.y;
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
                    } else if pos.z > vox_center.z {
                        total_correction.z += overlap_z;
                    } else {
                        total_correction.z -= overlap_z;
                    }
                }
            }
        }

        // Secondary ground check: scan the voxel just below the feet
        let feet_pos = current_pos + total_correction;
        let feet_y = feet_pos.y - half.y;
        let foot_vox_y = feet_y.floor() as i32 - 1;
        let foot_min_x = (feet_pos.x - half.x).floor() as i32;
        let foot_max_x = (feet_pos.x + half.x).ceil() as i32;
        let foot_min_z = (feet_pos.z - half.z).floor() as i32;
        let foot_max_z = (feet_pos.z + half.z).ceil() as i32;

        for fx in foot_min_x..foot_max_x {
            for fz in foot_min_z..foot_max_z {
                if let Some(voxel_id) = world.get_voxel(fx, foot_vox_y, fz) {
                    if voxel_id == 0 {
                        continue;
                    }
                    if let Ok(def) = registry.get_def(voxel_id)
                        && def.has_component::<IsSolid>()
                    {
                        let vox_top = foot_vox_y as f32 + 1.0;
                        if (feet_y - vox_top).abs() < 0.1 {
                            grounded = true;
                        }
                    }
                }
            }
        }

        // Apply correction and velocity cancellation
        if let Some(obj) = world.get_object_mut(snap.id) {
            if total_correction != Vector3::zero() {
                if let Ok(t) = obj.get_component_mut::<Transform>() {
                    t.global_position += total_correction;
                }
            }

            if let Ok(v) = obj.get_component_mut::<Velocity>() {
                v.is_grounded = grounded;

                if total_correction.y > 0.05 && v.linear_velocity.y < 0.0 {
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
