use std::any::TypeId;

use anyhow::{Error, Result};
use apostasy_macros::Resource;
use hashbrown::HashMap;
use rand::{RngExt, SeedableRng, rngs::SmallRng};

use crate::objects::component::{BoxedComponent, Component};

#[derive(Debug, Clone)]
pub struct VoxelTextures {
    pub top: Vec<u32>,
    pub bottom: Vec<u32>,
    pub front: Vec<u32>,
    pub back: Vec<u32>,
    pub left: Vec<u32>,
    pub right: Vec<u32>,
}

impl VoxelTextures {
    pub fn single(index: u32) -> Vec<u32> {
        vec![index]
    }

    pub fn all(index: u32) -> Self {
        Self {
            top: vec![index],
            bottom: vec![index],
            front: vec![index],
            back: vec![index],
            left: vec![index],
            right: vec![index],
        }
    }

    pub fn get_for_face(&self, face: u8, x: u32, y: u32, z: u32) -> u32 {
        let options = match face {
            0 => &self.right,
            1 => &self.left,
            2 => &self.top,
            3 => &self.bottom,
            4 => &self.front,
            5 => &self.back,
            _ => &self.top,
        };

        if options.len() == 1 {
            return options[0];
        }

        // seed with position so same block always gets same texture
        let seed = ((x as u64) << 32) ^ ((y as u64) << 16) ^ (z as u64) ^ ((face as u64) << 48);
        let mut rng = SmallRng::seed_from_u64(seed);
        let index = rng.random_range(0..options.len());
        options[index]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Voxel {
    pub id: VoxelId,
}

#[derive(Clone)]
pub struct VoxelDefinition {
    pub name: String,
    pub namespace: String,
    pub class: String,
    pub components: Vec<BoxedComponent>,
    pub textures: VoxelTextures,
}

impl std::fmt::Debug for VoxelDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoxelDefinition")
            .field("name", &self.name)
            .field("namespace", &self.namespace)
            .field("class", &self.class)
            .field("component_count", &self.components.len())
            .finish()
    }
}

impl VoxelDefinition {
    /// Checks if the voxel has a component of type T
    pub fn has_component<T: Component + 'static>(&self) -> bool {
        self.components
            .iter()
            .any(|component| component.as_any().downcast_ref::<T>().is_some())
    }

    /// Gets a component of voxel T from the node
    pub fn get_component<T: Component + 'static>(&self) -> Result<&T> {
        self.components
            .iter()
            .find(|c| c.as_any().type_id() == TypeId::of::<T>())
            .and_then(|c| c.as_any().downcast_ref())
            .ok_or(Error::msg("No Comopnent of type"))
    }
}

pub type VoxelId = u16;

#[derive(Resource, Clone, Debug)]
pub struct VoxelRegistry {
    pub defs: Vec<VoxelDefinition>,
    pub name_to_id: HashMap<String, VoxelId>,
    pub id_to_name: HashMap<VoxelId, String>,
}

impl Default for VoxelRegistry {
    fn default() -> Self {
        VoxelRegistry::new()
    }
}

impl VoxelRegistry {
    pub fn new() -> Self {
        let mut defs = Vec::new();
        let mut name_to_id = HashMap::new();
        let mut id_to_name = HashMap::new();

        // reserve id 0 for air
        defs.push(VoxelDefinition {
            name: "Air".to_string(),
            namespace: "Apostasy".to_string(),
            class: "Voxel".to_string(),
            components: vec![],
            textures: VoxelTextures::all(0),
        });
        name_to_id.insert("Apostasy:Air".to_string(), 0);
        id_to_name.insert(0, "Apostasy:Air".to_string());

        Self {
            defs,
            name_to_id,
            id_to_name,
        }
    }
    pub fn get_def(&self, id: VoxelId) -> Result<&VoxelDefinition> {
        let msg = format!("Voxel {} not found", id);
        self.defs.get(id as usize).ok_or(Error::msg(msg))
    }
}
