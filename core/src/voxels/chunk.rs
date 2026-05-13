use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use apostasy_macros::{Component, Resource};
use cgmath::Vector3;
use crossbeam_channel::{Receiver, Sender, unbounded};
use hashbrown::HashMap;
use rayon::ThreadPool;

use crate::{
    utils::flatten::flatten,
    voxels::{
        biome::BiomeId,
        meshes::{ChunkNeighbours, VoxelVertex},
        voxel::{Voxel, VoxelDefinition, VoxelId, VoxelRegistry},
    },
};

#[derive(Resource, Clone, Default)]
pub struct VoxelBreakProgress {
    pub progress: HashMap<(i32, i32, i32), u32>,
}

#[derive(Clone, Component, Debug)]
pub struct Chunk {
    pub voxels: Box<[VoxelId; 32 * 32 * 32]>,
    pub lod: u8,
    pub biome: BiomeId,
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            voxels: Box::new([VoxelId::default(); 32 * 32 * 32]),
            lod: 1,
            biome: 0,
        }
    }
}

impl Chunk {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }

    fn _get_def<'a>(
        &self,
        x: u32,
        y: u32,
        z: u32,
        registry: &'a VoxelRegistry,
    ) -> &'a VoxelDefinition {
        let id = self.voxels[flatten(x, y, z, 32)];
        &registry.defs[id as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, z: u32, voxel: Voxel) {
        self.voxels[flatten(x, y, z, 32)] = voxel.id;
    }

    pub fn set_if_empty(&mut self, x: u32, y: u32, z: u32, voxel: Voxel) {
        if self.voxels[flatten(x, y, z, 32)] == 0 {
            self.voxels[flatten(x, y, z, 32)] = voxel.id;
        }
    }

    pub fn set_lod(&mut self, lod: u8) {
        self.lod = lod;
    }

    pub fn has_visible_faces(&self, neighbours: &ChunkNeighbours) -> bool {
        const SIZE: usize = 32;
        const AREA: usize = SIZE * SIZE;
        const VOL: usize = SIZE * SIZE * SIZE;
        let v = &self.voxels;

        for i in 0..VOL {
            let id = v[i];
            if id == 0 {
                continue;
            }

            let x = i % SIZE;
            let y = (i / SIZE) % SIZE;
            let z = i / AREA;

            let neighbour_is_air = |dx: i32, dy: i32, dz: i32| -> bool {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                let nz = z as i32 + dz;

                if nx >= 0
                    && nx < SIZE as i32
                    && ny >= 0
                    && ny < SIZE as i32
                    && nz >= 0
                    && nz < SIZE as i32
                {
                    v[nz as usize * AREA + ny as usize * SIZE + nx as usize] == 0
                } else {
                    let (chunk, bx, by, bz): (&Option<Chunk>, usize, usize, usize) =
                        match (dx, dy, dz) {
                            (1, 0, 0) => (&neighbours.px, 0, y, z),
                            (-1, 0, 0) => (&neighbours.nx, SIZE - 1, y, z),
                            (0, 1, 0) => (&neighbours.py, x, 0, z),
                            (0, -1, 0) => (&neighbours.ny, x, SIZE - 1, z),
                            (0, 0, 1) => (&neighbours.pz, x, y, 0),
                            (0, 0, -1) => (&neighbours.nz, x, y, SIZE - 1),
                            _ => return true,
                        };
                    chunk
                        .as_ref()
                        .map(|n| n.voxels[bz * AREA + by * SIZE + bx] == 0)
                        .unwrap_or(true)
                }
            };

            if neighbour_is_air(1, 0, 0)
                || neighbour_is_air(-1, 0, 0)
                || neighbour_is_air(0, 1, 0)
                || neighbour_is_air(0, -1, 0)
                || neighbour_is_air(0, 0, 1)
                || neighbour_is_air(0, 0, -1)
            {
                return true;
            }
        }
        false
    }
}

pub struct GeneratedChunkData {
    pub position: Vector3<i32>,
    pub voxels: Box<[VoxelId; 32 * 32 * 32]>,
    pub lod: u8,
    pub biome: u16,
}

pub struct GeneratedMeshData {
    pub position: Vector3<i32>,
    pub opaque_vertices: Vec<VoxelVertex>,
    pub opaque_indices: Vec<u32>,
    pub water_vertices: Vec<VoxelVertex>,
    pub water_indices: Vec<u32>,
}

pub type MeshJobFn = Box<dyn FnOnce() + Send + 'static>;
#[derive(Resource, Clone)]
pub struct ChunkGenQueue {
    pub sender: Sender<GeneratedChunkData>,
    pub receiver: Receiver<GeneratedChunkData>,
    pub mesh_job_sender: Sender<MeshJobFn>,
    pub mesh_result_sender: Sender<GeneratedMeshData>,
    pub mesh_receiver: Receiver<GeneratedMeshData>,
    pub pool: Arc<Mutex<ThreadPool>>,
    pub in_flight: HashSet<Vector3<i32>>,
}

impl Default for ChunkGenQueue {
    fn default() -> Self {
        let (sender, receiver) = unbounded::<GeneratedChunkData>();
        let (mesh_job_tx, mesh_job_rx) = unbounded::<MeshJobFn>();
        let (mesh_result_tx, mesh_result_rx) = unbounded::<GeneratedMeshData>();

        let total = num_cpus::get();
        let gen_threads = (total / 2).max(1);
        let mesh_threads = (total - gen_threads).max(1);

        for _ in 0..mesh_threads {
            let job_rx = mesh_job_rx.clone();
            std::thread::Builder::new()
                .name("mesh-worker".into())
                .spawn(move || {
                    for job in &job_rx {
                        job();
                    }
                })
                .expect("Failed to spawn mesh worker thread");
        }

        Self {
            sender,
            receiver,
            mesh_job_sender: mesh_job_tx,
            mesh_result_sender: mesh_result_tx,
            mesh_receiver: mesh_result_rx,
            pool: Arc::new(Mutex::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(gen_threads)
                    .build()
                    .unwrap(),
            )),
            in_flight: HashSet::new(),
        }
    }
}
