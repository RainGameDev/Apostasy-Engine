use anyhow::Result;

pub trait AssetLoader: Send + Sync {
    fn class_name(&self) -> &'static str;
    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()>;
}
