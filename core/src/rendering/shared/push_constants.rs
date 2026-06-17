use std::mem::transmute;

use apostasy_macros::Resource;
use cgmath::{Matrix4, Quaternion, SquareMatrix, Vector3, Zero};

use crate::{
    objects::{Object, components::transform::Transform},
    rendering::components::camera::{Camera, get_perspective_projection, get_view_matrix},
};

#[derive(Resource, Clone, Debug)]
pub struct PushConstants {
    pub view_matrix: Matrix4<f32>,
    pub projection_matrix: Matrix4<f32>,
    pub model_matrix: Matrix4<f32>,
}

impl Default for PushConstants {
    fn default() -> Self {
        Self {
            view_matrix: Matrix4::identity(),
            projection_matrix: Matrix4::identity(),
            model_matrix: Matrix4::identity(),
        }
    }
}

impl PushConstants {
    #[allow(unnecessary_transmutes)]
    pub fn return_renderable(&self) -> Vec<u8> {
        unsafe {
            let mut data = Vec::with_capacity(128);
            let proj_view: [u8; 64] = transmute(self.projection_matrix * self.view_matrix);
            let model: [u8; 64] = transmute(self.model_matrix);
            data.extend_from_slice(&proj_view); // offset 0,  64 bytes
            data.extend_from_slice(&model); // offset 64, 64 bytes
            data // 128 bytes total
        }
    }

    pub fn set_camera_constants(&mut self, camera: Object, aspect: f32) {
        let transform = camera.get_component::<Transform>().unwrap();
        let cam = camera.get_component::<Camera>().unwrap();
        self.view_matrix = get_view_matrix(transform);
        self.projection_matrix = get_perspective_projection(cam, aspect);
        self.model_matrix = Matrix4::identity();
    }
}

#[derive(Resource, Clone, Debug)]
pub struct ModelPushConstants {
    pub world_position: Vector3<f32>,
    pub world_scale: Vector3<f32>,
    pub world_rotation: Quaternion<f32>,
    pub color_modifier: [f32; 4],
    /// Active terrain layer global IDs for the terrain splatting shader (up to 6).
    /// Unused by non-terrain shaders.
    pub active_layer_ids: [u32; 6],
    /// How many of active_layer_ids are actually in use.
    pub layer_count: u32,
}

impl Default for ModelPushConstants {
    fn default() -> Self {
        Self {
            world_position: Vector3::zero(),
            world_scale: Vector3::new(1.0, 1.0, 1.0),
            world_rotation: Quaternion::new(1.0, 0.0, 0.0, 0.0),
            color_modifier: [1.0, 1.0, 1.0, 1.0],
            active_layer_ids: [0; 6],
            layer_count: 1,
        }
    }
}

impl ModelPushConstants {
    #[allow(unnecessary_transmutes)]
    pub fn return_renderable(&self) -> Vec<u8> {
        unsafe {
            let mut data = Vec::with_capacity(92);
            let position: [u8; 12] = transmute(self.world_position);
            let scale: [u8; 12] = transmute(self.world_scale);
            let rotation: [u8; 16] = transmute(self.world_rotation);
            let color: [u8; 16] = transmute(self.color_modifier);
            let layer_ids: [u8; 24] = transmute(self.active_layer_ids);
            let layer_count: [u8; 4] = transmute(self.layer_count);
            let pad: [u8; 4] = [0u8; 4];
            data.extend_from_slice(&position);
            data.extend_from_slice(&pad);
            data.extend_from_slice(&scale);
            data.extend_from_slice(&pad);
            data.extend_from_slice(&rotation);
            data.extend_from_slice(&color);
            data.extend_from_slice(&layer_ids);
            data.extend_from_slice(&layer_count);
            data // 92 bytes total
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct VoxelPushConstants {
    pub atlas_tiles: u32, // how many tiles per row in the atlas
    pub world_position: Vector3<i32>,
    pub time: f32,
}

impl Default for VoxelPushConstants {
    fn default() -> Self {
        Self {
            atlas_tiles: 1,
            world_position: Vector3::zero(),
            time: 0.0,
        }
    }
}

impl VoxelPushConstants {
    #[allow(unnecessary_transmutes)]
    pub fn return_renderable(&self) -> Vec<u8> {
        unsafe {
            let mut data = Vec::with_capacity(28);
            let atlas: [u8; 4] = transmute(self.atlas_tiles);
            let pad: [u8; 12] = [0u8; 12];
            let position: [u8; 12] = transmute(self.world_position);
            let time: [u8; 4] = transmute(self.time);
            data.extend_from_slice(&atlas);
            data.extend_from_slice(&pad);
            data.extend_from_slice(&position);
            data.extend_from_slice(&time);
            data // 32 bytes total
        }
    }

    pub fn set_position(&mut self, position: Vector3<i32>) {
        self.world_position = position;
    }

    pub fn set_atlas_tiles(&mut self, tiles: u32) {
        self.atlas_tiles = tiles;
    }
}

/// Push constants for the depth-only shadow pass for model geometry (112 bytes).
/// Layout must match `sdr_default_shadow_model.vert`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ShadowModelPushConstants {
    pub light_space: [[f32; 4]; 4],
    pub pos: [f32; 3],
    _pad0: f32,
    pub scale: [f32; 3],
    _pad1: f32,
    pub rotation: [f32; 4],
}

impl ShadowModelPushConstants {
    pub fn new(
        light_space: [[f32; 4]; 4],
        pos: [f32; 3],
        scale: [f32; 3],
        rotation: [f32; 4],
    ) -> Self {
        Self { light_space, pos, _pad0: 0.0, scale, _pad1: 0.0, rotation }
    }

    #[allow(unnecessary_transmutes)]
    pub fn return_renderable(&self) -> Vec<u8> {
        unsafe {
            let bytes: [u8; 112] = transmute(*self);
            bytes.to_vec()
        }
    }
}

/// Push constants for the depth-only shadow pass for voxel geometry (80 bytes).
/// Layout must match `sdr_default_shadow_voxel.vert`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ShadowVoxelPushConstants {
    pub light_space: [[f32; 4]; 4],
    pub world_pos: [i32; 3],
    _pad: i32,
}

impl ShadowVoxelPushConstants {
    pub fn new(light_space: [[f32; 4]; 4], world_pos: [i32; 3]) -> Self {
        Self { light_space, world_pos, _pad: 0 }
    }

    #[allow(unnecessary_transmutes)]
    pub fn return_renderable(&self) -> Vec<u8> {
        unsafe {
            let bytes: [u8; 80] = transmute(*self);
            bytes.to_vec()
        }
    }
}

/// Push constants for the point-light cubemap shadow pass for model geometry (128 bytes).
/// light_pos/far come first so the fragment shader can use a single shared layout.
/// Layout must match `sdr_default_shadow_point_model.vert` and `sdr_default_shadow_point.frag`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ShadowPointModelPushConstants {
    pub light_pos: [f32; 3],
    pub far: f32,
    pub light_space: [[f32; 4]; 4],
    pub pos: [f32; 3],
    _pad0: f32,
    pub scale: [f32; 3],
    _pad1: f32,
    pub rotation: [f32; 4],
}

impl ShadowPointModelPushConstants {
    pub fn new(
        light_pos: [f32; 3],
        far: f32,
        light_space: [[f32; 4]; 4],
        pos: [f32; 3],
        scale: [f32; 3],
        rotation: [f32; 4],
    ) -> Self {
        Self { light_pos, far, light_space, pos, _pad0: 0.0, scale, _pad1: 0.0, rotation }
    }

    #[allow(unnecessary_transmutes)]
    pub fn return_renderable(&self) -> Vec<u8> {
        unsafe {
            let bytes: [u8; 128] = transmute(*self);
            bytes.to_vec()
        }
    }
}

/// Push constants for the point-light cubemap shadow pass for voxel geometry (96 bytes).
/// Layout must match `sdr_default_shadow_point_voxel.vert` and `sdr_default_shadow_point.frag`.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ShadowPointVoxelPushConstants {
    pub light_pos: [f32; 3],
    pub far: f32,
    pub light_space: [[f32; 4]; 4],
    pub world_pos: [i32; 3],
    _pad: i32,
}

impl ShadowPointVoxelPushConstants {
    pub fn new(
        light_pos: [f32; 3],
        far: f32,
        light_space: [[f32; 4]; 4],
        world_pos: [i32; 3],
    ) -> Self {
        Self { light_pos, far, light_space, world_pos, _pad: 0 }
    }

    #[allow(unnecessary_transmutes)]
    pub fn return_renderable(&self) -> Vec<u8> {
        unsafe {
            let bytes: [u8; 96] = transmute(*self);
            bytes.to_vec()
        }
    }
}
