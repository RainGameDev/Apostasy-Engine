use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use anyhow::Result;

use crate::assets::loader::YamlAssetLoader;

pub struct SceneRegistry {
    pub scenes: HashMap<String, serde_yaml::Value>,
}

impl Default for SceneRegistry {
    fn default() -> Self {
        Self {
            scenes: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct SceneLoader {
    pub registry: Arc<RwLock<SceneRegistry>>,
}

impl YamlAssetLoader for SceneLoader {
    fn class_name(&self) -> &'static str {
        "scene"
    }

    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()> {
        let name = raw["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' in scene"))?
            .to_string();

        let mut registry = self.registry.write().unwrap();
        registry.scenes.insert(name, raw.clone());
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
        let registry = self.registry.read().unwrap();
        registry
            .scenes
            .keys()
            .map(|name| ("scene".to_string(), name.clone()))
            .collect()
    }
}
