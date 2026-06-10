use apostasy_macros::{Component, Inspect};

use crate::rendering::shared::model::GpuModel;

#[derive(Component, Inspect, Clone, Debug)]
pub struct ModelRenderer {
    pub model: Option<Box<GpuModel>>,
    pub model_path: String,
    pub is_wireframe: bool,
}

impl Default for ModelRenderer {
    fn default() -> Self {
        Self {
            model: None,
            model_path: "cube".to_string(),
            is_wireframe: false,
        }
    }
}

impl ModelRenderer {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        if let Some(v) = value.get("model_path").and_then(|v| v.as_str()) {
            self.model_path = v.to_string();
        }
        if let Some(v) = value.get("is_wireframe").and_then(|v| v.as_bool()) {
            self.is_wireframe = v;
        }
        Ok(())
    }
    pub fn from_path(path: &str) -> Self {
        Self {
            model: None,
            model_path: path.to_string(),
            is_wireframe: false,
        }
    }
}
