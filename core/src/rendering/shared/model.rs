use std::sync::Arc;

use ash::vk::{Buffer, CommandPool, DeviceMemory};
use cgmath::{InnerSpace, Vector3, Zero};

use crate::{
    physics::collider::{DEBUG_VISUAL_SIZE_OFFSET, triangle_aabb},
    rendering::{shared::material::GpuMaterial, shared::vertex::Vertex, vulkan::rendering_context::VulkanRenderingContext},
};

/// Builds a one-off wireframe-only `GpuModel` from raw collision triangles. Used to
/// visualize mesh colliders (e.g. terrain) that have no backing named model — each
/// vertex is pushed out along its face normal by `DEBUG_VISUAL_SIZE_OFFSET`, matching
/// the size increase applied to primitive collider debug visuals.
pub fn build_collider_debug_model(
    context: &Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    triangles: &[[Vector3<f32>; 3]],
) -> anyhow::Result<GpuModel> {
    let mut vertices = Vec::with_capacity(triangles.len() * 3);
    let mut indices = Vec::with_capacity(triangles.len() * 3);
    for tri in triangles {
        let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
        for &p in tri {
            let idx = vertices.len() as u32;
            let offset_pos = p + normal * DEBUG_VISUAL_SIZE_OFFSET;
            vertices.push(Vertex {
                position: offset_pos.into(),
                normal: normal.into(),
                tex_coord: [0.0, 0.0],
                weights: [0.0; 32],
                color: [1.0, 1.0, 1.0],
            });
            indices.push(idx);
        }
    }

    let (vertex_buffer, vertex_buffer_memory) =
        context.create_vertex_buffer(&vertices, command_pool)?;
    let (index_buffer, index_buffer_memory) = context.create_index_buffer(&indices, command_pool)?;

    Ok(GpuModel {
        name: "collider_debug_mesh".to_string(),
        meshes: vec![Mesh {
            vertex_buffer,
            vertex_buffer_memory,
            index_buffer,
            index_buffer_memory,
            index_count: indices.len() as u32,
            material_name: String::new(),
            material: None,
        }],
        collision_bvh: None,
    })
}

#[derive(Clone, Debug)]
pub struct GpuModel {
    pub meshes: Vec<Mesh>,
    pub name: String,
    pub collision_bvh: Option<Arc<Bvh>>,
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertex_buffer: Buffer,
    pub vertex_buffer_memory: DeviceMemory,
    pub index_buffer: Buffer,
    pub index_buffer_memory: DeviceMemory,
    pub index_count: u32,
    pub material_name: String,
    pub material: Option<GpuMaterial>,
}

impl GpuMesh for Mesh {
    fn get_vertex_buffer(&self) -> Buffer {
        self.vertex_buffer.clone()
    }
    fn get_index_buffer(&self) -> Buffer {
        self.index_buffer.clone()
    }
    fn get_index_count(&self) -> u32 {
        self.index_count
    }
}

pub trait GpuMesh {
    fn get_vertex_buffer(&self) -> Buffer;
    fn get_index_buffer(&self) -> Buffer;
    fn get_index_count(&self) -> u32;
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bvh {
    pub nodes: Vec<BvhNode>,
    pub triangles: Vec<[Vector3<f32>; 3]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BvhNode {
    pub aabb_min: Vector3<f32>,
    pub aabb_max: Vector3<f32>,
    // if left == 0, this is a leaf
    pub left: u32,
    pub right: u32,
    // only meaningful on leaves
    pub tri_start: u32,
    pub tri_count: u32,
}

impl Default for BvhNode {
    fn default() -> Self {
        Self {
            aabb_max: Vector3::zero(),
            aabb_min: Vector3::zero(),
            right: 0,
            left: 0,
            tri_count: 0,
            tri_start: 0,
        }
    }
}

impl Bvh {
    /// Creates an empty Bvh.
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            triangles: Vec::new(),
        }
    }

    pub fn build(triangles: Vec<[Vector3<f32>; 3]>) -> Self {
        let mut bvh = Bvh {
            nodes: Vec::new(),
            triangles,
        };
        if !bvh.triangles.is_empty() {
            let end = bvh.triangles.len();
            bvh.build_recursive(0, end);
        }
        bvh
    }

    fn build_recursive(&mut self, start: usize, end: usize) -> u32 {
        let node_idx = self.nodes.len() as u32;
        self.nodes.push(BvhNode::default());

        let (aabb_min, aabb_max) = triangle_aabb(&self.triangles[start..end]);
        let count = end - start;

        if count <= 4 {
            self.nodes[node_idx as usize] = BvhNode {
                aabb_min,
                aabb_max,
                left: 0,
                right: 0,
                tri_start: start as u32,
                tri_count: count as u32,
            };
            return node_idx;
        }
        // split along the longest axis at the median center
        let extent = aabb_max - aabb_min;
        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0
        } else if extent.y >= extent.z {
            1
        } else {
            2
        };

        let mid = (start + end) / 2;
        self.triangles[start..end].sort_unstable_by(|a, b| {
            tri_centroid(a)[axis]
                .partial_cmp(&tri_centroid(b)[axis])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let left = self.build_recursive(start, mid);
        let right = self.build_recursive(mid, end);

        self.nodes[node_idx as usize] = BvhNode {
            aabb_min,
            aabb_max,
            left,
            right,
            tri_start: 0,
            tri_count: 0,
        };

        node_idx
    }
}

/// Calculates the true center of a triangle
fn tri_centroid(tri: &[Vector3<f32>; 3]) -> [f32; 3] {
    [
        (tri[0].x + tri[1].x + tri[2].x) / 3.0,
        (tri[0].y + tri[1].y + tri[2].y) / 3.0,
        (tri[0].z + tri[1].z + tri[2].z) / 3.0,
    ]
}
