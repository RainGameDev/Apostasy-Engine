use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, RwLock},
};

use anyhow::Result;

use crate::assets::loader::YamlAssetLoader;

#[derive(Default)]
pub struct WorldspaceRegistry {
    pub worldspaces: HashMap<String, serde_yaml::Value>,
}

#[derive(Clone)]
pub struct WorldspaceLoader {
    pub registry: Arc<RwLock<WorldspaceRegistry>>,
}

impl YamlAssetLoader for WorldspaceLoader {
    fn class_name(&self) -> &'static str {
        "worldspace"
    }

    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()> {
        let name = raw["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'name' in worldspace"))?
            .to_string();

        let mut registry = self.registry.write().unwrap();
        registry.worldspaces.insert(name, raw.clone());
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
            .worldspaces
            .keys()
            .map(|name| ("worldspace".to_string(), name.clone()))
            .collect()
    }
}
