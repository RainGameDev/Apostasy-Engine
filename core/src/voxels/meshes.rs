use std::sync::Arc;

use anyhow::Result;
use apostasy_macros::{Component, Tag};
use ash::vk::Buffer;
use ash::vk::{self, CommandPool, DeviceMemory};
use cgmath::Vector3;
use hashbrown::HashMap;

use crate::log;
use crate::objects::scene::ObjectId;
use crate::objects::world::World;
use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::vertex::VertexDefinition;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::utils::flatten::flatten;
use crate::voxels::VoxelTransform;
use crate::voxels::chunk::{Chunk, ChunkGenQueue, GeneratedMeshData, MeshVertex};
use crate::voxels::voxel::VoxelRegistry;
use crate::voxels::voxel_components::is_transparent::IsTransparent;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VoxelVertex(MeshVertex);

impl VoxelVertex {
    pub fn pack(x: u8, y: u8, z: u8, face: u8, u: u8, v: u8, texture_id: u16, ao: u8) -> Self {
        Self(MeshVertex::pack(x, y, z, face, u, v, texture_id, ao))
    }

    pub fn data_lo(&self) -> u32 {
        self.0.data_lo
    }

    pub fn data_hi(&self) -> u32 {
        self.0.data_hi
    }
}

impl VertexDefinition for VoxelVertex {
    fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<VoxelVertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
    }

    fn get_attribute_descriptions() -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(vk::Format::R32_UINT)
                .offset(0),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(vk::Format::R32_UINT)
                .offset(4),
        ]
    }
}

#[derive(Debug, Component, Clone, Default)]
pub struct VoxelChunkMesh {
    pub vertex_buffer: Buffer,
    pub vertex_buffer_memory: DeviceMemory,
    pub index_buffer: Buffer,
    pub index_buffer_memory: DeviceMemory,
    pub index_count: u32,
}

impl VoxelChunkMesh {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Tag, Clone, Default)]
pub struct NeedsRemeshing;

impl GpuMesh for VoxelChunkMesh {
    fn get_vertex_buffer(&self) -> Buffer {
        self.vertex_buffer
    }
    fn get_index_buffer(&self) -> Buffer {
        self.index_buffer
    }
    fn get_index_count(&self) -> u32 {
        self.index_count
    }
}

pub struct ChunkNeighbours {
    pub px: Option<Chunk>, // +X
    pub nx: Option<Chunk>, // -X
    pub py: Option<Chunk>, // +Y
    pub ny: Option<Chunk>, // -Y
    pub pz: Option<Chunk>, // +Z
    pub nz: Option<Chunk>, // -Z
}

impl ChunkNeighbours {
    pub fn empty() -> Self {
        Self {
            px: None,
            nx: None,
            py: None,
            ny: None,
            pz: None,
            nz: None,
        }
    }
}

pub fn dispatch_remesh_jobs(world: &mut World) -> Result<()> {
    const MAX_MESH_JOBS_PER_FRAME: usize = 6;

    let registry = Arc::new(
        world
            .get_resource::<VoxelRegistry>()
            .expect("No VoxelRegistry resource")
            .clone(),
    );

    // build a map from chunk positions to their object ids
    let mut chunk_positions: HashMap<(i32, i32, i32), ObjectId> = HashMap::new();
    for (id, obj) in world.get_objects_with_component_with_ids::<VoxelTransform>() {
        if let Ok(t) = obj.get_component::<VoxelTransform>() {
            if obj.has_component::<Chunk>() {
                chunk_positions.insert((t.position.x, t.position.y, t.position.z), id);
            }
        }
    }

    // get all objects with NeedsRemeshing tag and their positions
    let mut needs_remesh: Vec<(ObjectId, Vector3<i32>)> = world
        .get_objects_with_tag_with_ids::<NeedsRemeshing>()
        .into_iter()
        .filter_map(|(id, _o)| {
            let obj = world.get_object(id)?;
            let pos = obj.get_component::<VoxelTransform>().ok()?.position;
            Some((id, pos))
        })
        .collect();

    needs_remesh.truncate(MAX_MESH_JOBS_PER_FRAME);

    // get the pools
    let mesh_pool = world.get_resource::<ChunkGenQueue>()?.mesh_pool.clone();
    let mesh_sender = world.get_resource::<ChunkGenQueue>()?.mesh_sender.clone();

    let pool = mesh_pool.lock().unwrap();

    // for each chunk that needs remeshing, spawn a job
    for (id, pos) in needs_remesh {
        let registry = registry.clone();
        let mesh_sender = mesh_sender.clone();

        let chunk = if let Some(&chunk_id) = chunk_positions.get(&(pos.x, pos.y, pos.z)) {
            if let Some(obj) = world.get_object(chunk_id) {
                if let Ok(chunk) = obj.get_component::<Chunk>() {
                    chunk.clone()
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            continue;
        };

        // get the neighbours of the chunk
        let clone_neighbour = |offset: Vector3<i32>| -> Option<Chunk> {
            let neighbour_pos = (pos.x + offset.x, pos.y + offset.y, pos.z + offset.z);
            chunk_positions
                .get(&neighbour_pos)
                .and_then(|&neighbour_id| world.get_object(neighbour_id))
                .and_then(|obj| obj.get_component::<Chunk>().ok().cloned())
        };

        let neighbours = ChunkNeighbours {
            px: clone_neighbour(Vector3::new(1, 0, 0)),
            nx: clone_neighbour(Vector3::new(-1, 0, 0)),
            py: clone_neighbour(Vector3::new(0, 1, 0)),
            ny: clone_neighbour(Vector3::new(0, -1, 0)),
            pz: clone_neighbour(Vector3::new(0, 0, 1)),
            nz: clone_neighbour(Vector3::new(0, 0, -1)),
        };

        // spawn the job
        pool.spawn(move || {
            let (vertices, indices) = generate_mesh(&chunk, &registry, &neighbours);
            let mesh_data = crate::voxels::chunk::GeneratedMeshData {
                position: pos,
                vertices,
                indices,
            };
            let _ = mesh_sender.send(mesh_data);
        });

        if let Some(obj) = world.get_object_mut(id) {
            obj.remove_tag::<NeedsRemeshing>();
        }
    }

    drop(pool);

    Ok(())
}

pub fn receive_meshes(
    world: &mut World,
    ctx: &VulkanRenderingContext,
    command_pool: CommandPool,
) -> Result<()> {
    let completed: Vec<GeneratedMeshData> = {
        let queue = world.get_resource::<crate::voxels::chunk::ChunkGenQueue>()?;
        queue
            .mesh_receiver
            .try_iter()
            .take(2) // process up to 2 per frame
            .collect()
    };

    if completed.is_empty() {
        return Ok(());
    }

    // build a map from position to object id
    let pos_to_id: HashMap<Vector3<i32>, ObjectId> = world
        .get_objects_with_component_with_ids::<VoxelTransform>()
        .iter()
        .filter_map(|(id, obj)| {
            let pos = obj.get_component::<VoxelTransform>().ok()?.position;
            Some((pos, *id))
        })
        .collect();

    for mesh_data in completed {
        let Some(id) = pos_to_id.get(&mesh_data.position) else {
            continue;
        };

        let Some(object) = world.get_object_mut(*id) else {
            continue;
        };

        if mesh_data.vertices.is_empty() || mesh_data.indices.is_empty() {
            continue;
        }

        // Convert MeshVertex to VoxelVertex
        let vertices: Vec<VoxelVertex> = mesh_data
            .vertices
            .into_iter()
            .map(|mv| VoxelVertex(mv))
            .collect();

        if let Ok(mesh) = object.get_component::<VoxelChunkMesh>() {
            if mesh.vertex_buffer != vk::Buffer::null() {
                unsafe {
                    ctx.device.destroy_buffer(mesh.vertex_buffer, None);
                    ctx.device.free_memory(mesh.vertex_buffer_memory, None);
                    ctx.device.destroy_buffer(mesh.index_buffer, None);
                    ctx.device.free_memory(mesh.index_buffer_memory, None);
                }
            }
        }

        let (vertex_buffer, vertex_buffer_memory) =
            ctx.create_vertex_buffer(&vertices, command_pool)?;
        let (index_buffer, index_buffer_memory) =
            ctx.create_index_buffer(&mesh_data.indices, command_pool)?;

        if !object.has_component::<VoxelChunkMesh>() {
            object.add_component(VoxelChunkMesh::default());
        }

        let mesh = object.get_component_mut::<VoxelChunkMesh>().unwrap();
        mesh.vertex_buffer = vertex_buffer;
        mesh.vertex_buffer_memory = vertex_buffer_memory;
        mesh.index_buffer = index_buffer;
        mesh.index_buffer_memory = index_buffer_memory;
        mesh.index_count = mesh_data.indices.len() as u32;
    }

    Ok(())
}

pub fn generate_mesh(
    chunk: &Chunk,
    registry: &VoxelRegistry,
    neighbours: &ChunkNeighbours,
) -> (Vec<MeshVertex>, Vec<u32>) {
    let lod = chunk.lod as usize;
    let grid_size = 32 / lod;

    // compute voxels into easily accessable grid
    let mut grid = [0u16; 32 * 32 * 32];
    for gz in 0..grid_size {
        for gy in 0..grid_size {
            for gx in 0..grid_size {
                grid[gz * grid_size * grid_size + gy * grid_size + gx] =
                    get_representative_voxel(chunk, gx * lod, gy * lod, gz * lod, lod);
            }
        }
    }

    // get neighbours voxels on their neighbouring plain
    let mut border_px = [0u16; 32 * 32]; // [y * 32 + z]
    let mut border_nx = [0u16; 32 * 32];
    let mut border_py = [0u16; 32 * 32];
    let mut border_ny = [0u16; 32 * 32];
    let mut border_pz = [0u16; 32 * 32];
    let mut border_nz = [0u16; 32 * 32];

    // calculate the voxels on the neighbours
    if let Some(n) = &neighbours.px {
        for v in 0..grid_size {
            for u in 0..grid_size {
                border_px[v * grid_size + u] =
                    get_representative_voxel(n, 0, u * lod, v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.nx {
        for v in 0..grid_size {
            for u in 0..grid_size {
                border_nx[v * grid_size + u] =
                    get_representative_voxel(n, 31 - (lod - 1), u * lod, v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.py {
        for v in 0..grid_size {
            for u in 0..grid_size {
                border_py[v * grid_size + u] =
                    get_representative_voxel(n, u * lod, 0, v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.ny {
        for v in 0..grid_size {
            for u in 0..grid_size {
                border_ny[v * grid_size + u] =
                    get_representative_voxel(n, u * lod, 31 - (lod - 1), v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.pz {
        for v in 0..grid_size {
            for u in 0..grid_size {
                border_pz[v * grid_size + u] =
                    get_representative_voxel(n, u * lod, v * lod, 0, lod);
            }
        }
    }
    if let Some(n) = &neighbours.nz {
        for v in 0..grid_size {
            for u in 0..grid_size {
                border_nz[v * grid_size + u] =
                    get_representative_voxel(n, u * lod, v * lod, 31 - (lod - 1), lod);
            }
        }
    }

    let max_faces = grid_size * grid_size * grid_size * 6;
    let mut vertices: Vec<MeshVertex> = Vec::with_capacity(max_faces * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(max_faces * 6);

    let is_transparent_voxel = |id: u16| -> bool {
        if id == 0 {
            return true;
        }
        registry.defs[id as usize].has_component::<IsTransparent>()
    };

    let vertex_ao = |face: usize,
                     gx: usize,
                     gy: usize,
                     gz: usize,
                     corner_u: u8,
                     corner_v: u8|
     -> u8 {
        let solid = |dx: i32, dy: i32, dz: i32| -> bool {
            let nx = gx as i32 + dx;
            let ny = gy as i32 + dy;
            let nz = gz as i32 + dz;

            let id = if nx >= 0
                && nx < grid_size as i32
                && ny >= 0
                && ny < grid_size as i32
                && nz >= 0
                && nz < grid_size as i32
            {
                grid[nz as usize * grid_size * grid_size + ny as usize * grid_size + nx as usize]
            } else if nx < 0 && ny >= 0 && ny < grid_size as i32 && nz >= 0 && nz < grid_size as i32
            {
                border_nx[nz as usize * grid_size + ny as usize]
            } else if nx >= grid_size as i32
                && ny >= 0
                && ny < grid_size as i32
                && nz >= 0
                && nz < grid_size as i32
            {
                border_px[nz as usize * grid_size + ny as usize]
            } else if ny < 0 && nx >= 0 && nx < grid_size as i32 && nz >= 0 && nz < grid_size as i32
            {
                border_ny[nz as usize * grid_size + nx as usize]
            } else if ny >= grid_size as i32
                && nx >= 0
                && nx < grid_size as i32
                && nz >= 0
                && nz < grid_size as i32
            {
                border_py[nz as usize * grid_size + nx as usize]
            } else if nz < 0 && nx >= 0 && nx < grid_size as i32 && ny >= 0 && ny < grid_size as i32
            {
                border_nz[ny as usize * grid_size + nx as usize]
            } else if nz >= grid_size as i32
                && nx >= 0
                && nx < grid_size as i32
                && ny >= 0
                && ny < grid_size as i32
            {
                border_pz[ny as usize * grid_size + nx as usize]
            } else {
                0
            };

            id != 0 && !is_transparent_voxel(id)
        };

        let su = if corner_u == 0 { -1i32 } else { 1 };
        let sv = if corner_v == 0 { -1i32 } else { 1 };

        // For each face: normal axis is fixed, tangent axes are (u_axis, v_axis)
        // s1 = neighbour along u, s2 = neighbour along v, c = diagonal
        let (s1, s2, c) = match face {
            0 => (solid(1, su, 0), solid(1, 0, sv), solid(1, su, sv)), // +X: u=Y, v=Z
            1 => (solid(-1, su, 0), solid(-1, 0, sv), solid(-1, su, sv)), // -X: u=Y, v=Z
            2 => (solid(su, 1, 0), solid(0, 1, sv), solid(su, 1, sv)), // +Y: u=X, v=Z
            3 => (solid(su, -1, 0), solid(0, -1, sv), solid(su, -1, sv)), // -Y: u=X, v=Z
            4 => (solid(su, 0, 1), solid(0, sv, 1), solid(su, sv, 1)), // +Z: u=X, v=Y
            _ => (solid(su, 0, -1), solid(0, sv, -1), solid(su, sv, -1)), // -Z: u=X, v=Y
        };

        if s1 && s2 {
            0
        } else {
            3 - (s1 as u8 + s2 as u8 + c as u8)
        }
    };

    // get if the neighbour of the current voxel is solid (and not transparent)
    let neighbour_solid = |face: usize, gx: usize, gy: usize, gz: usize| -> bool {
        let neighbor_id = match face {
            0 => {
                // +X
                if gx + 1 < grid_size {
                    grid[gz * grid_size * grid_size + gy * grid_size + gx + 1]
                } else {
                    border_px[gz * grid_size + gy]
                }
            }
            1 => {
                // -X
                if gx > 0 {
                    grid[gz * grid_size * grid_size + gy * grid_size + gx - 1]
                } else {
                    border_nx[gz * grid_size + gy]
                }
            }
            2 => {
                // +Y
                if gy + 1 < grid_size {
                    grid[gz * grid_size * grid_size + (gy + 1) * grid_size + gx]
                } else {
                    border_py[gz * grid_size + gx]
                }
            }
            3 => {
                // -Y
                if gy > 0 {
                    grid[gz * grid_size * grid_size + (gy - 1) * grid_size + gx]
                } else {
                    border_ny[gz * grid_size + gx]
                }
            }
            4 => {
                // +Z
                if gz + 1 < grid_size {
                    grid[(gz + 1) * grid_size * grid_size + gy * grid_size + gx]
                } else {
                    border_pz[gy * grid_size + gx]
                }
            }
            _ => {
                // -Z
                if gz > 0 {
                    grid[(gz - 1) * grid_size * grid_size + gy * grid_size + gx]
                } else {
                    border_nz[gy * grid_size + gx]
                }
            }
        };

        neighbor_id != 0 && !is_transparent_voxel(neighbor_id)
    };

    // for each voxel
    for gz in 0..grid_size {
        for gy in 0..grid_size {
            let row_base = gz * grid_size * grid_size + gy * grid_size;
            for gx in 0..grid_size {
                let id = grid[row_base + gx];
                if id == 0 {
                    continue; // skip air immediately
                }

                let vx = (gx * lod) as u32;
                let vy = (gy * lod) as u32;
                let vz = (gz * lod) as u32;

                let voxel_def = &registry.defs[id as usize];

                // render each face
                for face in 0..6usize {
                    // if the neighbouring face is solid skip
                    if neighbour_solid(face, gx, gy, gz) {
                        continue;
                    }

                    let texture_id = voxel_def.textures.get_for_face(face as u8, vx, vy, vz);

                    let x = vx as u8;
                    let y = vy as u8;
                    let z = vz as u8;
                    let l = lod as u8;

                    let corners: [[u8; 3]; 4] = match face {
                        0 => [
                            [x + l, y, z],
                            [x + l, y + l, z],
                            [x + l, y + l, z + l],
                            [x + l, y, z + l],
                        ],
                        1 => [[x, y, z + l], [x, y + l, z + l], [x, y + l, z], [x, y, z]],
                        2 => [
                            [x, y + l, z + l],
                            [x + l, y + l, z + l],
                            [x + l, y + l, z],
                            [x, y + l, z],
                        ],
                        3 => [[x, y, z], [x + l, y, z], [x + l, y, z + l], [x, y, z + l]],
                        4 => [
                            [x + l, y, z + l],
                            [x + l, y + l, z + l],
                            [x, y + l, z + l],
                            [x, y, z + l],
                        ],
                        _ => [[x, y, z], [x, y + l, z], [x + l, y + l, z], [x + l, y, z]],
                    };

                    let base = vertices.len() as u32;

                    let (ao0, ao1, ao2, ao3) = match face {
                        0 => (
                            // +X: u=Y, v=Z. corners: (y=0,z=0),(y=1,z=0),(y=1,z=1),(y=0,z=1)
                            vertex_ao(face, gx, gy, gz, 0, 0),
                            vertex_ao(face, gx, gy, gz, 1, 0),
                            vertex_ao(face, gx, gy, gz, 1, 1),
                            vertex_ao(face, gx, gy, gz, 0, 1),
                        ),
                        1 => (
                            // -X: u=Y, v=Z. corners: (y=0,z=1),(y=1,z=1),(y=1,z=0),(y=0,z=0)
                            vertex_ao(face, gx, gy, gz, 0, 1),
                            vertex_ao(face, gx, gy, gz, 1, 1),
                            vertex_ao(face, gx, gy, gz, 1, 0),
                            vertex_ao(face, gx, gy, gz, 0, 0),
                        ),
                        2 => (
                            // +Y: u=X, v=Z. corners: (x=0,z=1),(x=1,z=1),(x=1,z=0),(x=0,z=0)
                            vertex_ao(face, gx, gy, gz, 0, 1),
                            vertex_ao(face, gx, gy, gz, 1, 1),
                            vertex_ao(face, gx, gy, gz, 1, 0),
                            vertex_ao(face, gx, gy, gz, 0, 0),
                        ),
                        3 => (
                            // -Y: u=X, v=Z. corners: (x=0,z=0),(x=1,z=0),(x=1,z=1),(x=0,z=1)
                            vertex_ao(face, gx, gy, gz, 0, 0),
                            vertex_ao(face, gx, gy, gz, 1, 0),
                            vertex_ao(face, gx, gy, gz, 1, 1),
                            vertex_ao(face, gx, gy, gz, 0, 1),
                        ),
                        4 => (
                            // +Z: u=X, v=Y. corners: (x=1,y=0),(x=1,y=1),(x=0,y=1),(x=0,y=0)
                            vertex_ao(face, gx, gy, gz, 1, 0),
                            vertex_ao(face, gx, gy, gz, 1, 1),
                            vertex_ao(face, gx, gy, gz, 0, 1),
                            vertex_ao(face, gx, gy, gz, 0, 0),
                        ),
                        _ => (
                            // -Z: u=X, v=Y. corners: (x=0,y=0),(x=0,y=1),(x=1,y=1),(x=1,y=0)
                            vertex_ao(face, gx, gy, gz, 0, 0),
                            vertex_ao(face, gx, gy, gz, 0, 1),
                            vertex_ao(face, gx, gy, gz, 1, 1),
                            vertex_ao(face, gx, gy, gz, 1, 0),
                        ),
                    };
                    // push to the buffers
                    vertices.push(MeshVertex::pack(
                        corners[0][0],
                        corners[0][1],
                        corners[0][2],
                        face as u8,
                        0,
                        0,
                        texture_id as u16,
                        ao0,
                    ));
                    vertices.push(MeshVertex::pack(
                        corners[1][0],
                        corners[1][1],
                        corners[1][2],
                        face as u8,
                        1,
                        0,
                        texture_id as u16,
                        ao1,
                    ));
                    vertices.push(MeshVertex::pack(
                        corners[2][0],
                        corners[2][1],
                        corners[2][2],
                        face as u8,
                        1,
                        1,
                        texture_id as u16,
                        ao2,
                    ));
                    vertices.push(MeshVertex::pack(
                        corners[3][0],
                        corners[3][1],
                        corners[3][2],
                        face as u8,
                        0,
                        1,
                        texture_id as u16,
                        ao3,
                    ));

                    // fixes diagonal artefacting on darker/occluded corners
                    if ao0 + ao2 > ao1 + ao3 {
                        // flip
                        indices.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 2,
                            base,
                            base + 2,
                            base + 3,
                        ]);
                    } else {
                        // standard
                        indices.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 3,
                            base + 1,
                            base + 2,
                            base + 3,
                        ]);
                    }
                }
            }
        }
    }

    (vertices, indices)
}

fn get_representative_voxel(chunk: &Chunk, x: usize, y: usize, z: usize, lod: usize) -> u16 {
    for dz in 0..lod {
        for dy in 0..lod {
            for dx in 0..lod {
                let sx = x + dx;
                let sy = y + dy;
                let sz = z + dz;

                if sx >= 32 || sy >= 32 || sz >= 32 {
                    continue;
                }
                let id = chunk.voxels[flatten(sx as u32, sy as u32, sz as u32, 32)];
                if id != 0 {
                    return id;
                }
            }
        }
    }
    0
}
