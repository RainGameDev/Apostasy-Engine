use cgmath::{Deg, InnerSpace, Matrix4, PerspectiveFov, Point3, Quaternion, SquareMatrix, Vector3};

use crate::{
    objects::components::transform::Transform,
    rendering::components::lighting::{Light, LightType},
};

pub const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
pub const LIGHT_TYPE_POINT: u32 = 1;
pub const LIGHT_TYPE_SPOT: u32 = 2;

pub const MAX_LIGHTS: usize = 64;

/// GPU representation of a light source.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GpuLight {
    pub position: [f32; 4],  // Offset 0
    pub direction: [f32; 4], // Offset 16
    pub color: [f32; 3],     // offset 32
    pub intensity: f32,      // offset 44
    pub light_type: u32,     // offset 48
    pub radius: f32,         // offset 52
    pub angle_cos: f32,      // offset 56
    pub length: f32,         // offset 60
}

impl GpuLight {
    pub fn from_component(light: &Light, transform: &Transform) -> Self {
        let pos = transform.global_position;
        let dir = rotate_vec3(transform.global_rotation, Vector3::new(0.0, 0.0, -1.0));

        match light.light_type {
            LightType::Directional => Self {
                position: [pos.x, pos.y, pos.z, 0.0],
                direction: [dir.x, dir.y, dir.z, 0.0],
                color: light.color.into(),
                intensity: light.intensity,
                light_type: LIGHT_TYPE_DIRECTIONAL,
                radius: 0.0,
                angle_cos: 0.0,
                length: 0.0,
            },
            LightType::Point { radius } => Self {
                position: [pos.x, pos.y, pos.z, 0.0],
                direction: [0.0, 0.0, 0.0, 0.0],
                color: light.color.into(),
                intensity: light.intensity,
                light_type: LIGHT_TYPE_POINT,
                radius,
                angle_cos: 0.0,
                length: 0.0,
            },
            LightType::Spot { length, angle } => Self {
                position: [pos.x, pos.y, pos.z, 0.0],
                direction: [dir.x, dir.y, dir.z, 0.0],
                color: light.color.into(),
                intensity: light.intensity,
                light_type: LIGHT_TYPE_SPOT,
                radius: length,
                angle_cos: (angle.to_radians() / 2.0).cos(),
                length,
            },
        }
    }
}

fn rotate_vec3(q: Quaternion<f32>, v: Vector3<f32>) -> Vector3<f32> {
    let t = q.v.cross(v) * 2.0;
    (v + t * q.s + q.v.cross(t)).normalize()
}

/// Vulkan NDC correction: flips Y and remaps depth from [-1,1] to [0,1].
fn vulkan_correction() -> Matrix4<f32> {
    Matrix4::new(
        1.0, 0.0, 0.0, 0.0,
        0.0, -1.0, 0.0, 0.0,
        0.0, 0.0, 0.5, 0.0,
        0.0, 0.0, 0.5, 1.0,
    )
}

/// Returns the light-space view-projection matrix used for shadow map rendering and sampling.
/// Returns identity for light types that don't support single-map shadows (e.g. Point).
pub fn compute_light_space_matrix(
    light: &Light,
    light_transform: &Transform,
    camera_pos: Vector3<f32>,
    shadow_distance: f32,
) -> Matrix4<f32> {
    let dir = rotate_vec3(light_transform.global_rotation, Vector3::new(0.0, 0.0, -1.0));
    let up = if dir.y.abs() > 0.99 {
        Vector3::unit_z()
    } else {
        Vector3::unit_y()
    };

    match light.light_type {
        LightType::Directional => {
            let light_pos = camera_pos - dir * shadow_distance;
            let view = Matrix4::look_at_rh(
                Point3::new(light_pos.x, light_pos.y, light_pos.z),
                Point3::new(camera_pos.x, camera_pos.y, camera_pos.z),
                up,
            );
            let s = shadow_distance;
            let ortho = cgmath::ortho(-s, s, -s, s, -s * 2.0, s * 2.0);
            vulkan_correction() * ortho * view
        }
        LightType::Spot { angle, length } => {
            let pos = light_transform.global_position;
            let view = Matrix4::look_at_rh(
                Point3::new(pos.x, pos.y, pos.z),
                Point3::new(pos.x + dir.x, pos.y + dir.y, pos.z + dir.z),
                up,
            );
            let mut proj: Matrix4<f32> = PerspectiveFov {
                fovy: Deg(angle * 2.0).into(),
                aspect: 1.0,
                near: 0.1,
                far: length,
            }
            .into();
            proj[1][1] *= -1.0;
            proj * view
        }
        _ => Matrix4::identity(),
    }
}
