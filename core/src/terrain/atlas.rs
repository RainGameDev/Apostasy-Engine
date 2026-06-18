use anyhow::Result;
use apostasy_macros::Resource;
use ash::vk::{self};

use crate::{
    rendering::vulkan::rendering_context::VulkanRenderingContext,
    terrain::texture_library::TerrainTextureLibrary,
};

/// GPU texture array holding all terrain textures as separate layers.
/// The shader samples it as `sampler2DArray`.
#[derive(Resource, Clone, Debug)]
pub struct TerrainTextureAtlas {
    pub image: vk::Image,
    pub image_memory: vk::DeviceMemory,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub descriptor_set: vk::DescriptorSet,
    pub tile_size: u32,
    pub num_layers: u32,
}

/// Build and upload a GPU texture array from the CPU-side texture library.
pub fn build_terrain_atlas(
    ctx: &VulkanRenderingContext,
    command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    library: &TerrainTextureLibrary,
) -> Result<TerrainTextureAtlas> {
    let tile_size = 256u32;
    let count = library.count().max(1);
    let pixels_per_layer = (tile_size * tile_size * 4) as vk::DeviceSize;

    // --- Staging: copy each layer into its own staging buffer ---
    let mut staging_buffers: Vec<(vk::Buffer, vk::DeviceMemory)> = Vec::with_capacity(count as usize);

    for i in 0..count {
        let def = &library.textures[i as usize];
        let img = def.image.resize_exact(
            tile_size,
            tile_size,
            image::imageops::FilterType::Nearest,
        );
        let raw = img.to_rgba8();

        let (buf, mem) = ctx.create_buffer(
            pixels_per_layer,
            vk::BufferUsageFlags::TRANSFER_SRC,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            let ptr = ctx
                .device
                .map_memory(mem, 0, pixels_per_layer, vk::MemoryMapFlags::empty())?
                as *mut u8;
            ptr.copy_from_nonoverlapping(raw.as_raw().as_ptr(), raw.as_raw().len());
            ctx.device.unmap_memory(mem);
        }

        staging_buffers.push((buf, mem));
    }

    // --- Create the GPU 2D array image ---
    let (vk_image, image_memory) = create_terrain_array_image(
        ctx, tile_size, count,
    )?;

    // --- Transition to TRANSFER_DST and copy each layer ---
    let cmd = ctx.begin_single_time_commands(command_pool);

    // UNDEFINED → TRANSFER_DST_OPTIMAL
    unsafe {
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(vk_image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: count,
            })
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);

        ctx.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );

        // Copy each staging buffer into its array layer
        for (i, (buf, _mem)) in staging_buffers.iter().enumerate() {
            let region = vk::BufferImageCopy::default()
                .buffer_offset(0)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: i as u32,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(vk::Extent3D {
                    width: tile_size,
                    height: tile_size,
                    depth: 1,
                });

            ctx.device.cmd_copy_buffer_to_image(
                cmd,
                *buf,
                vk_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        // TRANSFER_DST → SHADER_READ_ONLY_OPTIMAL
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(vk_image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: count,
            })
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);

        ctx.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }

    let queue = ctx.queues[&ctx.queue_families.transfer];
    ctx.end_single_time_commands(cmd, queue, command_pool);

    // Clean up staging buffers
    for (buf, mem) in &staging_buffers {
        unsafe {
            ctx.device.destroy_buffer(*buf, None);
            ctx.device.free_memory(*mem, None);
        }
    }

    // --- 2D array image view ---
    let image_view = unsafe {
        ctx.device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(vk_image)
                .view_type(vk::ImageViewType::TYPE_2D_ARRAY)
                .format(vk::Format::R8G8B8A8_SRGB)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: count,
                }),
            None,
        )?
    };

    // --- Sampler ---
    let sampler = unsafe {
        ctx.device.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .anisotropy_enable(false)
                .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                .unnormalized_coordinates(false)
                .compare_enable(false)
                .mipmap_mode(vk::SamplerMipmapMode::LINEAR),
            None,
        )?
    };

    // --- Descriptor set ---
    let descriptor_set = ctx.create_texture_descriptor_set(
        descriptor_pool,
        descriptor_set_layout,
        image_view,
        sampler,
    );

    Ok(TerrainTextureAtlas {
        image: vk_image,
        image_memory,
        image_view,
        sampler,
        descriptor_set,
        tile_size,
        num_layers: count,
    })
}

fn create_terrain_array_image(
    ctx: &VulkanRenderingContext,
    tile_size: u32,
    num_layers: u32,
) -> Result<(vk::Image, vk::DeviceMemory)> {
    use ash::vk::*;
    let image_info = ImageCreateInfo::default()
        .image_type(ImageType::TYPE_2D)
        .extent(Extent3D {
            width: tile_size,
            height: tile_size,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(num_layers)
        .format(Format::R8G8B8A8_SRGB)
        .tiling(ImageTiling::OPTIMAL)
        .initial_layout(ImageLayout::UNDEFINED)
        .usage(ImageUsageFlags::TRANSFER_DST | ImageUsageFlags::SAMPLED)
        .samples(vk::SampleCountFlags::TYPE_1)
        .sharing_mode(SharingMode::EXCLUSIVE);

    let image = unsafe { ctx.device.create_image(&image_info, None)? };
    let mem_reqs = unsafe { ctx.device.get_image_memory_requirements(image) };

    let alloc_info = MemoryAllocateInfo::default()
        .allocation_size(mem_reqs.size)
        .memory_type_index(ctx.find_memory_type(
            mem_reqs.memory_type_bits,
            MemoryPropertyFlags::DEVICE_LOCAL,
        )?);

    let memory = unsafe { ctx.device.allocate_memory(&alloc_info, None)? };
    unsafe { ctx.device.bind_image_memory(image, memory, 0)? };

    Ok((image, memory))
}
