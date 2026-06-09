use anyhow::Result;
use std::any::Any;

pub trait YamlAssetLoader: Send + Sync {
    fn class_name(&self) -> &'static str;
    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()>;
    fn clone_box(&self) -> Box<dyn YamlAssetLoader>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Returns (namespace, name) pairs for all loaded definitions.
    fn list_entries(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

impl Clone for Box<dyn YamlAssetLoader> {
    fn clone(&self) -> Box<dyn YamlAssetLoader> {
        self.clone_box()
    }
}
