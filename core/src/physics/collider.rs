use anyhow::Result;
use apostasy_macros::{update, Component};
use cgmath::{InnerSpace, Quaternion, Vector3};

use crate::{
    objects::{components::transform::Transform, scene::ObjectId, world::World},
    physics::velocity::Velocity,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ColliderShape {
    Cuboid { size: Vector3<f32> },
    Sphere { radius: f32 },
    Capsule { radius: f32, height: f32 },
    Cylinder { radius: f32, height: f32 },
}

impl ColliderShape {
    pub fn half_extents(&self) -> Vector3<f32> {
        match self {
            ColliderShape::Cuboid { size } => *size,
            ColliderShape::Sphere { radius } => Vector3::new(*radius, *radius, *radius),
            ColliderShape::Capsule { radius, height } => {
                Vector3::new(*radius, height * 0.5 + radius, *radius)
            }
            ColliderShape::Cylinder { radius, height } => {
                Vector3::new(*radius, height * 0.5, *radius)
            }
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct Collider {
    pub shape: ColliderShape,
    pub offset: Vector3<f32>,
    pub is_static: bool,
    pub is_area: bool,
}

impl Default for Collider {
    fn default() -> Self {
        Self {
            shape: ColliderShape::Cuboid {
                size: Vector3::new(1.0, 1.0, 1.0),
            },
            offset: Vector3::new(0.0, 0.0, 0.0),
            is_static: false,
            is_area: false,
        }
    }
}

impl Collider {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }

    /// Creates a dynamic collider
    pub fn new(shape: ColliderShape, offset: Vector3<f32>) -> Self {
        Self {
            shape,
            offset,
            is_static: false,
            is_area: false,
        }
    }

    /// Creates a static collider
    pub fn new_static(shape: ColliderShape, offset: Vector3<f32>) -> Self {
        Self {
            shape,
            offset,
            is_static: true,
            is_area: false,
        }
    }

    /// Returns the world-space center of this collider (offset rotated by entity rotation).
    pub fn world_center(&self, position: Vector3<f32>, rotation: Quaternion<f32>) -> Vector3<f32> {
        position + rotate_vector(rotation, self.offset)
    }

    /// Returns the three local axes of this OBB in world space.
    pub fn world_axes(&self, rotation: Quaternion<f32>) -> [Vector3<f32>; 3] {
        [
            rotate_vector(rotation, Vector3::new(1.0, 0.0, 0.0)),
            rotate_vector(rotation, Vector3::new(0.0, 1.0, 0.0)),
            rotate_vector(rotation, Vector3::new(0.0, 0.0, 1.0)),
        ]
    }

    /// Returns the half-extents (collider_size is already treated as half-extents).
    pub fn half_extents(&self) -> Vector3<f32> {
        self.shape.half_extents()
    }

    pub fn translation_vector_against(
        &self,
        pos_a: Vector3<f32>,
        rotation_a: Quaternion<f32>,
        other: &Collider,
        pos_b: Vector3<f32>,
        rotation_b: Quaternion<f32>,
    ) -> Option<Vector3<f32>> {
        let center_a = self.world_center(pos_a, rotation_a);
        let center_b = other.world_center(pos_b, rotation_b);
        let axes_a = self.world_axes(rotation_a);
        let axes_b = other.world_axes(rotation_b);
        let half_a = self.half_extents();
        let half_b = other.half_extents();

        // The vector from B's center to A's center
        let d = center_a - center_b;

        let mut min_overlap = f32::MAX;
        let mut min_axis = Vector3::new(0.0f32, 0.0, 0.0);

        let face_axes: [Vector3<f32>; 6] = [
            axes_a[0], axes_a[1], axes_a[2], axes_b[0], axes_b[1], axes_b[2],
        ];

        for axis in &face_axes {
            if axis.magnitude2() < 1e-10 {
                continue;
            }
            let axis = axis.normalize();

            let proj_a = project_obb(axis, &axes_a, half_a);
            let proj_b = project_obb(axis, &axes_b, half_b);
            let dist = d.dot(axis).abs();
            let overlap = proj_a + proj_b - dist;

            if overlap <= 0.0 {
                return None; // Separating axis found, no collision
            }
            if overlap < min_overlap {
                min_overlap = overlap;
                // Ensure the MTV points from B toward A
                min_axis = if d.dot(axis) >= 0.0 { axis } else { -axis };
            }
        }

        Some(min_axis * min_overlap)
    }

    pub fn contains_point(
        &self,
        position: Vector3<f32>,
        point: Vector3<f32>,
        rotation: Quaternion<f32>,
    ) -> bool {
        let axes = self.world_axes(rotation);
        let half = self.half_extents();
        let center = self.world_center(position, rotation);
        let local = point - center;

        // Project the point onto each local axis and check against half-extent
        local.dot(axes[0]).abs() <= half.x
            && local.dot(axes[1]).abs() <= half.y
            && local.dot(axes[2]).abs() <= half.z
    }
}

fn project_obb(axis: Vector3<f32>, obb_axes: &[Vector3<f32>; 3], half: Vector3<f32>) -> f32 {
    axis.dot(obb_axes[0]).abs() * half.x
        + axis.dot(obb_axes[1]).abs() * half.y
        + axis.dot(obb_axes[2]).abs() * half.z
}

/// Rotates a vector by a quaternion: q * v * q^-1
fn rotate_vector(q: Quaternion<f32>, v: Vector3<f32>) -> Vector3<f32> {
    let qv = Vector3::new(q.v.x, q.v.y, q.v.z);
    let t = qv.cross(v) * 2.0;
    v + t * q.s + qv.cross(t)
}

/// Contains information about a collision event
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    pub node_a: String,
    pub node_b: String,
    pub translation_vector: Vector3<f32>,
    pub depth: f32,
    pub normal: Vector3<f32>,
}

#[derive(Debug, Clone, Default, Component)]
pub struct CollisionEvents {
    pub events: Vec<CollisionEvent>,
}

impl CollisionEvents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}

fn get_object_id_by_name<'a>(world: &'a World, name: &str) -> Option<ObjectId> {
    world
        .get_objects_with_component_with_ids::<Transform>()
        .into_iter()
        .find(|(_, obj)| obj.name == name)
        .map(|(id, _)| id)
}

#[derive(Clone)]
struct Snapshot {
    id: ObjectId,
    name: String,
    position: Vector3<f32>,
    rotation: Quaternion<f32>,
    collider: Collider,
    is_static: bool,
}

fn build_snapshot(world: &World) -> Vec<Snapshot> {
    world
        .get_objects_with_component_with_ids::<Collider>()
        .into_iter()
        .filter_map(|(id, object)| {
            let transform = object.get_component::<Transform>().ok()?;
            let position = transform.global_position;
            let scale = transform.global_scale;
            let rotation = transform.global_rotation;
            let collider_raw = object.get_component::<Collider>().ok()?;
            let is_static = collider_raw.is_static;
            let mut collider = collider_raw.clone();

            // Bake scale into collider shape so collision matches world-space mesh size
            collider.shape = match collider.shape {
                ColliderShape::Cuboid { size } => ColliderShape::Cuboid {
                    size: Vector3::new(size.x * scale.x, size.y * scale.y, size.z * scale.z),
                },
                ColliderShape::Sphere { radius } => ColliderShape::Sphere {
                    radius: radius * scale.x.max(scale.y).max(scale.z),
                },
                ColliderShape::Capsule { radius, height } => ColliderShape::Capsule {
                    radius: radius * scale.x.max(scale.z),
                    height: height * scale.y,
                },
                ColliderShape::Cylinder { radius, height } => ColliderShape::Cylinder {
                    radius: radius * scale.x.max(scale.z),
                    height: height * scale.y,
                },
            };

            Some(Snapshot {
                id,
                name: object.name.clone(),
                position,
                rotation,
                collider,
                is_static,
            })
        })
        .collect()
}

/// Detects collisions between all objects using OBB vs OBB SAT
#[update]
pub fn collision_detection_system(world: &mut World) -> Result<()> {
    let snapshot = build_snapshot(world);
    let n = snapshot.len();

    let mut events: Vec<CollisionEvent> = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            let a = &snapshot[i];
            let b = &snapshot[j];

            if let Some(translation_vector) = a.collider.translation_vector_against(
                a.position,
                a.rotation,
                &b.collider,
                b.position,
                b.rotation,
            ) {
                let depth = translation_vector.magnitude();
                let normal = if depth > 1e-10 {
                    translation_vector / depth
                } else {
                    Vector3::new(0.0, 1.0, 0.0)
                };
                events.push(CollisionEvent {
                    node_a: a.name.clone(),
                    node_b: b.name.clone(),
                    translation_vector,
                    depth,
                    normal,
                });

                // is_static is already in the snapshot — no need to re-query world
                let normal_a = normal;
                let normal_b = -normal;

                match (a.is_static, b.is_static) {
                    // Both dynamic: split the correction evenly
                    (false, false) => {
                        let half = translation_vector * 0.5;
                        resolve_object(world, a.id, half, normal_a);
                        resolve_object(world, b.id, -half, normal_b);
                    }
                    // A is static: push B the full amount
                    (true, false) => {
                        resolve_object(world, b.id, -translation_vector, normal_b);
                    }
                    // B is static: push A the full amount
                    (false, true) => {
                        resolve_object(world, a.id, translation_vector, normal_a);
                    }
                    // Both static: do nothing
                    (true, true) => {}
                }
            }
        }
    }

    // Store events on whichever object holds the CollisionEvents component
    if let Some(ev_object) = world
        .get_objects_with_component_mut::<CollisionEvents>()
        .into_iter()
        .next()
    {
        if let Ok(ev) = ev_object.get_component_mut::<CollisionEvents>() {
            ev.events = events;
        }
    }

    Ok(())
}

fn resolve_object(world: &mut World, id: ObjectId, offset: Vector3<f32>, normal: Vector3<f32>) {
    if offset.magnitude2() < 1e-6 {
        return;
    }

    let Some(object) = world.get_object_mut(id) else {
        return;
    };

    // Extract values before mutable borrows to avoid multiple &mut refs
    let sphere_radius = object
        .get_component::<Collider>()
        .ok()
        .and_then(|c| match c.shape {
            ColliderShape::Sphere { radius } => Some(radius),
            _ => None,
        });

    if let Ok(transform) = object.get_component_mut::<Transform>() {
        transform.global_position += offset;
    }

    if let Ok(velocity) = object.get_component_mut::<Velocity>() {
        // Cancel velocity along the collision normal
        let v_dot_n = velocity.linear_velocity.dot(normal);
        if v_dot_n < 0.0 {
            velocity.linear_velocity -= normal * v_dot_n;
        }

        if let Some(radius) = sphere_radius {
            let v_tangential =
                velocity.linear_velocity - normal * velocity.linear_velocity.dot(normal);
            velocity.angular_velocity = v_tangential.cross(normal) * (1.0 / radius);
            velocity.sync_linear_from_angular(radius, normal);
        }
    }
}
