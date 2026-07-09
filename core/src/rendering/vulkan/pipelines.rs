use anyhow::Result;
use ash::vk::{self, Pipeline};

use crate::log;
use crate::rendering::shared::anti_aliasing::AntiAliasingAmount;
use crate::rendering::shared::rendering_settings::{
    DynamicStateSettings, PipelineOptions, RasterizationSettings, RenderingSettings,
};
use crate::rendering::shared::vertex::{Vertex, VertexDefinition};
use crate::rendering::vulkan::VulkanRenderer;
use crate::voxels::meshes::VoxelVertex;

impl VulkanRenderer {
    pub(crate) fn load_shader_module(&self, path: &str) -> Result<ash::vk::ShaderModule> {
        self.pipeline_manager
            .create_shader_module(&self.context, path)
    }

    pub(crate) fn get_pipeline(&self, key: &str) -> Pipeline {
        *self
            .pipeline_manager
            .pipeline_cache
            .get(key)
            .unwrap_or_else(|| panic!("Pipeline '{}' not found in cache", key))
    }

    pub(crate) fn rebuild_pipelines(&mut self, aa_amount: AntiAliasingAmount) -> Result<()> {
        // Ensure GPU is idle before destroying/recreating pipelines.
        unsafe { self.context.device.device_wait_idle()? };
        let vertex_shader = self.load_shader_module(&self.default_vertex_shader)?;
        let fragment_shader = self.load_shader_module(&self.default_fragment_shader)?;
        let collider_debug_fragment_shader = self.load_shader_module("sdr_collider_debug.frag")?;
        let voxel_vertex_shader = self.load_shader_module(&self.voxel_vertex_shader)?;
        let voxel_fragment_shader = self.load_shader_module(&self.voxel_fragment_shader)?;
        let water_vertex_shader = self.load_shader_module(&self.water_vertex_shader)?;
        let water_fragment_shader = self.load_shader_module(&self.water_fragment_shader)?;
        let skybox_vertex_shader = self.load_shader_module("sdr_default_skybox.vert")?;
        let skybox_fragment_shader = self.load_shader_module("sdr_default_skybox.frag")?;
        let shadow_model_vert = self.load_shader_module(&self.shadow_model_vertex_shader)?;
        let shadow_voxel_vert = self.load_shader_module(&self.shadow_voxel_vertex_shader)?;
        let shadow_frag = self.load_shader_module(&self.shadow_fragment_shader)?;
        let shadow_point_model_vert =
            self.load_shader_module(&self.shadow_point_model_vertex_shader)?;
        let shadow_point_voxel_vert =
            self.load_shader_module(&self.shadow_point_voxel_vertex_shader)?;
        let shadow_point_frag = self.load_shader_module(&self.shadow_point_fragment_shader)?;

        let swapchain = self.swapchain.clone();
        let context = self.context.clone();
        let pipeline_layout = self.pipeline_layout;
        let voxel_pipeline_layout = self.voxel_pipeline_layout;

        unsafe {
            let pipeline_options = PipelineOptions {
                image_format: Some(swapchain.format),
                image_extent: swapchain.extent,
                depth_format: Some(swapchain.depth_format),
                vertex_shader,
                fragment_shader,
                vertex_bindings: vec![Vertex::get_binding_description()],
                vertex_attributes: Vertex::get_attribute_descriptions(),
            };

            let pipeline = context.create_graphics_pipeline(
                pipeline_options.clone(),
                RenderingSettings::default(),
                pipeline_layout,
                aa_amount,
            )?;
            let wireframe_pipeline = context.create_graphics_pipeline(
                pipeline_options.clone(),
                RenderingSettings::wireframe(),
                pipeline_layout,
                aa_amount,
            )?;
            let collider_debug_pipeline = context.create_graphics_pipeline(
                PipelineOptions {
                    fragment_shader: collider_debug_fragment_shader,
                    ..pipeline_options
                },
                RenderingSettings::collider_debug(),
                pipeline_layout,
                aa_amount,
            )?;

            let pipeline_options = PipelineOptions {
                image_format: Some(swapchain.format),
                image_extent: swapchain.extent,
                depth_format: Some(swapchain.depth_format),
                vertex_shader: voxel_vertex_shader,
                fragment_shader: voxel_fragment_shader,
                vertex_bindings: vec![VoxelVertex::get_binding_description()],
                vertex_attributes: VoxelVertex::get_attribute_descriptions(),
            };

            let voxel_pipeline = context.create_graphics_pipeline(
                pipeline_options.clone(),
                RenderingSettings::default(),
                voxel_pipeline_layout,
                aa_amount,
            )?;

            let voxel_wireframe_pipeline = context.create_graphics_pipeline(
                pipeline_options,
                RenderingSettings::wireframe(),
                voxel_pipeline_layout,
                aa_amount,
            )?;

            let pipeline_options = PipelineOptions {
                image_format: Some(swapchain.format),
                image_extent: swapchain.extent,
                depth_format: Some(swapchain.depth_format),
                vertex_shader: water_vertex_shader,
                fragment_shader: water_fragment_shader,
                vertex_bindings: vec![VoxelVertex::get_binding_description()],
                vertex_attributes: VoxelVertex::get_attribute_descriptions(),
            };

            let water_pipeline = context.create_graphics_pipeline(
                pipeline_options,
                RenderingSettings::default(),
                voxel_pipeline_layout,
                aa_amount,
            )?;

            let skybox_pipeline_options = PipelineOptions {
                image_format: Some(swapchain.format),
                image_extent: swapchain.extent,
                depth_format: Some(swapchain.depth_format),
                vertex_shader: skybox_vertex_shader,
                fragment_shader: skybox_fragment_shader,
                vertex_bindings: vec![Vertex::get_binding_description()],
                vertex_attributes: Vertex::get_attribute_descriptions(),
            };
            let skybox_pipeline = context.create_graphics_pipeline(
                skybox_pipeline_options.clone(),
                RenderingSettings::skybox(),
                pipeline_layout,
                aa_amount,
            )?;
            let skybox_additive_pipeline = context.create_graphics_pipeline(
                skybox_pipeline_options,
                RenderingSettings::skybox_additive(),
                pipeline_layout,
                aa_amount,
            )?;

            self.context
                .device
                .destroy_shader_module(vertex_shader, None);
            self.context
                .device
                .destroy_shader_module(voxel_vertex_shader, None);
            self.context
                .device
                .destroy_shader_module(fragment_shader, None);
            self.context
                .device
                .destroy_shader_module(collider_debug_fragment_shader, None);
            self.context
                .device
                .destroy_shader_module(voxel_fragment_shader, None);
            self.context
                .device
                .destroy_shader_module(water_vertex_shader, None);
            self.context
                .device
                .destroy_shader_module(water_fragment_shader, None);
            self.context
                .device
                .destroy_shader_module(skybox_vertex_shader, None);
            self.context
                .device
                .destroy_shader_module(skybox_fragment_shader, None);

            let shadow_extent = vk::Extent2D {
                width: 2048,
                height: 2048,
            };
            let shadow_rasterization = RasterizationSettings {
                cull_mode: vk::CullModeFlags::FRONT,
                depth_bias_enable: true,
                ..Default::default()
            };
            let shadow_dynamic_states = DynamicStateSettings {
                dynamic_states: vec![
                    vk::DynamicState::VIEWPORT,
                    vk::DynamicState::SCISSOR,
                    vk::DynamicState::DEPTH_BIAS,
                ],
            };
            let shadow_model_pipeline = context.create_graphics_pipeline(
                PipelineOptions {
                    image_format: None,
                    image_extent: shadow_extent,
                    depth_format: Some(vk::Format::D32_SFLOAT),
                    vertex_shader: shadow_model_vert,
                    fragment_shader: shadow_frag,
                    vertex_bindings: vec![Vertex::get_binding_description()],
                    vertex_attributes: Vertex::get_attribute_descriptions(),
                },
                RenderingSettings {
                    rasterization_settings: shadow_rasterization,
                    dynamic_state_settings: shadow_dynamic_states.clone(),
                    ..Default::default()
                },
                self.shadow_model_pipeline_layout,
                AntiAliasingAmount::X0,
            )?;

            let shadow_voxel_pipeline = context.create_graphics_pipeline(
                PipelineOptions {
                    image_format: None,
                    image_extent: shadow_extent,
                    depth_format: Some(vk::Format::D32_SFLOAT),
                    vertex_shader: shadow_voxel_vert,
                    fragment_shader: shadow_frag,
                    vertex_bindings: vec![VoxelVertex::get_binding_description()],
                    vertex_attributes: VoxelVertex::get_attribute_descriptions(),
                },
                RenderingSettings {
                    rasterization_settings: shadow_rasterization,
                    dynamic_state_settings: shadow_dynamic_states.clone(),
                    ..Default::default()
                },
                self.shadow_voxel_pipeline_layout,
                AntiAliasingAmount::X0,
            )?;

            context
                .device
                .destroy_shader_module(shadow_model_vert, None);
            context
                .device
                .destroy_shader_module(shadow_voxel_vert, None);
            context.device.destroy_shader_module(shadow_frag, None);

            let shadow_point_model_pipeline = context.create_graphics_pipeline(
                PipelineOptions {
                    image_format: None,
                    image_extent: shadow_extent,
                    depth_format: Some(vk::Format::D32_SFLOAT),
                    vertex_shader: shadow_point_model_vert,
                    fragment_shader: shadow_point_frag,
                    vertex_bindings: vec![Vertex::get_binding_description()],
                    vertex_attributes: Vertex::get_attribute_descriptions(),
                },
                RenderingSettings {
                    rasterization_settings: shadow_rasterization,
                    dynamic_state_settings: shadow_dynamic_states.clone(),
                    ..Default::default()
                },
                self.shadow_point_model_pipeline_layout,
                AntiAliasingAmount::X0,
            )?;

            let shadow_point_voxel_pipeline = context.create_graphics_pipeline(
                PipelineOptions {
                    image_format: None,
                    image_extent: shadow_extent,
                    depth_format: Some(vk::Format::D32_SFLOAT),
                    vertex_shader: shadow_point_voxel_vert,
                    fragment_shader: shadow_point_frag,
                    vertex_bindings: vec![VoxelVertex::get_binding_description()],
                    vertex_attributes: VoxelVertex::get_attribute_descriptions(),
                },
                RenderingSettings {
                    rasterization_settings: shadow_rasterization,
                    dynamic_state_settings: shadow_dynamic_states,
                    ..Default::default()
                },
                self.shadow_point_voxel_pipeline_layout,
                AntiAliasingAmount::X0,
            )?;

            context
                .device
                .destroy_shader_module(shadow_point_model_vert, None);
            context
                .device
                .destroy_shader_module(shadow_point_voxel_vert, None);
            context
                .device
                .destroy_shader_module(shadow_point_frag, None);

            // Destroy every cached pipeline (built-ins and any custom shader variants).
            for (_, old_pipeline) in self.pipeline_manager.pipeline_cache.drain() {
                self.context.device.destroy_pipeline(old_pipeline, None);
            }

            self.pipeline_manager
                .pipeline_cache
                .insert("model".to_string(), pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("model::wireframe".to_string(), wireframe_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("model::collider_debug".to_string(), collider_debug_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("voxel".to_string(), voxel_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("voxel::wireframe".to_string(), voxel_wireframe_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("water".to_string(), water_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("skybox".to_string(), skybox_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("skybox::additive".to_string(), skybox_additive_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("shadow_model".to_string(), shadow_model_pipeline);
            self.pipeline_manager
                .pipeline_cache
                .insert("shadow_voxel".to_string(), shadow_voxel_pipeline);
            self.pipeline_manager.pipeline_cache.insert(
                "shadow_point_model".to_string(),
                shadow_point_model_pipeline,
            );
            self.pipeline_manager.pipeline_cache.insert(
                "shadow_point_voxel".to_string(),
                shadow_point_voxel_pipeline,
            );

            self.pipeline_manager.model_pipeline_template = Some(
                crate::rendering::vulkan::pipeline_manager::ModelPipelineTemplate {
                    layout: pipeline_layout,
                    image_format: Some(swapchain.format),
                    depth_format: Some(swapchain.depth_format),
                    image_extent: swapchain.extent,
                    aa_amount,
                },
            );
        }

        Ok(())
    }

    pub fn reload_shader(
        &mut self,
        shader_name: &str,
        aa_amount: AntiAliasingAmount,
    ) -> Result<bool> {
        if self.pipeline_manager.reload_shader(shader_name)? {
            self.rebuild_pipelines(aa_amount)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn reload_shaders(&mut self) -> Result<bool> {
        let reloaded = self
            .pipeline_manager
            .shader_registry
            .reload_changed_shaders()?;
        if reloaded.is_empty() {
            return Ok(false);
        }

        log!("Reloaded shaders: {}", reloaded.join(", "));
        unsafe { self.context.device.device_wait_idle()? };
        self.pipeline_manager.pipeline_cache.clear();
        self.rebuild_pipelines(self.aa_amount)?;
        Ok(true)
    }
}
