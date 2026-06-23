use cgmath::{Deg, InnerSpace, Matrix4, PerspectiveFov, Point3, Quaternion, SquareMatrix, Vector3};

use crate::{
    ecs::components::transform::Transform,
    rendering::components::lighting::{Light, LightType},
};

pub const LIGHT_TYPE_DIRECTIONAL: u32 = 0;
pub const LIGHT_TYPE_POINT: u32 = 1;
pub const LIGHT_TYPE_SPOT: u32 = 2;

pub const MAX_LIGHTS: usize = 64;
pub const CSM_CASCADE_COUNT: usize = 4;

/// Shadow data passed to `set_lights` for directional and spot lights.
pub struct ShadowData {
    /// One matrix per cascade (4 for directional, 1 for spot - spot replicates into all 4 slots).
    pub matrices: Vec<[[f32; 4]; 4]>,
    /// View-distance threshold for each cascade (positive, increasing).
    pub splits: [f32; 4],
    /// 4 for directional CSM, 1 for spot.
    pub cascade_count: u32,
    /// Index of this light in the gpu_lights array - the shader only applies the shadow to this light.
    pub shadow_light_index: u32,
}

/// Shadow data for a single point light (omnidirectional cube shadow map).
#[derive(Clone)]
pub struct PointShadowData {
    /// Index of the point light in the gpu_lights array.
    pub light_index: u32,
    /// Far plane (matches the light's attenuation radius).
    pub far: f32,
    /// View-projection matrices for each of the 6 cubemap faces (+X, -X, +Y, -Y, +Z, -Z).
    pub face_matrices: [[[f32; 4]; 4]; 6],
}

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

/// Vulkan NDC correction, flips Y and remaps depth from [-1,1] to [0,1].
fn vulkan_correction() -> Matrix4<f32> {
    Matrix4::new(
        1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
    )
}

/// Depth only Vulkan correction, remaps depth from [-1,1] to [0,1] without flipping Y.
fn vulkan_depth_correction() -> Matrix4<f32> {
    Matrix4::new(
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
    )
}

/// Returns 4 cascade light space matrices and their corresponding split distances for a directional light.
/// Only valid for `LightType::Directional`.
pub fn compute_csm_matrices(
    light: &Light,
    light_transform: &Transform,
    camera_pos: Vector3<f32>,
    fov_y_degrees: f32,
    aspect: f32,
    camera_near: f32,
    camera_far: f32,
    shadow_distance: f32,
    cascade_count: usize,
    shadow_map_size: u32,
) -> ([Matrix4<f32>; CSM_CASCADE_COUNT], [f32; CSM_CASCADE_COUNT]) {
    let count = cascade_count.clamp(1, CSM_CASCADE_COUNT);

    let light_dir = rotate_vec3(
        light_transform.global_rotation,
        Vector3::new(0.0, 0.0, -1.0),
    );
    let light_up = if light_dir.y.abs() > 0.99 {
        Vector3::unit_z()
    } else {
        Vector3::unit_y()
    };

    // Light bias, matches the axes used for rendering.
    let light_s = light_dir.cross(light_up).normalize();
    let light_u = light_s.cross(light_dir);

    let tan_half_fovy = (fov_y_degrees.to_radians() * 0.5).tan();
    let far_clip = shadow_distance.min(camera_far);
    let lambda = 0.75_f32;

    let mut splits = [far_clip; CSM_CASCADE_COUNT];
    for i in 0..count {
        let p = (i as f32 + 1.0) / count as f32;
        let log_split = camera_near * (far_clip / camera_near).powf(p);
        let uni_split = camera_near + (far_clip - camera_near) * p;
        splits[i] = lambda * log_split + (1.0 - lambda) * uni_split;
    }

    let mut matrices = [Matrix4::identity(); CSM_CASCADE_COUNT];
    let mut prev_near = camera_near;

    for i in 0..count {
        let _sub_near = prev_near;
        let sub_far = splits[i];
        prev_near = sub_far;

        // frustum from the camera mapped into a sphere
        let far_h = tan_half_fovy * sub_far;
        let far_w = far_h * aspect;
        let radius = (sub_far * sub_far + far_h * far_h + far_w * far_w).sqrt();

        // snaps the cameras position to a texel grid
        let units_per_texel = (2.0 * radius) / shadow_map_size as f32;
        let snapped_s = (camera_pos.dot(light_s) / units_per_texel).floor() * units_per_texel;
        let snapped_u = (camera_pos.dot(light_u) / units_per_texel).floor() * units_per_texel;
        let center =
            light_s * snapped_s + light_u * snapped_u + light_dir * camera_pos.dot(light_dir);

        let light_pos = center - light_dir * radius;
        let view = Matrix4::look_at_rh(
            Point3::new(light_pos.x, light_pos.y, light_pos.z),
            Point3::new(center.x, center.y, center.z),
            light_up,
        );
        // Extend the depth range to capture shadow casters outside the frustum.
        let ortho = cgmath::ortho(
            -radius,
            radius,
            -radius,
            radius,
            -radius * 3.0,
            radius * 3.0,
        );
        matrices[i] = vulkan_correction() * ortho * view;
    }
    // Fill unused cascade slots with the last valid matrix so the shader never samples garbage.
    for i in count..CSM_CASCADE_COUNT {
        matrices[i] = matrices[count - 1];
    }

    let _ = light;
    (matrices, splits)
}

/// Returns the light space view projection matrix used for shadow map rendering and sampling.
pub fn compute_light_space_matrix(
    light: &Light,
    light_transform: &Transform,
    camera_pos: Vector3<f32>,
    shadow_distance: f32,
) -> Matrix4<f32> {
    let dir = rotate_vec3(
        light_transform.global_rotation,
        Vector3::new(0.0, 0.0, -1.0),
    );
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
            let proj: Matrix4<f32> = PerspectiveFov {
                fovy: Deg(angle * 2.0).into(),
                aspect: 1.0,
                near: 0.1,
                far: length,
            }
            .into();
            vulkan_correction() * proj * view
        }
        _ => Matrix4::identity(),
    }
}

/// Computes 6 view-projection matrices for omnidirectional point light shadow mapping.
/// Each matrix covers one face of the cube.
pub fn compute_point_shadow_matrices(light_pos: Vector3<f32>, far: f32) -> [Matrix4<f32>; 6] {
    let near = 0.05_f32;
    let proj: Matrix4<f32> = PerspectiveFov {
        fovy: Deg(90.0).into(),
        aspect: 1.0,
        near,
        far,
    }
    .into();
    let dc = vulkan_depth_correction();
    let lp = Point3::new(light_pos.x, light_pos.y, light_pos.z);

    // Standard cubemap face  pairs.
    let faces: [(Vector3<f32>, Vector3<f32>); 6] = [
        (Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, -1.0, 0.0)),
        (Vector3::new(-1.0, 0.0, 0.0), Vector3::new(0.0, -1.0, 0.0)),
        (Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, 0.0, 1.0)),
        (Vector3::new(0.0, -1.0, 0.0), Vector3::new(0.0, 0.0, -1.0)),
        (Vector3::new(0.0, 0.0, 1.0), Vector3::new(0.0, -1.0, 0.0)),
        (Vector3::new(0.0, 0.0, -1.0), Vector3::new(0.0, -1.0, 0.0)),
    ];

    let mut matrices = [Matrix4::identity(); 6];
    for (i, (dir, up)) in faces.iter().enumerate() {
        let view = Matrix4::look_at_rh(lp, lp + dir, *up);
        matrices[i] = dc * proj * view;
    }
    matrices
}
