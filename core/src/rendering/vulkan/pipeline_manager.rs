use anyhow::Result;
use std::sync::Arc;

use crate::assets::shader_registry::ShaderRegistry;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use ash::vk;

pub struct PipelineManager {
    pub shader_registry: ShaderRegistry,
}

impl PipelineManager {
    pub fn new() -> Self {
        Self {
            shader_registry: ShaderRegistry::new(),
        }
    }

    pub fn create_shader_module(
        &self,
        context: &Arc<VulkanRenderingContext>,
        name: &str,
    ) -> Result<vk::ShaderModule> {
        let shader = self.shader_registry.load_shader(name)?;
        let shader = shader.read().unwrap();
        Ok(context.create_shader_module(&shader.bytes)?)
    }

    pub fn reload_shader(&self, name: &str) -> Result<bool> {
        self.shader_registry.reload_shader(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_manager_can_construct() {
        let manager = PipelineManager::new();
        assert!(manager.shader_registry.get_shader("unknown").is_none());
    }
}
