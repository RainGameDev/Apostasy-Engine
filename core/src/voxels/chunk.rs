use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use apostasy_macros::{Component, Resource};
use cgmath::Vector3;
use crossbeam_channel::{Receiver, Sender, unbounded};
use hashbrown::HashMap;
use rayon::ThreadPool;

use crate::{
    utils::flatten::flatten,
    voxels::{
        biome::BiomeId,
        meshes::VoxelVertex,
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
}

pub struct GeneratedChunkData {
    pub position: Vector3<i32>,
    pub voxels: Box<[VoxelId; 32 * 32 * 32]>,
    pub lod: u8,
    pub biome: u16,
}

pub struct GeneratedMeshData {
    pub position: Vector3<i32>,
    pub vertices: Vec<VoxelVertex>,
    pub indices: Vec<u32>,
}

#[derive(Resource, Clone)]
pub struct ChunkGenQueue {
    pub sender: Sender<GeneratedChunkData>,
    pub receiver: Receiver<GeneratedChunkData>,
    pub mesh_sender: Sender<GeneratedMeshData>,
    pub mesh_receiver: Receiver<GeneratedMeshData>,
    pub pool: Arc<Mutex<ThreadPool>>,
    pub mesh_pool: Arc<Mutex<ThreadPool>>,
    pub in_flight: HashSet<Vector3<i32>>,
}

impl Default for ChunkGenQueue {
    fn default() -> Self {
        let (sender, receiver) = unbounded();
        let (mesh_sender, mesh_receiver) = unbounded();
        let total = num_cpus::get();
        let gen_threads = (total / 2).max(1);
        let mesh_threads = (total - gen_threads).max(1);
        Self {
            sender,
            receiver,
            mesh_sender,
            mesh_receiver,
            pool: Arc::new(Mutex::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(gen_threads)
                    .build()
                    .unwrap(),
            )),
            mesh_pool: Arc::new(Mutex::new(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(mesh_threads)
                    .build()
                    .unwrap(),
            )),
            in_flight: HashSet::new(),
        }
    }
}
