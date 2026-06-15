use anyhow::Result;
use ash::vk::CommandPool;
use cgmath::Vector3;

use crate::{
    objects::cell::CELL_SIZE,
    rendering::{shared::vertex::Vertex, vulkan::rendering_context::VulkanRenderingContext},
};

use super::chunk::{TerrainChunk, TerrainMesh};

/// Builds a CPU-side vertex/index list from a TerrainChunk and uploads it to GPU.
pub fn build_terrain_mesh(
    chunk: &TerrainChunk,
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

            let normal = compute_normal(chunk, x, z, step);

            // UV: tile once per cell
            let u = x as f32 / r as f32;
            let v = z as f32 / r as f32;

            vertices.push(Vertex {
                position: [wx, h, wz],
                normal: [normal.x, normal.y, normal.z],
                tex_coord: [u, v],
            });
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity(r * r * 6);
    for z in 0..r {
        for x in 0..r {
            let tl = (x + z * side) as u32;
            let tr = (x + 1 + z * side) as u32;
            let bl = (x + (z + 1) * side) as u32;
            let br = (x + 1 + (z + 1) * side) as u32;
            // two triangles per quad
            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    let (vb, vbm) = context.create_vertex_buffer(&vertices, command_pool)?;
    let (ib, ibm) = context.create_index_buffer(&indices, command_pool)?;

    Ok(TerrainMesh {
        vertex_buffer: vb,
        vertex_buffer_memory: vbm,
        index_buffer: ib,
        index_buffer_memory: ibm,
        index_count: indices.len() as u32,
    })
}

/// Computes a smooth vertex normal using central differences of neighboring heights.
fn compute_normal(chunk: &TerrainChunk, x: usize, z: usize, step: f32) -> Vector3<f32> {
    let r = chunk.resolution as usize;

    let hx_pos = if x < r { chunk.height_at(x + 1, z) } else { chunk.height_at(x, z) };
    let hx_neg = if x > 0 { chunk.height_at(x - 1, z) } else { chunk.height_at(x, z) };
    let hz_pos = if z < r { chunk.height_at(x, z + 1) } else { chunk.height_at(x, z) };
    let hz_neg = if z > 0 { chunk.height_at(x, z - 1) } else { chunk.height_at(x, z) };

    let dx = if x == 0 || x == r { step * 2.0 } else { step * 2.0 };
    let dz = if z == 0 || z == r { step * 2.0 } else { step * 2.0 };

    let n = Vector3::new(
        -(hx_pos - hx_neg) / dx,
        1.0,
        -(hz_pos - hz_neg) / dz,
    );

    let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
    if len > 0.0 { Vector3::new(n.x / len, n.y / len, n.z / len) } else { Vector3::new(0.0, 1.0, 0.0) }
}
