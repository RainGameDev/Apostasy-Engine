use apostasy_macros::{Component, Inspect, Tag};
use ash::vk;
use cgmath::Vector3;

use crate::{
    ecs::cell::{CELL_SIZE, CellCoord},
    rendering::shared::model::GpuMesh,
};

pub const MAX_ACTIVE_LAYERS: u8 = 32;

/// Heightmap data for a single terrain cell.
#[derive(Debug, Inspect, Component, Clone)]
pub struct TerrainChunk {
    /// Cell coordinate this terrain piece occupies.
    pub cell_coord: CellCoord,
    /// Vertices per side. Vertex count is (resolution+1)^2.
    pub resolution: u32,
    /// Flattened (resolution+1)^2 heightmap.
    pub heights: Vec<f32>,
    /// Active texture layer global IDs for this chunk (indices into TerrainSettings.texture_layers).
    /// 0 = unused slot. Sorted so slot 0 is always the base layer.
    pub active_layer_ids: [u32; MAX_ACTIVE_LAYERS as usize],
    /// How many of the 32 slots are active.
    pub active_layer_count: u8,
    /// Per-vertex weights for each active layer. Always sums to 1.0 per vertex.
    /// Length is (resolution+1)^2, each entry is [f32; 32].
    pub vertex_weights: Vec<[f32; MAX_ACTIVE_LAYERS as usize]>,
    /// Per-vertex RGB tint color. Multiplied over the splat texture in the shader.
    /// Default is white [1,1,1] (no tint). Length is (resolution+1)^2.
    pub vertex_colors: Vec<[f32; 3]>,
}

impl Default for TerrainChunk {
    fn default() -> Self {
        Self::new(Vector3::new(0, 0, 0), 32)
    }
}

impl TerrainChunk {
    pub fn new(cell_coord: CellCoord, resolution: u32) -> Self {
        let side = (resolution + 1) as usize;
        let count = side * side;
        let mut default_weights = [0.0f32; MAX_ACTIVE_LAYERS as usize];
        default_weights[0] = 1.0;
        let weights = vec![default_weights; count];
        Self {
            cell_coord,
            resolution,
            heights: vec![0.0; count],
            active_layer_ids: [0; MAX_ACTIVE_LAYERS as usize],
            active_layer_count: 1,
            vertex_weights: weights,
            vertex_colors: vec![[1.0, 1.0, 1.0]; count],
        }
    }

    /// Find the slot index for a given global layer ID, or allocate a new slot if available.
    /// Returns None if no slot is free and the layer is not already present.
    pub fn find_or_allocate_slot(&mut self, global_layer_id: u32) -> Option<usize> {
        // Check if already present
        for i in 0..self.active_layer_count as usize {
            if self.active_layer_ids[i] == global_layer_id {
                return Some(i);
            }
        }
        // Allocate a new slot
        let count = self.active_layer_count as usize;
        if count < MAX_ACTIVE_LAYERS as usize {
            self.active_layer_ids[count] = global_layer_id;
            self.active_layer_count += 1;
            Some(count)
        } else {
            None
        }
    }

    #[inline]
    pub fn height_at(&self, x: usize, z: usize) -> f32 {
        let side = (self.resolution + 1) as usize;
        self.heights[x + z * side]
    }

    #[inline]
    pub fn height_at_mut(&mut self, x: usize, z: usize) -> &mut f32 {
        let side = (self.resolution + 1) as usize;
        &mut self.heights[x + z * side]
    }

    /// World space origin of this cell (top-left corner, Y=0).
    pub fn world_origin(&self) -> (f32, f32) {
        (
            self.cell_coord.x as f32 * CELL_SIZE as f32,
            self.cell_coord.z as f32 * CELL_SIZE as f32,
        )
    }

}

/// GPU mesh buffers for a terrain chunk.
#[derive(Debug, Inspect, Component, Clone, Default)]
pub struct TerrainMesh {
    pub vertex_buffer: vk::Buffer,
    pub vertex_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
    pub index_count: u32,
    pub host_visible: bool,
}


impl GpuMesh for TerrainMesh {
    fn get_vertex_buffer(&self) -> vk::Buffer {
        self.vertex_buffer
    }
    fn get_index_buffer(&self) -> vk::Buffer {
        self.index_buffer
    }
    fn get_index_count(&self) -> u32 {
        self.index_count
    }
}

/// Tag placed on terrain entities that need their GPU mesh rebuilt.
#[derive(Tag, Clone)]
pub struct NeedsTerrainRebuild;
