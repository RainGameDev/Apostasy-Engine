use std::sync::{Arc, Mutex};
use std::fs;

use crate::assets::shader_loader::load_shader_bytes;
use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ModelPushConstants, PushConstants, VoxelPushConstants,
};
use crate::rendering::vulkan::image_layout::ImageLayouts;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::rendering::vulkan::{frame::VulkanFrame, swapchain::VulkanSwapchain};
use crate::rendering::{RenderingAPI, RenderingInfo};
use crate::ui::UIRenderer;
use crate::voxels::texture_atlas::VoxelTextureAtlas;
use anyhow::Result;
use ash::vk::{
    self, ClearColorValue, CommandBufferResetFlags, CommandPool, Pipeline, PipelineLayout,
    PipelineLayoutCreateInfo,
};
use egui::{Context, TextureId};
use epaint::ImageDelta;
use winit::event::WindowEvent;
use winit::window::Window;

pub mod device;
pub mod frame;
pub mod image_layout;
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
    pub command_pool: CommandPool,
    pub image_layouts: ImageLayouts,

    pub pipeline: Pipeline,
    pub pipeline_layout: PipelineLayout,
    pub wireframe_pipeline: Pipeline,

    pub voxel_pipeline: Pipeline,
    pub voxel_wireframe_pipeline: Pipeline,
    pub voxel_pipeline_layout: PipelineLayout,
    pub voxel_descriptor_pool: vk::DescriptorPool,
    pub voxel_descriptor_set_layout: vk::DescriptorSetLayout,

    pub ui_renderer: UIRenderer,

    pub push_constants: PushConstants,

    pub ubo: Ubo,
    context: Arc<VulkanRenderingContext>,
}

fn load_shader_module(
    context: &Arc<VulkanRenderingContext>,
    path: &str,
) -> Result<ash::vk::ShaderModule> {
    let code = load_shader_bytes(path)?;
    Ok(context.create_shader_module(&code)?)
}

impl RenderingAPI for VulkanRenderer {
    fn new(rendering_info: Arc<Mutex<RenderingInfo>>, window: Arc<Window>) -> Result<()> {
        let mut rendering_info = rendering_info.lock().unwrap();
        let mut swapchain = VulkanSwapchain::new(
            rendering_info.context.clone().into(),
            rendering_info.window.clone(),
        )?;
        swapchain.resize()?;

        let vertex_shader = load_shader_module(
            &rendering_info.context.clone().into(),
            &rendering_info.settings.default_vertex_shader,
        )?;
        let fragment_shader = load_shader_module(
            &rendering_info.context.clone().into(),
            &rendering_info.settings.default_fragment_shader,
        )?;
        let voxel_vertex_shader =
            load_shader_module(&rendering_info.context.clone().into(), "voxel.vert")?;
        let voxel_fragment_shader =
            load_shader_module(&rendering_info.context.clone().into(), "voxel.frag")?;

        unsafe {
            let context = rendering_info.context.clone();
            let pipeline_layout = rendering_info.context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default().push_constant_ranges(&[
                    vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX)
                        .offset(0)
                        .size(176),
                ]),
                None,
            )?;

            let sampler_binding = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);

            let descriptor_set_layout = context.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::default().bindings(&[sampler_binding]),
                None,
            )?;

            let descriptor_pool = context.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::default()
                    .max_sets(200)
                    .pool_sizes(&[vk::DescriptorPoolSize {
                        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        descriptor_count: 100,
                    }]),
                None,
            )?;

            let pipeline = context.create_graphics_pipeline(
                vertex_shader,
                fragment_shader,
                swapchain.extent,
                swapchain.format,
                swapchain.depth_format,
                pipeline_layout,
                Default::default(),
            )?;
            let wireframe_pipeline = context.create_wireframe_pipeline(
                vertex_shader,
                fragment_shader,
                swapchain.extent,
                swapchain.format,
                swapchain.depth_format,
                pipeline_layout,
                Default::default(),
            )?;

            let voxel_pipeline_layout = context.device.create_pipeline_layout(
                &PipelineLayoutCreateInfo::default()
                    .push_constant_ranges(&[vk::PushConstantRange::default()
                        .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                        .offset(0)
                        .size(156)])
                    .set_layouts(&[descriptor_set_layout]),
                None,
            )?;

            let voxel_pipeline = context.create_voxel_graphics_pipeline(
                voxel_vertex_shader,
                voxel_fragment_shader,
                swapchain.extent,
                swapchain.format,
                swapchain.depth_format,
                voxel_pipeline_layout,
                Default::default(),
            )?;

            let voxel_wireframe_pipeline = context.create_voxel_wireframe_pipeline(
                voxel_vertex_shader,
                voxel_fragment_shader,
                swapchain.extent,
                swapchain.format,
                swapchain.depth_format,
                voxel_pipeline_layout,
                Default::default(),
            )?;

            context.device.destroy_shader_module(vertex_shader, None);
            context
                .device
                .destroy_shader_module(voxel_vertex_shader, None);
            context.device.destroy_shader_module(fragment_shader, None);
            context
                .device
                .destroy_shader_module(voxel_fragment_shader, None);

            let command_pool = context.device.create_command_pool(
                &ash::vk::CommandPoolCreateInfo::default()
                    .queue_family_index(context.queue_families.graphics)
                    .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?;

            let in_flight_frames_count = 1;

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

            let renderer = VulkanRenderer {
                current_image_index: 0,
                in_flight_frames_count,
                current_frame: 0,
                frames,
                command_pool,
                image_layouts: ImageLayouts::default(),

                pipeline,
                wireframe_pipeline,
                pipeline_layout,
                voxel_pipeline_layout,

                ui_renderer,

                voxel_pipeline,
                voxel_wireframe_pipeline,
                voxel_descriptor_pool: descriptor_pool,
                voxel_descriptor_set_layout: descriptor_set_layout,

                push_constants: PushConstants::default(),
                ubo,
                context: Arc::new(rendering_info.context.clone()),
                swapchain,
            };

            rendering_info.renderer = Some(Box::new(renderer));
        }

        Ok(())
    }

    fn begin_frame(&mut self, _push_constants: PushConstants) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        
        // Recreate swapchain if it was marked dirty
        if self.swapchain.is_dirty {
            if let Err(e) = self.swapchain.resize() {
                eprintln!("Failed to recreate swapchain: {}", e);
                return Err(anyhow::anyhow!("Failed to recreate swapchain: {}", e));
            }
        }
        
        unsafe {
            // Use a 5-second timeout instead of infinite to prevent device hangs
            const FENCE_TIMEOUT_NS: u64 = 5_000_000_000; // 5 seconds in nanoseconds
            
            match self.context
                .device
                .wait_for_fences(&[frame.in_flight_fence], true, FENCE_TIMEOUT_NS) {
                Ok(()) => {},
                Err(e) => {
                    eprintln!("Fence wait failed (likely device timeout): {}", e);
                    // Reset the device state and try to recover
                    let _ = self.context.device.device_wait_idle();
                    return Err(anyhow::anyhow!("Device timeout during frame wait"));
                }
            }
            
            if let Err(e) = self.context.device.reset_fences(&[frame.in_flight_fence]) {
                eprintln!("Failed to reset fences: {}", e);
                return Err(anyhow::anyhow!("Failed to reset in-flight fence: {}", e));
            }
            
            if let Err(e) = self.context
                .device
                .reset_command_buffer(frame.command_buffer, CommandBufferResetFlags::empty()) {
                eprintln!("Failed to reset command buffer: {}", e);
                return Err(anyhow::anyhow!("Failed to reset command buffer: {}", e));
            }

            // Attempt to acquire next image; if device is lost, try to recover by recreating swapchain
            match self.swapchain.acquire_next_image(frame.image_available_semaphore) {
                Ok(index) => self.current_image_index = index,
                Err(e) => {
                    eprintln!("Failed to acquire next image: {}", e);
                    // Try device wait idle first
                    if let Err(idle_err) = self.context.device.device_wait_idle() {
                        eprintln!("Device wait idle failed: {}", idle_err);
                        return Err(anyhow::anyhow!("Device lost during acquire: {}", idle_err));
                    }
                    // Try to recover by recreating the swapchain
                    if let Err(resize_err) = self.swapchain.resize() {
                        eprintln!("Failed to recreate swapchain during recovery: {}", resize_err);
                        return Err(anyhow::anyhow!("Failed to recover swapchain: {}", resize_err));
                    }
                    // After swapchain recreation, try acquiring again
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
                self.image_layouts.undefined,
                self.image_layouts.renderable,
                vk::ImageAspectFlags::COLOR,
            );
            self.context.transition_image_layout(
                frame.command_buffer,
                self.swapchain.depth_image,
                self.image_layouts.undefined,
                self.image_layouts.depth,
                vk::ImageAspectFlags::DEPTH,
            );

            self.context.begin_rendering(
                frame.command_buffer,
                self.swapchain.views[self.current_image_index as usize],
                self.swapchain.depth_image_view,
                ClearColorValue {
                    float32: [0.0, 0.2, 0.8, 1.0],
                },
                vk::Rect2D::default().extent(self.swapchain.extent),
            );

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
                self.context.queues[self.context.queue_families.graphics as usize],
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

            if let Err(e) = self.swapchain
                .present_image(self.current_image_index, frame.render_finished_semaphore) {
                eprintln!("Failed to present image: {}", e);
                // Mark swapchain as dirty for recreation
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
    ) -> anyhow::Result<()> {
        let frame = &self.frames[self.current_frame];

        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );

            let mut data = push_constants.return_renderable();
            data.extend(model_push_constants.return_renderable());
            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.pipeline_layout,
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
    fn wireframe_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
    ) -> anyhow::Result<()> {
        let frame = &self.frames[self.current_frame];

        let mut data = push_constants.return_renderable();
        data.extend(model_push_constants.return_renderable());

        unsafe {
            self.context.device.cmd_bind_pipeline(
                frame.command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.wireframe_pipeline,
            );

            self.context.device.cmd_push_constants(
                frame.command_buffer,
                self.pipeline_layout,
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

    fn voxel_render(
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
                self.voxel_pipeline,
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
        let raw_input = self
            .ui_renderer
            .state
            .take_egui_input(&self.ui_renderer.window);
        self.ui_renderer.context.begin_pass(raw_input);
    }

    fn end_ui(&mut self) -> Result<()> {
        let full_output = self.ui_renderer.context.end_pass();

        self.ui_renderer
            .state
            .handle_platform_output(&self.ui_renderer.window, full_output.platform_output);

        let clipped_primitives = self
            .ui_renderer
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let texture_updates: Vec<(TextureId, ImageDelta)> = full_output
            .textures_delta
            .set
            .iter()
            .map(|(id, delta)| (*id, delta.clone()))
            .collect();

        if !texture_updates.is_empty() {
            self.ui_renderer.renderer.set_textures(
                self.context.queues[self.context.queue_families.graphics as usize],
                self.command_pool,
                &texture_updates,
            )?;
        }

        self.ui_renderer.renderer.cmd_draw(
            self.frames[self.current_frame].command_buffer,
            self.swapchain.extent,
            full_output.pixels_per_point,
            &clipped_primitives,
        )?;

        Ok(())
    }

    fn handle_ui_event(&mut self, event: &WindowEvent) -> bool {
        self.ui_renderer
            .state
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

    fn get_command_pool(&self) -> Result<CommandPool> {
        Ok(self.command_pool)
    }

    fn get_aspect(&self) -> f32 {
        self.swapchain.extent.width as f32 / self.swapchain.extent.height as f32
    }

    fn get_descriptor_pool(&self) -> vk::DescriptorPool {
        self.voxel_descriptor_pool
    }
    fn get_voxel_descriptor_set_layout(&self) -> vk::DescriptorSetLayout {
        self.voxel_descriptor_set_layout
    }
}
