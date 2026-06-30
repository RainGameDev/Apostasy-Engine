use anyhow::Result;
use apostasy_macros::Resource;
use ash::vk::{self, CommandPool};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::assets::gltf::{ModelLoader, ModelRegistry};
use crate::assets::loader::YamlAssetLoader;
use crate::assets::loaders::material_loader::MaterialLoader;
use crate::assets::audio::list_available_audio;
use crate::assets::shader::list_available_shaders;
use crate::assets::texture::list_available_textures;
use crate::assets::shader_registry::ShaderRegistry;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::{log, log_warn};

#[derive(Clone, Resource)]
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

    pub fn get_loader<L: YamlAssetLoader + 'static>(&self) -> Option<&L> {
        self.yaml_loaders
            .values()
            .find_map(|l| l.as_any().downcast_ref::<L>())
    }

    pub fn get_loader_mut<L: YamlAssetLoader + 'static>(&mut self) -> Option<&mut L> {
        self.yaml_loaders
            .values_mut()
            .find_map(|l| l.as_any_mut().downcast_mut::<L>())
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
        descriptor_pool: vk::DescriptorPool,
        descriptor_set_layout: vk::DescriptorSetLayout,
    ) -> Result<ModelRegistry> {
        let mat_registry = self
            .get_loader::<MaterialLoader>()
            .map(|l| l.registry.read().clone())
            .unwrap_or_default();

        let models = ModelLoader::load_all_models(
            path,
            context,
            command_pool,
            descriptor_pool,
            descriptor_set_layout,
            &mat_registry,
        )?;

        let mut registry = self.model_loader.registry.write();
        for (name, model) in models {
            registry.paths.insert(name, model);
        }

        Ok(registry.clone())
    }
    pub fn model_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .model_loader
            .registry
            .read()
            .paths
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }

    pub fn shader_names(&self) -> Vec<String> {
        list_available_shaders()
    }

    pub fn texture_names(&self) -> Vec<String> {
        list_available_textures()
    }

    pub fn audio_names(&self) -> Vec<String> {
        list_available_audio()
    }

    /// Returns (class_name, [(namespace, name)]) for all registered loaders.
    pub fn all_loader_entries(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut result: Vec<(String, Vec<(String, String)>)> = self
            .yaml_loaders
            .iter()
            .map(|(class, loader)| (class.clone(), loader.list_entries()))
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Recursively load all .yaml files in a directory
    pub fn load_directory(&mut self, path: &Path) -> Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if dir_name == ".editor" {
                        continue;
                    }
                }
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
