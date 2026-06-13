use cgmath::{InnerSpace, Quaternion, Vector3};

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
