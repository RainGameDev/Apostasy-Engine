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

    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        let x = value.get("cell_x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let y = value.get("cell_y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let z = value.get("cell_z").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        self.cell_coord = Vector3::new(x, y, z);

        if let Some(res) = value.get("resolution").and_then(|v| v.as_u64()) {
            let res = res as u32;
            let side = (res + 1) as usize;
            let count = side * side;
            self.resolution = res;
            self.heights = vec![0.0; count];
            self.texture_weights = vec![[255, 0, 0, 0]; count];
        }

        if let Some(hex) = value.get("heights").and_then(|v| v.as_str()) {
            let decoded = hex_to_f32s(hex);
            if !decoded.is_empty() {
                self.heights = decoded;
            }
        }

        if let Some(hex) = value.get("texture_weights").and_then(|v| v.as_str()) {
            let decoded = hex_to_weights(hex);
            if !decoded.is_empty() {
                self.texture_weights = decoded;
            }
        }

        Ok(())
    }
}

pub fn f32s_to_hex(values: &[f32]) -> String {
    let mut s = String::with_capacity(values.len() * 8);
    for &v in values {
        let [a, b, c, d] = v.to_le_bytes();
        s.push_str(&format!("{:02x}{:02x}{:02x}{:02x}", a, b, c, d));
    }
    s
}

fn hex_to_f32s(hex: &str) -> Vec<f32> {
    hex.as_bytes()
        .chunks_exact(8)
        .filter_map(|c| {
            let s = std::str::from_utf8(c).ok()?;
            let n = u32::from_str_radix(s, 16).ok()?;
            Some(f32::from_bits(n))
        })
        .collect()
}

pub fn weights_to_hex(weights: &[[u8; 4]]) -> String {
    let mut s = String::with_capacity(weights.len() * 8);
    for w in weights {
        s.push_str(&format!("{:02x}{:02x}{:02x}{:02x}", w[0], w[1], w[2], w[3]));
    }
    s
}

fn hex_to_weights(hex: &str) -> Vec<[u8; 4]> {
    hex.as_bytes()
        .chunks_exact(8)
        .filter_map(|c| {
            let s = std::str::from_utf8(c).ok()?;
            let b0 = u8::from_str_radix(&s[0..2], 16).ok()?;
            let b1 = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b2 = u8::from_str_radix(&s[4..6], 16).ok()?;
            let b3 = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some([b0, b1, b2, b3])
        })
        .collect()
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
