use parking_lot::RwLock;
use std::{any::Any, sync::Arc};

use anyhow::Result;
use apostasy_macros::Resource;
use hashbrown::HashMap;

use crate::assets::loader::YamlAssetLoader;

/// CPU-side material definition loaded from YAML. No GPU resources.
#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub namespace: String,
    /// Path relative to res/ for the albedo texture, e.g. "textures/a_brick.png"
    pub albedo_path: Option<String>,
    /// Path relative to res/ for the normal texture, e.g. "textures/n_brick.png"
    pub normal_path: Option<String>,
    /// RGBA base color multiplier (default white)
    pub color: [f32; 4],
    /// Path relative to res/ for the custom shader, e.g. "shaders/brick"
    pub shader_path: Option<String>,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct MaterialRegistry {
    /// Keyed by material name as declared in the YAML
    pub materials: HashMap<String, Material>,
    pub version: u64,
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

        let namespace = raw["namespace"].as_str().unwrap_or("default").to_string();

        let albedo_path = raw["albedo"].as_str().map(|s| s.to_string());

        let normal_path = raw["normal"].as_str().map(|s| s.to_string());

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

        let shader = raw["shader"].as_str().map(|s| s.to_string());

        let mat = Material {
            name: name.clone(),
            namespace,
            albedo_path,
            normal_path,
            color,
            shader_path: shader,
        };
        let mut registry = self.registry.write();
        registry.materials.insert(name, mat);
        registry.version = registry.version.wrapping_add(1);
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
        let reg = self.registry.read();
        reg.materials
            .values()
            .map(|m| (m.namespace.clone(), m.name.clone()))
            .collect()
    }
}
