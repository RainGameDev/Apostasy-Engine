use anyhow::Result;
use ash::vk::{self, CommandPool};
use cgmath::Vector3;

use crate::{
    objects::cell::CELL_SIZE,
    rendering::{shared::vertex::Vertex, vulkan::rendering_context::VulkanRenderingContext},
};

use super::chunk::{TerrainChunk, TerrainMesh};

/// Handles to an existing terrain mesh whose buffers can be reused in-place.
pub struct ExistingMesh {
    pub vertex_buffer: vk::Buffer,
    pub vertex_buffer_memory: vk::DeviceMemory,
    pub index_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
}

/// Border heights from neighboring chunks, used to compute seamless normals at chunk edges.
pub struct NeighborBorders {
    /// x = (r-1) column of (cx-1, cz) hx_neg sample for vertices at this chunk's x=0 border.
    pub left_col: Option<Vec<f32>>,
    /// x = 1 column of (cx+1, cz) hx_pos sample for vertices at this chunk's x=r border.
    pub right_col: Option<Vec<f32>>,
    /// z = (r-1) row of (cx, cz-1) hz_neg sample for vertices at this chunk's z=0 border.
    pub top_row: Option<Vec<f32>>,
    /// z = 1 row of (cx, cz+1) hz_pos sample for vertices at this chunk's z=r border.
    pub bottom_row: Option<Vec<f32>>,
}

impl Default for NeighborBorders {
    fn default() -> Self {
        Self {
            left_col: None,
            right_col: None,
            top_row: None,
            bottom_row: None,
        }
    }
}

/// Builds or updates a terrain mesh.
pub fn build_terrain_mesh(
    chunk: &TerrainChunk,
    neighbors: &NeighborBorders,
    existing: Option<ExistingMesh>,
    context: &VulkanRenderingContext,
    command_pool: CommandPool,
) -> Result<TerrainMesh> {
    let r = chunk.resolution as usize;
    let side = r + 1;
    let cell_size = CELL_SIZE as f32;
    let step = cell_size / r as f32;

    let origin_x = chunk.cell_coord.x as f32 * cell_size;
    let origin_z = chunk.cell_coord.z as f32 * cell_size;

    let mut vertices: Vec<Vertex> = Vec::with_capacity(side * side);
    for z in 0..side {
        for x in 0..side {
            let h = chunk.height_at(x, z);
            let wx = origin_x + x as f32 * step;
            let wz = origin_z + z as f32 * step;

            let normal = compute_normal(chunk, x, z, step, neighbors);

            let u = x as f32 / r as f32 * 16.0;
            let v = z as f32 / r as f32 * 16.0;

            let ti = chunk
                .texture_index
                .get(x + z * side)
                .copied()
                .unwrap_or(0.0);
            vertices.push(Vertex {
                position: [wx, h, wz],
                normal: [normal.x, normal.y, normal.z],
                tex_coord: [u, v],
                texture_index: ti,
            });
        }
    }

    match existing {
        Some(e) => {
            // Fast path: write vertex data directly, reuse index buffer untouched.
            context.write_host_buffer(e.vertex_buffer_memory, &vertices)?;
            Ok(TerrainMesh {
                vertex_buffer: e.vertex_buffer,
                vertex_buffer_memory: e.vertex_buffer_memory,
                index_buffer: e.index_buffer,
                index_buffer_memory: e.index_buffer_memory,
                index_count: (r * r * 6) as u32,
                host_visible: true,
            })
        }
        None => {
            // First build: compute indices and allocate both buffers.
            let mut indices: Vec<u32> = Vec::with_capacity(r * r * 6);
            for z in 0..r {
                for x in 0..r {
                    let tl = (x + z * side) as u32;
                    let tr = (x + 1 + z * side) as u32;
                    let bl = (x + (z + 1) * side) as u32;
                    let br = (x + 1 + (z + 1) * side) as u32;
                    indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
                }
            }

            let (vb, vbm) = context.create_host_vertex_buffer::<Vertex>(vertices.len())?;
            context.write_host_buffer(vbm, &vertices)?;
            let (ib, ibm) = context.create_index_buffer(&indices, command_pool)?;

            Ok(TerrainMesh {
                vertex_buffer: vb,
                vertex_buffer_memory: vbm,
                index_buffer: ib,
                index_buffer_memory: ibm,
                index_count: indices.len() as u32,
                host_visible: true,
            })
        }
    }
}

/// Smooth vertex normal using central differences, sampling into neighbor chunks at borders.
fn compute_normal(
    chunk: &TerrainChunk,
    x: usize,
    z: usize,
    step: f32,
    neighbors: &NeighborBorders,
) -> Vector3<f32> {
    let r = chunk.resolution as usize;

    let hx_pos = if x < r {
        chunk.height_at(x + 1, z)
    } else if let Some(col) = &neighbors.right_col {
        col.get(z).copied().unwrap_or_else(|| chunk.height_at(x, z))
    } else {
        chunk.height_at(x, z)
    };

    let hx_neg = if x > 0 {
        chunk.height_at(x - 1, z)
    } else if let Some(col) = &neighbors.left_col {
        col.get(z).copied().unwrap_or_else(|| chunk.height_at(x, z))
    } else {
        chunk.height_at(x, z)
    };

    let hz_pos = if z < r {
        chunk.height_at(x, z + 1)
    } else if let Some(row) = &neighbors.bottom_row {
        row.get(x).copied().unwrap_or_else(|| chunk.height_at(x, z))
    } else {
        chunk.height_at(x, z)
    };

    let hz_neg = if z > 0 {
        chunk.height_at(x, z - 1)
    } else if let Some(row) = &neighbors.top_row {
        row.get(x).copied().unwrap_or_else(|| chunk.height_at(x, z))
    } else {
        chunk.height_at(x, z)
    };

    let n = Vector3::new(
        -(hx_pos - hx_neg) / (step * 2.0),
        1.0,
        -(hz_pos - hz_neg) / (step * 2.0),
    );
    let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
    if len > 0.0 {
        Vector3::new(n.x / len, n.y / len, n.z / len)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    }
}
