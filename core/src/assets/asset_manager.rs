use anyhow::Result;
use ash::vk::CommandPool;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::assets::gltf::{ModelLoader, ModelRegistry};
use crate::assets::loader::YamlAssetLoader;
use crate::assets::shader_registry::ShaderRegistry;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::{log, log_warn};

pub struct AssetManager {
    yaml_loaders: HashMap<String, Box<dyn YamlAssetLoader>>,
    pub model_loader: ModelLoader,
    pub shader_registry: ShaderRegistry,
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            yaml_loaders: HashMap::new(),
            model_loader: ModelLoader::default(),
            shader_registry: ShaderRegistry::new(),
        }
    }

    pub fn register_loader<L: YamlAssetLoader + 'static>(&mut self, loader: L) {
        self.yaml_loaders
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

        match self.yaml_loaders.get_mut(class) {
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

    pub fn load_models(
        &mut self,
        path: &Path,
        context: Arc<VulkanRenderingContext>,
        command_pool: CommandPool,
    ) -> Result<ModelRegistry> {
        let models = ModelLoader::load_all_models(path, context, command_pool)?;

        let mut registry = self.model_loader.registry.write().unwrap();
        for (name, model) in models {
            registry.paths.insert(name, model);
        }

        Ok(registry.clone())
    }
    /// Recursively load all .yaml files in a directory
    pub fn load_directory(&mut self, path: &Path) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.load_directory(&path)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("yaml")
                && let Err(e) = self.load_file(&path)
            {
                log_warn!("Failed to load {:?}: {}", path, e);
            }
        }

        Ok(())
    }
}
