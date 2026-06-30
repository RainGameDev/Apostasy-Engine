use anyhow::Result;
use hashbrown::HashMap;
use std::sync::Arc;

use crate::assets::shader_registry::ShaderRegistry;
use crate::rendering::shared::anti_alisaing::AntiAliasingAmount;
use crate::rendering::shared::rendering_settings::{PipelineOptions, RenderingSettings};
use crate::rendering::shared::vertex::{Vertex, VertexDefinition};
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use ash::vk;
use ash::vk::*;

/// Cached parameters for the standard model pipeline — used to create custom shader variants.
pub struct ModelPipelineTemplate {
    pub layout: PipelineLayout,
    pub image_format: Option<vk::Format>,
    pub depth_format: Option<vk::Format>,
    pub image_extent: Extent2D,
    pub aa_amount: AntiAliasingAmount,
}

pub struct PipelineManager {
    pub shader_registry: ShaderRegistry,
    pub pipeline_cache: HashMap<String, Pipeline>,
    pub model_pipeline_template: Option<ModelPipelineTemplate>,
}

impl Default for PipelineManager {
    fn default() -> Self {
        PipelineManager::new()
    }
}

impl PipelineManager {
    pub fn new() -> Self {
        let shader_registry = ShaderRegistry::new();
        shader_registry.preload_all();
        Self {
            shader_registry,
            pipeline_cache: HashMap::new(),
            model_pipeline_template: None,
        }
    }

    pub fn create_shader_module(
        &self,
        context: &Arc<VulkanRenderingContext>,
        name: &str,
    ) -> Result<vk::ShaderModule> {
        let shader = self.shader_registry.load_shader(name)?;
        let shader = shader.read();
        Ok(context.create_shader_module(&shader.bytes)?)
    }

    pub fn reload_shader(&self, name: &str) -> Result<bool> {
        self.shader_registry.reload_shader(name)
    }

    /// Returns a cached pipeline for `shader_name`, creating it from `{shader_name}.vert` /
    /// `{shader_name}.frag` if it doesn't exist yet.
    /// Uses the same layout and options as the standard model pipeline.
    pub fn get_or_create_model_pipeline(
        &mut self,
        shader_name: &str,
        context: &Arc<VulkanRenderingContext>,
        wireframe: bool,
    ) -> Result<Pipeline> {
        let key = if wireframe {
            format!("model::{}::wireframe", shader_name)
        } else {
            format!("model::{}", shader_name)
        };
        if let Some(&p) = self.pipeline_cache.get(&key) {
            return Ok(p);
        }

        let template = self
            .model_pipeline_template
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Model pipeline template not initialised"))?;

        let layout = template.layout;
        let image_format = template.image_format;
        let depth_format = template.depth_format;
        let image_extent = template.image_extent;
        let aa_amount = template.aa_amount;

        let vert_name = if self
            .shader_registry
            .get_shader(&format!("{}.vert", shader_name))
            .is_some()
            || crate::assets::shader::resolve_shader_path(&format!("{}.vert", shader_name))
                .is_some()
        {
            format!("{}.vert", shader_name)
        } else {
            "sdr_default_shader.vert".to_string()
        };
        let frag_name = if self
            .shader_registry
            .get_shader(&format!("{}.frag", shader_name))
            .is_some()
            || crate::assets::shader::resolve_shader_path(&format!("{}.frag", shader_name))
                .is_some()
        {
            format!("{}.frag", shader_name)
        } else {
            "sdr_default_shader.frag".to_string()
        };

        let vert = self.create_shader_module(context, &vert_name)?;
        let frag = self.create_shader_module(context, &frag_name)?;

        let pipeline = context.create_graphics_pipeline(
            PipelineOptions {
                image_format,
                image_extent,
                depth_format,
                vertex_shader: vert,
                fragment_shader: frag,
                vertex_bindings: vec![Vertex::get_binding_description()],
                vertex_attributes: Vertex::get_attribute_descriptions(),
            },
            if wireframe {
                RenderingSettings::wireframe()
            } else {
                RenderingSettings::default()
            },
            layout,
            aa_amount,
        )?;

        unsafe {
            context.device.destroy_shader_module(vert, None);
            context.device.destroy_shader_module(frag, None);
        }

        self.pipeline_cache.insert(key, pipeline);
        Ok(pipeline)
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
