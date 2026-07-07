use cgmath::{Quaternion, Rotation, Vector3};
use hail::memory_usage::MemoryUsage;
use hail_macro::hail;

use crate::ecs::components::get_component_registration;
use crate::ecs::components::transform::Transform;
use crate::physics::velocity::Velocity;
use crate::scripting::hail::runtime::WorldHandle;
use crate::scripting::hail::world::EntityHandle;

/// Script-facing 2D vector, registered as `Vec2`.
#[derive(Clone, Copy, Default)]
pub struct Vec2Value {
    pub x: f64,
    pub y: f64,
}

impl MemoryUsage for Vec2Value {}

#[hail(free, "vec2")]
pub fn vec2(x: f64, y: f64) -> Vec2Value {
    Vec2Value { x, y }
}

#[hail(property, "x")]
pub fn vec2_x(v: &Vec2Value) -> f64 {
    v.x
}

#[hail(property, "y")]
pub fn vec2_y(v: &Vec2Value) -> f64 {
    v.y
}

#[hail(setter, "x")]
pub fn vec2_set_x(v: &mut Vec2Value, value: f64) {
    v.x = value;
}

#[hail(setter, "y")]
pub fn vec2_set_y(v: &mut Vec2Value, value: f64) {
    v.y = value;
}

/// Script-facing vector, registered as `Vec3`. Engine f32 vectors convert at the edge.
#[derive(Clone, Copy, Default)]
pub struct Vec3Value {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl MemoryUsage for Vec3Value {}

impl Vec3Value {
    pub(crate) fn from_f32(v: Vector3<f32>) -> Self {
        Self {
            x: v.x as f64,
            y: v.y as f64,
            z: v.z as f64,
        }
    }

    pub(crate) fn to_f32(self) -> Vector3<f32> {
        Vector3::new(self.x as f32, self.y as f32, self.z as f32)
    }
}

#[hail(free, "vec3")]
pub fn vec3(x: f64, y: f64, z: f64) -> Vec3Value {
    Vec3Value { x, y, z }
}

#[hail(property, "x")]
pub fn vec3_x(v: &Vec3Value) -> f64 {
    v.x
}

#[hail(property, "y")]
pub fn vec3_y(v: &Vec3Value) -> f64 {
    v.y
}

#[hail(property, "z")]
pub fn vec3_z(v: &Vec3Value) -> f64 {
    v.z
}

#[hail(setter, "x")]
pub fn vec3_set_x(v: &mut Vec3Value, value: f64) {
    v.x = value;
}

#[hail(setter, "y")]
pub fn vec3_set_y(v: &mut Vec3Value, value: f64) {
    v.y = value;
}

#[hail(setter, "z")]
pub fn vec3_set_z(v: &mut Vec3Value, value: f64) {
    v.z = value;
}

/// Snapshot of an entity's local Transform, registered as `Transform`.
/// Mutate it locally, then write it back with `world.set_transform(entity, t)`.
#[derive(Clone, Copy)]
pub struct HailTransform {
    pub position: Vec3Value,
    pub euler_angles: Vec3Value,
    pub scale: Vec3Value,
    /// Read-only view of the derived global rotation; used by `rotate`.
    pub global_rotation: Quaternion<f32>,
}

impl MemoryUsage for HailTransform {}

impl Default for HailTransform {
    fn default() -> Self {
        Self {
            position: Vec3Value::default(),
            euler_angles: Vec3Value::default(),
            scale: Vec3Value::default(),
            global_rotation: Quaternion::new(1.0, 0.0, 0.0, 0.0),
        }
    }
}

impl From<&Transform> for HailTransform {
    fn from(t: &Transform) -> Self {
        Self {
            position: Vec3Value::from_f32(t.local_position),
            euler_angles: Vec3Value::from_f32(t.local_euler_angles),
            scale: Vec3Value::from_f32(t.local_scale),
            global_rotation: t.global_rotation,
        }
    }
}

/// Rotates a direction vector by the transform's global rotation
/// (e.g. turn local movement input into world space).
#[hail(method, "rotate")]
pub fn transform_rotate(t: &HailTransform, v: Vec3Value) -> Vec3Value {
    Vec3Value::from_f32(t.global_rotation.rotate_vector(v.to_f32()))
}

#[hail(property, "position")]
pub fn transform_position(t: &HailTransform) -> Vec3Value {
    t.position
}

#[hail(property, "euler_angles")]
pub fn transform_euler_angles(t: &HailTransform) -> Vec3Value {
    t.euler_angles
}

#[hail(property, "scale")]
pub fn transform_scale(t: &HailTransform) -> Vec3Value {
    t.scale
}

#[hail(setter, "position")]
pub fn transform_set_position(t: &mut HailTransform, value: Vec3Value) {
    t.position = value;
}

#[hail(setter, "euler_angles")]
pub fn transform_set_euler_angles(t: &mut HailTransform, value: Vec3Value) {
    t.euler_angles = value;
}

#[hail(setter, "scale")]
pub fn transform_set_scale(t: &mut HailTransform, value: Vec3Value) {
    t.scale = value;
}

#[hail(method, "get_transform")]
pub fn get_transform(world: &WorldHandle, entity: EntityHandle) -> Option<HailTransform> {
    world
        .world()
        .get_component::<Transform>(entity.0)
        .map(HailTransform::from)
}

/// Writes the snapshot's local fields back; globals are re-derived by
/// `transform_update` each frame.
#[hail(method, "set_transform")]
pub fn set_transform(world: &WorldHandle, entity: EntityHandle, transform: HailTransform) {
    let Some(t) = world.world().get_component_mut::<Transform>(entity.0) else {
        crate::log_error!("[hail] set_transform: entity has no Transform");
        return;
    };
    t.local_position = transform.position.to_f32();
    t.local_euler_angles = transform.euler_angles.to_f32();
    t.local_scale = transform.scale.to_f32();
}

/// Snapshot of an entity's Velocity, registered as `Velocity`.
/// `is_grounded` is read-only (set by the collision system each frame).
#[derive(Clone, Copy, Default)]
pub struct HailVelocity {
    pub linear_velocity: Vec3Value,
    pub is_grounded: bool,
}

impl MemoryUsage for HailVelocity {}

#[hail(property, "linear_velocity")]
pub fn velocity_linear(v: &HailVelocity) -> Vec3Value {
    v.linear_velocity
}

#[hail(setter, "linear_velocity")]
pub fn velocity_set_linear(v: &mut HailVelocity, value: Vec3Value) {
    v.linear_velocity = value;
}

#[hail(property, "is_grounded")]
pub fn velocity_is_grounded(v: &HailVelocity) -> bool {
    v.is_grounded
}

impl From<&Velocity> for HailVelocity {
    fn from(v: &Velocity) -> Self {
        Self {
            linear_velocity: Vec3Value::from_f32(v.linear_velocity),
            is_grounded: v.is_grounded,
        }
    }
}

#[hail(method, "get_velocity")]
pub fn get_velocity(world: &WorldHandle, entity: EntityHandle) -> Option<HailVelocity> {
    world
        .world()
        .get_component::<Velocity>(entity.0)
        .map(HailVelocity::from)
}

/// Writes `linear_velocity` back; the other physics fields stay untouched.
#[hail(method, "set_velocity")]
pub fn set_velocity(world: &WorldHandle, entity: EntityHandle, velocity: HailVelocity) {
    let Some(v) = world.world().get_component_mut::<Velocity>(entity.0) else {
        crate::log_error!("[hail] set_velocity: entity has no Velocity");
        return;
    };
    v.linear_velocity = velocity.linear_velocity.to_f32();
}

// ---- generic by-name component access ----

#[hail(method, "add_component")]
pub fn add_component(world: &WorldHandle, entity: EntityHandle, name: String) {
    if let Err(e) = world.world().add_component_by_name(entity.0, &name) {
        crate::log_error!("[hail] add_component '{name}': {e}");
    }
}

#[hail(method, "remove_component")]
pub fn remove_component(world: &WorldHandle, entity: EntityHandle, name: String) {
    match get_component_registration(&name) {
        Some(reg) => (reg.remove)(world.world(), entity.0),
        None => {
            crate::log_error!("[hail] remove_component: unknown component '{name}'");
        }
    }
}

#[hail(method, "has_component")]
pub fn has_component(world: &WorldHandle, entity: EntityHandle, name: String) -> bool {
    get_component_registration(&name).is_some_and(|reg| (reg.contains)(world.world(), entity.0))
}

/// Every entity currently carrying the named component.
#[hail(method, "entities_with")]
pub fn entities_with(world: &WorldHandle, name: String) -> Vec<EntityHandle> {
    let Some(reg) = get_component_registration(&name) else {
        crate::log_error!("[hail] entities_with: unknown component '{name}'");
        return Vec::new();
    };
    let w = world.world();
    w.get_all_ids()
        .into_iter()
        .filter(|id| (reg.contains)(w, *id))
        .map(EntityHandle)
        .collect()
}
