use apostasy_macros::{Component, Inspect, Tag};
use ash::vk;
use cgmath::Vector3;

use crate::{
    objects::cell::{CellCoord, CELL_SIZE},
    rendering::shared::model::GpuMesh,
};

/// Heightmap data for a single terrain cell.
#[derive(Debug, Inspect, Component, Clone)]
pub struct TerrainChunk {
    /// Cell coordinate this terrain piece occupies.
    pub cell_coord: CellCoord,
    /// Vertices per side (default 128). Vertex count is (resolution+1)².
    pub resolution: u32,
    /// Flattened (resolution+1)² heightmap, row-major (x + z*(resolution+1)).
    pub heights: Vec<f32>,
    /// Per-vertex RGBA blend weights for up to 4 texture layers.
    pub texture_weights: Vec<[u8; 4]>,
}

impl Default for TerrainChunk {
    fn default() -> Self {
        Self::new(Vector3::new(0, 0, 0), 128)
    }
}

impl TerrainChunk {
    pub fn new(cell_coord: CellCoord, resolution: u32) -> Self {
        let side = (resolution + 1) as usize;
        let count = side * side;
        Self {
            cell_coord,
            resolution,
            heights: vec![0.0; count],
            texture_weights: vec![[255, 0, 0, 0]; count],
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

    /// World-space origin of this cell (top-left corner, Y=0).
    pub fn world_origin(&self) -> (f32, f32) {
        (
            self.cell_coord.x as f32 * CELL_SIZE as f32,
            self.cell_coord.z as f32 * CELL_SIZE as f32,
        )
    }

    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
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
}

impl TerrainMesh {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
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

/// Tag placed on terrain objects that need their GPU mesh rebuilt.
#[derive(Tag, Clone)]
pub struct NeedsTerrainRebuild;
