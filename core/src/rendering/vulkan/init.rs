use std::sync::{Arc, Mutex};

use anyhow::Result;
use ash::vk::{self, PipelineLayout, PipelineLayoutCreateInfo};
use winit::window::Window;

use crate::assets::gltf::upload_texture_from_pixels;
use crate::rendering::RenderingInfo;
use crate::rendering::lighting::gpu_light::{CSM_CASCADE_COUNT, GpuLight, MAX_LIGHTS};
use crate::rendering::shared::anti_aliasing::AntiAliasingAmount;
use crate::rendering::shared::material::GpuMaterial;
use crate::rendering::vulkan::frame::VulkanFrame;
use crate::rendering::vulkan::image_layout::ImageLayouts;
use crate::rendering::vulkan::pipeline_manager::PipelineManager;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::rendering::vulkan::shadow::{
    DEFAULT_SHADOW_MAP_SIZE, create_depth_array_target, write_shadow_descriptor,
};
use crate::rendering::vulkan::swapchain::VulkanSwapchain;
use crate::rendering::vulkan::viewport::create_viewport_targets;
use crate::rendering::vulkan::{Ubo, VulkanRenderer, aa_sample_count};
use crate::ui::UIRenderer;

const IN_FLIGHT_FRAMES: usize = 3;

/// Light SSBO plus the descriptor set it lives in (bindings: 0 = SSBO,
/// 1 = CSM shadow map, 2 = point shadow cubemap).
struct LightResources {
    ssbo: vk::Buffer,
    ssbo_memory: vk::DeviceMemory,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_set: vk::DescriptorSet,
}

/// One pipeline layout per pass family.
/// Push constant sizes must match the corresponding structs in `shared::push_constants`.
struct PipelineLayouts {
    model: PipelineLayout,
    voxel: PipelineLayout,
    water: PipelineLayout,
    shadow_model: PipelineLayout,
    shadow_voxel: PipelineLayout,
    shadow_point_model: PipelineLayout,
    shadow_point_voxel: PipelineLayout,
}

fn sampler_binding(binding: u32) -> vk::DescriptorSetLayoutBinding<'static> {
    vk::DescriptorSetLayoutBinding::default()
        .binding(binding)
        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
}

fn create_light_resources(context: &VulkanRenderingContext) -> Result<LightResources> {
    // Header: 336 bytes (see SSBO layout in set_lights)
    let ssbo_size = (336 + size_of::<GpuLight>() * MAX_LIGHTS) as vk::DeviceSize;

    let (ssbo, ssbo_memory) = context.create_buffer(
        ssbo_size,
        vk::BufferUsageFlags::STORAGE_BUFFER,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    let ssbo_binding = vk::DescriptorSetLayoutBinding::default()
        .binding(0)
        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
        .descriptor_count(1)
        .stage_flags(vk::ShaderStageFlags::FRAGMENT);

    unsafe {
        let descriptor_set_layout = context.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default().bindings(&[
                ssbo_binding,
                sampler_binding(1),
                sampler_binding(2),
            ]),
            None,
        )?;

        let descriptor_pool = context.device.create_descriptor_pool(
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

        let descriptor_set = context
            .device
            .allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::default()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&[descriptor_set_layout]),
            )?
            .remove(0);

        let buffer_info = vk::DescriptorBufferInfo::default()
            .buffer(ssbo)
            .offset(0)
            .range(ssbo_size);
        context.device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .buffer_info(&[buffer_info])],
            &[],
        );

        Ok(LightResources {
            ssbo,
            ssbo_memory,
            descriptor_pool,
            descriptor_set_layout,
            descriptor_set,
        })
    }
}

/// Layout and pool for per-material texture sets (binding 0 = albedo, 1 = normal).
fn create_texture_descriptor_resources(
    context: &VulkanRenderingContext,
) -> Result<(vk::DescriptorSetLayout, vk::DescriptorPool)> {
    unsafe {
        let layout = context.device.create_descriptor_set_layout(
            &vk::DescriptorSetLayoutCreateInfo::default()
                .bindings(&[sampler_binding(0), sampler_binding(1)]),
            None,
        )?;

        let pool = context.device.create_descriptor_pool(
            &vk::DescriptorPoolCreateInfo::default()
                .max_sets(500)
                .pool_sizes(&[vk::DescriptorPoolSize {
                    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                    descriptor_count: 500,
                }]),
            None,
        )?;

        Ok((layout, pool))
    }
}

fn push_constant_layout(
    context: &VulkanRenderingContext,
    stages: vk::ShaderStageFlags,
    size: u32,
    set_layouts: &[vk::DescriptorSetLayout],
) -> Result<PipelineLayout> {
    unsafe {
        Ok(context.device.create_pipeline_layout(
            &PipelineLayoutCreateInfo::default()
                .push_constant_ranges(&[vk::PushConstantRange::default()
                    .stage_flags(stages)
                    .offset(0)
                    .size(size)])
                .set_layouts(set_layouts),
            None,
        )?)
    }
}

fn create_pipeline_layouts(
    context: &VulkanRenderingContext,
    light_layout: vk::DescriptorSetLayout,
    texture_layout: vk::DescriptorSetLayout,
) -> Result<PipelineLayouts> {
    let vf = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
    Ok(PipelineLayouts {
        model: push_constant_layout(context, vf, 228, &[light_layout, texture_layout])?,
        voxel: push_constant_layout(context, vf, 160, &[texture_layout, light_layout])?,
        water: push_constant_layout(context, vf, 160, &[texture_layout, light_layout])?,
        shadow_model: push_constant_layout(context, vk::ShaderStageFlags::VERTEX, 112, &[])?,
        shadow_voxel: push_constant_layout(context, vk::ShaderStageFlags::VERTEX, 80, &[])?,
        shadow_point_model: push_constant_layout(context, vf, 128, &[])?,
        shadow_point_voxel: push_constant_layout(context, vf, 96, &[])?,
    })
}

/// Creates the per-frame command buffers and sync objects.
fn create_frames(context: &VulkanRenderingContext, count: usize) -> Result<Vec<VulkanFrame>> {
    unsafe {
        let command_pool = context.device.create_command_pool(
            &vk::CommandPoolCreateInfo::default()
                .queue_family_index(context.queue_families.graphics)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
            None,
        )?;

        let command_buffers = context.device.allocate_command_buffers(
            &vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::PRIMARY)
                .command_buffer_count(count as u32),
        )?;

        let mut frames = Vec::with_capacity(command_buffers.len());
        for &command_buffer in &command_buffers {
            frames.push(VulkanFrame {
                command_buffer,
                image_available_semaphore: context
                    .device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                render_finished_semaphore: context
                    .device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
                in_flight_fence: context.device.create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )?,
            });
        }
        Ok(frames)
    }
}

/// Creates shadow map sampler.
fn create_shadow_sampler(
    context: &VulkanRenderingContext,
    clamp_to_border: bool,
) -> Result<vk::Sampler> {
    let address_mode = if clamp_to_border {
        vk::SamplerAddressMode::CLAMP_TO_BORDER
    } else {
        vk::SamplerAddressMode::CLAMP_TO_EDGE
    };
    let info = vk::SamplerCreateInfo::default()
        .mag_filter(vk::Filter::LINEAR)
        .min_filter(vk::Filter::LINEAR)
        .address_mode_u(address_mode)
        .address_mode_v(address_mode)
        .address_mode_w(address_mode)
        .border_color(vk::BorderColor::FLOAT_OPAQUE_WHITE)
        .compare_enable(true)
        .compare_op(vk::CompareOp::LESS_OR_EQUAL);
    Ok(unsafe { context.device.create_sampler(&info, None)? })
}

/// white texture used when a mesh has no material.
fn create_default_white_material(
    context: &VulkanRenderingContext,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Result<GpuMaterial> {
    let white = upload_texture_from_pixels(
        &[255, 255, 255, 255],
        1,
        1,
        "default_white",
        vk::Format::R8G8B8A8_SRGB,
        context,
        context.command_pool,
        descriptor_pool,
        descriptor_set_layout,
    )?;
    // Flat tangent-space normal (0,0,1) → RGB (128,128,255), linear.
    let flat_normal = upload_texture_from_pixels(
        &[128, 128, 255, 255],
        1,
        1,
        "default_normal",
        vk::Format::R8G8B8A8_UNORM,
        context,
        context.command_pool,
        descriptor_pool,
        descriptor_set_layout,
    )?;

    let descriptor_set = context.create_material_descriptor_set(
        descriptor_pool,
        descriptor_set_layout,
        white.image_view,
        white.sampler,
        flat_normal.image_view,
        flat_normal.sampler,
    );

    Ok(GpuMaterial {
        albedo: Some(white),
        normal: Some(flat_normal),
        color: [1.0, 1.0, 1.0, 1.0],
        shader: None,
        descriptor_set,
    })
}

impl VulkanRenderer {
    /// Creates the renderer and all its GPU resources, then installs it on
    /// `rendering_info`. Pipelines are built by `rebuild_pipelines` at the end.
    pub(crate) fn initialize(
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

        let context = rendering_info.context.clone();

        // ============================== LIGHTS ==============================
        // Descriptors and pipeline layouts.
        let lights = create_light_resources(&context)?;

        // ============================== PIPELINES ==============================
        let (texture_set_layout, texture_pool) = create_texture_descriptor_resources(&context)?;
        let layouts =
            create_pipeline_layouts(&context, lights.descriptor_set_layout, texture_set_layout)?;

        // ============================== UI ==============================
        // Per-frame state and UI.
        let frames = create_frames(&context, IN_FLIGHT_FRAMES)?;
        let (ubo_buffer, ubo_memory) = context.create_buffer(
            256,
            vk::BufferUsageFlags::UNIFORM_BUFFER,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;
        let ui_renderer = UIRenderer::new(context.clone(), &swapchain, window)?;

        // ============================== VIEWPORT ==============================
        // Offscreen viewport, exposed to egui as a texture.
        let viewport_extent = swapchain.extent;
        let viewport_targets = create_viewport_targets(
            &context,
            viewport_extent,
            swapchain.format,
            swapchain.depth_format,
            aa_sample_count(aa_amount),
        )?;
        let viewport_descriptor_set = context.create_texture_descriptor_set(
            texture_pool,
            texture_set_layout,
            viewport_targets.color_view,
            viewport_targets.sampler,
        );
        let viewport_texture_id = ui_renderer
            .renderer
            .lock()
            .unwrap()
            .add_user_texture(viewport_descriptor_set);

        // ============================== SHADOWS ==============================
        // Shadow maps: CSM cascade array (binding 1) and point cubemap (binding 2).
        let csm_target = create_depth_array_target(
            &context,
            DEFAULT_SHADOW_MAP_SIZE,
            CSM_CASCADE_COUNT as u32,
            false,
        )?;
        let shadow_sampler = create_shadow_sampler(&context, true)?;
        write_shadow_descriptor(
            &context,
            lights.descriptor_set,
            1,
            csm_target.array_view,
            shadow_sampler,
        );

        let point_target = create_depth_array_target(&context, DEFAULT_SHADOW_MAP_SIZE, 6, true)?;
        let point_shadow_sampler = create_shadow_sampler(&context, false)?;
        write_shadow_descriptor(
            &context,
            lights.descriptor_set,
            2,
            point_target.array_view,
            point_shadow_sampler,
        );

        let default_white_material =
            create_default_white_material(&context, texture_pool, texture_set_layout)?;

        let mut renderer = VulkanRenderer {
            current_image_index: 0,
            in_flight_frames_count: IN_FLIGHT_FRAMES,
            current_frame: 0,
            frames,
            image_layouts: ImageLayouts::default(),

            pipeline_layout: layouts.model,
            voxel_pipeline_layout: layouts.voxel,
            water_pipeline_layout: layouts.water,
            voxel_descriptor_pool: texture_pool,
            voxel_descriptor_set_layout: texture_set_layout,

            default_white_material,

            ui_renderer,
            buffer_graveyard: Vec::new(),

            viewport_image: viewport_targets.color_image,
            viewport_image_memory: viewport_targets.color_memory,
            viewport_image_view: viewport_targets.color_view,
            msaa_color_image: viewport_targets.msaa_image,
            msaa_color_memory: viewport_targets.msaa_memory,
            msaa_color_view: viewport_targets.msaa_view,
            viewport_depth_image: viewport_targets.depth_image,
            viewport_depth_memory: viewport_targets.depth_memory,
            viewport_depth_view: viewport_targets.depth_view,
            viewport_sampler: viewport_targets.sampler,
            viewport_descriptor_set,
            viewport_texture_id,
            viewport_extent,
            viewport_target_initialized: false,
            viewport_depth_initialized: false,
            last_fence_wait_ns: 0,

            light_ssbo: lights.ssbo,
            light_ssbo_memory: lights.ssbo_memory,
            light_descriptor_pool: lights.descriptor_pool,
            light_descriptor_set_layout: lights.descriptor_set_layout,
            light_descriptor_set: lights.descriptor_set,

            shadow_image: csm_target.image,
            shadow_image_memory: csm_target.memory,
            shadow_image_view: csm_target.array_view,
            shadow_cascade_views: csm_target.layer_views.try_into().unwrap(),
            shadow_sampler,
            shadow_map_size: DEFAULT_SHADOW_MAP_SIZE,
            shadow_model_pipeline_layout: layouts.shadow_model,
            shadow_voxel_pipeline_layout: layouts.shadow_voxel,
            shadow_model_vertex_shader: "sdr_default_shadow_model.vert".to_string(),
            shadow_voxel_vertex_shader: "sdr_default_shadow_voxel.vert".to_string(),
            shadow_fragment_shader: "sdr_default_shadow.frag".to_string(),

            point_shadow_image: point_target.image,
            point_shadow_image_memory: point_target.memory,
            point_shadow_cube_view: point_target.array_view,
            point_shadow_face_views: point_target.layer_views.try_into().unwrap(),
            point_shadow_sampler,
            point_shadow_map_size: DEFAULT_SHADOW_MAP_SIZE,
            shadow_point_model_pipeline_layout: layouts.shadow_point_model,
            shadow_point_voxel_pipeline_layout: layouts.shadow_point_voxel,
            shadow_point_model_vertex_shader: "sdr_default_shadow_point_model.vert".to_string(),
            shadow_point_voxel_vertex_shader: "sdr_default_shadow_point_voxel.vert".to_string(),
            shadow_point_fragment_shader: "sdr_default_shadow_point.frag".to_string(),

            ubo: Ubo {
                buffer: ubo_buffer,
                memory: ubo_memory,
            },
            pipeline_manager: PipelineManager::new(),
            default_vertex_shader: rendering_info.settings.default_vertex_shader.clone(),
            default_fragment_shader: rendering_info.settings.default_fragment_shader.clone(),
            voxel_vertex_shader: "sdr_default_voxel.vert".to_string(),
            voxel_fragment_shader: "sdr_default_voxel.frag".to_string(),
            water_vertex_shader: "sdr_default_water.vert".to_string(),
            water_fragment_shader: "sdr_default_water.frag".to_string(),
            context: Arc::new(rendering_info.context.clone()),
            swapchain,

            aa_amount,
            ui_cached_primitives: Vec::new(),
            ui_pending_texture_frees: Vec::new(),
        };

        renderer.rebuild_pipelines(aa_amount)?;
        rendering_info.renderer = Some(Box::new(renderer));
        Ok(())
    }
}
