use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use crate::assets::shader::{
    ShaderKind, load_shader_bytes, resolve_shader_path, shader_kind_from_path,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
}

impl ShaderStage {
    pub fn vk_stage_flag(&self) -> ash::vk::ShaderStageFlags {
        match self {
            ShaderStage::Vertex => ash::vk::ShaderStageFlags::VERTEX,
            ShaderStage::Fragment => ash::vk::ShaderStageFlags::FRAGMENT,
        }
    }
}

#[derive(Debug)]
pub struct ShaderAsset {
    pub name: String,
    pub path: PathBuf,
    pub stage: ShaderStage,
    pub last_modified: Option<SystemTime>,
    pub bytes: Vec<u8>,
}

impl ShaderAsset {
    pub fn load(name: &str) -> Result<Self> {
        let path = resolve_shader_path(name)
            .ok_or_else(|| anyhow::anyhow!("Shader '{}' was not found", name))?;

        let bytes = load_shader_bytes(name)?;
        let kind = shader_kind_from_path(&path)?;
        let stage = match kind {
            ShaderKind::Vertex => ShaderStage::Vertex,
            ShaderKind::Fragment => ShaderStage::Fragment,
        };

        let last_modified = fs_metadata_modified(&path);

        Ok(Self {
            name: name.to_string(),
            path,
            stage,
            last_modified,
            bytes,
        })
    }

    pub fn reload_if_needed(&mut self) -> Result<bool> {
        let current_modified = fs_metadata_modified(&self.path);
        if current_modified != self.last_modified {
            self.bytes = load_shader_bytes(self.name.as_str())?;
            self.last_modified = current_modified;
            return Ok(true);
        }
        Ok(false)
    }
}

fn fs_metadata_modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

pub struct ShaderRegistry {
    shaders: RwLock<HashMap<String, Arc<RwLock<ShaderAsset>>>>,
}

impl Default for ShaderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderRegistry {
    pub fn new() -> Self {
        Self {
            shaders: RwLock::new(HashMap::new()),
        }
    }

    pub fn load_shader(&self, name: &str) -> Result<Arc<RwLock<ShaderAsset>>> {
        let asset = ShaderAsset::load(name)?;
        let asset = Arc::new(RwLock::new(asset));
        self.shaders
            .write()
            .unwrap()
            .insert(name.to_string(), asset.clone());
        Ok(asset)
    }

    pub fn get_shader(&self, name: &str) -> Option<Arc<RwLock<ShaderAsset>>> {
        self.shaders.read().unwrap().get(name).cloned()
    }

    pub fn reload_shader(&self, name: &str) -> Result<bool> {
        if let Some(asset) = self.shaders.read().unwrap().get(name).cloned() {
            let mut asset = asset.write().unwrap();
            asset.reload_if_needed()
        } else {
            Ok(false)
        }
    }

    pub fn reload_changed_shaders(&self) -> Result<Vec<String>> {
        let mut reloaded = Vec::new();
        for (name, asset) in self.shaders.read().unwrap().iter() {
            let mut asset = asset.write().unwrap();
            if asset.reload_if_needed()? {
                reloaded.push(name.clone());
            }
        }
        Ok(reloaded)
    }

    pub fn load_directory(&self, directory: &Path) -> Result<()> {
        for entry in std::fs::read_dir(directory).context("Failed to read shader directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.load_directory(&path)?;
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "vert" || ext == "frag" || ext == "spv" {
                    let name = path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .ok_or_else(|| anyhow::anyhow!("Invalid shader filename"))?;
                    self.load_shader(name)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_constructs() {
        let registry = ShaderRegistry::new();
        assert!(registry.get_shader("does-not-exist").is_none());
    }
}
