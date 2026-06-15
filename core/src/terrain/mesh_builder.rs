use anyhow::Result;
use ash::vk::CommandPool;
use cgmath::Vector3;

use crate::{
    objects::cell::CELL_SIZE,
    rendering::{shared::vertex::Vertex, vulkan::rendering_context::VulkanRenderingContext},
};

use super::chunk::{TerrainChunk, TerrainMesh};

/// Border data from neighboring chunks used for seamless normals and texture blending.
pub struct NeighborBorders {
    // Heights: one step inside each neighbor for central-difference normal computation.
    pub left_col: Option<Vec<f32>>,
    pub right_col: Option<Vec<f32>>,
    pub top_row: Option<Vec<f32>>,
    pub bottom_row: Option<Vec<f32>>,
    // Texture indices: the neighbor's edge column/row, for cross-chunk blend computation.
    pub left_tex: Option<Vec<f32>>,
    pub right_tex: Option<Vec<f32>>,
    pub top_tex: Option<Vec<f32>>,
    pub bottom_tex: Option<Vec<f32>>,
}

impl Default for NeighborBorders {
    fn default() -> Self {
        Self {
            left_col: None,
            right_col: None,
            top_row: None,
            bottom_row: None,
            left_tex: None,
            right_tex: None,
            top_tex: None,
            bottom_tex: None,
        }
    }
}

/// Builds a terrain mesh using per-triangle vertices (6 per quad, no sharing).
///
/// Each vertex stores integer texture layers (layer_a, layer_b) and a blend factor
/// derived purely from how many of its 8-adjacent grid neighbors (cardinal + diagonal)
/// have a different layer. This means the blend is 0 deep inside a texture region and
/// rises toward 0.5 at the one-vertex-wide boundary, letting the GPU interpolate a
/// smooth gradient across each transition quad. The fragment shader then adds
/// world-space noise on top to break up any remaining straight edges.
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
    let mut grid_uvs: Vec<[f32; 2]> = Vec::with_capacity(side * side);
    let mut grid_layers: Vec<i32> = Vec::with_capacity(side * side);

    for z in 0..side {
        for x in 0..side {
            let h = chunk.height_at(x, z);
            let wx = origin_x + x as f32 * step;
            let wz = origin_z + z as f32 * step;

            let normal = compute_normal(chunk, x, z, step, neighbors);

            let u = x as f32 / r as f32 * 16.0;
            let v = z as f32 / r as f32 * 16.0;

            let layer = chunk
                .texture_index
                .get(x + z * side)
                .copied()
                .unwrap_or(0.0)
                .round() as i32;

            grid_positions.push([wx, h, wz]);
            grid_normals.push([normal.x, normal.y, normal.z]);
            grid_uvs.push([u, v]);
            grid_layers.push(layer);
        }
    }

    // Pre-compute per-vertex neighbor-based blend weights.
    // Counts how many of the 8 surrounding neighbors (cardinal + diagonal) have a
    // different integer layer, then divides by 4 (not by total) so diagonal differences
    // contribute to the blend without diluting it.  A vertex with one different diagonal
    // neighbor gets weight 0.25, same as one different cardinal neighbor.
    let mut blend_weights: Vec<f32> = Vec::with_capacity(side * side);
    for z in 0..side {
        for x in 0..side {
            let own = grid_layers[x + z * side];
            let mut diff = 0u32;
            for (dx, dz) in [
                (-1i32, 0i32), (1, 0), (0, -1), (0, 1),
            ] {
                let nx = x as i32 + dx;
                let nz = z as i32 + dz;
                let neighbor_layer: Option<i32> = if nx >= 0
                    && nz >= 0
                    && (nx as usize) < side
                    && (nz as usize) < side
                {
                    Some(grid_layers[nx as usize + nz as usize * side])
                } else if dx != 0 && dz == 0 {
                    if nx < 0 {
                        neighbors.left_tex.as_ref().and_then(|t| t.get(z)).map(|v| v.round() as i32)
                    } else {
                        neighbors.right_tex.as_ref().and_then(|t| t.get(z)).map(|v| v.round() as i32)
                    }
                } else if dx == 0 && dz != 0 {
                    if nz < 0 {
                        neighbors.top_tex.as_ref().and_then(|t| t.get(x)).map(|v| v.round() as i32)
                    } else {
                        neighbors.bottom_tex.as_ref().and_then(|t| t.get(x)).map(|v| v.round() as i32)
                    }
                } else {
                    // Diagonal crossing a chunk corner — no border data available.
                    None
                };
                if let Some(n) = neighbor_layer {
                    if n != own {
                        diff += 1;
                    }
                }
            }
            blend_weights.push((diff as f32 / 4.0).min(1.0));
        }
    }

    // Generate 6 per-triangle vertices per quad.
    // Both triangles in a quad share the same integer (layer_a, layer_b) pair so the
    // diagonal edge is seamless. Per-vertex tex_blend encodes where in that range
    // the vertex sits, derived from neighbor topology not interpolated indices.
    let mut vertices: Vec<Vertex> = Vec::with_capacity(r * r * 6);

    for z in 0..r {
        for x in 0..r {
            let tl = x + z * side;
            let tr = (x + 1) + z * side;
            let bl = x + (z + 1) * side;
            let br = (x + 1) + (z + 1) * side;

            let quad_layers = [grid_layers[tl], grid_layers[tr], grid_layers[bl], grid_layers[br]];
            let layer_a_int = *quad_layers.iter().min().unwrap();
            let layer_b_int = *quad_layers.iter().max().unwrap();
            let layer_a = layer_a_int as f32;
            let layer_b = layer_b_int as f32;

            let vertex_blend = |gi: usize| -> f32 {
                let own = grid_layers[gi];
                let w = blend_weights[gi];
                if layer_a_int == layer_b_int {
                    // Homogeneous quad — no blending needed.
                    0.0
                } else if own == layer_a_int {
                    // A-side vertex: approach 0.5 from below so the gradient across
                    // the transition quad goes 0 → ~0.4 → ~0.6 → 1 (monotonic).
                    w * 0.5
                } else if own == layer_b_int {
                    // B-side vertex: approach 0.5 from above.
                    1.0 - w * 0.5
                } else {
                    // 3+ layer quad (rare): fall back to linear position in range.
                    ((own - layer_a_int) as f32 / (layer_b_int - layer_a_int) as f32)
                        .clamp(0.0, 1.0)
                }
            };

            // Triangle 1: tl, bl, tr
            for &gi in &[tl, bl, tr] {
                vertices.push(Vertex {
                    position: grid_positions[gi],
                    normal: grid_normals[gi],
                    tex_coord: grid_uvs[gi],
                    tex_layer_a: layer_a,
                    tex_layer_b: layer_b,
                    tex_blend: vertex_blend(gi).clamp(0.0, 1.0),
                });
            }

            // Triangle 2: tr, bl, br
            for &gi in &[tr, bl, br] {
                vertices.push(Vertex {
                    position: grid_positions[gi],
                    normal: grid_normals[gi],
                    tex_coord: grid_uvs[gi],
                    tex_layer_a: layer_a,
                    tex_layer_b: layer_b,
                    tex_blend: vertex_blend(gi).clamp(0.0, 1.0),
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
