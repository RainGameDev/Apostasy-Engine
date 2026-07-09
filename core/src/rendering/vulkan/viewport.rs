use anyhow::Result;
use ash::vk::{self, ClearColorValue, ImageView, SampleCountFlags};

use crate::rendering::shared::anti_aliasing::AntiAliasingAmount;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::rendering::vulkan::{VulkanRenderer, aa_sample_count};

/// The offscreen images the viewport is rendered into: an MSAA color target,
/// its single-sample resolve target (sampled by egui), and a depth buffer.
pub(crate) struct ViewportTargets {
    pub color_image: vk::Image,
    pub color_memory: vk::DeviceMemory,
    pub color_view: vk::ImageView,
    pub msaa_image: vk::Image,
    pub msaa_memory: vk::DeviceMemory,
    pub msaa_view: vk::ImageView,
    pub depth_image: vk::Image,
    pub depth_memory: vk::DeviceMemory,
    pub depth_view: vk::ImageView,
    pub sampler: vk::Sampler,
}

pub(crate) fn create_viewport_targets(
    context: &VulkanRenderingContext,
    extent: vk::Extent2D,
    color_format: vk::Format,
    depth_format: vk::Format,
    aa_samples: SampleCountFlags,
) -> Result<ViewportTargets> {
    // Resolve target is always single-sample so egui can sample it.
    let (color_image, color_memory) = context.create_image(
        extent,
        color_format,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        SampleCountFlags::TYPE_1,
    )?;
    let color_view =
        context.create_image_view(color_image, color_format, vk::ImageAspectFlags::COLOR)?;

    let (msaa_image, msaa_memory) = context.create_image(
        extent,
        color_format,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::TRANSIENT_ATTACHMENT | vk::ImageUsageFlags::COLOR_ATTACHMENT,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        aa_samples,
    )?;
    let msaa_view =
        context.create_image_view(msaa_image, color_format, vk::ImageAspectFlags::COLOR)?;

    let (depth_image, depth_memory) = context.create_image(
        extent,
        depth_format,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        aa_samples,
    )?;
    let depth_view =
        context.create_image_view(depth_image, depth_format, vk::ImageAspectFlags::DEPTH)?;

    let sampler = unsafe {
        context.device.create_sampler(
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

    Ok(ViewportTargets {
        color_image,
        color_memory,
        color_view,
        msaa_image,
        msaa_memory,
        msaa_view,
        depth_image,
        depth_memory,
        depth_view,
        sampler,
    })
}

impl VulkanRenderer {
    fn apply_viewport_targets(&mut self, targets: ViewportTargets, extent: vk::Extent2D) {
        self.viewport_image = targets.color_image;
        self.viewport_image_memory = targets.color_memory;
        self.viewport_image_view = targets.color_view;
        self.msaa_color_image = targets.msaa_image;
        self.msaa_color_memory = targets.msaa_memory;
        self.msaa_color_view = targets.msaa_view;
        self.viewport_depth_image = targets.depth_image;
        self.viewport_depth_memory = targets.depth_memory;
        self.viewport_depth_view = targets.depth_view;
        self.viewport_sampler = targets.sampler;
        self.viewport_extent = extent;
        self.viewport_target_initialized = false;
        self.viewport_depth_initialized = false;
    }

    unsafe fn destroy_viewport_targets(&mut self) {
        unsafe {
            self.context
                .device
                .destroy_image_view(self.msaa_color_view, None);
            self.context
                .device
                .destroy_image(self.msaa_color_image, None);
            self.context
                .device
                .free_memory(self.msaa_color_memory, None);

            self.context
                .device
                .destroy_image_view(self.viewport_image_view, None);
            self.context.device.destroy_image(self.viewport_image, None);
            self.context
                .device
                .free_memory(self.viewport_image_memory, None);

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

        unsafe { self.context.device.device_wait_idle()? };

        // Rebuild pipelines only if MSAA sample count changed
        if aa_changed {
            self.rebuild_pipelines(aa_amount)?;
        }

        unsafe { self.destroy_viewport_targets() };

        let targets = create_viewport_targets(
            &self.context,
            new_extent,
            self.swapchain.format,
            self.swapchain.depth_format,
            aa_sample_count(aa_amount),
        )?;

        // Write the new resolve target into the existing descriptor set
        unsafe {
            let image_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(targets.color_view)
                .sampler(targets.sampler);

            self.context.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(self.viewport_descriptor_set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[image_info])],
                &[],
            );
        }

        self.apply_viewport_targets(targets, new_extent);
        Ok(())
    }

    pub(crate) fn begin_viewport_render(&mut self) -> Result<()> {
        let frame = &self.frames[self.current_frame];

        let old_color_layout = if self.viewport_target_initialized {
            self.image_layouts.shader_read_only
        } else {
            self.image_layouts.undefined
        };

        self.context.transition_image_layout(
            frame.command_buffer,
            self.viewport_image,
            old_color_layout,
            self.image_layouts.renderable,
            vk::ImageAspectFlags::COLOR,
        );

        if self.aa_amount != AntiAliasingAmount::X0 {
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
        }

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

        // With MSAA: msaa_color_view → render target; viewport_image_view → resolve target.
        // Without: render straight into viewport_image_view.
        let (render_target, resolve_target) = if self.aa_amount == AntiAliasingAmount::X0 {
            (self.viewport_image_view, ImageView::null())
        } else {
            (self.msaa_color_view, self.viewport_image_view)
        };

        self.context.begin_rendering(
            frame.command_buffer,
            render_target,
            resolve_target,
            self.viewport_depth_view,
            ClearColorValue {
                float32: [0.05, 0.05, 0.05, 1.0],
            },
            vk::Rect2D::default().extent(self.viewport_extent),
        );

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

    pub(crate) fn end_viewport_render(&mut self) -> Result<()> {
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
}
