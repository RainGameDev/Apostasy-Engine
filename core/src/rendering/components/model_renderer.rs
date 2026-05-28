use apostasy_macros::Component;

use crate::rendering::shared::model::GpuModel;

#[derive(Component, Clone, Debug)]
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
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
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
