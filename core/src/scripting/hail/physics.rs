use hail::memory_usage::MemoryUsage;
use hail_macro::hail;

use crate::ecs::cell::EntityId;
use crate::physics::raycast::{
    ColliderHit, Ray, build_collider_snapshot, collider_raycast_camera, raycast_colliders_raw,
};
use crate::scripting::hail::components::Vec3Value;
use crate::scripting::hail::runtime::WorldHandle;
use crate::scripting::hail::world::EntityHandle;

/// Nearest collider hit from `world.raycast`, registered as `RaycastHit`.
#[derive(Clone, Copy)]
pub struct HailRaycastHit {
    pub entity: EntityHandle,
    pub point: Vec3Value,
    pub normal: Vec3Value,
    pub distance: f64,
    /// Struck face: 0=−X, 1=+X, 2=−Y, 3=+Y, 4=−Z, 5=+Z (0 for spheres/meshes).
    pub face: f64,
}

impl MemoryUsage for HailRaycastHit {}

impl From<ColliderHit> for HailRaycastHit {
    fn from(h: ColliderHit) -> Self {
        Self {
            entity: EntityHandle(h.entity_id),
            point: Vec3Value::from_f32(h.point),
            normal: Vec3Value::from_f32(h.normal),
            distance: h.distance as f64,
            face: h.face as f64,
        }
    }
}

#[hail(property, "entity")]
pub fn raycast_entity(hit: &HailRaycastHit) -> EntityHandle {
    hit.entity
}

#[hail(property, "point")]
pub fn raycast_point(hit: &HailRaycastHit) -> Vec3Value {
    hit.point
}

#[hail(property, "normal")]
pub fn raycast_normal(hit: &HailRaycastHit) -> Vec3Value {
    hit.normal
}

#[hail(property, "distance")]
pub fn raycast_distance(hit: &HailRaycastHit) -> f64 {
    hit.distance
}

#[hail(property, "face")]
pub fn raycast_face(hit: &HailRaycastHit) -> f64 {
    hit.face
}

/// Shared cast: nearest non-area collider hit within `max_distance`, or none.
fn cast(
    world: &WorldHandle,
    origin: Vec3Value,
    direction: Vec3Value,
    max_distance: f64,
    ignore: Option<EntityId>,
) -> Option<HailRaycastHit> {
    let ray = Ray::new(origin.to_f32(), direction.to_f32());
    let snapshots = build_collider_snapshot(world.world());
    raycast_colliders_raw(&ray, max_distance as f32, &snapshots, ignore.map(|id| vec![id]))
        .map(HailRaycastHit::from)
}

/// Casts a ray against every collider, returning the nearest hit within
/// `max_distance`, or nothing. `direction` need not be normalized.
#[hail(method, "raycast")]
pub fn raycast(
    world: &WorldHandle,
    origin: Vec3Value,
    direction: Vec3Value,
    max_distance: f64,
) -> Option<HailRaycastHit> {
    cast(world, origin, direction, max_distance, None)
}

/// Like `raycast`, but skips `ignore` (e.g. the caster's own entity).
#[hail(method, "raycast_ignore")]
pub fn raycast_ignore(
    world: &WorldHandle,
    origin: Vec3Value,
    direction: Vec3Value,
    max_distance: f64,
    ignore: EntityHandle,
) -> Option<HailRaycastHit> {
    cast(world, origin, direction, max_distance, Some(ignore.0))
}

/// Casts forward from the active camera in world space, ignoring the camera
/// itself. Returns the nearest hit within `max_distance`, or nothing.
#[hail(method, "raycast_camera")]
pub fn raycast_camera(world: &WorldHandle, max_distance: f64) -> Option<HailRaycastHit> {
    collider_raycast_camera(world.world(), max_distance as f32).map(HailRaycastHit::from)
}
