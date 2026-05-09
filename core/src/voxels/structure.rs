use std::{collections::HashMap, path::Path};

use anyhow::Result;
use apostasy_macros::Resource;

pub type StructureId = u16;

/// A global registry of loaded structure assets.
///
/// `defs` holds the ordered list of assets, while the hash maps provide
/// fast lookup by full name or by id.
#[derive(Resource, Default, Clone, Debug)]
pub struct StructureRegistry {
    pub defs: Vec<StructureAsset>,
    pub name_to_id: HashMap<String, StructureId>,
    pub id_to_name: HashMap<StructureId, String>,
}

/// A single block inside a structure asset.
///
/// `position` is relative to the structure's origin, and `voxel` is the
/// registered voxel name used when placing the structure into the world.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StructureBlock {
    pub position: [i32; 3],
    pub voxel: String,
}

/// A reusable structure definition asset.
///
/// `origin` is the placement anchor within block space, `size` is an optional
/// bounding box for the structure, `blocks` contains the voxel layout, and
/// `metadata` can carry custom YAML values for tooling or behavior.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StructureAsset {
    pub name: String,
    pub namespace: String,
    pub class: String,
    pub origin: [i32; 3],
    pub size: Option<[i32; 3]>,
    pub blocks: Vec<StructureBlock>,
    pub metadata: HashMap<String, serde_yaml::Value>,
}

impl StructureAsset {
    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(path, yaml)?;
        Ok(())
    }
}
