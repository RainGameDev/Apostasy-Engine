use crate::objects::components::transform::Transform;
use crate::objects::world::World;
use crate::rendering::components::camera::Camera;
use crate::voxels::voxel::VoxelId;
use anyhow::{Error, Result};
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

pub struct Ray {
    pub origin: Vector3<f32>,
    pub direction: Vector3<f32>,
}

impl Ray {
    pub fn new(origin: Vector3<f32>, direction: Vector3<f32>) -> Self {
        let len =
            (direction.x * direction.x + direction.y * direction.y + direction.z * direction.z)
                .sqrt();
        Self {
            origin,
            direction: Vector3::new(direction.x / len, direction.y / len, direction.z / len),
        }
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum Direction {
    Forward,
    Backwards,
    Left,
    Right,
    Up,
    Down,
}

#[inline(always)]
pub fn get_camera_ray(transform: &Transform, direction: Direction) -> Ray {
    match direction {
        Direction::Forward => Ray::new(
            transform.global_position,
            transform.calculate_global_forward(),
        ),
        Direction::Backwards => Ray::new(
            transform.global_position,
            -transform.calculate_global_forward(),
        ),
        Direction::Left => Ray::new(
            transform.global_position,
            transform.calculate_global_right(),
        ),
        Direction::Right => Ray::new(
            transform.global_position,
            -transform.calculate_global_right(),
        ),
        Direction::Up => Ray::new(transform.global_position, transform.calculate_global_up()),
        Direction::Down => Ray::new(transform.global_position, -transform.calculate_global_up()),
    }
}

/// Voxel DDA algorithm
#[inline]
pub fn raycast_raw(
    ray: &Ray,
    max_distance: f32,
    chunk_map: &HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]>,
    set_to: Option<VoxelId>,
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

        if id != 0 {
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
    let camera_obj = world
        .get_objects_with_component::<Camera>()
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("No camera"))?;

    let transform = camera_obj.get_component::<Transform>()?.clone();
    let ray = get_camera_ray(&transform, Direction::Forward);
    let chunk_map = world.build_raw_chunk_lookup();

    if let Some(hit) = raycast_raw(&ray, range, &chunk_map, set_to) {
        world.insert_resource(hit);
    }

    Ok(())
}

/// Raycast from a transform in a given direction, returns the hit
pub fn voxel_raycast(
    world: &mut World,
    transform: &Transform,
    distance: f32,
    direction: Direction,
) -> Result<RaycastHit> {
    let ray = get_camera_ray(transform, direction);
    let chunk_map = world.build_raw_chunk_lookup();
    raycast_raw(&ray, distance, &chunk_map, None).ok_or_else(|| Error::msg("Hit nothing"))
}

/// Raycast from the camera forward, returns the hit
pub fn voxel_raycast_camera(world: &mut World, range: f32) -> Result<RaycastHit> {
    let camera_obj = world
        .get_objects_with_component::<Camera>()
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("No camera"))?;

    let transform = camera_obj.get_component::<Transform>()?.clone();
    let ray = get_camera_ray(&transform, Direction::Forward);
    let chunk_map = world.build_raw_chunk_lookup();
    raycast_raw(&ray, range, &chunk_map, None).ok_or_else(|| Error::msg("Hit nothing"))
}

pub fn voxel_raycast_with_map(
    _world: &mut World,
    transform: &Transform,
    distance: f32,
    direction: Direction,
    chunk_map: &HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]>,
) -> Result<RaycastHit> {
    let ray = get_camera_ray(transform, direction);
    raycast_raw(&ray, distance, chunk_map, None).ok_or_else(|| Error::msg("Hit nothing"))
}
