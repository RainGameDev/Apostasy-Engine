use anyhow::Result;
use ash::vk::{self, SampleCountFlags};

use crate::rendering::lighting::gpu_light::CSM_CASCADE_COUNT;
use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ShadowModelPushConstants, ShadowVoxelPushConstants,
};
use crate::rendering::vulkan::VulkanRenderer;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;

pub(crate) const DEFAULT_SHADOW_MAP_SIZE: u32 = 2048;

/// A depth-only layered image (CSM cascade array or point-light cubemap) with a
/// full-array view for sampling and one view per layer for rendering.
pub(crate) struct DepthArrayTarget {
    pub image: vk::Image,
    pub memory: vk::DeviceMemory,
    pub array_view: vk::ImageView,
    pub layer_views: Vec<vk::ImageView>,
}

/// Creates a D32 layered depth target and transitions all layers to
/// DEPTH_STENCIL_READ_ONLY_OPTIMAL so it can be sampled before the first pass.
pub(crate) fn create_depth_array_target(
    context: &VulkanRenderingContext,
    size: u32,
    layers: u32,
    cube: bool,
) -> Result<DepthArrayTarget> {
    unsafe {
        let image_info = vk::ImageCreateInfo::default()
            .flags(if cube {
                vk::ImageCreateFlags::CUBE_COMPATIBLE
            } else {
                vk::ImageCreateFlags::empty()
            })
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D {
                width: size,
                height: size,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(layers)
            .format(vk::Format::D32_SFLOAT)
            .tiling(vk::ImageTiling::OPTIMAL)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::SAMPLED)
            .samples(SampleCountFlags::TYPE_1)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let image = context.device.create_image(&image_info, None)?;
        let mem_reqs = context.device.get_image_memory_requirements(image);
        let memory = context.device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(mem_reqs.size)
                .memory_type_index(context.find_memory_type(
                    mem_reqs.memory_type_bits,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )?),
            None,
        )?;
        context.device.bind_image_memory(image, memory, 0)?;

        let all_layers = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::DEPTH)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(layers);

        let array_view = context.device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(if cube {
                    vk::ImageViewType::CUBE
                } else {
                    vk::ImageViewType::TYPE_2D_ARRAY
                })
                .format(vk::Format::D32_SFLOAT)
                .subresource_range(all_layers),
            None,
        )?;

        let mut layer_views = Vec::with_capacity(layers as usize);
        for i in 0..layers {
            layer_views.push(
                context.device.create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(vk::Format::D32_SFLOAT)
                        .subresource_range(
                            vk::ImageSubresourceRange::default()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .base_mip_level(0)
                                .level_count(1)
                                .base_array_layer(i)
                                .layer_count(1),
                        ),
                    None,
                )?,
            );
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
                .image(image)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::SHADER_READ)
                .subresource_range(all_layers)],
        );
        context.end_single_time_commands(
            init_cmd,
            context.queues[&context.queue_families.graphics],
            context.command_pool,
        );

        Ok(DepthArrayTarget {
            image,
            memory,
            array_view,
            layer_views,
        })
    }
}

/// Writes a shadow map view + comparison sampler into the light descriptor set.
pub(crate) fn write_shadow_descriptor(
    context: &VulkanRenderingContext,
    set: vk::DescriptorSet,
    binding: u32,
    view: vk::ImageView,
    sampler: vk::Sampler,
) {
    let image_info = vk::DescriptorImageInfo::default()
        .image_layout(vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL)
        .image_view(view)
        .sampler(sampler);
    unsafe {
        context.device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::default()
                .dst_set(set)
                .dst_binding(binding)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&[image_info])],
            &[],
        );
    }
}

/// Transitions all layers of a shadow image between shader-readable and
/// depth-attachment layouts (`to_write = true` before rendering, `false` after).
pub(crate) fn transition_depth_layers(
    context: &VulkanRenderingContext,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    layers: u32,
    to_write: bool,
) {
    let attachment_access = vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
        | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
    let fragment_tests =
        vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS;

    let (src_stage, dst_stage, old_layout, new_layout, src_access, dst_access) = if to_write {
        (
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            fragment_tests,
            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            vk::AccessFlags::SHADER_READ,
            attachment_access,
        )
    } else {
        (
            fragment_tests,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            attachment_access,
            vk::AccessFlags::SHADER_READ,
        )
    };

    unsafe {
        context.device.cmd_pipeline_barrier(
            cmd,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier::default()
                .old_layout(old_layout)
                .new_layout(new_layout)
                .image(image)
                .src_access_mask(src_access)
                .dst_access_mask(dst_access)
                .subresource_range(
                    vk::ImageSubresourceRange::default()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .base_mip_level(0)
                        .level_count(1)
                        .base_array_layer(0)
                        .layer_count(layers),
                )],
        );
    }
}

impl VulkanRenderer {
    /// Sets the depth bias, viewport and scissor for a depth-only shadow pass.
    pub(crate) fn set_shadow_pass_state(&self, size: u32, bias_constant: f32, bias_slope: f32) {
        let cmd = self.cmd();
        let extent = vk::Extent2D {
            width: size,
            height: size,
        };
        unsafe {
            // Hardware slope-scaled depth bias - eliminates shadow acne without Peter Panning.
            // bias_constant = constant depth offset (in depth units).
            // bias_slope    = multiplied by max depth slope of each polygon.
            self.context
                .device
                .cmd_set_depth_bias(cmd, bias_constant, 0.0, bias_slope);
            self.context.device.cmd_set_viewport(
                cmd,
                0,
                &[vk::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: size as f32,
                    height: size as f32,
                    min_depth: 0.0,
                    max_depth: 1.0,
                }],
            );
            self.context.device.cmd_set_scissor(
                cmd,
                0,
                &[vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent,
                }],
            );
        }
    }

    pub(crate) fn rebuild_shadow_map(&mut self, size: u32) -> Result<()> {
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
        }

        let target =
            create_depth_array_target(&self.context, size, CSM_CASCADE_COUNT as u32, false)?;
        write_shadow_descriptor(
            &self.context,
            self.light_descriptor_set,
            1,
            target.array_view,
            self.shadow_sampler,
        );

        self.shadow_image = target.image;
        self.shadow_image_memory = target.memory;
        self.shadow_image_view = target.array_view;
        self.shadow_cascade_views = target.layer_views.try_into().unwrap();
        self.shadow_map_size = size;
        Ok(())
    }

    pub(crate) fn begin_shadow_pass(
        &mut self,
        cascade_index: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()> {
        // On the first cascade, transition all layers from read-only to depth-write at once.
        if cascade_index == 0 {
            transition_depth_layers(
                &self.context,
                self.cmd(),
                self.shadow_image,
                CSM_CASCADE_COUNT as u32,
                true,
            );
        }

        // Render into the per-layer view for this cascade.
        self.context.begin_depth_only_rendering(
            self.cmd(),
            self.shadow_cascade_views[cascade_index],
            vk::Rect2D::default().extent(vk::Extent2D {
                width: self.shadow_map_size,
                height: self.shadow_map_size,
            }),
        );

        self.set_shadow_pass_state(self.shadow_map_size, bias_constant, bias_slope);
        Ok(())
    }

    pub(crate) fn end_shadow_pass(&mut self, _cascade_index: usize) -> Result<()> {
        unsafe {
            self.context.device.cmd_end_rendering(self.cmd());
        }

        // Transition all cascade layers back to shader-readable after each pass.
        // For directional CSM the next cascade's begin_shadow_pass will handle the
        // forward barrier; for spot lights (single pass) this restores the layout correctly.
        transition_depth_layers(
            &self.context,
            self.cmd(),
            self.shadow_image,
            CSM_CASCADE_COUNT as u32,
            false,
        );
        Ok(())
    }

    pub(crate) fn shadow_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowModelPushConstants,
    ) -> Result<()> {
        self.depth_pass_draw(
            "shadow_model",
            self.shadow_model_pipeline_layout,
            vk::ShaderStageFlags::VERTEX,
            &pc.return_renderable(),
            &*mesh,
        );
        Ok(())
    }

    pub(crate) fn shadow_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowVoxelPushConstants,
    ) -> Result<()> {
        self.depth_pass_draw(
            "shadow_voxel",
            self.shadow_voxel_pipeline_layout,
            vk::ShaderStageFlags::VERTEX,
            &pc.return_renderable(),
            &*mesh,
        );
        Ok(())
    }
}
