use std::sync::{Arc, RwLock};

use anyhow::{Error, Result};

use crate::{assets::loader::AssetLoader, voxels::structure::{StructureAsset, StructureId, StructureRegistry}};

pub struct StructureLoader {
    pub registry: Arc<RwLock<StructureRegistry>>,
}

impl AssetLoader for StructureLoader {
    fn class_name(&self) -> &'static str {
        "Structure"
    }

    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()> {
        let name: String = raw["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'name'"))?
            .to_string();

        let namespace: String = raw["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'namespace'"))?
            .to_string();

        {
            let registry = self.registry.read().unwrap();
            for reg in registry.defs.iter() {
                if reg.name == name && reg.namespace == namespace {
                    let msg = format!(
                        "Structure with the name: {} exists in name space {} already",
                        name.to_string(),
                        namespace.to_string()
                    );

                    return Err(Error::msg(msg));
                }
            }
        }

        let origin = if let Some(origin_seq) = raw["origin"].as_sequence() {
            let coords = origin_seq
                .iter()
                .take(3)
                .filter_map(|v| v.as_i64())
                .map(|v| v as i32)
                .collect::<Vec<_>>();
            if coords.len() == 3 {
                [coords[0], coords[1], coords[2]]
            } else {
                [0, 0, 0]
            }
        } else {
            [0, 0, 0]
        };

        let size = if let Some(size_seq) = raw["size"].as_sequence() {
            let coords = size_seq
                .iter()
                .take(3)
                .filter_map(|v| v.as_i64())
                .map(|v| v as i32)
                .collect::<Vec<_>>();
            if coords.len() == 3 {
                Some([coords[0], coords[1], coords[2]])
            } else {
                None
            }
        } else {
            None
        };

        let mut blocks: Vec<crate::voxels::structure::StructureBlock> = Vec::new();
        if let Some(block_seq) = raw["blocks"].as_sequence() {
            for block_value in block_seq {
                let position = block_value["position"]
                    .as_sequence()
                    .ok_or_else(|| anyhow::anyhow!("Structure block missing 'position'"))?
                    .iter()
                    .take(3)
                    .filter_map(|v| v.as_i64())
                    .map(|v| v as i32)
                    .collect::<Vec<_>>();
                if position.len() != 3 {
                    return Err(anyhow::anyhow!("Structure block position must have 3 values"));
                }

                let voxel = block_value["voxel"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Structure block missing 'voxel'"))?
                    .to_string();

                blocks.push(crate::voxels::structure::StructureBlock {
                    position: [position[0], position[1], position[2]],
                    voxel,
                });
            }
        } else {
            return Err(anyhow::anyhow!("Missing 'blocks' for Structure"));
        }

        let mut metadata = std::collections::HashMap::new();
        if let Some(meta_map) = raw["metadata"].as_mapping() {
            for (key, value) in meta_map {
                if let Some(k) = key.as_str() {
                    metadata.insert(k.to_string(), value.clone());
                }
            }
        }

        let def = StructureAsset {
            name: name.clone(),
            namespace: namespace.clone(),
            class: "Structure".to_string(),
            origin,
            size,
            blocks,
            metadata,
        };

        let mut registry = self.registry.write().unwrap();
        for reg in registry.defs.iter() {
            if reg.name == name && reg.namespace == namespace {
                let msg = format!(
                    "Structure with the name: {} exists in name space {} already",
                    name.to_string(),
                    namespace.to_string()
                );
                return Err(Error::msg(msg));
            }
        }

        let id = registry.defs.len() as StructureId;
        let full_name = format!("{}:Structure:{}", namespace, name);
        registry.defs.push(def);
        registry.name_to_id.insert(full_name.clone(), id);
        registry.id_to_name.insert(id, full_name);

        Ok(())
    }
}
