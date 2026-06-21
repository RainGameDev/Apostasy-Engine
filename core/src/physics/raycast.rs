use crate::ecs::cell::ObjectId;
use crate::ecs::components::transform::Transform;
use crate::ecs::world::World;
use crate::physics::collider::{Collider, ColliderShape};
use crate::rendering::components::camera::Camera;
use crate::rendering::shared::model::Bvh;
use cgmath::{InnerSpace, Matrix4, Quaternion, Vector3, Vector4};

// Definition of a ray in a raycast, direction and start point
pub struct Ray {
    /// Where does the ray start?
    pub origin: Vector3<f32>,
    /// Where does the ray go?
    pub direction: Vector3<f32>,
}

impl Ray {
    /// Creates a new ray with a position and direction
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
/// Core direction of a ray
pub enum Direction {
    Forward,
    Backwards,
    Left,
    Right,
    Up,
    Down,
}

#[inline(always)]
/// Takes a position and a core direction and converts it into a ray
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

/// Result of a successful collider raycast
#[derive(Debug, Clone)]
pub struct ColliderHit {
    pub object_id: ObjectId,
    /// World space point where the ray enters the collider surface
    pub point: Vector3<f32>,
    /// Sacing surface normal at the hit point
    pub normal: Vector3<f32>,
    /// Distance along the ray
    pub distance: f32,
    /// Which face was struck: 0=−X, 1=+X, 2=−Y, 3=+Y, 4=−Z, 5=+Z
    /// For spheres this is always 0; for capsules/cylinders the end-cap faces are 2/3.
    pub face: u8,
}

/// Snapshot of a collider needed for ray testing
pub struct ColliderSnapshot {
    id: ObjectId,
    center: Vector3<f32>,
    axes: [Vector3<f32>; 3],
    half_extents: Vector3<f32>,
    shape: ColliderShape,
    collider: Collider,
    position: Vector3<f32>,
    rotation: Quaternion<f32>,
}

/// Takes a snapshot of every object in the world that has a Collider component
pub fn build_collider_snapshot(world: &World) -> Vec<ColliderSnapshot> {
    world
        .get_objects_with_component_with_ids::<Collider>()
        .into_iter()
        .filter_map(|(id, object)| {
            let transform = object.get_component::<Transform>().ok()?;
            let position = transform.global_position;
            let scale = transform.global_scale;
            let rotation = transform.global_rotation;
            let collider_raw = object.get_component::<Collider>().ok()?;
            let mut collider = collider_raw.clone();

            // Bake scale to matches collision_detection_system
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
                ColliderShape::Mesh { .. } => collider.shape.clone(),
            };

            let center = collider.world_center(position, rotation);
            let axes = collider.world_axes(rotation);
            let half_extents = collider.half_extents();

            Some(ColliderSnapshot {
                id,
                center,
                axes,
                half_extents,
                shape: collider.shape.clone(),
                collider,
                position,
                rotation,
            })
        })
        .collect()
}

/// Tests a ray against an oriented bounding box using the slab method, returning the entry/exit distances and the index of the face that was hit, or None if the ray misses
pub(crate) fn ray_vs_obb(
    ray: &Ray,
    center: Vector3<f32>,
    axes: &[Vector3<f32>; 3],
    half: Vector3<f32>,
) -> Option<(f32, f32, u8)> {
    let d = center - ray.origin;
    let mut t_min = f32::NEG_INFINITY;
    let mut t_max = f32::INFINITY;
    let mut hit_face: u8 = 0;

    for i in 0..3 {
        let e = axes[i].dot(d); // projection of center-offset onto axis
        let f = axes[i].dot(ray.direction); // projection of ray direction onto axis

        if f.abs() > 1e-8 {
            let inv_f = 1.0 / f;
            let mut t1 = (e - half[i]) * inv_f;
            let mut t2 = (e + half[i]) * inv_f;
            // face indices: axis 0->0/1, axis 1->2/3, axis 2->4/5
            let (face_neg, face_pos) = ((i * 2) as u8, (i * 2 + 1) as u8);

            let entering_face = if t1 < t2 { face_neg } else { face_pos };

            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }

            if t1 > t_min {
                t_min = t1;
                hit_face = entering_face;
            }
            t_max = t_max.min(t2);

            if t_min > t_max {
                return None; // separating slab found
            }
        } else if (-e - half[i]) > 0.0 || (-e + half[i]) < 0.0 {
            // Ray is parallel to this slab and outside it
            return None;
        }
    }

    if t_max < 0.0 {
        return None; // OBB is entirely behind the ray origin
    }

    Some((t_min, t_max, hit_face))
}

/// Sphere intersection test Returns (t_enter, normal) or None
fn ray_vs_sphere(ray: &Ray, center: Vector3<f32>, radius: f32) -> Option<(f32, Vector3<f32>)> {
    let oc = ray.origin - center;
    let b = oc.dot(ray.direction);
    let c = oc.magnitude2() - radius * radius;
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_d = discriminant.sqrt();
    let t = {
        let t0 = -b - sqrt_d;
        let t1 = -b + sqrt_d;
        if t0 >= 0.0 {
            t0
        } else if t1 >= 0.0 {
            t1 // origin inside sphere
        } else {
            return None; // sphere behind ray
        }
    };
    let hit = ray.origin + ray.direction * t;
    let normal = (hit - center).normalize();
    Some((t, normal))
}

/// Test a single snapshot against the ray
/// Returns (distance, point, normal, face) on hit
#[allow(unused)]
fn test_snapshot(
    ray: &Ray,
    snap: &ColliderSnapshot,
    max_distance: f32,
) -> Option<(f32, Vector3<f32>, Vector3<f32>, u8)> {
    match &snap.shape {
        ColliderShape::Sphere { radius } => {
            let (t, normal) = ray_vs_sphere(ray, snap.center, *radius)?;
            if t > max_distance {
                return None;
            }
            let point = ray.origin + ray.direction * t;
            Some((t, point, normal, 0))
        }

        ColliderShape::Cuboid { .. } | ColliderShape::Cylinder { .. } => {
            let (t_enter, _, face) = ray_vs_obb(ray, snap.center, &snap.axes, snap.half_extents)?;
            let t = if t_enter >= 0.0 { t_enter } else { return None };
            if t > max_distance {
                return None;
            }
            let point = ray.origin + ray.direction * t;
            // World-space outward normal: the struck axis, signed away from ray origin
            let axis_idx = (face / 2) as usize;
            let axis = snap.axes[axis_idx];
            let sign = if face % 2 == 0 { -1.0f32 } else { 1.0 };
            let normal = axis * sign;
            Some((t, point, normal, face))
        }

        ColliderShape::Capsule { radius, height } => {
            // Approximate as OBB for intersection, then refine normal.
            let (t_enter, _, face) = ray_vs_obb(ray, snap.center, &snap.axes, snap.half_extents)?;
            let t = if t_enter >= 0.0 { t_enter } else { return None };
            if t > max_distance {
                return None;
            }
            let point = ray.origin + ray.direction * t;

            // Determine if the hit is on a cylinder body or an end-cap
            let local_y = point - snap.center;
            let along_y = snap.axes[1].dot(local_y);
            let half_h = height * 0.5;

            let normal = if along_y.abs() >= half_h {
                // End cap: normal is the Y axis
                snap.axes[1] * along_y.signum()
            } else {
                // Cylindrical body: normal is radial in XZ plane
                let radial = local_y - snap.axes[1] * along_y;
                let len = radial.magnitude();
                if len > 1e-8 {
                    radial / len
                } else {
                    snap.axes[1]
                }
            };
            Some((t, point, normal, face))
        }

        ColliderShape::Mesh { bvh, .. } => {
            let (t, normal) = ray_vs_mesh(ray, bvh, max_distance, snap.position, snap.rotation)?;
            if t > max_distance {
                return None;
            }
            let point = ray.origin + ray.direction * t;
            Some((t, point, normal, 0))
        }
    }
}

/// Core collider raycast Iterates all colliders, returns the nearest hit within max_distance
/// Optionally skips a single object (e.g. the caster itself).
pub fn raycast_colliders_raw(
    ray: &Ray,
    max_distance: f32,
    snapshots: &[ColliderSnapshot],
    ignore_id: Option<ObjectId>,
) -> Option<ColliderHit> {
    let mut nearest: Option<ColliderHit> = None;
    let mut nearest_dist = max_distance;

    for snap in snapshots {
        if Some(snap.id) == ignore_id {
            continue;
        }
        if snap.collider.is_area {
            continue;
        }

        if let Some((dist, point, normal, face)) = test_snapshot(ray, snap, nearest_dist) {
            nearest_dist = dist;
            nearest = Some(ColliderHit {
                object_id: snap.id,
                point,
                normal,
                distance: dist,
                face,
            });
        }
    }

    nearest
}

/// Raycast from an arbitrary transform in a given direction
pub fn collider_raycast(
    world: &mut World,
    transform: &Transform,
    distance: f32,
    direction: Direction,
    ignore_id: Option<ObjectId>,
) -> Option<ColliderHit> {
    let ray = get_camera_ray(transform, direction);
    let snapshots = build_collider_snapshot(world);
    raycast_colliders_raw(&ray, distance, &snapshots, ignore_id)
}

/// Raycast forward from the active camera
pub fn collider_raycast_camera(world: &mut World, range: f32) -> Option<ColliderHit> {
    let camera_obj = world
        .get_objects_with_component::<Camera>()
        .first()
        .copied()?;
    let transform = camera_obj.get_component::<Transform>().ok()?.clone();
    let ray = get_camera_ray(&transform, Direction::Forward);
    let snapshots = build_collider_snapshot(world);
    // Ignore the camera object itself so it can't hit its own collider
    raycast_colliders_raw(&ray, range, &snapshots, Some(camera_obj.id))
}

/// Raycast from an arbitrary transform with a pre-built snapshot (avoids rebuilding
pub fn collider_raycast_with_snapshot(
    _world: &mut World,
    transform: &Transform,
    distance: f32,
    direction: Direction,
    snapshots: &[ColliderSnapshot],
    ignore_id: Option<ObjectId>,
) -> Option<ColliderHit> {
    let ray = get_camera_ray(transform, direction);
    raycast_colliders_raw(&ray, distance, snapshots, ignore_id)
}
pub fn unproject(
    ndc_x: f32,
    ndc_y: f32,
    inv_view_proj: &Matrix4<f32>,
    camera_position: Vector3<f32>,
) -> Vector3<f32> {
    let clip = Vector4::new(ndc_x, ndc_y, -1.0, 1.0);
    let world = inv_view_proj * clip;
    let world = Vector3::new(world.x / world.w, world.y / world.w, world.z / world.w);
    (world - camera_position).normalize()
}

fn ray_vs_mesh(
    ray: &Ray,
    bvh: &Bvh,
    max_distance: f32,
    mesh_pos: Vector3<f32>,
    mesh_rot: Quaternion<f32>,
) -> Option<(f32, Vector3<f32>)> {
    if bvh.nodes.is_empty() {
        return None;
    }

    let inv_rot = mesh_rot.conjugate();
    let local_ray = Ray {
        origin: inv_rot * (ray.origin - mesh_pos),
        direction: inv_rot * ray.direction,
    };

    let identity = [
        Vector3::new(1.0f32, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
    ];

    let mut stack = vec![0u32];
    let mut nearest_t = max_distance;
    let mut nearest_normal: Option<Vector3<f32>> = None;

    while let Some(idx) = stack.pop() {
        let node = &bvh.nodes[idx as usize];
        let center = (node.aabb_min + node.aabb_max) * 0.5;
        let half = (node.aabb_max - node.aabb_min) * 0.5;

        match ray_vs_obb(&local_ray, center, &identity, half) {
            None => continue,
            Some((t_enter, _, _)) if t_enter > nearest_t => continue,
            _ => {}
        }

        if node.left == 0 {
            for i in node.tri_start..node.tri_start + node.tri_count {
                if let Some((t, local_normal)) = ray_vs_triangle(&local_ray, &bvh.triangles[i as usize]) {
                    if t < nearest_t {
                        nearest_t = t;
                        nearest_normal = Some(mesh_rot * local_normal);
                    }
                }
            }
        } else {
            stack.push(node.left);
            stack.push(node.right);
        }
    }

    nearest_normal.map(|n| (nearest_t, n))
}

fn ray_vs_triangle(ray: &Ray, tri: &[Vector3<f32>; 3]) -> Option<(f32, Vector3<f32>)> {
    let e1 = tri[1] - tri[0];
    let e2 = tri[2] - tri[0];
    let h = ray.direction.cross(e2);
    let a = e1.dot(h);
    if a.abs() < 1e-8 {
        return None;
    } // ray parallel to triangle

    let f = 1.0 / a;
    let s = ray.origin - tri[0];
    let u = f * s.dot(h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(e1);
    let v = f * ray.direction.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * e2.dot(q);
    if t < 1e-8 {
        return None;
    } // behind or at origin

    let mut normal = e1.cross(e2).normalize();
    if normal.dot(ray.direction) > 0.0 {
        normal = -normal;
    } // always face the ray
    Some((t, normal))
}
