use std::sync::{Arc, Mutex};

use crate::log;
use crate::rendering::lighting::gpu_light::{
    CSM_CASCADE_COUNT, GpuLight, MAX_LIGHTS, PointShadowData, ShadowData,
};
use crate::rendering::shared::anti_alisaing::AntiAliasingAmount;
use crate::rendering::shared::material::GpuMaterial;
use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ModelPushConstants, PushConstants, ShadowModelPushConstants, ShadowPointModelPushConstants,
    ShadowPointVoxelPushConstants, ShadowVoxelPushConstants, VoxelPushConstants,
};
use crate::rendering::shared::rendering_settings::{
    DynamicStateSettings, PipelineOptions, RasterizationSettings, RenderingSettings,
};
use crate::rendering::shared::vertex::{Vertex, VertexDefinition};
use crate::rendering::vulkan::image_layout::ImageLayouts;
use crate::rendering::vulkan::pipeline_manager::PipelineManager;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::rendering::vulkan::{frame::VulkanFrame, swapchain::VulkanSwapchain};
use crate::rendering::{RenderingAPI, RenderingInfo};
use crate::ui::UIRenderer;
use crate::voxels::meshes::VoxelVertex;
use crate::voxels::texture_atlas::VoxelTextureAtlas;
use anyhow::Result;
use ash::vk::{
    self, ClearColorValue, CommandBufferResetFlags, CommandPool, Extent2D, ImageView, Pipeline,
    PipelineLayout, PipelineLayoutCreateInfo, SampleCountFlags,
};
use egui::{ClippedPrimitive, Context, TextureId};
use epaint::ImageDelta;
use winit::event::WindowEvent;
use winit::window::Window;

pub mod device;
pub mod frame;
pub mod image_layout;
pub mod pipeline_manager;
pub mod queue_family;
pub mod rendering_context;
pub mod surface;
pub mod swapchain;

/// A container for a descriptor and it's data
pub struct Descriptor {
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub descriptor_set: vk::DescriptorSet,
}

/// A container for a UBO
#[derive(Clone)]
pub struct Ubo {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
}

pub struct VulkanRenderer {
    pub current_image_index: u32,
    pub in_flight_frames_count: usize,
    pub swapchain: VulkanSwapchain,
    pub frames: Vec<VulkanFrame>,
    pub current_frame: usize,
    pub image_layouts: ImageLayouts,

    pub pipeline_layout: PipelineLayout,
    pub voxel_pipeline_layout: PipelineLayout,
    pub water_pipeline_layout: PipelineLayout,
    pub voxel_descriptor_pool: vk::DescriptorPool,
    pub voxel_descriptor_set_layout: vk::DescriptorSetLayout,

    pub default_white_material: GpuMaterial,

    pub ui_renderer: UIRenderer,
    pub buffer_graveyard: Vec<(vk::Buffer, vk::DeviceMemory)>,

    pub viewport_image: vk::Image,
    pub viewport_image_memory: vk::DeviceMemory,
    pub viewport_image_view: vk::ImageView,
    // MSAA color buffer
    pub msaa_color_image: vk::Image,
    pub msaa_color_memory: vk::DeviceMemory,
    pub msaa_color_view: vk::ImageView,
    // MSAA depth buffer
    pub viewport_depth_image: vk::Image,
    pub viewport_depth_memory: vk::DeviceMemory,
    pub viewport_depth_view: vk::ImageView,

    pub viewport_sampler: vk::Sampler,
    pub viewport_descriptor_set: vk::DescriptorSet,
    pub viewport_texture_id: TextureId,
    pub viewport_extent: vk::Extent2D,
    pub viewport_target_initialized: bool,
    pub viewport_depth_initialized: bool,
    pub last_fence_wait_ns: u64,

    pub light_ssbo: vk::Buffer,
    pub light_ssbo_memory: vk::DeviceMemory,
    pub light_descriptor_pool: vk::DescriptorPool,
    pub light_descriptor_set_layout: vk::DescriptorSetLayout,
    pub light_descriptor_set: vk::DescriptorSet,

    pub shadow_image: vk::Image,
    pub shadow_image_memory: vk::DeviceMemory,
    pub shadow_image_view: vk::ImageView,
    pub shadow_cascade_views: [vk::ImageView; CSM_CASCADE_COUNT],
    pub shadow_sampler: vk::Sampler,
    pub shadow_map_size: u32,
    pub shadow_model_pipeline_layout: PipelineLayout,
    pub shadow_voxel_pipeline_layout: PipelineLayout,
    pub shadow_model_vertex_shader: String,
    pub shadow_voxel_vertex_shader: String,
    pub shadow_fragment_shader: String,

    pub point_shadow_image: vk::Image,
    pub point_shadow_image_memory: vk::DeviceMemory,
    pub point_shadow_cube_view: vk::ImageView,
    pub point_shadow_face_views: [vk::ImageView; 6],
    pub point_shadow_sampler: vk::Sampler,
    pub point_shadow_map_size: u32,
    pub shadow_point_model_pipeline_layout: PipelineLayout,
    pub shadow_point_voxel_pipeline_layout: PipelineLayout,
    pub shadow_point_model_vertex_shader: String,
    pub shadow_point_voxel_vertex_shader: String,
    pub shadow_point_fragment_shader: String,

    pub ubo: Ubo,
    pub pipeline_manager: PipelineManager,
    pub default_vertex_shader: String,
    pub default_fragment_shader: String,
    pub voxel_vertex_shader: String,
    pub voxel_fragment_shader: String,
    pub water_vertex_shader: String,
    pub water_fragment_shader: String,
    context: Arc<VulkanRenderingContext>,

    pub aa_amount: AntiAliasingAmount,
    pub ui_cached_primitives: Vec<ClippedPrimitive>,
    /// egui textures whose `free` was reported last frame. Freed at the start of the
    /// next `end_ui`, after `begin_frame` has waited on the previous frame's fence, so
    /// we never destroy a descriptor set/image still referenced by in-flight GPU work.
    pub ui_pending_texture_frees: Vec<TextureId>,
}

impl VulkanRenderer {
    fn load_shader_module(&self, path: &str) -> Result<ash::vk::ShaderModule> {
        self.pipeline_manager
            .create_shader_module(&self.context, path)
    }

    fn get_pipeline(&self, key: &str) -> Pipeline {
        *self
            .pipeline_manager
            .pipeline_cache
            .get(key)
            .unwrap_or_else(|| panic!("Pipeline '{}' not found in cache", key))
    }

    fn rebuild_pipelines(&mut self, aa_amount: AntiAliasingAmount) -> Result<()> {
        // Ensure GPU is idle before destroying/recreating pipelines.
        unsafe { self.context.device.device_wait_idle()? };
        let vertex_shader = self.load_shader_module(&self.default_vertex_shader)?;
        let fragment_shader = self.load_shader_module(&self.default_fragment_shader)?;
        let collider_debug_fragment_shader =
            self.load_shader_module("sdr_collider_debug.frag")?;
        let voxel_vertex_shader = self.load_shader_module(&self.voxel_vertex_shader)?;
        let voxel_fragment_shader = self.load_shader_module(&self.voxel_fragment_shader)?;
        let water_vertex_shader = self.load_shader_module(&self.water_vertex_shader)?;
        let water_fragment_shader = self.load_shader_module(&self.water_fragment_shader)?;
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

    pub fn resize_viewport(
        &mut self,
        new_extent: vk::Extent2D,
        aa_amount: AntiAliasingAmount,
    ) -> Result<()> {
        if self.viewport_extent == new_extent && self.aa_amount == aa_amount {
            return Ok(());
        }

        let aa_changed = self.aa_amount != aa_amount;
        self.aa_amount = aa_amount;

        let aa_samples = match aa_amount {
            AntiAliasingAmount::X0 => SampleCountFlags::TYPE_1,
            AntiAliasingAmount::X2 => SampleCountFlags::TYPE_2,
            AntiAliasingAmount::X4 => SampleCountFlags::TYPE_4,
            AntiAliasingAmount::X8 => SampleCountFlags::TYPE_8,
        };

        unsafe { self.context.device.device_wait_idle()? };

        // Rebuild pipelines only if MSAA sample count changed
        if aa_changed {
            self.rebuild_pipelines(aa_amount)?;
        }

        unsafe {
            // Destroy old MSAA color buffer
            self.context
                .device
                .destroy_image_view(self.msaa_color_view, None);
            self.context
                .device
                .destroy_image(self.msaa_color_image, None);
            self.context
                .device
                .free_memory(self.msaa_color_memory, None);

            // Destroy old resolve target
            self.context
                .device
                .destroy_image_view(self.viewport_image_view, None);
            self.context.device.destroy_image(self.viewport_image, None);
            self.context
                .device
                .free_memory(self.viewport_image_memory, None);

            // Destroy old depth buffer
            self.context
                .device
                .destroy_image_view(self.viewport_depth_view, None);
            self.context
                .device
                .destroy_image(self.viewport_depth_image, None);
            self.context
                .device
                .free_memory(self.viewport_depth_memory, None);

            self.context
                .device
                .destroy_sampler(self.viewport_sampler, None);
        }

        let (viewport_image, viewport_image_memory) = self.context.create_image(
            new_extent,
            self.swapchain.format,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            SampleCountFlags::TYPE_1,
        )?;
        let viewport_image_view = self.context.create_image_view(
            viewport_image,
            self.swapchain.format,
            vk::ImageAspectFlags::COLOR,
        )?;

        let (msaa_color_image, msaa_color_memory) = self.context.create_image(
            new_extent,
            self.swapchain.format,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::TRANSIENT_ATTACHMENT | vk::ImageUsageFlags::COLOR_ATTACHMENT,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            aa_samples,
        )?;
        let msaa_color_view = self.context.create_image_view(
            msaa_color_image,
            self.swapchain.format,
            vk::ImageAspectFlags::COLOR,
        )?;

        // MSAA depth buffer
        let (viewport_depth_image, viewport_depth_memory) = self.context.create_image(
            new_extent,
            self.swapchain.depth_format,
            vk::ImageTiling::OPTIMAL,
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            aa_samples,
        )?;
        let viewport_depth_view = self.context.create_image_view(
            viewport_depth_image,
            self.swapchain.depth_format,
            vk::ImageAspectFlags::DEPTH,
        )?;

        if aa_amount == AntiAliasingAmount::X0 && aa_samples != SampleCountFlags::TYPE_1 {
            log!("Warning: AA requested X0 but created image samples != TYPE_1");
        }

        let viewport_sampler = unsafe {
            self.context.device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .anisotropy_enable(false)
                    .max_anisotropy(1.0),
                None,
            )?
        };

        // Write the new resolve target into the existing descriptor set
        unsafe {
            let image_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(viewport_image_view)
                .sampler(viewport_sampler);

            self.context.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(self.viewport_descriptor_set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[image_info])],
                &[],
            );
        }

        self.viewport_image = viewport_image;
        self.viewport_image_memory = viewport_image_memory;
        self.viewport_image_view = viewport_image_view;
        self.msaa_color_image = msaa_color_image;
        self.msaa_color_memory = msaa_color_memory;
        self.msaa_color_view = msaa_color_view;
        self.viewport_depth_image = viewport_depth_image;
        self.viewport_depth_memory = viewport_depth_memory;
        self.viewport_depth_view = viewport_depth_view;
        self.viewport_sampler = viewport_sampler;
        self.viewport_extent = new_extent;

        self.viewport_target_initialized = false;
        self.viewport_depth_initialized = false;

        Ok(())
    }
}

impl RenderingAPI for VulkanRenderer {
    fn new(
        rendering_info: Arc<Mutex<RenderingInfo>>,
        window: Arc<Window>,
        aa_amount: AntiAliasingAmount,
    ) -> Result<()> {
        let mut rendering_info = rendering_info.lock().unwrap();
        let mut swapchain = VulkanSwapchain::new(
            rendering_info.context.clone().into(),
            rendering_info.window.clone(),
        )?;
        swapchain.resize()?;

        let aa_samples = match aa_amount {
            AntiAliasingAmount::X0 => SampleCountFlags::TYPE_1,
            AntiAliasingAmount::X2 => SampleCountFlags::TYPE_2,
            AntiAliasingAmount::X4 => SampleCountFlags::TYPE_4,
            AntiAliasingAmount::X8 => SampleCountFlags::TYPE_8,
        };

        let mut pipeline_manager = PipelineManager::new();

        let default_vertex_shader = rendering_info.settings.default_vertex_shader.clone();
        let default_fragment_shader = rendering_info.settings.default_fragment_shader.clone();
        let voxel_vertex_shader = "sdr_default_voxel.vert".to_string();
        let voxel_fragment_shader = "sdr_default_voxel.frag".to_string();
        let water_vertex_shader = "sdr_default_water.vert".to_string();
        let water_fragment_shader = "sdr_default_water.frag".to_string();

        let vertex_shader = pipeline_manager.create_shader_module(
            &rendering_info.context.clone().into(),
            &default_vertex_shader,
        )?;
        let fragment_shader = pipeline_manager.create_shader_module(
            &rendering_info.context.clone().into(),
            &default_fragment_shader,
        )?;
        let collider_debug_fragment_shader = pipeline_manager.create_shader_module(
            &rendering_info.context.clone().into(),
            "sdr_collider_debug.frag",
        )?;
        let voxel_vertex_shader = pipeline_manager
            .create_shader_module(&rendering_info.context.clone().into(), &voxel_vertex_shader)?;
        let voxel_fragment_shader = pipeline_manager.create_shader_module(
            &rendering_info.context.clone().into(),
            &voxel_fragment_shader,
        )?;
        let water_vertex_shader = pipeline_manager
            .create_shader_module(&rendering_info.context.clone().into(), &water_vertex_shader)?;
        let water_fragment_shader = pipeline_manager.create_shader_module(
            &rendering_info.context.clone().into(),
            &water_fragment_shader,
        )?;

        unsafe {
            let context = rendering_info.context.clone();

            // Header: 336 bytes (see SSBO layout in set_lights)
            let light_ssbo_size = (336 + size_of::<GpuLight>() * MAX_LIGHTS) as vk::DeviceSize;

            let (light_ssbo, light_ssbo_memory) = context.create_buffer(
                light_ssbo_size,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;

            let light_ssbo_binding = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);

            let shadow_sampler_binding = vk::DescriptorSetLayoutBinding::default()
                .binding(1)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);

            let point_shadow_sampler_binding = vk::DescriptorSetLayoutBinding::default()
                .binding(2)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);

            let light_descriptor_set_layout = context.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&[
                    light_ssbo_binding,
                    shadow_sampler_binding,
                    point_shadow_sampler_binding,
                ]),
                None,
            )?;

            let light_descriptor_pool = context.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(1)
                    .pool_sizes(&[
                        vk::DescriptorPoolSize {
                            ty: vk::DescriptorType::STORAGE_BUFFER,
                            descriptor_count: 1,
                        },
                        vk::DescriptorPoolSize {
                            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            descriptor_count: 2,
                        },
                    ]),
                None,
            )?;

            let light_descriptor_set = context
                .device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(light_descriptor_pool)
                        .set_layouts(&[light_descriptor_set_layout]),
                )?
                .remove(0);

            let light_buffer_info = vk::DescriptorBufferInfo::default()
                .buffer(light_ssbo)
                .offset(0)
                .range(light_ssbo_size);

            // Shared layout for any combined-image-sampler binding (material textures, voxel atlas,
            // viewport preview). Created first so it can be referenced by pipeline_layout.
            let sampler_binding = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);

            let descriptor_set_layout = context.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&[sampler_binding]),
                None,
            )?;

            // Pool for all sampler descriptor sets: voxel atlas, material textures, viewport.
            // Up to 200 sets / 500 individual descriptors.
            let descriptor_pool = context.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(500)
                    .pool_sizes(&[vk::DescriptorPoolSize {
                        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        descriptor_count: 500,
                    }]),
                None,
            )?;

            let pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(&[vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                        .offset(0)
                        .size(228)])
                    .set_layouts(&[light_descriptor_set_layout, descriptor_set_layout]),
                None,
            )?;

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

            let voxel_pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(&[vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                        .offset(0)
                        .size(160)])
                    .set_layouts(&[descriptor_set_layout, light_descriptor_set_layout]),
                None,
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

            let water_pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(&[vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                        .offset(0)
                        .size(160)])
                    .set_layouts(&[descriptor_set_layout, light_descriptor_set_layout]),
                None,
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

            context.device.destroy_shader_module(vertex_shader, None);
            context
                .device
                .destroy_shader_module(voxel_vertex_shader, None);
            context.device.destroy_shader_module(fragment_shader, None);
            context
                .device
                .destroy_shader_module(collider_debug_fragment_shader, None);
            context
                .device
                .destroy_shader_module(voxel_fragment_shader, None);
            context
                .device
                .destroy_shader_module(water_vertex_shader, None);
            context
                .device
                .destroy_shader_module(water_fragment_shader, None);

            let command_pool = context.device.create_command_pool(
                &ash::vk::CommandPoolCreateInfo::default()
                    .queue_family_index(context.queue_families.graphics)
                    .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?;

            let in_flight_frames_count = 3;

            let command_buffers = context.device.allocate_command_buffers(
                &ash::vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(ash::vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(in_flight_frames_count as u32),
            )?;

            let mut frames = Vec::with_capacity(command_buffers.len());
            for (_index, &command_buffer) in command_buffers.iter().enumerate() {
                let image_available_semaphore = context
                    .device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
                let render_finished_semaphore = context
                    .device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;
                let in_flight_fence = context.device.create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )?;
                frames.push(VulkanFrame {
                    command_buffer,
                    image_available_semaphore,
                    render_finished_semaphore,
                    in_flight_fence,
                });
            }

            let (default_ubo, default_ubo_mem) = context.create_buffer(
                256,
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;
            let ubo = Ubo {
                buffer: default_ubo,
                memory: default_ubo_mem,
            };

            let ui_renderer = UIRenderer::new(context.clone(), &swapchain, window)?;

            let voxel_vertex_shader = "sdr_default_voxel.vert".to_string();
            let voxel_fragment_shader = "sdr_default_voxel.frag".to_string();
            let water_vertex_shader = "sdr_default_water.vert".to_string();
            let water_fragment_shader = "sdr_default_water.frag".to_string();

            let viewport_extent = swapchain.extent;

            let (viewport_image, viewport_image_memory) = context.create_image(
                viewport_extent,
                swapchain.format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                SampleCountFlags::TYPE_1,
            )?;
            let viewport_image_view = context.create_image_view(
                viewport_image,
                swapchain.format,
                vk::ImageAspectFlags::COLOR,
            )?;

            let (msaa_color_image, msaa_color_memory) = context.create_image(
                viewport_extent,
                swapchain.format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::TRANSIENT_ATTACHMENT | vk::ImageUsageFlags::COLOR_ATTACHMENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                aa_samples,
            )?;
            let msaa_color_view = context.create_image_view(
                msaa_color_image,
                swapchain.format,
                vk::ImageAspectFlags::COLOR,
            )?;

            let (viewport_depth_image, viewport_depth_memory) = context.create_image(
                viewport_extent,
                swapchain.depth_format,
                vk::ImageTiling::OPTIMAL,
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
                aa_samples,
            )?;
            let viewport_depth_view = context.create_image_view(
                viewport_depth_image,
                swapchain.depth_format,
                vk::ImageAspectFlags::DEPTH,
            )?;

            let viewport_sampler = context.device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .anisotropy_enable(false)
                    .max_anisotropy(1.0),
                None,
            )?;

            // Descriptor set points at the resolve target (TYPE_1)
            let viewport_descriptor_set = context.create_texture_descriptor_set(
                descriptor_pool,
                descriptor_set_layout,
                viewport_image_view,
                viewport_sampler,
            );

            let viewport_texture_id = ui_renderer
                .renderer
                .lock()
                .unwrap()
                .add_user_texture(viewport_descriptor_set);
            context.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(light_descriptor_set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(&[light_buffer_info])],
                &[],
            );

            // Shadow map resources: 2048×2048 texture array with CSM_CASCADE_COUNT layers.
            const SHADOW_MAP_SIZE: u32 = 2048;
            let shadow_extent = vk::Extent2D {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
            };
            let _image_layouts = ImageLayouts::default();

            // Create the shadow image as a 2D array (one layer per cascade).
            let shadow_image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .extent(vk::Extent3D {
                    width: SHADOW_MAP_SIZE,
                    height: SHADOW_MAP_SIZE,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(CSM_CASCADE_COUNT as u32)
                .format(vk::Format::D32_SFLOAT)
                .tiling(vk::ImageTiling::OPTIMAL)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                .samples(SampleCountFlags::TYPE_1)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let shadow_image = context.device.create_image(&shadow_image_info, None)?;
            let mem_reqs = context.device.get_image_memory_requirements(shadow_image);
            let shadow_image_memory = context.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_reqs.size)
                    .memory_type_index(context.find_memory_type(
                        mem_reqs.memory_type_bits,
                        vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    )?),
                None,
            )?;
            context
                .device
                .bind_image_memory(shadow_image, shadow_image_memory, 0)?;

            // Full array view for shader sampling (TYPE_2D_ARRAY, all layers).
            let shadow_image_view = context.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(shadow_image)
                    .view_type(vk::ImageViewType::TYPE_2D_ARRAY)
                    .format(vk::Format::D32_SFLOAT)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(CSM_CASCADE_COUNT as u32),
                    ),
                None,
            )?;

            // Per-layer views for rendering (TYPE_2D, one layer each).
            let mut shadow_cascade_views = [vk::ImageView::null(); CSM_CASCADE_COUNT];
            for i in 0..CSM_CASCADE_COUNT {
                shadow_cascade_views[i] = context.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(shadow_image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(i as u32)
                                .layer_count(1),
                        ),
                    None,
                )?;
            }

            // Transition all 4 layers to DEPTH_STENCIL_READ_ONLY_OPTIMAL before first frame.
            let init_cmd = context.begin_single_time_commands(context.command_pool);
            context.device.cmd_pipeline_barrier(
                init_cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                    .image(shadow_image)
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(CSM_CASCADE_COUNT as u32),
                    )],
            );
            context.end_single_time_commands(
                init_cmd,
                context.queues[&context.queue_families.graphics],
                context.command_pool,
            );

            let shadow_sampler = context.device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_BORDER)
                    .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
                    .compare_enable(true)
                    .compare_op(vk::CompareOp::LESS_OR_EQUAL),
                None,
            )?;

            // Write the full array view into the light descriptor set at binding 1.
            let shadow_desc_image_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                .image_view(shadow_image_view)
                .sampler(shadow_sampler);
            context.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(light_descriptor_set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[shadow_desc_image_info])],
                &[],
            );

            // Shadow pipeline layouts and pipelines.
            let shadow_model_vert_module = pipeline_manager.create_shader_module(
                &rendering_info.context.clone().into(),
                "sdr_default_shadow_model.vert",
            )?;
            let shadow_voxel_vert_module = pipeline_manager.create_shader_module(
                &rendering_info.context.clone().into(),
                "sdr_default_shadow_voxel.vert",
            )?;
            let shadow_frag_module = pipeline_manager.create_shader_module(
                &rendering_info.context.clone().into(),
                "sdr_default_shadow.frag",
            )?;

            let shadow_model_pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default().push_constant_ranges(&[
                    vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX)
                        .offset(0)
                        .size(112),
                ]),
                None,
            )?;

            let shadow_voxel_pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default().push_constant_ranges(&[
                    vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX)
                        .offset(0)
                        .size(80),
                ]),
                None,
            )?;

            let shadow_rasterization = RasterizationSettings {
                cull_mode: vk::CullModeFlags::FRONT,
                // Enable so cmd_set_depth_bias can override values dynamically each pass.
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
                    vertex_shader: shadow_model_vert_module,
                    fragment_shader: shadow_frag_module,
                    vertex_bindings: vec![Vertex::get_binding_description()],
                    vertex_attributes: Vertex::get_attribute_descriptions(),
                },
                RenderingSettings {
                    rasterization_settings: shadow_rasterization,
                    dynamic_state_settings: shadow_dynamic_states.clone(),
                    ..Default::default()
                },
                shadow_model_pipeline_layout,
                AntiAliasingAmount::X0,
            )?;

            let shadow_voxel_pipeline = context.create_graphics_pipeline(
                PipelineOptions {
                    image_format: None,
                    image_extent: shadow_extent,
                    depth_format: Some(vk::Format::D32_SFLOAT),
                    vertex_shader: shadow_voxel_vert_module,
                    fragment_shader: shadow_frag_module,
                    vertex_bindings: vec![VoxelVertex::get_binding_description()],
                    vertex_attributes: VoxelVertex::get_attribute_descriptions(),
                },
                RenderingSettings {
                    rasterization_settings: shadow_rasterization,
                    dynamic_state_settings: shadow_dynamic_states.clone(),
                    ..Default::default()
                },
                shadow_voxel_pipeline_layout,
                AntiAliasingAmount::X0,
            )?;

            context
                .device
                .destroy_shader_module(shadow_model_vert_module, None);
            context
                .device
                .destroy_shader_module(shadow_voxel_vert_module, None);
            context
                .device
                .destroy_shader_module(shadow_frag_module, None);

            // --- Point light cubemap shadow resources ---
            let (
                point_shadow_image,
                point_shadow_image_memory,
                point_shadow_cube_view,
                point_shadow_face_views,
            ) = {
                let image_info = vk::ImageCreateInfo::default()
                    .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE)
                    .image_type(vk::ImageType::TYPE_2D)
                    .extent(vk::Extent3D {
                        width: SHADOW_MAP_SIZE,
                        height: SHADOW_MAP_SIZE,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(6)
                    .format(vk::Format::D32_SFLOAT)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .usage(
                        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
                            | vk::ImageUsageFlags::SAMPLED,
                    )
                    .samples(SampleCountFlags::TYPE_1)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                let point_shadow_image = context.device.create_image(&image_info, None)?;
                let mem_reqs = context
                    .device
                    .get_image_memory_requirements(point_shadow_image);
                let point_shadow_image_memory = context.device.allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(mem_reqs.size)
                        .memory_type_index(context.find_memory_type(
                            mem_reqs.memory_type_bits,
                            vk::MemoryPropertyFlags::DEVICE_LOCAL,
                        )?),
                    None,
                )?;
                context.device.bind_image_memory(
                    point_shadow_image,
                    point_shadow_image_memory,
                    0,
                )?;

                let all_faces = vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(6);

                let point_shadow_cube_view = context.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(point_shadow_image)
                        .view_type(vk::ImageViewType::CUBE)
                        .format(vk::Format::D32_SFLOAT)
                        .subresource_range(all_faces),
                    None,
                )?;

                let mut point_shadow_face_views = [vk::ImageView::null(); 6];
                for i in 0..6 {
                    point_shadow_face_views[i] = context.device.create_image_view(
                        &vk::ImageViewCreateInfo::default()
                            .image(point_shadow_image)
                            .view_type(vk::ImageViewType::TYPE_2D)
                            .format(vk::Format::D32_SFLOAT)
                            .subresource_range(
                                vk::ImageSubresourceRange::default()
                                    .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                    .base_mip_level(0)
                                    .level_count(1)
                                    .base_array_layer(i as u32)
                                    .layer_count(1),
                            ),
                        None,
                    )?;
                }

                let init_cmd = context.begin_single_time_commands(context.command_pool);
                context.device.cmd_pipeline_barrier(
                    init_cmd,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                        .image(point_shadow_image)
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::SHADER_READ)
                        .subresource_range(all_faces)],
                );
                context.end_single_time_commands(
                    init_cmd,
                    context.queues[&context.queue_families.graphics],
                    context.command_pool,
                );

                (
                    point_shadow_image,
                    point_shadow_image_memory,
                    point_shadow_cube_view,
                    point_shadow_face_views,
                )
            };

            let point_shadow_sampler = context.device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .compare_enable(true)
                    .compare_op(vk::CompareOp::LESS_OR_EQUAL),
                None,
            )?;

            // Write point shadow cube view into binding 2 of the light descriptor set.
            let point_shadow_desc_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                .image_view(point_shadow_cube_view)
                .sampler(point_shadow_sampler);
            context.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(light_descriptor_set)
                    .dst_binding(2)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[point_shadow_desc_info])],
                &[],
            );

            // Point shadow pipelines.
            let shadow_point_model_vert = pipeline_manager.create_shader_module(
                &rendering_info.context.clone().into(),
                "sdr_default_shadow_point_model.vert",
            )?;
            let shadow_point_voxel_vert = pipeline_manager.create_shader_module(
                &rendering_info.context.clone().into(),
                "sdr_default_shadow_point_voxel.vert",
            )?;
            let shadow_point_frag = pipeline_manager.create_shader_module(
                &rendering_info.context.clone().into(),
                "sdr_default_shadow_point.frag",
            )?;

            let shadow_point_model_pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default().push_constant_ranges(&[
                    vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                        .offset(0)
                        .size(128),
                ]),
                None,
            )?;

            let shadow_point_voxel_pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default().push_constant_ranges(&[
                    vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                        .offset(0)
                        .size(96),
                ]),
                None,
            )?;

            let shadow_point_rasterization = RasterizationSettings {
                cull_mode: vk::CullModeFlags::FRONT,
                depth_bias_enable: true,
                ..Default::default()
            };

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
                    rasterization_settings: shadow_point_rasterization,
                    dynamic_state_settings: shadow_dynamic_states.clone(),
                    ..Default::default()
                },
                shadow_point_model_pipeline_layout,
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
                    rasterization_settings: shadow_point_rasterization,
                    dynamic_state_settings: shadow_dynamic_states,
                    ..Default::default()
                },
                shadow_point_voxel_pipeline_layout,
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

            // 1×1 white RGBA pixel — used as the albedo texture when a mesh has no material.
            let white_pixels: [u8; 4] = [255, 255, 255, 255];
            let default_white_material = {
                let size = 4u64;
                let (staging_buf, staging_mem) = context.create_buffer(
                    size,
                    vk::BufferUsageFlags::TRANSFER_SRC,
                    vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
                )?;
                let ptr =
                    context
                        .device
                        .map_memory(staging_mem, 0, size, vk::MemoryMapFlags::empty())?
                        as *mut u8;
                ptr.copy_from_nonoverlapping(white_pixels.as_ptr(), 4);
                context.device.unmap_memory(staging_mem);

                let (white_image, white_memory) = context.create_image(
                    vk::Extent2D {
                        width: 1,
                        height: 1,
                    },
                    vk::Format::R8G8B8A8_SRGB,
                    vk::ImageTiling::OPTIMAL,
                    vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    SampleCountFlags::TYPE_1,
                )?;

                let init_cmd = context.begin_single_time_commands(context.command_pool);
                let sub = vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                };
                context.device.cmd_pipeline_barrier(
                    init_cmd,
                    vk::PipelineStageFlags::TOP_OF_PIPE,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .image(white_image)
                        .subresource_range(sub)
                        .src_access_mask(vk::AccessFlags::empty())
                        .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)],
                );
                context.device.cmd_copy_buffer_to_image(
                    init_cmd,
                    staging_buf,
                    white_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[vk::BufferImageCopy::default()
                        .image_subresource(vk::ImageSubresourceLayers {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            mip_level: 0,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .image_extent(vk::Extent3D {
                            width: 1,
                            height: 1,
                            depth: 1,
                        })],
                );
                context.device.cmd_pipeline_barrier(
                    init_cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                        .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .image(white_image)
                        .subresource_range(sub)
                        .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                        .dst_access_mask(vk::AccessFlags::SHADER_READ)],
                );
                context.end_single_time_commands(
                    init_cmd,
                    context.queues[&context.queue_families.transfer],
                    context.command_pool,
                );
                context.device.destroy_buffer(staging_buf, None);
                context.device.free_memory(staging_mem, None);

                let white_view = context.create_image_view(
                    white_image,
                    vk::Format::R8G8B8A8_SRGB,
                    vk::ImageAspectFlags::COLOR,
                )?;
                let white_sampler = context.device.create_sampler(
                    &vk::SamplerCreateInfo::default()
                        .mag_filter(vk::Filter::NEAREST)
                        .min_filter(vk::Filter::NEAREST)
                        .address_mode_u(vk::SamplerAddressMode::REPEAT)
                        .address_mode_v(vk::SamplerAddressMode::REPEAT),
                    None,
                )?;
                let white_ds = context.create_texture_descriptor_set(
                    descriptor_pool,
                    descriptor_set_layout,
                    white_view,
                    white_sampler,
                );

                GpuMaterial {
                    albedo: Some(crate::rendering::shared::texture::GpuTexture {
                        name: "default_white".to_string(),
                        image: white_image,
                        image_view: white_view,
                        memory: white_memory,
                        sampler: white_sampler,
                        descriptor_set: white_ds,
                    }),
                    color: [1.0, 1.0, 1.0, 1.0],
                    shader: None,
                }
            };

            // Populate the pipeline cache and set the model template before moving pipeline_manager.
            pipeline_manager
                .pipeline_cache
                .insert("model".to_string(), pipeline);
            pipeline_manager
                .pipeline_cache
                .insert("model::wireframe".to_string(), wireframe_pipeline);
            pipeline_manager
                .pipeline_cache
                .insert("model::collider_debug".to_string(), collider_debug_pipeline);
            pipeline_manager
                .pipeline_cache
                .insert("voxel".to_string(), voxel_pipeline);
            pipeline_manager
                .pipeline_cache
                .insert("voxel::wireframe".to_string(), voxel_wireframe_pipeline);
            pipeline_manager
                .pipeline_cache
                .insert("water".to_string(), water_pipeline);
            pipeline_manager
                .pipeline_cache
                .insert("shadow_model".to_string(), shadow_model_pipeline);
            pipeline_manager
                .pipeline_cache
                .insert("shadow_voxel".to_string(), shadow_voxel_pipeline);
            pipeline_manager.pipeline_cache.insert(
                "shadow_point_model".to_string(),
                shadow_point_model_pipeline,
            );
            pipeline_manager.pipeline_cache.insert(
                "shadow_point_voxel".to_string(),
                shadow_point_voxel_pipeline,
            );
            pipeline_manager.model_pipeline_template = Some(
                crate::rendering::vulkan::pipeline_manager::ModelPipelineTemplate {
                    layout: pipeline_layout,
                    image_format: Some(swapchain.format),
                    depth_format: Some(swapchain.depth_format),
                    image_extent: swapchain.extent,
                    aa_amount,
                },
            );

            let renderer = VulkanRenderer {
                current_image_index: 0,
                in_flight_frames_count,
                current_frame: 0,
                frames,
                image_layouts: ImageLayouts::default(),

                pipeline_layout,
                voxel_pipeline_layout,

                ui_renderer,

                voxel_descriptor_pool: descriptor_pool,
                voxel_descriptor_set_layout: descriptor_set_layout,
                default_white_material,
                water_pipeline_layout,
                buffer_graveyard: Vec::new(),

                viewport_image,
                viewport_image_memory,
                viewport_image_view,
                msaa_color_image,
                msaa_color_memory,
                msaa_color_view,
                viewport_depth_image,
                viewport_depth_memory,
                viewport_depth_view,
                viewport_sampler,
                viewport_descriptor_set,
                viewport_texture_id,
                viewport_extent,
                viewport_target_initialized: false,
                viewport_depth_initialized: false,
                last_fence_wait_ns: 0,

                light_ssbo,
                light_ssbo_memory,
                light_descriptor_pool,
                light_descriptor_set_layout,
                light_descriptor_set,

                shadow_image,
                shadow_image_memory,
                shadow_image_view,
                shadow_cascade_views,
                shadow_sampler,
                shadow_map_size: SHADOW_MAP_SIZE,
                shadow_model_pipeline_layout,
                shadow_voxel_pipeline_layout,
                shadow_model_vertex_shader: "sdr_default_shadow_model.vert".to_string(),
                shadow_voxel_vertex_shader: "sdr_default_shadow_voxel.vert".to_string(),
                shadow_fragment_shader: "sdr_default_shadow.frag".to_string(),

                point_shadow_image,
                point_shadow_image_memory,
                point_shadow_cube_view,
                point_shadow_face_views,
                point_shadow_sampler,
                point_shadow_map_size: SHADOW_MAP_SIZE,
                shadow_point_model_pipeline_layout,
                shadow_point_voxel_pipeline_layout,
                shadow_point_model_vertex_shader: "sdr_default_shadow_point_model.vert".to_string(),
                shadow_point_voxel_vertex_shader: "sdr_default_shadow_point_voxel.vert".to_string(),
                shadow_point_fragment_shader: "sdr_default_shadow_point.frag".to_string(),

                ubo,
                pipeline_manager,
                default_vertex_shader,
                default_fragment_shader,
                voxel_vertex_shader,
                voxel_fragment_shader,
                water_vertex_shader,
                water_fragment_shader,
                context: Arc::new(rendering_info.context.clone()),
                swapchain,

                aa_amount,
                ui_cached_primitives: Vec::new(),
                ui_pending_texture_frees: Vec::new(),
            };

            rendering_info.renderer = Some(Box::new(renderer));
        }

        Ok(())
    }

    fn last_fence_wait_ns(&self) -> u64 {
        self.last_fence_wait_ns
    }

    fn begin_frame(&mut self) -> Result<()> {
        let frame = &self.frames[self.current_frame];

        if self.swapchain.is_dirty
            && let Err(e) = self.swapchain.resize()
        {
            eprintln!("Failed to recreate swapchain: {}", e);
            return Err(anyhow::anyhow!("Failed to recreate swapchain: {}", e));
        }

        unsafe {
            const FENCE_TIMEOUT_NS: u64 = 20_000_000_000; // 20 seconds (better for iGPU)

            let fence_start = std::time::Instant::now();
            match self.context.device.wait_for_fences(
                &[frame.in_flight_fence],
                true,
                FENCE_TIMEOUT_NS,
            ) {
                Ok(()) => {
                    self.last_fence_wait_ns = fence_start.elapsed().as_nanos() as u64;
                    // The fence is now signalled, so the GPU is done with any work
                    // that referenced these buffers. Actually destroy them — draining
                    // alone just drops the raw handles and leaks the GPU allocations.
                    for (buffer, memory) in self.buffer_graveyard.drain(..) {
                        if buffer != vk::Buffer::null() {
                            self.context.device.destroy_buffer(buffer, None);
                        }
                        if memory != vk::DeviceMemory::null() {
                            self.context.device.free_memory(memory, None);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Fence wait failed (likely device timeout): {}", e);
                    let _ = self.context.device.device_wait_idle();
                    return Err(anyhow::anyhow!("Device timeout during frame wait"));
                }
            }

            if let Err(e) = self.context.device.reset_fences(&[frame.in_flight_fence]) {
                eprintln!("Failed to reset fences: {}", e);
                return Err(anyhow::anyhow!("Failed to reset in-flight fence: {}", e));
            }

            if let Err(e) = self
                .context
                .device
                .reset_command_buffer(frame.command_buffer, CommandBufferResetFlags::empty())
            {
                eprintln!("Failed to reset command buffer: {}", e);
                return Err(anyhow::anyhow!("Failed to reset command buffer: {}", e));
            }

            match self
                .swapchain
                .acquire_next_image(frame.image_available_semaphore)
            {
                Ok(index) => self.current_image_index = index,
                Err(e) => {
                    eprintln!("Failed to acquire next image: {}", e);
                    if let Err(idle_err) = self.context.device.device_wait_idle() {
                        eprintln!("Device wait idle failed: {}", idle_err);
                        return Err(anyhow::anyhow!("Device lost during acquire: {}", idle_err));
                    }
                    if let Err(resize_err) = self.swapchain.resize() {
                        eprintln!(
                            "Failed to recreate swapchain during recovery: {}",
                            resize_err
                        );
                        return Err(anyhow::anyhow!(
                            "Failed to recover swapchain: {}",
                            resize_err
                        ));
                    }
                    self.current_image_index = self
                        .swapchain
                        .acquire_next_image(frame.image_available_semaphore)?;
                }
            }

            if let Err(e) = self.context.device.begin_command_buffer(
                frame.command_buffer,
                &ash::vk::CommandBufferBeginInfo::default(),
            ) {
                eprintln!("Failed to begin command buffer: {}", e);
                return Err(anyhow::anyhow!("Failed to begin command buffer: {}", e));
            }

            self.context.transition_image_layout(
                frame.command_buffer,
                self.swapchain.images[self.current_image_index as usize],
                self.image_layouts.present,
                self.image_layouts.renderable,
                vk::ImageAspectFlags::COLOR,
            );
            self.context.transition_image_layout(
                frame.command_buffer,
                self.swapchain.depth_image,
                self.image_layouts.depth,
                self.image_layouts.depth,
                vk::ImageAspectFlags::DEPTH,
            );
        }
        Ok(())
    }

    fn begin_viewport_render(&mut self) -> Result<()> {
        let frame = &self.frames[self.current_frame];

        let old_color_layout = if self.viewport_target_initialized {
            self.image_layouts.shader_read_only
        } else {
            self.image_layouts.undefined
        };

        if self.aa_amount == AntiAliasingAmount::X0 {
            self.context.transition_image_layout(
                frame.command_buffer,
                self.viewport_image,
                old_color_layout,
                self.image_layouts.renderable,
                vk::ImageAspectFlags::COLOR,
            );

            let old_depth_layout = if self.viewport_depth_initialized {
                self.image_layouts.depth
            } else {
                self.image_layouts.undefined
            };

            self.context.transition_image_layout(
                frame.command_buffer,
                self.viewport_depth_image,
                old_depth_layout,
                self.image_layouts.depth,
                vk::ImageAspectFlags::DEPTH,
            );

            self.context.begin_rendering(
                frame.command_buffer,
                self.viewport_image_view,
                ImageView::null(),
                self.viewport_depth_view,
                ClearColorValue {
                    float32: [0.05, 0.05, 0.05, 1.0],
                },
                vk::Rect2D::default().extent(self.viewport_extent),
            );
        } else {
            self.context.transition_image_layout(
                frame.command_buffer,
                self.viewport_image,
                old_color_layout,
                self.image_layouts.renderable,
                vk::ImageAspectFlags::COLOR,
            );

            let msaa_old_layout = if self.viewport_target_initialized {
                self.image_layouts.renderable
            } else {
                self.image_layouts.undefined
            };
            self.context.transition_image_layout(
                frame.command_buffer,
                self.msaa_color_image,
                msaa_old_layout,
                self.image_layouts.renderable,
                vk::ImageAspectFlags::COLOR,
            );

            let old_depth_layout = if self.viewport_depth_initialized {
                self.image_layouts.depth
            } else {
                self.image_layouts.undefined
            };

            self.context.transition_image_layout(
                frame.command_buffer,
                self.viewport_depth_image,
                old_depth_layout,
                self.image_layouts.depth,
                vk::ImageAspectFlags::DEPTH,
            );

            // msaa_color_view → render target; viewport_image_view → resolve target
            self.context.begin_rendering(
                frame.command_buffer,
                self.msaa_color_view,
                self.viewport_image_view,
                self.viewport_depth_view,
                ClearColorValue {
                    float32: [0.05, 0.05, 0.05, 1.0],
                },
                vk::Rect2D::default().extent(self.viewport_extent),
            );
        }

        unsafe {
            self.context.device.cmd_set_viewport(
                frame.command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.viewport_extent.width as f32,
                    height: self.viewport_extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.context.device.cmd_set_scissor(
                frame.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.viewport_extent,
                }],
            );
        }

        self.viewport_target_initialized = true;
        self.viewport_depth_initialized = true;
        Ok(())
    }

    fn end_viewport_render(&mut self) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        unsafe {
            self.context.device.cmd_end_rendering(frame.command_buffer);
        }

        // Transition the resolve target so egui can sample it
        self.context.transition_image_layout(
            frame.command_buffer,
            self.viewport_image,
            self.image_layouts.renderable,
            self.image_layouts.shader_read_only,
            vk::ImageAspectFlags::COLOR,
        );

        Ok(())
    }

    fn begin_swapchain_render(&mut self) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        self.context.begin_rendering(
            frame.command_buffer,
            self.swapchain.views[self.current_image_index as usize],
            ImageView::null(),
            self.swapchain.depth_image_view,
            ClearColorValue {
                float32: [0.2, 0.2, 0.2, 1.0],
            },
            vk::Rect2D::default().extent(self.swapchain.extent),
        );

        unsafe {
            self.context.device.cmd_set_viewport(
                frame.command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: self.swapchain.extent.width as f32,
                    height: self.swapchain.extent.height as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.context.device.cmd_set_scissor(
                frame.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: self.swapchain.extent,
                }],
            );
        }

        Ok(())
    }

    fn get_viewport_texture_id(&self) -> Option<TextureId> {
        Some(self.viewport_texture_id)
    }

    fn end_frame(&mut self) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        unsafe {
            self.context.device.cmd_end_rendering(frame.command_buffer);

            self.context.transition_image_layout(
                frame.command_buffer,
                self.swapchain.images[self.current_image_index as usize],
                self.image_layouts.renderable,
                self.image_layouts.present,
                vk::ImageAspectFlags::COLOR,
            );

            if let Err(e) = self.context.device.end_command_buffer(frame.command_buffer) {
                eprintln!("Failed to end command buffer: {}", e);
                return Err(anyhow::anyhow!("Failed to end command buffer: {}", e));
            }

            if let Err(e) = self.context.device.queue_submit(
                self.context.queues[&self.context.queue_families.graphics],
                &[ash::vk::SubmitInfo::default()
                    .wait_semaphores(&[frame.image_available_semaphore])
                    .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                    .command_buffers(&[frame.command_buffer])
                    .signal_semaphores(&[frame.render_finished_semaphore])],
                frame.in_flight_fence,
            ) {
                eprintln!("Failed to submit command buffer: {}", e);
                let _ = self.context.device.device_wait_idle();
                return Err(anyhow::anyhow!("Failed to submit graphics queue: {}", e));
            }

            if let Err(e) = self
                .swapchain
                .present_image(self.current_image_index, frame.render_finished_semaphore)
            {
                eprintln!("Failed to present image: {}", e);
                self.swapchain.is_dirty = true;
                return Err(anyhow::anyhow!("Failed to present image: {}", e));
            }
        }
        Ok(())
    }

    fn render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        albedo_descriptor_set: Option<vk::DescriptorSet>,
        shader_override: Option<&str>,
    ) -> anyhow::Result<()> {
        let frame = &self.frames[self.current_frame];
        let albedo_ds = albedo_descriptor_set
            .filter(|ds| *ds != vk::DescriptorSet::null())
            .unwrap_or(
                self.default_white_material
                    .albedo
                    .as_ref()
                    .unwrap()
                    .descriptor_set,
            );

        let pipeline = match shader_override {
            Some(name) => self
                .pipeline_manager
                .get_or_create_model_pipeline(name, &self.context.clone(), false)?,
            None => self.get_pipeline("model"),
        };

        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );
            self.context.device.cmd_bind_descriptor_sets(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.light_descriptor_set, albedo_ds],
                &[],
            );

            let mut data = push_constants.return_renderable();
            data.extend(model_push_constants.return_renderable());
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &data,
            );

            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }

        Ok(())
    }

    fn wireframe_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        albedo_descriptor_set: Option<vk::DescriptorSet>,
        shader_override: Option<&str>,
    ) -> anyhow::Result<()> {
        let frame = &self.frames[self.current_frame];
        let albedo_ds = albedo_descriptor_set
            .filter(|ds| *ds != vk::DescriptorSet::null())
            .unwrap_or(
                self.default_white_material
                    .albedo
                    .as_ref()
                    .unwrap()
                    .descriptor_set,
            );

        let pipeline = match shader_override {
            Some(name) => self
                .pipeline_manager
                .get_or_create_model_pipeline(name, &self.context.clone(), true)?,
            None => self.get_pipeline("model::wireframe"),
        };

        let mut data = push_constants.return_renderable();
        data.extend(model_push_constants.return_renderable());

        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );
            self.context.device.cmd_bind_descriptor_sets(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.light_descriptor_set, albedo_ds],
                &[],
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &data,
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }

        Ok(())
    }

    fn collider_debug_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
    ) -> anyhow::Result<()> {
        let frame = &self.frames[self.current_frame];
        let albedo_ds = self
            .default_white_material
            .albedo
            .as_ref()
            .unwrap()
            .descriptor_set;

        let pipeline = self.get_pipeline("model::collider_debug");

        let mut data = push_constants.return_renderable();
        data.extend(model_push_constants.return_renderable());

        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                pipeline,
            );
            self.context.device.cmd_bind_descriptor_sets(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.light_descriptor_set, albedo_ds],
                &[],
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &data,
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }

        Ok(())
    }

    fn voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
        wireframe: bool,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let mut data = push_constants.return_renderable();
        data.extend(voxel_push_constants.return_renderable());
        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.get_pipeline(if wireframe { "voxel::wireframe" } else { "voxel" }),
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.voxel_pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &data,
            );
            self.context.device.cmd_bind_descriptor_sets(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.voxel_pipeline_layout,
                0,
                &[atlas.descriptor_set],
                &[],
            );
            self.context.device.cmd_bind_descriptor_sets(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.voxel_pipeline_layout,
                1,
                &[self.light_descriptor_set],
                &[],
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }
        Ok(())
    }

    fn water_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let mut data = push_constants.return_renderable();
        data.extend(voxel_push_constants.return_renderable());
        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.get_pipeline("water"),
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.water_pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &data,
            );
            self.context.device.cmd_bind_descriptor_sets(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.water_pipeline_layout,
                0,
                &[atlas.descriptor_set],
                &[],
            );
            self.context.device.cmd_bind_descriptor_sets(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.water_pipeline_layout,
                1,
                &[self.light_descriptor_set],
                &[],
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }
        Ok(())
    }

    fn begin_ui(&mut self) {
        let mut state = self.ui_renderer.state.lock().unwrap();
        let raw_input = state.take_egui_input(&self.ui_renderer.window);
        self.ui_renderer.context.begin_pass(raw_input);
    }

    fn end_ui(&mut self) -> Result<()> {
        let full_output = self.ui_renderer.context.end_pass();
        let mut state = self.ui_renderer.state.lock().unwrap();
        let mut renderer = self.ui_renderer.renderer.lock().unwrap();

        state.handle_platform_output(&self.ui_renderer.window, full_output.platform_output);

        // Free textures egui retired last frame. `begin_frame` already waited on the
        // previous frame's fence, so the GPU is done with them. Deferring one frame
        // avoids destroying a texture still referenced by an in-flight command buffer.
        if !self.ui_pending_texture_frees.is_empty() {
            let to_free = std::mem::take(&mut self.ui_pending_texture_frees);
            renderer.free_textures(&to_free)?;
        }

        let pixels_per_point = full_output.pixels_per_point;

        self.ui_cached_primitives = self
            .ui_renderer
            .context
            .tessellate(full_output.shapes, pixels_per_point);

        if !full_output.textures_delta.set.is_empty() {
            let texture_updates: Vec<(TextureId, ImageDelta)> = full_output
                .textures_delta
                .set
                .iter()
                .map(|(id, delta)| (*id, delta.clone()))
                .collect();
            renderer.set_textures(
                self.context.queues[&self.context.queue_families.graphics],
                self.context.command_pool,
                &texture_updates,
            )?;
        }

        renderer.cmd_draw(
            self.frames[self.current_frame].command_buffer,
            self.swapchain.extent,
            pixels_per_point,
            &self.ui_cached_primitives,
        )?;

        // Defer these frees until next frame (see `ui_pending_texture_frees`).
        self.ui_pending_texture_frees = full_output.textures_delta.free;

        Ok(())
    }

    fn handle_ui_event(&mut self, event: &WindowEvent) -> bool {
        let mut state = self.ui_renderer.state.lock().unwrap();
        state
            .on_window_event(&self.ui_renderer.window, event)
            .consumed
    }

    fn get_egui_context(&self) -> Context {
        self.ui_renderer.context.clone()
    }

    fn update_command_buffer(&mut self) {}

    fn recreate_swapchain(&mut self) {
        if let Err(e) = self.swapchain.resize() {
            eprintln!("Failed to recreate swapchain: {}", e);
        }
    }

    fn resize(&mut self) -> anyhow::Result<()> {
        self.swapchain.resize()
    }

    fn get_buffer_graveyard(&mut self) -> &mut Vec<(vk::Buffer, vk::DeviceMemory)> {
        &mut self.buffer_graveyard
    }

    fn get_command_pool(&self) -> Result<CommandPool> {
        Ok(self.context.command_pool)
    }

    fn get_aspect(&self) -> f32 {
        self.viewport_extent.width as f32 / self.viewport_extent.height as f32
    }

    fn get_descriptor_pool(&self) -> vk::DescriptorPool {
        self.voxel_descriptor_pool
    }

    fn get_voxel_descriptor_set_layout(&self) -> vk::DescriptorSetLayout {
        self.voxel_descriptor_set_layout
    }

    fn set_lights(
        &mut self,
        lights: &[GpuLight],
        shadow_data: Option<ShadowData>,
        point_shadow_data: Option<PointShadowData>,
        shadow_distance: f32,
        camera_pos: [f32; 3],
        camera_dir: [f32; 3],
    ) {
        let count = lights.len().min(MAX_LIGHTS) as u32;
        // SSBO layout (std430, 336-byte header):
        //   offset   0: uint  count
        //   offset   4: uint  shadow_enabled  (0=off, 1=spot, 2=directional CSM)
        //   offset   8: uint  cascade_count
        //   offset  12: float shadow_distance
        //   offset  16: vec4  camera_world_pos
        //   offset  32: vec4  camera_world_dir
        //   offset  48: mat4  light_space[4]         (256 bytes)
        //   offset 304: float cascade_splits[4]      (16 bytes)
        //   offset 320: uint  shadow_light_index
        //   offset 324: uint  point_shadow_enabled
        //   offset 328: uint  point_shadow_light_index
        //   offset 332: float point_shadow_far
        //   offset 336: GpuLight[]
        const LIGHTS_OFFSET: usize = 336;

        unsafe {
            let ptr = self
                .context
                .device
                .map_memory(
                    self.light_ssbo_memory,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap() as *mut u8;

            let (shadow_enabled, cascade_count, shadow_light_index) = match &shadow_data {
                None => (0u32, 0u32, 0u32),
                Some(d) if d.cascade_count == 1 => (1u32, 1u32, d.shadow_light_index),
                Some(d) => (2u32, CSM_CASCADE_COUNT as u32, d.shadow_light_index),
            };

            let (point_shadow_enabled, point_shadow_light_index, point_shadow_far) =
                match &point_shadow_data {
                    None => (0u32, 0u32, 1.0f32),
                    Some(d) => (1u32, d.light_index, d.far),
                };

            (ptr as *mut u32).write(count);
            (ptr.add(4) as *mut u32).write(shadow_enabled);
            (ptr.add(8) as *mut u32).write(cascade_count);
            (ptr.add(12) as *mut f32).write(shadow_distance);
            (ptr.add(16) as *mut [f32; 4]).write([
                camera_pos[0],
                camera_pos[1],
                camera_pos[2],
                0.0,
            ]);
            (ptr.add(32) as *mut [f32; 4]).write([
                camera_dir[0],
                camera_dir[1],
                camera_dir[2],
                0.0,
            ]);

            if let Some(ref d) = shadow_data {
                for i in 0..CSM_CASCADE_COUNT {
                    let mat = d.matrices.get(i).copied().unwrap_or([[0.0f32; 4]; 4]);
                    (ptr.add(48 + i * 64) as *mut [[f32; 4]; 4]).write(mat);
                }
                (ptr.add(304) as *mut [f32; 4]).write(d.splits);
            } else {
                std::ptr::write_bytes(ptr.add(48), 0, 256 + 16);
            }

            (ptr.add(320) as *mut u32).write(shadow_light_index);
            (ptr.add(324) as *mut u32).write(point_shadow_enabled);
            (ptr.add(328) as *mut u32).write(point_shadow_light_index);
            (ptr.add(332) as *mut f32).write(point_shadow_far);

            (ptr.add(LIGHTS_OFFSET) as *mut GpuLight)
                .copy_from_nonoverlapping(lights.as_ptr(), count as usize);

            self.context.device.unmap_memory(self.light_ssbo_memory);
        }
    }

    fn rebuild_shadow_map(&mut self, size: u32) -> Result<()> {
        if size == self.shadow_map_size {
            return Ok(());
        }
        unsafe {
            self.context.device.device_wait_idle()?;

            for view in &self.shadow_cascade_views {
                self.context.device.destroy_image_view(*view, None);
            }
            self.context
                .device
                .destroy_image_view(self.shadow_image_view, None);
            self.context.device.destroy_image(self.shadow_image, None);
            self.context
                .device
                .free_memory(self.shadow_image_memory, None);

            let shadow_image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .extent(vk::Extent3D {
                    width: size,
                    height: size,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(CSM_CASCADE_COUNT as u32)
                .format(vk::Format::D32_SFLOAT)
                .tiling(vk::ImageTiling::OPTIMAL)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                .samples(SampleCountFlags::TYPE_1)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let shadow_image = self.context.device.create_image(&shadow_image_info, None)?;
            let mem_reqs = self
                .context
                .device
                .get_image_memory_requirements(shadow_image);
            let shadow_image_memory = self.context.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_reqs.size)
                    .memory_type_index(self.context.find_memory_type(
                        mem_reqs.memory_type_bits,
                        vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    )?),
                None,
            )?;
            self.context
                .device
                .bind_image_memory(shadow_image, shadow_image_memory, 0)?;

            let subresource_all = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(CSM_CASCADE_COUNT as u32);

            let shadow_image_view = self.context.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(shadow_image)
                    .view_type(vk::ImageViewType::TYPE_2D_ARRAY)
                    .format(vk::Format::D32_SFLOAT)
                    .subresource_range(subresource_all),
                None,
            )?;

            let mut shadow_cascade_views = [vk::ImageView::null(); CSM_CASCADE_COUNT];
            for i in 0..CSM_CASCADE_COUNT {
                shadow_cascade_views[i] = self.context.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(shadow_image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(i as u32)
                                .layer_count(1),
                        ),
                    None,
                )?;
            }

            let cmd = self
                .context
                .begin_single_time_commands(self.context.command_pool);
            self.context.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                    .image(shadow_image)
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .subresource_range(subresource_all)],
            );
            self.context.end_single_time_commands(
                cmd,
                self.context.queues[&self.context.queue_families.graphics],
                self.context.command_pool,
            );

            let shadow_desc_image_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                .image_view(shadow_image_view)
                .sampler(self.shadow_sampler);
            self.context.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(self.light_descriptor_set)
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[shadow_desc_image_info])],
                &[],
            );

            self.shadow_image = shadow_image;
            self.shadow_image_memory = shadow_image_memory;
            self.shadow_image_view = shadow_image_view;
            self.shadow_cascade_views = shadow_cascade_views;
            self.shadow_map_size = size;
        }
        Ok(())
    }

    fn begin_shadow_pass(
        &mut self,
        cascade_index: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let s = self.shadow_map_size;
        let shadow_extent = vk::Extent2D {
            width: s,
            height: s,
        };

        // On the first cascade, transition all layers from read-only to depth-write at once.
        if cascade_index == 0 {
            unsafe {
                self.context.device.cmd_pipeline_barrier(
                    frame.command_buffer,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                        .new_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                        .image(self.shadow_image)
                        .src_access_mask(vk::AccessFlags::SHADER_READ)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(0)
                                .layer_count(CSM_CASCADE_COUNT as u32),
                        )],
                );
            }
        }

        // Render into the per-layer view for this cascade.
        self.context.begin_depth_only_rendering(
            frame.command_buffer,
            self.shadow_cascade_views[cascade_index],
            vk::Rect2D::default().extent(shadow_extent),
        );

        unsafe {
            // Hardware slope-scaled depth bias - eliminates shadow acne without Peter Panning.
            // bias_constant = constant depth offset (in depth units).
            // bias_slope    = multiplied by max depth slope of each polygon.
            self.context.device.cmd_set_depth_bias(
                frame.command_buffer,
                bias_constant,
                0.0, // clamp (0 = unclamped)
                bias_slope,
            );

            self.context.device.cmd_set_viewport(
                frame.command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: s as f32,
                    height: s as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.context.device.cmd_set_scissor(
                frame.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: shadow_extent,
                }],
            );
        }

        Ok(())
    }

    fn end_shadow_pass(&mut self, _cascade_index: usize) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        unsafe {
            self.context.device.cmd_end_rendering(frame.command_buffer);

            // Transition all cascade layers back to shader-readable after each pass.
            // For directional CSM the next cascade's begin_shadow_pass will handle the
            // forward barrier; for spot lights (single pass) this restores the layout correctly.
            self.context.device.cmd_pipeline_barrier(
                frame.command_buffer,
                vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                    .image(self.shadow_image)
                    .src_access_mask(
                        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                            | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                    )
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::DEPTH)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(CSM_CASCADE_COUNT as u32),
                    )],
            );
        }

        Ok(())
    }

    fn shadow_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowModelPushConstants,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let data = pc.return_renderable();
        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.get_pipeline("shadow_model"),
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.shadow_model_pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                &data,
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }
        Ok(())
    }

    fn shadow_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowVoxelPushConstants,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let data = pc.return_renderable();
        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.get_pipeline("shadow_voxel"),
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.shadow_voxel_pipeline_layout,
                vk::ShaderStageFlags::VERTEX,
                0,
                &data,
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }
        Ok(())
    }

    fn rebuild_point_shadow_map(&mut self, size: u32) -> Result<()> {
        if size == self.point_shadow_map_size {
            return Ok(());
        }
        unsafe {
            self.context.device.device_wait_idle()?;

            for view in &self.point_shadow_face_views {
                self.context.device.destroy_image_view(*view, None);
            }
            self.context
                .device
                .destroy_image_view(self.point_shadow_cube_view, None);
            self.context
                .device
                .destroy_image(self.point_shadow_image, None);
            self.context
                .device
                .free_memory(self.point_shadow_image_memory, None);

            let image_info = vk::ImageCreateInfo::default()
                .flags(vk::ImageCreateFlags::CUBE_COMPATIBLE)
                .image_type(vk::ImageType::TYPE_2D)
                .extent(vk::Extent3D {
                    width: size,
                    height: size,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(6)
                .format(vk::Format::D32_SFLOAT)
                .tiling(vk::ImageTiling::OPTIMAL)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
                .samples(SampleCountFlags::TYPE_1)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let point_shadow_image = self.context.device.create_image(&image_info, None)?;
            let mem_reqs = self
                .context
                .device
                .get_image_memory_requirements(point_shadow_image);
            let point_shadow_image_memory = self.context.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(mem_reqs.size)
                    .memory_type_index(self.context.find_memory_type(
                        mem_reqs.memory_type_bits,
                        vk::MemoryPropertyFlags::DEVICE_LOCAL,
                    )?),
                None,
            )?;
            self.context.device.bind_image_memory(
                point_shadow_image,
                point_shadow_image_memory,
                0,
            )?;

            let all_faces = vk::ImageSubresourceRange::default()
                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(6);

            let point_shadow_cube_view = self.context.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(point_shadow_image)
                    .view_type(vk::ImageViewType::CUBE)
                    .format(vk::Format::D32_SFLOAT)
                    .subresource_range(all_faces),
                None,
            )?;

            let mut point_shadow_face_views = [vk::ImageView::null(); 6];
            for i in 0..6usize {
                point_shadow_face_views[i] = self.context.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(point_shadow_image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(i as u32)
                                .layer_count(1),
                        ),
                    None,
                )?;
            }

            let cmd = self
                .context
                .begin_single_time_commands(self.context.command_pool);
            self.context.device.cmd_pipeline_barrier(
                cmd,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[vk::ImageMemoryBarrier::default()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                    .image(point_shadow_image)
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)
                    .subresource_range(all_faces)],
            );
            self.context.end_single_time_commands(
                cmd,
                self.context.queues[&self.context.queue_families.graphics],
                self.context.command_pool,
            );

            let desc_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                .image_view(point_shadow_cube_view)
                .sampler(self.point_shadow_sampler);
            self.context.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(self.light_descriptor_set)
                    .dst_binding(2)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[desc_info])],
                &[],
            );

            self.point_shadow_image = point_shadow_image;
            self.point_shadow_image_memory = point_shadow_image_memory;
            self.point_shadow_cube_view = point_shadow_cube_view;
            self.point_shadow_face_views = point_shadow_face_views;
            self.point_shadow_map_size = size;
        }
        Ok(())
    }

    fn begin_point_shadow_pass(
        &mut self,
        face: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let s = self.point_shadow_map_size;

        // On face 0, transition all 6 layers from read-only to depth-write at once.
        if face == 0 {
            unsafe {
                self.context.device.cmd_pipeline_barrier(
                    frame.command_buffer,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                        .new_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                        .image(self.point_shadow_image)
                        .src_access_mask(vk::AccessFlags::SHADER_READ)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(0)
                                .layer_count(6),
                        )],
                );
            }
        }

        self.context.begin_depth_only_rendering(
            frame.command_buffer,
            self.point_shadow_face_views[face],
            vk::Rect2D::default().extent(vk::Extent2D {
                width: s,
                height: s,
            }),
        );

        unsafe {
            self.context.device.cmd_set_depth_bias(
                frame.command_buffer,
                bias_constant,
                0.0,
                bias_slope,
            );
            self.context.device.cmd_set_viewport(
                frame.command_buffer,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: s as f32,
                    height: s as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.context.device.cmd_set_scissor(
                frame.command_buffer,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: vk::Extent2D {
                        width: s,
                        height: s,
                    },
                }],
            );
        }
        Ok(())
    }

    fn end_point_shadow_pass(&mut self, face: usize) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        unsafe {
            self.context.device.cmd_end_rendering(frame.command_buffer);

            // After the last face, transition all 6 layers back to shader-readable.
            if face == 5 {
                self.context.device.cmd_pipeline_barrier(
                    frame.command_buffer,
                    vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                        | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                    vk::PipelineStageFlags::FRAGMENT_SHADER,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[],
                    &[vk::ImageMemoryBarrier::default()
                        .old_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
                        .image(self.point_shadow_image)
                        .src_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .dst_access_mask(vk::AccessFlags::SHADER_READ)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(0)
                                .layer_count(6),
                        )],
                );
            }
        }
        Ok(())
    }

    fn shadow_point_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointModelPushConstants,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let data = pc.return_renderable();
        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.get_pipeline("shadow_point_model"),
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.shadow_point_model_pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &data,
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }
        Ok(())
    }

    fn shadow_point_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointVoxelPushConstants,
    ) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let data = pc.return_renderable();
        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.get_pipeline("shadow_point_voxel"),
            );
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.shadow_point_voxel_pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                &data,
            );
            self.context.device.cmd_bind_vertex_buffers(
                frame.command_buffer,
                0,
                &[mesh.get_vertex_buffer()],
                &[0],
            );
            self.context.device.cmd_bind_index_buffer(
                frame.command_buffer,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context.device.cmd_draw_indexed(
                frame.command_buffer,
                mesh.get_index_count(),
                1,
                0,
                0,
                0,
            );
        }
        Ok(())
    }

    fn reload_shaders(&mut self) -> Result<bool> {
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

    fn resize_viewport(
        &mut self,
        width: u32,
        height: u32,
        aa_amount: AntiAliasingAmount,
    ) -> Result<()> {
        let new_extent = vk::Extent2D { width, height };
        self.resize_viewport(new_extent, aa_amount)
    }

    fn get_viewport_extent(&mut self) -> Extent2D {
        self.viewport_extent
    }
}
