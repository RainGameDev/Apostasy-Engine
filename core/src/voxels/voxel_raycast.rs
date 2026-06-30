use crate::ecs::world::World;
use crate::physics::raycast::{Direction, Ray, get_camera_ray};
use crate::rendering::components::camera::Camera;
use crate::voxels::voxel::{VoxelId, VoxelRegistry};
use crate::{ecs::components::transform::Transform, voxels::voxel_components::is_solid::IsSolid};
use anyhow::Result;
use apostasy_macros::Resource;
use cgmath::Vector3;
use hashbrown::HashMap;

#[derive(Resource, Debug, Clone)]
pub struct RaycastHit {
    pub voxel_pos: Vector3<i32>,
    pub chunk_pos: Vector3<i32>,
    pub local_pos: Vector3<i32>,
    pub face: u8,
    pub distance: f32,
    pub set_to: Option<VoxelId>,
}

/// Voxel DDA algorithm
#[inline]
pub fn raycast_raw(
    ray: &Ray,
    max_distance: f32,
    chunk_map: &HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]>,
    set_to: Option<VoxelId>,
    voxel_registry: &VoxelRegistry,
) -> Option<RaycastHit> {
    let mut voxel = Vector3::new(
        ray.origin.x.floor() as i32,
        ray.origin.y.floor() as i32,
        ray.origin.z.floor() as i32,
    );

    let step = Vector3::new(
        if ray.direction.x >= 0.0 { 1i32 } else { -1 },
        if ray.direction.y >= 0.0 { 1i32 } else { -1 },
        if ray.direction.z >= 0.0 { 1i32 } else { -1 },
    );

    let t_delta = Vector3::new(
        if ray.direction.x.abs() < 1e-8 {
            f32::MAX
        } else {
            1.0 / ray.direction.x.abs()
        },
        if ray.direction.y.abs() < 1e-8 {
            f32::MAX
        } else {
            1.0 / ray.direction.y.abs()
        },
        if ray.direction.z.abs() < 1e-8 {
            f32::MAX
        } else {
            1.0 / ray.direction.z.abs()
        },
    );

    let mut t_max = Vector3::new(
        if ray.direction.x >= 0.0 {
            (voxel.x as f32 + 1.0 - ray.origin.x) / ray.direction.x.abs().max(1e-8)
        } else {
            (ray.origin.x - voxel.x as f32) / ray.direction.x.abs().max(1e-8)
        },
        if ray.direction.y >= 0.0 {
            (voxel.y as f32 + 1.0 - ray.origin.y) / ray.direction.y.abs().max(1e-8)
        } else {
            (ray.origin.y - voxel.y as f32) / ray.direction.y.abs().max(1e-8)
        },
        if ray.direction.z >= 0.0 {
            (voxel.z as f32 + 1.0 - ray.origin.z) / ray.direction.z.abs().max(1e-8)
        } else {
            (ray.origin.z - voxel.z as f32) / ray.direction.z.abs().max(1e-8)
        },
    );

    let mut last_face: u8 = 0;
    let mut distance = 0.0f32;

    while distance < max_distance {
        // O(1) voxel sample with no bounds check
        let id = unsafe { World::get_voxel_raw(chunk_map, voxel.x, voxel.y, voxel.z) };

        if id != 0
            && voxel_registry
                .get_def(id)
                .unwrap()
                .has_component::<IsSolid>()
        {
            return Some(RaycastHit {
                voxel_pos: voxel,
                chunk_pos: Vector3::new(voxel.x >> 5, voxel.y >> 5, voxel.z >> 5),
                local_pos: Vector3::new(voxel.x & 31, voxel.y & 31, voxel.z & 31),
                face: last_face,
                distance,
                set_to,
            });
        }

        if t_max.x < t_max.y && t_max.x < t_max.z {
            voxel.x += step.x;
            distance = t_max.x;
            t_max.x += t_delta.x;
            last_face = if step.x > 0 { 1 } else { 0 };
        } else if t_max.y < t_max.z {
            voxel.y += step.y;
            distance = t_max.y;
            t_max.y += t_delta.y;
            last_face = if step.y > 0 { 3 } else { 2 };
        } else {
            voxel.z += step.z;
            distance = t_max.z;
            t_max.z += t_delta.z;
            last_face = if step.z > 0 { 5 } else { 4 };
        }
    }

    None
}

/// Submits a raycast hit as a world resource
pub fn voxel_raycast_system(world: &mut World, set_to: Option<VoxelId>, range: f32) -> Result<()> {
    let camera_ids = world.get_entities_with_component::<Camera>();
    let camera_id = camera_ids
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("No camera"))?;
    let transform = world
        .get_component::<Transform>(camera_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No transform"))?;
    let ray = get_camera_ray(&transform, Direction::Forward);
    let chunk_map = world.build_raw_chunk_lookup();

    let registry = world.get_resource::<VoxelRegistry>()?;

    if let Some(hit) = raycast_raw(&ray, range, &chunk_map, set_to, &registry) {
        world.insert_resource(hit);
    }

    Ok(())
}

pub fn voxel_raycast(
    world: &mut World,
    transform: &Transform,
    distance: f32,
    direction: Direction,
) -> Option<RaycastHit> {
    let ray = get_camera_ray(transform, direction);
    let chunk_map = world.build_raw_chunk_lookup();
    let registry = world.get_resource::<VoxelRegistry>().ok()?;
    raycast_raw(&ray, distance, &chunk_map, None, registry)
}

pub fn voxel_raycast_camera(world: &mut World, range: f32) -> Option<RaycastHit> {
    let camera_ids = world.get_entities_with_component::<Camera>();
    let camera_id = camera_ids.first().copied()?;
    let transform = world.get_component::<Transform>(camera_id)?.clone();
    let ray = get_camera_ray(&transform, Direction::Forward);
    let chunk_map = world.build_raw_chunk_lookup();
    let registry = world.get_resource::<VoxelRegistry>().unwrap();
    raycast_raw(&ray, range, &chunk_map, None, registry)
}

pub fn voxel_raycast_with_map(
    world: &mut World,
    transform: &Transform,
    distance: f32,
    direction: Direction,
    chunk_map: &HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]>,
) -> Option<RaycastHit> {
    let ray = get_camera_ray(transform, direction);
    let registry = world.get_resource::<VoxelRegistry>().unwrap();
    raycast_raw(&ray, distance, chunk_map, None, registry)
}
