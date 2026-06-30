use std::path::Path;

use anyhow::Result;
use ash::vk::{self, SampleCountFlags};
use image::DynamicImage;
use apostasy_macros::Resource;

use crate::{
    log_warn,
    rendering::vulkan::rendering_context::VulkanRenderingContext,
};

/// Marker resource: set this to signal the render loop to rebuild the terrain atlas.
#[derive(Resource, Clone, Default)]
pub struct TerrainAtlasNeedsRebuild;

/// A 2D texture array holding terrain layer textures.
#[derive(Resource, Clone, Debug)]
pub struct TerrainTextureAtlas {
    pub image: vk::Image,
    pub image_memory: vk::DeviceMemory,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub layer_count: u32,
    pub tile_size: u32,
    pub descriptor_set: vk::DescriptorSet,
}

/// Loads terrain textures from paths and uploads them as a 2D array texture.
pub fn upload_terrain_textures(
    ctx: &VulkanRenderingContext,
    command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    texture_paths: &[String],
    tile_size: u32,
) -> Result<TerrainTextureAtlas> {
    let layer_count = texture_paths.len().min(64).max(1) as u32;
    let tile_size = tile_size.max(1);

    // Load each texture, resizing to tile_size x tile_size
    let mut layers: Vec<image::RgbaImage> = Vec::with_capacity(layer_count as usize);
    for path in texture_paths.iter().take(layer_count as usize) {
        let img = load_terrain_texture(path);
        let resized = img.resize_exact(tile_size, tile_size, image::imageops::FilterType::Triangle);
        let rgba = resized.to_rgba8();
        layers.push(rgba);
    }

    // Pad to at least 1 layer
    if layers.is_empty() {
        let mut blank = image::RgbaImage::new(tile_size, tile_size);
        for pixel in blank.pixels_mut() {
            *pixel = image::Rgba([128, 128, 128, 255]);
        }
        layers.push(blank);
    }

    let width = tile_size;
    let height = tile_size;
    let layer_bytes = (width * height * 4) as vk::DeviceSize;
    let total_bytes = layer_bytes * layer_count as vk::DeviceSize;

    // Staging buffer: enough for all layers
    let (staging_buffer, staging_memory) = ctx.create_buffer(
        total_bytes,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    unsafe {
        let ptr = ctx
            .device
            .map_memory(staging_memory, 0, total_bytes, vk::MemoryMapFlags::empty())?
            as *mut u8;
        let mut offset = 0usize;
        for layer in &layers {
            let raw = layer.as_raw();
            ptr.add(offset).copy_from_nonoverlapping(raw.as_ptr(), raw.len());
            offset += raw.len();
        }
        ctx.device.unmap_memory(staging_memory);
    }

    // Create 2D array image on GPU
    let extent = vk::Extent3D {
        width,
        height,
        depth: 1,
    };
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .extent(extent)
        .mip_levels(1)
        .array_layers(layer_count)
        .format(vk::Format::R8G8B8A8_SRGB)
        .tiling(vk::ImageTiling::OPTIMAL)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .samples(SampleCountFlags::TYPE_1)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let vk_image = unsafe { ctx.device.create_image(&image_info, None)? };
    let mem_reqs = unsafe { ctx.device.get_image_memory_requirements(vk_image) };
    let image_memory = unsafe {
        ctx.device.allocate_memory(
            &vk::MemoryAllocateInfo::default()
                .allocation_size(mem_reqs.size)
                .memory_type_index(ctx.find_memory_type(
                    mem_reqs.memory_type_bits,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )?),
            None,
        )?
    };
    unsafe { ctx.device.bind_image_memory(vk_image, image_memory, 0)? };

    // Transition to TRANSFER_DST, copy each layer, transition to SHADER_READ_ONLY
    let cmd = ctx.begin_single_time_commands(command_pool);

    unsafe {
        // UNDEFINED -> TRANSFER_DST_OPTIMAL
        let pre_barrier = vk::ImageMemoryBarrier::default()
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
                layer_count,
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
            &[pre_barrier],
        );

        // Copy each layer from staging buffer
        for i in 0..layer_count {
            let region = vk::BufferImageCopy::default()
                .buffer_offset(i as vk::DeviceSize * layer_bytes)
                .buffer_row_length(0)
                .buffer_image_height(0)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: i,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
                .image_extent(extent);

            ctx.device.cmd_copy_buffer_to_image(
                cmd,
                staging_buffer,
                vk_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        // TRANSFER_DST -> SHADER_READ_ONLY_OPTIMAL
        let post_barrier = vk::ImageMemoryBarrier::default()
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
                layer_count,
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
            &[post_barrier],
        );
    }

    let queue = ctx.queues[&ctx.queue_families.transfer];
    ctx.end_single_time_commands(cmd, queue, command_pool);

    unsafe {
        ctx.device.destroy_buffer(staging_buffer, None);
        ctx.device.free_memory(staging_memory, None);
    }

    // 2D array image view
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
                    layer_count,
                }),
            None,
        )?
    };

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
        layer_count,
        tile_size,
        descriptor_set,
    })
}

/// Scans a directory for image files (png, jpg, jpeg, tga, bmp, hdr).
/// Returns full absolute paths sorted alphabetically.
/// Useful for auto-discovering terrain textures from a `textures/terrain` folder.
pub fn discover_terrain_textures(dir: &Path) -> Vec<String> {
    const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "tga", "bmp", "hdr"];
    let mut paths: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if IMAGE_EXTS.contains(&ext.as_str()) {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    paths.sort();
    paths
}

pub fn load_terrain_texture(path: &str) -> DynamicImage {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = vec![
        manifest.join("../project").join(path),
        Path::new("res/").join(path),
        manifest.join("res/").join(path),
        manifest.join("../game/res/").join(path),
        manifest.join("../editor/res/").join(path),
    ];
    // Also try the path as-is (for absolute paths from drag-drop or typed paths)
    candidates.push(Path::new(path).to_path_buf());

    for candidate in &candidates {
        if candidate.exists() {
            if let Ok(img) = image::open(candidate) {
                return img;
            }
        }
    }

    log_warn!("Terrain texture not found: {}", path);
    let mut fallback = image::RgbaImage::new(1, 1);
    fallback.put_pixel(0, 0, image::Rgba([128, 128, 128, 255]));
    DynamicImage::ImageRgba8(fallback)
}
