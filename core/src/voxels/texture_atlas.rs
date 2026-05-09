use std::path::Path;

use anyhow::Result;
use ash::vk;
use hashbrown::HashMap;
use image::{DynamicImage, GenericImageView, RgbaImage};

use apostasy_macros::Resource;

use crate::{log_warn, rendering::vulkan::rendering_context::VulkanRenderingContext};

#[derive(Resource, Clone)]
pub struct PendingAtlas {
    pub image: RgbaImage,
    pub tiles: u32,
}

#[derive(Resource, Clone, Debug)]
pub struct VoxelTextureAtlas {
    pub image: vk::Image,
    pub image_memory: vk::DeviceMemory,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub texture_index: HashMap<String, u32>,
    pub texture_size: u32,
    pub atlas_size: u32,
    pub descriptor_set: vk::DescriptorSet,
}

#[derive(Debug)]
pub struct AtlasBuilder {
    pub tile_size: u32,
    pub tiles: Vec<(String, DynamicImage)>,
}

impl AtlasBuilder {
    pub fn new(tile_size: u32) -> Self {
        Self {
            tile_size,
            tiles: Vec::new(),
        }
    }

    pub fn add_texture(&mut self, path: &str) -> u32 {
        if let Some(idx) = self.tiles.iter().position(|(p, _)| p == path) {
            return idx as u32;
        }

        // check game res/ first
        let game_path = Path::new("res/").join(path);
        // fall back to core res/
        let core_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("res/")
            .join(path);

        let img = if game_path.exists() {
            image::open(game_path).expect("Failed to load texture")
        } else if core_path.exists() {
            image::open(core_path).expect("Failed to load texture")
        } else {
            log_warn!("Texture not found: {}, using missing texture", path);
            // fall back to missing texture at slot 0
            return 0;
        };

        let idx = self.tiles.len() as u32;
        self.tiles.push((path.to_string(), img));
        idx
    }

    pub fn build(&self) -> (RgbaImage, u32) {
        let count = self.tiles.len() as u32;
        let atlas_tiles = (count as f32).sqrt().ceil() as u32;
        let atlas_px = atlas_tiles * self.tile_size;
        let count = self.tiles.len() as u32;
        println!("Atlas builder has {} tiles", count);
        let atlas_tiles = (count as f32).sqrt().ceil() as u32;
        println!("Atlas tiles grid: {}", atlas_tiles);

        let mut atlas = RgbaImage::new(atlas_px, atlas_px);

        for (i, (_, img)) in self.tiles.iter().enumerate() {
            let tx = (i as u32 % atlas_tiles) * self.tile_size;
            let ty = (i as u32 / atlas_tiles) * self.tile_size;
            let resized = img.resize_exact(
                self.tile_size,
                self.tile_size,
                image::imageops::FilterType::Nearest,
            );
            for (px, py, pixel) in resized.pixels() {
                atlas.put_pixel(tx + px, ty + py, pixel);
            }
        }

        (atlas, atlas_tiles)
    }
}

pub fn upload_atlas(
    ctx: &VulkanRenderingContext,
    command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    image: &RgbaImage,
    tiles: u32,
) -> Result<VoxelTextureAtlas> {
    let width = image.width();
    let height = image.height();

    if width == 0 || height == 0 || tiles == 0 {
        return Err(anyhow::anyhow!("Cannot upload empty texture atlas"));
    }

    let pixels = image.as_raw();
    let size = pixels.len() as vk::DeviceSize;

    // staging buffer
    let (staging_buffer, staging_memory) = ctx.create_buffer(
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    // copy pixels into staging buffer
    unsafe {
        let ptr = ctx
            .device
            .map_memory(staging_memory, 0, size, vk::MemoryMapFlags::empty())?
            as *mut u8;
        ptr.copy_from_nonoverlapping(pixels.as_ptr(), pixels.len());
        ctx.device.unmap_memory(staging_memory);
    }

    // create GPU image
    let (vk_image, image_memory) = ctx.create_image(
        vk::Extent2D { width, height },
        vk::Format::R8G8B8A8_SRGB,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;

    let cmd = ctx.begin_single_time_commands(command_pool);

    unsafe {
        // transition to transfer dst
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
                layer_count: 1,
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

        // copy buffer to image
        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D {
                width,
                height,
                depth: 1,
            });

        ctx.device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            vk_image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );

        // transition to shader read
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
                layer_count: 1,
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

    let queue = ctx.queues[ctx.queue_families.transfer as usize];
    ctx.end_single_time_commands(cmd, queue, command_pool);

    // cleanup staging
    unsafe {
        ctx.device.destroy_buffer(staging_buffer, None);
        ctx.device.free_memory(staging_memory, None);
    }

    // image view
    let image_view = ctx.create_image_view(
        vk_image,
        vk::Format::R8G8B8A8_SRGB,
        vk::ImageAspectFlags::COLOR,
    )?;

    let sampler = unsafe {
        ctx.device.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::NEAREST)
                .min_filter(vk::Filter::NEAREST)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .anisotropy_enable(false)
                .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
                .unnormalized_coordinates(false)
                .compare_enable(false)
                .mipmap_mode(vk::SamplerMipmapMode::NEAREST),
            None,
        )?
    };

    let descriptor_set = ctx.create_texture_descriptor_set(
        descriptor_pool,
        descriptor_set_layout,
        image_view,
        sampler,
    );

    Ok(VoxelTextureAtlas {
        image: vk_image,
        image_memory,
        image_view,
        sampler,
        texture_index: HashMap::new(),
        texture_size: width / tiles,
        atlas_size: tiles,
        descriptor_set,
    })
}
