use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use anyhow::Result;
use apostasy_macros::Resource;
use hashbrown::HashMap;

use crate::assets::loader::YamlAssetLoader;

/// CPU-side material definition loaded from YAML. No GPU resources.
#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub namespace: String,
    /// Path relative to res/ for the albedo texture, e.g. "textures/brick.png"
    pub albedo_path: Option<String>,
    /// RGBA base color multiplier (default white)
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default, Resource)]
pub struct MaterialRegistry {
    /// Keyed by material name as declared in the YAML
    pub materials: HashMap<String, Material>,
}

#[derive(Clone)]
pub struct MaterialLoader {
    pub registry: Arc<RwLock<MaterialRegistry>>,
}

impl MaterialLoader {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(MaterialRegistry::default())),
        }
    }
}

impl YamlAssetLoader for MaterialLoader {
    fn class_name(&self) -> &'static str {
        "Material"
    }

    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()> {
        let name = raw["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Material missing 'name'"))?
            .to_string();

        let namespace = raw["namespace"]
            .as_str()
            .unwrap_or("default")
            .to_string();

        let albedo_path = raw["albedo"].as_str().map(|s| s.to_string());

        let color = if let Some(seq) = raw["color"].as_sequence() {
            let vals: Vec<f32> = seq
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            if vals.len() == 4 {
                [vals[0], vals[1], vals[2], vals[3]]
            } else if vals.len() == 3 {
                [vals[0], vals[1], vals[2], 1.0]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            }
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        let mat = Material { name: name.clone(), namespace, albedo_path, color };
        self.registry.write().unwrap().materials.insert(name, mat);
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn YamlAssetLoader> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn list_entries(&self) -> Vec<(String, String)> {
        let reg = self.registry.read().unwrap();
        reg.materials
            .values()
            .map(|m| (m.namespace.clone(), m.name.clone()))
            .collect()
    }
}
