use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::{InnerSpace, Quaternion, Vector3, Zero};

use crate::{
    objects::{components::transform::Transform, scene::ObjectId, world::World},
    physics::velocity::Velocity,
};

///  A shape of a collider, might add more if needed
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

/// A component that defines a colliders data
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

        let d = center_a - center_b;

        match (&self.shape, &other.shape) {
            (ColliderShape::Sphere { radius: ra }, ColliderShape::Sphere { radius: rb }) => {
                return sphere_vs_sphere(center_a, *ra, center_b, *rb);
            }
            (ColliderShape::Sphere { radius }, _) => {
                return sphere_vs_obb(center_a, *radius, center_b, &axes_b, half_b);
            }
            (_, ColliderShape::Sphere { radius }) => {
                return sphere_vs_obb(center_b, *radius, center_a, &axes_a, half_a).map(|v| -v);
            }
            _ => {}
        }
        let mut min_overlap = f32::MAX;
        let mut min_axis = Vector3::new(0.0f32, 0.0, 0.0);
        let face_axes: [Vector3<f32>; 6] = [
            axes_a[0], axes_a[1], axes_a[2], axes_b[0], axes_b[1], axes_b[2],
        ];

        let mut edge_axes: Vec<Vector3<f32>> = Vec::new();
        for i in 0..3 {
            for j in 0..3 {
                let cross = axes_a[i].cross(axes_b[j]);
                if cross.magnitude2() > 1e-10 {
                    edge_axes.push(cross.normalize());
                }
            }
        }

        let all_axes: Vec<Vector3<f32>> = face_axes.iter().copied().chain(edge_axes).collect();

        for axis in &all_axes {
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

/// A snapshot of a collider and it's needed data
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

/// Detects collisions between all objects using OBB vs OBB
#[update(priority = 10)]
pub fn collision_detection_system(world: &mut World) -> Result<()> {
    // Reset grounded flag itll be set by active collisions
    for object in world.get_objects_with_component_mut::<Velocity>() {
        if let Ok(v) = object.get_component_mut::<Velocity>() {
            v.is_grounded = false;
        }
    }

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
                let center_a = a.collider.world_center(a.position, a.rotation);
                let center_b = b.collider.world_center(b.position, b.rotation);

                let depth = translation_vector.magnitude();
                let normal = if depth > 1e-10 {
                    translation_vector / depth
                } else {
                    Vector3::new(0.0, 1.0, 0.0)
                };
                let upness = normal.y.abs();

                let (r_a, r_b) = match (&a.collider.shape, &b.collider.shape) {
                    (ColliderShape::Sphere { radius }, _) => {
                        let contact_point = center_a - normal * *radius;
                        (contact_point - center_a, contact_point - center_b)
                    }
                    (_, ColliderShape::Sphere { radius }) => {
                        let contact_point = center_b + normal * *radius;
                        (contact_point - center_a, contact_point - center_b)
                    }
                    _ => (Vector3::zero(), Vector3::zero()),
                };

                events.push(CollisionEvent {
                    node_a: a.name.clone(),
                    node_b: b.name.clone(),
                    translation_vector,
                    depth,
                    normal,
                });

                let vel_a = world
                    .get_object(a.id)
                    .and_then(|o| o.get_component::<Velocity>().ok().cloned());
                let vel_b = world
                    .get_object(b.id)
                    .and_then(|o| o.get_component::<Velocity>().ok().cloned());

                if let (Some(mut va), Some(mut vb)) = (vel_a, vel_b) {
                    // Positional correction
                    match (a.is_static, b.is_static) {
                        (false, false) => {
                            apply_position_correction(world, a.id, translation_vector * 0.5);
                            apply_position_correction(world, b.id, -translation_vector * 0.5);
                            // Cancel velocity along normal for both
                            if let Some(obj) = world.get_object_mut(a.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    let vn = v.linear_velocity.dot(normal);
                                    if vn < 0.0 {
                                        v.linear_velocity -= normal * vn;
                                    }
                                }
                            }
                            if let Some(obj) = world.get_object_mut(b.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    let vn = v.linear_velocity.dot(-normal);
                                    if vn < 0.0 {
                                        v.linear_velocity += normal * vn;
                                    }
                                }
                            }
                        }
                        (true, false) => {
                            apply_position_correction(world, b.id, -translation_vector);
                            if let Some(obj) = world.get_object_mut(b.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    let vn = v.linear_velocity.dot(normal);
                                    if vn > 0.0 {
                                        v.linear_velocity -= normal * vn;
                                    }
                                }
                            }
                        }
                        (false, true) => {
                            apply_position_correction(world, a.id, translation_vector);
                            if let Some(obj) = world.get_object_mut(a.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    let vn = v.linear_velocity.dot(normal);
                                    if vn < 0.0 {
                                        v.linear_velocity -= normal * vn;
                                    }
                                }
                            }
                        }
                        (true, true) => {}
                    }
                    // zero out velocity for static objects so impulse math is correct
                    if a.is_static {
                        va.linear_velocity = Vector3::zero();
                        va.angular_velocity = Vector3::zero();
                        va.mass = 0.0;
                    }
                    if b.is_static {
                        vb.linear_velocity = Vector3::zero();
                        vb.angular_velocity = Vector3::zero();
                        vb.mass = 0.0;
                    }

                    resolve_impulse(&mut va, &mut vb, r_a, r_b, normal);

                    // skip static objects
                    if !a.is_static
                        && let Some(obj) = world.get_object_mut(a.id)
                        && let Ok(v) = obj.get_component_mut::<Velocity>()
                    {
                        *v = va;
                    }
                    if !b.is_static
                        && let Some(obj) = world.get_object_mut(b.id)
                        && let Ok(v) = obj.get_component_mut::<Velocity>()
                    {
                        *v = vb;
                    }

                    if upness > 0.7 {
                        if !a.is_static {
                            if let Some(obj) = world.get_object_mut(a.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    v.is_grounded = true;
                                }
                            }
                        }
                        if !b.is_static {
                            if let Some(obj) = world.get_object_mut(b.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    v.is_grounded = true;
                                }
                            }
                        }
                    }

                    // On near-up contacts, zero tiny tangential velocity to stop landing jitter
                    let upness = normal.y.abs();
                    if upness > 0.7 {
                        let tang_threshold = 0.5; // tuneable
                        if !a.is_static {
                            if let Some(obj) = world.get_object_mut(a.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    let tang =
                                        v.linear_velocity - normal * v.linear_velocity.dot(normal);
                                    if tang.magnitude() < tang_threshold {
                                        v.linear_velocity -= tang; // zero small tangential
                                    }
                                }
                            }
                        }
                        if !b.is_static {
                            if let Some(obj) = world.get_object_mut(b.id) {
                                if let Ok(v) = obj.get_component_mut::<Velocity>() {
                                    let tang =
                                        v.linear_velocity - normal * v.linear_velocity.dot(normal);
                                    if tang.magnitude() < tang_threshold {
                                        v.linear_velocity -= tang;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Store events on whichever object holds the CollisionEvents component
    if let Some(ev_object) = world
        .get_objects_with_component_mut::<CollisionEvents>()
        .into_iter()
        .next()
        && let Ok(ev) = ev_object.get_component_mut::<CollisionEvents>()
    {
        ev.events = events;
    }

    Ok(())
}
fn apply_position_correction(world: &mut World, id: ObjectId, offset: Vector3<f32>) {
    if offset.magnitude2() < 1e-6 {
        return;
    }
    let max_correction = 0.25;
    let offset = if offset.magnitude2() > max_correction * max_correction {
        offset.normalize() * max_correction
    } else {
        offset
    };
    // Compute local-space delta before taking mutable borrows on `world`.
    let local_delta = if let Some(parent_id) = world.get_parent_id(id) {
        if let Some(parent) = world.get_object(parent_id) {
            if let Ok(parent_t) = parent.get_component::<Transform>() {
                let pr = parent_t.global_rotation;
                let inv_pr = Quaternion::new(pr.s, -pr.v.x, -pr.v.y, -pr.v.z);
                rotate_vector(inv_pr, offset)
            } else {
                offset
            }
        } else {
            offset
        }
    } else {
        offset
    };

    if let Some(obj) = world.get_object_mut(id)
        && let Ok(t) = obj.get_component_mut::<Transform>()
    {
        // Apply correction in world space
        t.global_position += offset;
        // Apply the precomputed local delta
        t.local_position += local_delta;
    }
}

fn resolve_impulse(
    vel_a: &mut Velocity,
    vel_b: &mut Velocity,
    r_a: Vector3<f32>,
    r_b: Vector3<f32>,
    normal: Vector3<f32>,
) {
    let inv_mass_a = if vel_a.mass > 0.0 {
        1.0 / vel_a.mass
    } else {
        0.0
    };
    let inv_mass_b = if vel_b.mass > 0.0 {
        1.0 / vel_b.mass
    } else {
        0.0
    };
    let inv_i_a = vel_a
        .inertia_tensor
        .map(|v| if v > 0.0 { 1.0 / v } else { 0.0 });
    let inv_i_b = vel_b
        .inertia_tensor
        .map(|v| if v > 0.0 { 1.0 / v } else { 0.0 });

    // Velocity at contact point including angular contribution (v + ω × r)
    let va_contact = vel_a.linear_velocity + vel_a.angular_velocity.cross(r_a);
    let vb_contact = vel_b.linear_velocity + vel_b.angular_velocity.cross(r_b);
    let relative_vel = va_contact - vb_contact;

    let vel_along_normal = relative_vel.dot(normal);

    // Already separating
    if vel_along_normal > 0.0 {
        return;
    }

    // Angular contribution
    let ang_term = |inv_i: Vector3<f32>, r: Vector3<f32>| {
        let rxn = r.cross(normal);
        // apply diagonal inverse inertia tensor
        let i_inv_rxn = Vector3::new(inv_i.x * rxn.x, inv_i.y * rxn.y, inv_i.z * rxn.z);
        i_inv_rxn.cross(r).dot(normal)
    };

    let restitution = if vel_along_normal > -0.5 {
        0.0
    } else {
        vel_a.restitution.min(vel_b.restitution)
    };
    let j_denom = inv_mass_a + inv_mass_b + ang_term(inv_i_a, r_a) + ang_term(inv_i_b, r_b);
    if j_denom <= 1e-6 {
        return;
    }
    let j = -(1.0 + restitution) * vel_along_normal / j_denom;

    // Apply normal impulse
    vel_a.linear_velocity += normal * (j * inv_mass_a);
    vel_b.linear_velocity -= normal * (j * inv_mass_b);
    vel_a.angular_velocity += apply_inv_inertia(inv_i_a, r_a.cross(normal * j));
    vel_b.angular_velocity -= apply_inv_inertia(inv_i_b, r_b.cross(normal * j));

    // Friction
    // Recompute relative vel after normal impulse
    let va_contact = vel_a.linear_velocity + vel_a.angular_velocity.cross(r_a);
    let vb_contact = vel_b.linear_velocity + vel_b.angular_velocity.cross(r_b);
    let relative_vel = va_contact - vb_contact;

    let tangential = relative_vel - normal * relative_vel.dot(normal);

    // No sliding
    if tangential.magnitude2() < 1e-8 {
        return;
    }
    let tangent = tangential.normalize();

    // Same denominator structure but along tangent
    let ang_term_t = |inv_i: Vector3<f32>, r: Vector3<f32>| {
        let rxt = r.cross(tangent);
        let i_inv_rxt = Vector3::new(inv_i.x * rxt.x, inv_i.y * rxt.y, inv_i.z * rxt.z);
        i_inv_rxt.cross(r).dot(tangent)
    };

    let jt_denom = inv_mass_a + inv_mass_b + ang_term_t(inv_i_a, r_a) + ang_term_t(inv_i_b, r_b);
    let jt = -relative_vel.dot(tangent) / jt_denom;

    // clamp static vs kinetic
    let mu_s = (vel_a.mu_static * vel_b.mu_static).sqrt();
    let mu_k = (vel_a.mu_kinetic * vel_b.mu_kinetic).sqrt();

    let friction_impulse = if jt.abs() <= j * mu_s {
        tangent * jt
    } else {
        tangent * (j * mu_k * -jt.signum())
    };

    // Don't let friction increase tangential speed.
    let orig_va_contact = vel_a.linear_velocity + vel_a.angular_velocity.cross(r_a);
    let orig_vb_contact = vel_b.linear_velocity + vel_b.angular_velocity.cross(r_b);
    let orig_rel_tang = orig_va_contact
        - orig_vb_contact
        - normal * (orig_va_contact - orig_vb_contact).dot(normal);
    let orig_tang_mag = orig_rel_tang.magnitude();

    // Tentatively apply linear part of friction impulse to test effect on tangential velocity
    let test_va_lin = vel_a.linear_velocity + friction_impulse * inv_mass_a;
    let test_vb_lin = vel_b.linear_velocity - friction_impulse * inv_mass_b;
    let test_va_contact = test_va_lin + vel_a.angular_velocity.cross(r_a);
    let test_vb_contact = test_vb_lin + vel_b.angular_velocity.cross(r_b);
    let test_rel_tang = test_va_contact
        - test_vb_contact
        - normal * (test_va_contact - test_vb_contact).dot(normal);
    let test_tang_mag = test_rel_tang.magnitude();

    let final_friction = if test_tang_mag > orig_tang_mag + 1e-6 {
        // scale down impulse to not increase tangential speed
        if test_tang_mag.abs() > 1e-9 {
            friction_impulse * (orig_tang_mag / test_tang_mag)
        } else {
            Vector3::new(0.0, 0.0, 0.0)
        }
    } else {
        friction_impulse
    };

    vel_a.linear_velocity += final_friction * inv_mass_a;
    vel_b.linear_velocity -= final_friction * inv_mass_b;
    vel_a.angular_velocity += apply_inv_inertia(inv_i_a, r_a.cross(final_friction));
    vel_b.angular_velocity -= apply_inv_inertia(inv_i_b, r_b.cross(final_friction));
}

fn apply_inv_inertia(inv_i: Vector3<f32>, torque: Vector3<f32>) -> Vector3<f32> {
    Vector3::new(inv_i.x * torque.x, inv_i.y * torque.y, inv_i.z * torque.z)
}

fn sphere_vs_sphere(
    center_a: Vector3<f32>,
    ra: f32,
    center_b: Vector3<f32>,
    rb: f32,
) -> Option<Vector3<f32>> {
    let d = center_a - center_b;
    let dist2 = d.magnitude2();
    let sum = ra + rb;
    if dist2 >= sum * sum {
        return None;
    }
    let dist = dist2.sqrt();
    let normal = if dist > 1e-10 {
        d / dist
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    Some(normal * (sum - dist))
}

fn sphere_vs_obb(
    sphere_center: Vector3<f32>,
    radius: f32,
    obb_center: Vector3<f32>,
    obb_axes: &[Vector3<f32>; 3],
    obb_half: Vector3<f32>,
) -> Option<Vector3<f32>> {
    let d = sphere_center - obb_center;
    // Find closest point on OBB to sphere center
    let closest = obb_center
        + obb_axes[0] * d.dot(obb_axes[0]).clamp(-obb_half.x, obb_half.x)
        + obb_axes[1] * d.dot(obb_axes[1]).clamp(-obb_half.y, obb_half.y)
        + obb_axes[2] * d.dot(obb_axes[2]).clamp(-obb_half.z, obb_half.z);

    let diff = sphere_center - closest;
    let dist2 = diff.magnitude2();
    if dist2 >= radius * radius {
        return None;
    }
    let dist = dist2.sqrt();
    let normal = if dist > 1e-10 {
        diff / dist
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    Some(normal * (radius - dist))
}
