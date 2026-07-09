use anyhow::Result;
use ash::vk::{self, ClearColorValue, CommandBufferResetFlags, ImageView};

use crate::rendering::vulkan::VulkanRenderer;

use ash::vk::{CommandBuffer, Fence, Semaphore};

#[derive(Clone)]
pub struct VulkanFrame {
    pub command_buffer: CommandBuffer,
    pub image_available_semaphore: Semaphore,
    pub render_finished_semaphore: Semaphore,
    pub in_flight_fence: Fence,
}

impl VulkanRenderer {
    pub(crate) fn begin_frame(&mut self) -> Result<()> {
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

    pub(crate) fn begin_swapchain_render(&mut self) -> Result<()> {
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

    pub(crate) fn end_frame(&mut self) -> Result<()> {
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
}
