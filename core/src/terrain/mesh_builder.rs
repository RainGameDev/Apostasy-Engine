use anyhow::Result;
use ash::vk::CommandPool;
use cgmath::Vector3;

use crate::{
    objects::cell::CELL_SIZE,
    rendering::{shared::vertex::Vertex, vulkan::rendering_context::VulkanRenderingContext},
};

use super::chunk::{TerrainChunk, TerrainMesh};

/// Border data from neighboring chunks used for seamless normals.
pub struct NeighborBorders {
    pub left_col: Option<Vec<f32>>,
    pub right_col: Option<Vec<f32>>,
    pub top_row: Option<Vec<f32>>,
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

/// Builds a terrain mesh using per-triangle vertices (6 per quad, no sharing).
///
/// Per-vertex weights are copied directly from chunk.vertex_weights.
/// The shader samples the texture atlas using the chunk's active_layer_ids
/// (passed via push constants) and blends by the vertex weights.
pub fn build_terrain_mesh(
    chunk: &TerrainChunk,
    neighbors: &NeighborBorders,
    context: &VulkanRenderingContext,
    command_pool: CommandPool,
) -> Result<TerrainMesh> {
    let r = chunk.resolution as usize;
    let side = r + 1;
    let cell_size = CELL_SIZE as f32;
    let step = cell_size / r as f32;

    let origin_x = chunk.cell_coord.x as f32 * cell_size;
    let origin_z = chunk.cell_coord.z as f32 * cell_size;

    // Pre-compute per-grid-position data
    let mut grid_positions: Vec<[f32; 3]> = Vec::with_capacity(side * side);
    let mut grid_normals: Vec<[f32; 3]> = Vec::with_capacity(side * side);

    for z in 0..side {
        for x in 0..side {
            let h = chunk.height_at(x, z);
            let wx = origin_x + x as f32 * step;
            let wz = origin_z + z as f32 * step;

            let normal = compute_normal(chunk, x, z, step, neighbors);

            grid_positions.push([wx, h, wz]);
            grid_normals.push([normal.x, normal.y, normal.z]);
        }
    }

    // Generate 6 per-triangle vertices per quad.
    let mut vertices: Vec<Vertex> = Vec::with_capacity(r * r * 6);

    for z in 0..r {
        for x in 0..r {
            let tl = x + z * side;
            let tr = (x + 1) + z * side;
            let bl = x + (z + 1) * side;
            let br = (x + 1) + (z + 1) * side;

            // Copy per-vertex weights directly from chunk data.
            // The shader will blend using chunk.active_layer_ids via push constants.
            let get_weights = |gi: usize| -> [f32; 32] {
                let mut default = [0.0f32; 32];
                default[0] = 1.0;
                chunk.vertex_weights.get(gi).copied().unwrap_or(default)
            };

            let get_color = |gi: usize| -> [f32; 3] {
                chunk.vertex_colors.get(gi).copied().unwrap_or([1.0, 1.0, 1.0])
            };

            // Triangle 1: tl, bl, tr
            for &gi in &[tl, bl, tr] {
                vertices.push(Vertex {
                    position: grid_positions[gi],
                    normal: grid_normals[gi],
                    tex_coord: [0.0; 2],
                    weights: get_weights(gi),
                    color: get_color(gi),
                });
            }

            // Triangle 2: tr, bl, br
            for &gi in &[tr, bl, br] {
                vertices.push(Vertex {
                    position: grid_positions[gi],
                    normal: grid_normals[gi],
                    tex_coord: [0.0; 2],
                    weights: get_weights(gi),
                    color: get_color(gi),
                });
            }
        }
    }

    // Sequential indices
    let indices: Vec<u32> = (0..vertices.len() as u32).collect();

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
