use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::assets::loader::AssetLoader;
use crate::{log, log_warn};

pub struct AssetManager {
    loaders: HashMap<String, Box<dyn AssetLoader>>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            loaders: HashMap::new(),
        }
    }

    pub fn register_loader<L: AssetLoader + 'static>(&mut self, loader: L) {
        self.loaders
            .insert(loader.class_name().to_string(), Box::new(loader));
    }

    /// Load a single .yaml file
    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let raw: serde_yaml::Value = serde_yaml::from_str(&content)?;

        let name = raw["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'class' field in {:?}", path))?;
        let namespace = raw["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'class' field in {:?}", path))?;
        let class = raw["class"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'class' field in {:?}", path))?;

        match self.loaders.get_mut(class) {
            Some(loader) => {
                loader.load(&raw)?;
                log!("Loaded {:?}:{:?} as class '{}'", namespace, name, class);
            }
            None => {
                // log_warn!("No loader registered for class '{}' in {:?}", class, path);
            }
        }

        Ok(())
    }

    /// Recursively load all .yaml files in a directory
    pub fn load_directory(&mut self, path: &Path) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.load_directory(&path)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
                if let Err(e) = self.load_file(&path) {
                    log_warn!("Failed to load {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }
}
