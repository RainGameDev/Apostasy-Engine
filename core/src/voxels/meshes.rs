use std::num::NonZeroU16;
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use apostasy_macros::{Component, Tag};
use ash::vk::{self, Buffer, CommandPool, DeviceMemory};
use cgmath::Vector3;
use hashbrown::HashMap;

use crate::objects::Object;
use crate::objects::scene::ObjectId;
use crate::objects::world::World;
use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::vertex::VertexDefinition;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::utils::flatten::flatten;
use crate::voxels::VoxelTransform;
use crate::voxels::biome::BiomeRegistry;
use crate::voxels::chunk::{Chunk, ChunkGenQueue, GeneratedMeshData, MeshJobFn};
use crate::voxels::chunk_loader::ChunkLoadBounds;
use crate::voxels::voxel::VoxelRegistry;
use crate::voxels::voxel_components::is_transparent::IsTransparent;
use crate::voxels::voxel_components::tints::{HasTint, TintType};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VoxelVertex {
    pub data_lo: u32,
    pub data_hi: u32,
    pub tint: Option<NonZeroU16>,
}

impl VoxelVertex {
    #[inline]
    pub fn pack(
        x: u8,
        y: u8,
        z: u8,
        face: u8,
        u: u8,
        v: u8,
        is_top: bool,
        texture_id: u16,
        ao: u8,
        r: u8,
        g: u8,
        b: u8,
    ) -> Self {
        let data_lo = (x as u32)
            | ((y as u32) << 6)
            | ((z as u32) << 12)
            | ((face as u32) << 18)
            | ((u as u32) << 21)
            | ((v as u32) << 23)
            | ((is_top as u32) << 25);

        let data_hi = (texture_id as u32) | ((ao as u32 & 0x3) << 16);

        // pack rgb into 4 bits each; NonZeroU16::new returns None if all zero
        let packed: u16 = ((r as u16 >> 4) & 0xF)
            | (((g as u16 >> 4) & 0xF) << 4)
            | (((b as u16 >> 4) & 0xF) << 8);

        Self {
            data_lo,
            data_hi,
            tint: NonZeroU16::new(packed),
        }
    }

    pub fn data_lo(self) -> u32 {
        self.data_lo
    }
    pub fn data_hi(self) -> u32 {
        self.data_hi
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
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(2)
                .format(vk::Format::R16_UINT)
                .offset(8),
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

#[derive(Debug, Component, Clone, Default)]
pub struct WaterMesh {
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
impl WaterMesh {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Tag, Clone, Default)]
pub struct NeedsRemeshing;

// voxel break remeshes are higher priority than normal generation remeshes
#[derive(Debug, Tag, Clone, Default)]
pub struct VoxelBreakRemesh;

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

impl GpuMesh for WaterMesh {
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

const MAX_MESH_JOBS_PER_FRAME: usize = 32;
const MAX_MESH_RESULTS_PER_FRAME: usize = 16;

// builds a flat position -> id lookup for every loaded chunk
fn chunk_position_map(world: &World) -> HashMap<(i32, i32, i32), ObjectId> {
    world
        .get_objects_with_component_with_ids::<VoxelTransform>()
        .into_iter()
        .filter_map(|(id, obj)| {
            let t = obj.get_component::<VoxelTransform>().ok()?;
            obj.has_component::<Chunk>()
                .then_some(((t.position.x, t.position.y, t.position.z), id))
        })
        .collect()
}

// collects all chunks needing a remesh, voxel-break jobs sorted first
fn sorted_remesh_candidates(world: &World) -> Vec<(ObjectId, Vector3<i32>)> {
    let mut candidates: Vec<(ObjectId, Vector3<i32>)> = world
        .get_objects_with_tag_with_ids::<NeedsRemeshing>()
        .into_iter()
        .filter_map(|(id, _)| {
            let pos = world
                .get_object(id)?
                .get_component::<VoxelTransform>()
                .ok()?
                .position;
            Some((id, pos))
        })
        .collect();

    candidates.sort_by_key(|(id, _)| {
        !world
            .get_object(*id)
            .is_some_and(|o| o.has_tag::<VoxelBreakRemesh>())
    });

    candidates
}

// clones the chunk at pos + offset if it exists
fn get_neighbour(
    pos: Vector3<i32>,
    offset: Vector3<i32>,
    chunk_positions: &HashMap<(i32, i32, i32), ObjectId>,
    world: &World,
) -> Option<Chunk> {
    let key = (pos.x + offset.x, pos.y + offset.y, pos.z + offset.z);
    chunk_positions
        .get(&key)
        .and_then(|&id| world.get_object(id))
        .and_then(|obj| obj.get_component::<Chunk>().ok().cloned())
}

fn gather_neighbours(
    pos: Vector3<i32>,
    chunk_positions: &HashMap<(i32, i32, i32), ObjectId>,
    world: &World,
) -> ChunkNeighbours {
    ChunkNeighbours {
        px: get_neighbour(pos, Vector3::new(1, 0, 0), chunk_positions, world),
        nx: get_neighbour(pos, Vector3::new(-1, 0, 0), chunk_positions, world),
        py: get_neighbour(pos, Vector3::new(0, 1, 0), chunk_positions, world),
        ny: get_neighbour(pos, Vector3::new(0, -1, 0), chunk_positions, world),
        pz: get_neighbour(pos, Vector3::new(0, 0, 1), chunk_positions, world),
        nz: get_neighbour(pos, Vector3::new(0, 0, -1), chunk_positions, world),
    }
}

// a neighbour is ready if it either exists or is outside the load radius
fn neighbour_ready(
    neighbour_pos: Vector3<i32>,
    neighbour: &Option<Chunk>,
    player_pos: Vector3<i32>,
    load_radius: i32,
    v_load_radius: i32,
) -> bool {
    let dx = (neighbour_pos.x - player_pos.x).abs();
    let dy = (neighbour_pos.y - player_pos.y).abs();
    let dz = (neighbour_pos.z - player_pos.z).abs();
    let in_radius = dx <= load_radius && dy <= v_load_radius && dz <= load_radius;
    !in_radius || neighbour.is_some()
}

fn all_neighbours_ready(
    pos: Vector3<i32>,
    neighbours: &ChunkNeighbours,
    player_pos: Vector3<i32>,
    load_radius: i32,
    v_load_radius: i32,
) -> bool {
    let check = |offset: Vector3<i32>, nb: &Option<Chunk>| {
        neighbour_ready(pos + offset, nb, player_pos, load_radius, v_load_radius)
    };

    check(Vector3::new(1, 0, 0), &neighbours.px)
        && check(Vector3::new(-1, 0, 0), &neighbours.nx)
        && check(Vector3::new(0, 1, 0), &neighbours.py)
        && check(Vector3::new(0, -1, 0), &neighbours.ny)
        && check(Vector3::new(0, 0, 1), &neighbours.pz)
        && check(Vector3::new(0, 0, -1), &neighbours.nz)
}

struct Job {
    id: ObjectId,
    pos: Vector3<i32>,
    chunk: Chunk,
    neighbours: ChunkNeighbours,
}

pub fn dispatch_remesh_jobs(world: &mut World) -> Result<()> {
    let registry = Arc::new(
        world
            .get_resource::<VoxelRegistry>()
            .expect("no VoxelRegistry resource")
            .clone(),
    );
    let biome_registry = Arc::new(
        world
            .get_resource::<BiomeRegistry>()
            .expect("no BiomeRegistry resource")
            .clone(),
    );

    let chunk_positions = chunk_position_map(world);
    let candidates = sorted_remesh_candidates(world);

    let (load_radius, v_load_radius, player_pos) = {
        let loader = world.get_resource::<ChunkLoadBounds>()?;
        (
            loader.load_radius,
            loader.v_load_radius,
            loader.player_chunk_pos,
        )
    };

    let mesh_job_sender = world
        .get_resource::<ChunkGenQueue>()?
        .mesh_job_sender
        .clone();
    let mesh_result_sender = world
        .get_resource::<ChunkGenQueue>()?
        .mesh_result_sender
        .clone();
    // phase 1: find chunks that are ready to mesh
    let mut ready: Vec<Job> = Vec::new();

    for (id, pos) in candidates {
        let Some(&chunk_id) = chunk_positions.get(&(pos.x, pos.y, pos.z)) else {
            continue;
        };
        let Some(chunk) = world
            .get_object(chunk_id)
            .and_then(|o| o.get_component::<Chunk>().ok().cloned())
        else {
            continue;
        };

        let neighbours = gather_neighbours(pos, &chunk_positions, world);

        // leave NeedsRemeshing on and retry next frame if neighbours aren't ready
        if !all_neighbours_ready(pos, &neighbours, player_pos, load_radius, v_load_radius) {
            continue;
        }

        // no visible faces means nothing to mesh
        if !chunk.has_visible_faces(&neighbours) {
            if let Some(obj) = world.get_object_mut(id) {
                obj.remove_tag::<NeedsRemeshing>();
                obj.remove_tag::<VoxelBreakRemesh>();
            }
            continue;
        }

        ready.push(Job {
            id,
            pos,
            chunk,
            neighbours,
        });

        if ready.len() == MAX_MESH_JOBS_PER_FRAME {
            break;
        }
    }

    // phase 2: remove tags and submit jobs for the ready set
    for job in &ready {
        if let Some(obj) = world.get_object_mut(job.id) {
            obj.remove_tag::<NeedsRemeshing>();
            obj.remove_tag::<VoxelBreakRemesh>();
        }
    }

    for Job {
        chunk,
        neighbours,
        pos,
        ..
    } in ready
    {
        let registry = registry.clone();
        let biome_registry = biome_registry.clone();
        let sender = mesh_result_sender.clone();

        let job: MeshJobFn = Box::new(move || {
            let (opaque_vertices, opaque_indices, water_vertices, water_indices) =
                generate_mesh(&chunk, &registry, &neighbours, &biome_registry);

            let _ = sender.send(GeneratedMeshData {
                position: pos,
                opaque_vertices,
                opaque_indices,
                water_vertices,
                water_indices,
            });
        });

        let _ = mesh_job_sender.send(job);
    }

    Ok(())
}

// queues a buffer pair for deferred cleanup at end of frame
fn defer_destroy(
    graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>,
    buffer: vk::Buffer,
    memory: vk::DeviceMemory,
) {
    if buffer != vk::Buffer::null() {
        graveyard.push((buffer, memory));
    }
}

// destroys a buffer memory immediately
fn destroy_now(device: &ash::Device, buffer: vk::Buffer, memory: vk::DeviceMemory) {
    unsafe {
        if buffer != vk::Buffer::null() {
            device.destroy_buffer(buffer, None);
            device.free_memory(memory, None);
        }
    }
}

fn upload_opaque_mesh(
    object: &mut Object,
    ctx: &VulkanRenderingContext,
    command_pool: CommandPool,
    vertices: &[VoxelVertex],
    indices: &[u32],
    graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>,
) -> Result<()> {
    // queue old buffers for deferred cleanup
    if let Ok(old) = object.get_component::<VoxelChunkMesh>() {
        defer_destroy(graveyard, old.vertex_buffer, old.vertex_buffer_memory);
        defer_destroy(graveyard, old.index_buffer, old.index_buffer_memory);
    }

    let (vb, vbm) = ctx.create_vertex_buffer(vertices, command_pool)?;
    let (ib, ibm) = ctx.create_index_buffer(indices, command_pool)?;

    if !object.has_component::<VoxelChunkMesh>() {
        object.add_component(VoxelChunkMesh::default());
    }

    let mesh = object.get_component_mut::<VoxelChunkMesh>().unwrap();
    mesh.vertex_buffer = vb;
    mesh.vertex_buffer_memory = vbm;
    mesh.index_buffer = ib;
    mesh.index_buffer_memory = ibm;
    mesh.index_count = indices.len() as u32;

    Ok(())
}

fn upload_water_mesh(
    object: &mut Object,
    ctx: &VulkanRenderingContext,
    command_pool: CommandPool,
    vertices: &[VoxelVertex],
    indices: &[u32],
) -> Result<()> {
    // water buffers are destroyed immediately rather than deferred
    if let Ok(old) = object.get_component::<WaterMesh>() {
        destroy_now(&ctx.device, old.vertex_buffer, old.vertex_buffer_memory);
        destroy_now(&ctx.device, old.index_buffer, old.index_buffer_memory);
    }

    let (vb, vbm) = ctx.create_vertex_buffer(vertices, command_pool)?;
    let (ib, ibm) = ctx.create_index_buffer(indices, command_pool)?;

    if !object.has_component::<WaterMesh>() {
        object.add_component(WaterMesh::default());
    }

    let mesh = object.get_component_mut::<WaterMesh>().unwrap();
    mesh.vertex_buffer = vb;
    mesh.vertex_buffer_memory = vbm;
    mesh.index_buffer = ib;
    mesh.index_buffer_memory = ibm;
    mesh.index_count = indices.len() as u32;

    Ok(())
}

pub fn receive_meshes(
    world: &mut World,
    ctx: &VulkanRenderingContext,
    command_pool: CommandPool,
    buffer_graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>,
) -> Result<()> {
    let completed: Vec<GeneratedMeshData> = {
        let queue = world.get_resource::<ChunkGenQueue>()?;
        queue
            .mesh_receiver
            .try_iter()
            .take(MAX_MESH_RESULTS_PER_FRAME)
            .collect()
    };

    if completed.is_empty() {
        return Ok(());
    }

    // build position -> id map once for all results
    let pos_to_id: HashMap<Vector3<i32>, ObjectId> = world
        .get_objects_with_component_with_ids::<VoxelTransform>()
        .iter()
        .filter_map(|(id, obj)| {
            let pos = obj.get_component::<VoxelTransform>().ok()?.position;
            Some((pos, *id))
        })
        .collect();

    for mesh_data in completed {
        // chunk may have been unloaded while the job was in flight
        let Some(&id) = pos_to_id.get(&mesh_data.position) else {
            continue;
        };
        let Some(object) = world.get_object_mut(id) else {
            continue;
        };

        let has_opaque =
            !mesh_data.opaque_vertices.is_empty() && !mesh_data.opaque_indices.is_empty();
        let has_water = !mesh_data.water_vertices.is_empty() && !mesh_data.water_indices.is_empty();

        if !has_opaque && !has_water {
            continue;
        }

        if has_opaque {
            upload_opaque_mesh(
                object,
                ctx,
                command_pool,
                &mesh_data.opaque_vertices,
                &mesh_data.opaque_indices,
                buffer_graveyard,
            )?;
        }

        if has_water {
            upload_water_mesh(
                object,
                ctx,
                command_pool,
                &mesh_data.water_vertices,
                &mesh_data.water_indices,
            )?;
        }
    }

    Ok(())
}

static TINT_KERNEL_1D: OnceLock<Vec<f32>> = OnceLock::new();

fn tint_kernel_1d() -> &'static [f32] {
    TINT_KERNEL_1D.get_or_init(|| {
        const RADIUS: i32 = 8;
        let sigma = RADIUS as f32 / 2.0;
        let denom = 2.0 * sigma * sigma;
        let size = (RADIUS * 2 + 1) as usize;
        let raw: Vec<f32> = (0..size)
            .map(|i| {
                let d = i as i32 - RADIUS;
                (-(d * d) as f32 / denom).exp()
            })
            .collect();
        // normalise so the 1-D weights sum to 1
        let sum: f32 = raw.iter().sum();
        raw.into_iter().map(|v| v / sum).collect()
    })
}

pub struct BiomeColors {
    id: u16,
    fr: f32,
    fg: f32,
    fb: f32,
    wr: f32,
    wg: f32,
    wb: f32,
}

fn build_tint_maps(
    gs: usize,
    chunk: &Chunk,
    neighbours: &ChunkNeighbours,
    biome_registry: &BiomeRegistry,
) -> (Vec<(u8, u8, u8)>, Vec<(u8, u8, u8)>) {
    const RADIUS: i32 = 8;
    let kernel = tint_kernel_1d();
    let ksize = (RADIUS * 2 + 1) as usize;
    let pad = RADIUS as usize;
    let sw = gs + 2 * pad;

    // collect the set of distinct biome ids in this chunk/border
    let mut biome_ids: Vec<u16> = Vec::with_capacity(8);
    biome_ids.push(chunk.biome);
    for nb in [
        neighbours.px.as_ref(),
        neighbours.nx.as_ref(),
        neighbours.py.as_ref(),
        neighbours.ny.as_ref(),
        neighbours.pz.as_ref(),
        neighbours.nz.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if !biome_ids.contains(&nb.biome) {
            biome_ids.push(nb.biome);
        }
    }

    // resolve light colours for each unique biome id
    let palette: Vec<BiomeColors> = biome_ids
        .iter()
        .map(|&id| {
            if let Ok(biome) = biome_registry.get_def(id) {
                BiomeColors {
                    id,
                    fr: to_linear(biome.foliage_color.0),
                    fg: to_linear(biome.foliage_color.1),
                    fb: to_linear(biome.foliage_color.2),
                    wr: to_linear(biome.water_color.0),
                    wg: to_linear(biome.water_color.1),
                    wb: to_linear(biome.water_color.2),
                }
            } else {
                BiomeColors {
                    id,
                    fr: 0.0,
                    fg: 0.0,
                    fb: 0.0,
                    wr: 0.0,
                    wg: 0.0,
                    wb: 0.0,
                }
            }
        })
        .collect();

    //  fill grid using palette index , resolve_biome + array scan
    let mut foliage_r = vec![0f32; sw * sw];
    let mut foliage_g = vec![0f32; sw * sw];
    let mut foliage_b = vec![0f32; sw * sw];
    let mut water_r = vec![0f32; sw * sw];
    let mut water_g = vec![0f32; sw * sw];
    let mut water_b = vec![0f32; sw * sw];

    for sz in -(RADIUS)..gs as i32 + RADIUS {
        for sx in -(RADIUS)..gs as i32 + RADIUS {
            let biome_id = resolve_biome(sx, sz, gs as i32, chunk, neighbours);
            // find in palette
            let c = palette.iter().find(|c| c.id == biome_id).unwrap();
            let idx = (sz + RADIUS) as usize * sw + (sx + RADIUS) as usize;
            foliage_r[idx] = c.fr;
            foliage_g[idx] = c.fg;
            foliage_b[idx] = c.fb;
            water_r[idx] = c.wr;
            water_g[idx] = c.wg;
            water_b[idx] = c.wb;
        }
    }

    // uniform biome
    if palette.len() == 1 {
        let c = &palette[0];
        let fc = (to_gamma(c.fr), to_gamma(c.fg), to_gamma(c.fb));
        let wc = (to_gamma(c.wr), to_gamma(c.wg), to_gamma(c.wb));
        return (vec![fc; gs * gs], vec![wc; gs * gs]);
    }

    // separable horizontal pass to intermediate sw x gs buffer
    let mut tmp_fr = vec![0f32; sw * gs];
    let mut tmp_fg = vec![0f32; sw * gs];
    let mut tmp_fb = vec![0f32; sw * gs];
    let mut tmp_wr = vec![0f32; sw * gs];
    let mut tmp_wg = vec![0f32; sw * gs];
    let mut tmp_wb = vec![0f32; sw * gs];

    for gz in 0..gs {
        let src_row = gz + pad;
        for sx in 0..sw {
            let mut fr = 0f32;
            let mut fg = 0f32;
            let mut fb = 0f32;
            let mut wr = 0f32;
            let mut wg = 0f32;
            let mut wb = 0f32;
            for k in 0..ksize {
                let tap_col = sx as i32 + k as i32 - RADIUS;
                if tap_col < 0 || tap_col >= sw as i32 {
                    continue;
                }
                let w = kernel[k];
                let i = src_row * sw + tap_col as usize;
                fr += foliage_r[i] * w;
                fg += foliage_g[i] * w;
                fb += foliage_b[i] * w;
                wr += water_r[i] * w;
                wg += water_g[i] * w;
                wb += water_b[i] * w;
            }
            let t = gz * sw + sx;
            tmp_fr[t] = fr;
            tmp_fg[t] = fg;
            tmp_fb[t] = fb;
            tmp_wr[t] = wr;
            tmp_wg[t] = wg;
            tmp_wb[t] = wb;
        }
    }

    let mut foliage = vec![(0u8, 0u8, 0u8); gs * gs];
    let mut water = vec![(0u8, 0u8, 0u8); gs * gs];

    for gz in 0..gs {
        for gx in 0..gs {
            let col = gx + pad;
            let mut fr = 0f32;
            let mut fg = 0f32;
            let mut fb = 0f32;
            let mut wr = 0f32;
            let mut wg = 0f32;
            let mut wb = 0f32;
            for k in 0..ksize {
                let tap_row = gz as i32 + k as i32 - RADIUS;
                if tap_row < 0 || tap_row >= gs as i32 {
                    continue;
                }
                let w = kernel[k];
                let t = tap_row as usize * sw + col;
                fr += tmp_fr[t] * w;
                fg += tmp_fg[t] * w;
                fb += tmp_fb[t] * w;
                wr += tmp_wr[t] * w;
                wg += tmp_wg[t] * w;
                wb += tmp_wb[t] * w;
            }
            foliage[gz * gs + gx] = (to_gamma(fr), to_gamma(fg), to_gamma(fb));
            water[gz * gs + gx] = (to_gamma(wr), to_gamma(wg), to_gamma(wb));
        }
    }

    (foliage, water)
}
#[inline]
fn to_linear(c: u8) -> f32 {
    (c as f32 / 255.0).powf(2.2)
}
#[inline]
fn to_gamma(c: f32) -> u8 {
    (c.powf(1.0 / 2.2) * 255.0).clamp(0.0, 255.0) as u8
}

pub fn generate_mesh(
    chunk: &Chunk,
    registry: &VoxelRegistry,
    neighbours: &ChunkNeighbours,
    biome_registry: &BiomeRegistry,
) -> (Vec<VoxelVertex>, Vec<u32>, Vec<VoxelVertex>, Vec<u32>) {
    let lod = chunk.lod as usize;
    let gs = 32 / lod;

    // phase 1: fill the voxel grid at this lod
    let mut grid = vec![0u16; gs * gs * gs];
    for gz in 0..gs {
        for gy in 0..gs {
            for gx in 0..gs {
                grid[gz * gs * gs + gy * gs + gx] =
                    representative_voxel(chunk, gx * lod, gy * lod, gz * lod, lod);
            }
        }
    }

    // phase 2: fill one-voxel-deep border slabs from each neighbour chunk
    // each border is a gs*gs slab indexed [v * gs + u]
    let mut border_px = vec![0u16; gs * gs];
    let mut border_nx = vec![0u16; gs * gs];
    let mut border_py = vec![0u16; gs * gs];
    let mut border_ny = vec![0u16; gs * gs];
    let mut border_pz = vec![0u16; gs * gs];
    let mut border_nz = vec![0u16; gs * gs];

    if let Some(n) = &neighbours.px {
        for v in 0..gs {
            for u in 0..gs {
                border_px[v * gs + u] = representative_voxel(n, 0, u * lod, v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.nx {
        for v in 0..gs {
            for u in 0..gs {
                border_nx[v * gs + u] =
                    representative_voxel(n, 31 - (lod - 1), u * lod, v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.py {
        for v in 0..gs {
            for u in 0..gs {
                border_py[v * gs + u] = representative_voxel(n, u * lod, 0, v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.ny {
        for v in 0..gs {
            for u in 0..gs {
                border_ny[v * gs + u] =
                    representative_voxel(n, u * lod, 31 - (lod - 1), v * lod, lod);
            }
        }
    }
    if let Some(n) = &neighbours.pz {
        for v in 0..gs {
            for u in 0..gs {
                border_pz[v * gs + u] = representative_voxel(n, u * lod, v * lod, 0, lod);
            }
        }
    }
    if let Some(n) = &neighbours.nz {
        for v in 0..gs {
            for u in 0..gs {
                border_nz[v * gs + u] =
                    representative_voxel(n, u * lod, v * lod, 31 - (lod - 1), lod);
            }
        }
    }

    // phase 3: precompute per-id property tables
    // avoids repeated registry lookups inside the hot face loop
    let n_defs = registry.defs.len();

    // transparent[id] id 0 (air) is always transparent
    let transparent: Vec<bool> = (0..n_defs)
        .map(|i| i == 0 || registry.defs[i].has_component::<IsTransparent>())
        .collect();

    // look up the water id once
    let water_id: u16 = registry.get("Apostasy:Voxel:Water").unwrap_or(0);

    // tint_type[id] - None if this voxel has no tint
    let tint_type: Vec<Option<TintType>> = (0..n_defs)
        .map(|i| {
            registry.defs[i]
                .get_component::<HasTint>()
                .ok()
                .map(|ht| ht.0)
        })
        .collect();

    // phase 4: build per-cell biome tint maps
    let (foliage_map, water_map) = build_tint_maps(gs, chunk, neighbours, biome_registry);
    // phase 5: iterate faces and emit vertices

    // look up a voxel from the grid or border slabs; returns 0 for corners
    let voxel_at = |gx: i32, gy: i32, gz: i32| -> u16 {
        let in_x = (0..gs as i32).contains(&gx);
        let in_y = (0..gs as i32).contains(&gy);
        let in_z = (0..gs as i32).contains(&gz);
        match (in_x, in_y, in_z) {
            (true, true, true) => grid[gz as usize * gs * gs + gy as usize * gs + gx as usize],
            (false, true, true) if gx < 0 => border_nx[gz as usize * gs + gy as usize],
            (false, true, true) => border_px[gz as usize * gs + gy as usize],
            (true, false, true) if gy < 0 => border_ny[gz as usize * gs + gx as usize],
            (true, false, true) => border_py[gz as usize * gs + gx as usize],
            (true, true, false) if gz < 0 => border_nz[gy as usize * gs + gx as usize],
            (true, true, false) => border_pz[gy as usize * gs + gx as usize],
            _ => 0,
        }
    };

    let is_solid = |id: u16| id != 0 && !transparent[id as usize];

    // should the face between current and neighbour be culled
    let culls = |neighbour_id: u16, current_id: u16| -> bool {
        neighbour_id != 0
            && (!transparent[neighbour_id as usize]
                || (current_id == water_id && neighbour_id == water_id))
    };

    // face normal and two tangent axes per face (nx,ny,nz, ux,uy,uz, vx,vy,vz)
    const FACE_AXES: [(i32, i32, i32, i32, i32, i32, i32, i32, i32); 6] = [
        (1, 0, 0, 0, 1, 0, 0, 0, 1),  // +X
        (-1, 0, 0, 0, 1, 0, 0, 0, 1), // -X
        (0, 1, 0, 1, 0, 0, 0, 0, 1),  // +Y
        (0, -1, 0, 1, 0, 0, 0, 0, 1), // -Y
        (0, 0, 1, 1, 0, 0, 0, 1, 0),  // +Z
        (0, 0, -1, 1, 0, 0, 0, 1, 0), // -Z
    ];

    // ao for one corner: sample the three voxels that shade it
    let corner_ao = |face: usize, gx: i32, gy: i32, gz: i32, su: i32, sv: i32| -> u8 {
        let (nx, ny, nz, ux, uy, uz, vx, vy, vz) = FACE_AXES[face];
        let s1 = is_solid(voxel_at(
            gx + nx + su * ux,
            gy + ny + su * uy,
            gz + nz + su * uz,
        ));
        let s2 = is_solid(voxel_at(
            gx + nx + sv * vx,
            gy + ny + sv * vy,
            gz + nz + sv * vz,
        ));
        let s3 = is_solid(voxel_at(
            gx + nx + su * ux + sv * vx,
            gy + ny + su * uy + sv * vy,
            gz + nz + su * uz + sv * vz,
        ));
        if s1 && s2 {
            0
        } else {
            3 - (s1 as u8 + s2 as u8 + s3 as u8)
        }
    };

    let max_faces = gs * gs * gs * 6;
    let mut vertices: Vec<VoxelVertex> = Vec::with_capacity(max_faces * 4);
    let mut indices: Vec<u32> = Vec::with_capacity(max_faces * 6);
    let mut water_vertices: Vec<VoxelVertex> = Vec::with_capacity(max_faces * 4);
    let mut water_indices: Vec<u32> = Vec::with_capacity(max_faces * 6);

    // corner positions as unit offsets from the voxel origin, per face
    const CORNER_OFFSETS: [[[u8; 3]; 4]; 6] = [
        [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]], // +X
        [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]], // -X
        [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]], // +Y
        [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]], // -Y
        [[1, 0, 1], [1, 1, 1], [0, 1, 1], [0, 0, 1]], // +Z
        [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]], // -Z
    ];

    // tangent signs (su, sv) for each corner's ao sample, per face
    const CORNER_SIGNS: [[(i32, i32); 4]; 6] = [
        [(-1, -1), (1, -1), (1, 1), (-1, 1)], // +X
        [(-1, 1), (1, 1), (1, -1), (-1, -1)], // -X
        [(-1, 1), (1, 1), (1, -1), (-1, -1)], // +Y
        [(-1, -1), (1, -1), (1, 1), (-1, 1)], // -Y
        [(1, -1), (1, 1), (-1, 1), (-1, -1)], // +Z
        [(-1, -1), (-1, 1), (1, 1), (1, -1)], // -Z
    ];

    for gz in 0..gs {
        for gy in 0..gs {
            let row_base = gz * gs * gs + gy * gs;
            for gx in 0..gs {
                let id = grid[row_base + gx];
                if id == 0 {
                    continue;
                }

                let l = lod as u8;
                let vx = (gx * lod) as u8;
                let vy = (gy * lod) as u8;
                let vz = (gz * lod) as u8;

                let is_water = id == water_id;
                let tint = match tint_type[id as usize] {
                    Some(TintType::Foliage) => foliage_map[gz * gs + gx],
                    Some(TintType::Water) => water_map[gz * gs + gx],
                    None => (0, 0, 0),
                };

                let voxel_def = &registry.defs[id as usize];
                let (igx, igy, igz) = (gx as i32, gy as i32, gz as i32);

                for face in 0..6usize {
                    let (nx, ny, nz, ..) = FACE_AXES[face];
                    let neighbour_id = voxel_at(igx + nx, igy + ny, igz + nz);
                    if culls(neighbour_id, id) {
                        continue;
                    }

                    let texture_id = voxel_def
                        .textures
                        .get_for_face(face as u8, vx as u32, vy as u32, vz as u32);

                    let offsets = &CORNER_OFFSETS[face];
                    let signs = &CORNER_SIGNS[face];

                    let mut ao = [0u8; 4];
                    for (ci, &(su, sv)) in signs.iter().enumerate() {
                        ao[ci] = corner_ao(face, igx, igy, igz, su, sv);
                    }

                    let (target_v, target_i) = if is_water {
                        (&mut water_vertices, &mut water_indices)
                    } else {
                        (&mut vertices, &mut indices)
                    };

                    let base = target_v.len() as u32;
                    let uvs = [(0u8, 0u8), (1, 0), (1, 1), (0, 1)];

                    for ci in 0..4 {
                        let [ox, oy, oz] = offsets[ci];
                        let cx = vx + ox * l;
                        let cy = vy + oy * l;
                        let cz = vz + oz * l;
                        let is_top = cy == vy + l;
                        target_v.push(VoxelVertex::pack(
                            cx,
                            cy,
                            cz,
                            face as u8,
                            uvs[ci].0,
                            uvs[ci].1,
                            is_top,
                            texture_id as u16,
                            ao[ci],
                            tint.0,
                            tint.1,
                            tint.2,
                        ));
                    }

                    // flip the quad diagonal to avoid ao anisotropy artefacts
                    if ao[0] + ao[2] > ao[1] + ao[3] {
                        target_i.extend_from_slice(&[
                            base,
                            base + 1,
                            base + 2,
                            base,
                            base + 2,
                            base + 3,
                        ]);
                    } else {
                        target_i.extend_from_slice(&[
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

    (vertices, indices, water_vertices, water_indices)
}

// returns the first non-air voxel in the lod*lod*lod sub-block at (x, y, z)
fn representative_voxel(chunk: &Chunk, x: usize, y: usize, z: usize, lod: usize) -> u16 {
    for dz in 0..lod {
        for dy in 0..lod {
            for dx in 0..lod {
                let (sx, sy, sz) = (x + dx, y + dy, z + dz);
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

// resolves the biome id for a sample that may be in a neighbouring chunk
fn resolve_biome(sx: i32, sz: i32, gs: i32, chunk: &Chunk, neighbours: &ChunkNeighbours) -> u16 {
    let in_x = (0..gs).contains(&sx);
    let in_z = (0..gs).contains(&sz);

    let neighbour = match (in_x, in_z) {
        (true, true) => return chunk.biome,
        (false, true) => {
            if sx < 0 {
                neighbours.nx.as_ref()
            } else {
                neighbours.px.as_ref()
            }
        }
        (true, false) => {
            if sz < 0 {
                neighbours.nz.as_ref()
            } else {
                neighbours.pz.as_ref()
            }
        }
        (false, false) => neighbours.px.as_ref().or(neighbours.pz.as_ref()), // corner, pick any
    };

    neighbour.map_or(chunk.biome, |n| n.biome)
}
