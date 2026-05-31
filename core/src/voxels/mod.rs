use apostasy_macros::{Component, Inspect};
use cgmath::{Vector3, Zero};

pub mod biome;
pub mod chunk;
pub mod chunk_loader;
pub mod heightmap;
pub mod meshes;
pub mod structure;
pub mod texture_atlas;
pub mod voxel;
pub mod voxel_components;
pub mod voxel_raycast;

#[derive(Component, Inspect, Clone, Debug)]
pub struct VoxelTransform {
    pub position: Vector3<i32>,
}
impl VoxelTransform {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}

impl Default for VoxelTransform {
    fn default() -> Self {
        Self {
            position: Vector3::zero(),
        }
    }
}
