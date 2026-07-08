use std::sync::Arc;

use anyhow::Result;
use apostasy_macros::Component;
use ash::vk::{self, SampleCountFlags};

use crate::{
    ecs::component::Inspect,
    egui::{self, DragAndDrop, StrokeKind},
    rendering::shared::{model::Mesh, vertex::Vertex},
    rendering::vulkan::rendering_context::VulkanRenderingContext,
    terrain::texture_atlas::load_terrain_texture,
    ui::LABEL_WIDTH,
};

/// GPU resources for the uploaded day/night sky texture array (layer 0 = day, 1 = night).
#[derive(Clone, Debug)]
pub struct SkyboxTextures {
    pub image: vk::Image,
    pub image_memory: vk::DeviceMemory,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub descriptor_set: vk::DescriptorSet,
}

/// Equirectangular skybox rendered behind all scene geometry on a sky sphere.
/// `blend` cross-fades day → night; the sky rotates with this entity's `Transform`.
#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(serde)]
#[serde(default)]
pub struct Skybox {
    /// Equirectangular (panorama) day sky texture path.
    pub day: String,
    /// Equirectangular night sky texture path; falls back to `day` if empty.
    pub night: String,
    /// 0.0 = full day, 1.0 = full night.
    pub blend: f32,
    /// Uploaded GPU textures; rebuilt when paths change.
    #[serde(skip)]
    pub textures: Option<SkyboxTextures>,
    #[serde(skip)]
    pub sky_mesh: Option<Mesh>,
    /// Set after a failed upload so it isn't retried every frame.
    #[serde(skip)]
    pub load_failed: bool,
}

impl Default for Skybox {
    fn default() -> Self {
        Self {
            day: String::new(),
            night: String::new(),
            blend: 0.0,
            textures: None,
            sky_mesh: None,
            load_failed: false,
        }
    }
}

impl Inspect for Skybox {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        let row_h = 20.0;
        let mut changed = false;

        let has_texture_drag = DragAndDrop::has_payload_of_type::<String>(ui.ctx());
        for (label, path) in [("Day", &mut self.day), ("Night", &mut self.night)] {
            let inner = ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new(label));
                ui.add_sized(
                    [ui.available_width(), row_h],
                    egui::TextEdit::singleline(path).hint_text("drag a texture here…"),
                )
            });
            let resp = inner.inner;
            if has_texture_drag && ui.rect_contains_pointer(resp.rect) {
                ui.painter().rect_stroke(
                    resp.rect.expand(2.0),
                    3.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 160, 220)),
                    StrokeKind::Outside,
                );
            }
            if let Some(payload) = resp.dnd_release_payload::<String>() {
                if let Some(name) = payload.strip_prefix("texture:") {
                    *path = name.to_string();
                    changed = true;
                }
            }
            changed |= resp.changed();
        }

        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Blend"));
            ui.add(egui::Slider::new(&mut self.blend, 0.0..=1.0));
        });

        if changed {
            self.textures = None;
            self.load_failed = false;
        }
    }
}

/// Loads the day/night equirectangular textures and uploads them as a 2-layer
/// texture array with a combined-image-sampler descriptor set (same layout as
/// material albedos). An empty night path reuses the day texture.
pub fn upload_skybox_textures(
    ctx: &VulkanRenderingContext,
    command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    day_path: &str,
    night_path: &str,
) -> Result<SkyboxTextures> {
    if day_path.trim().is_empty() {
        anyhow::bail!("Skybox requires at least a day texture path");
    }

    // Both layers must match the day texture's dimensions.
    let day = load_terrain_texture(day_path).to_rgba8();
    let (width, height) = (day.width().max(1), day.height().max(1));
    let night = if night_path.trim().is_empty() {
        day.clone()
    } else {
        load_terrain_texture(night_path)
            .resize_exact(width, height, image::imageops::FilterType::Triangle)
            .to_rgba8()
    };
    let layers = [day, night];

    let layer_bytes = (width * height * 4) as vk::DeviceSize;
    let total_bytes = layer_bytes * 2;

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
            ptr.add(offset)
                .copy_from_nonoverlapping(raw.as_ptr(), raw.len());
            offset += raw.len();
        }
        ctx.device.unmap_memory(staging_memory);
    }

    let extent = vk::Extent3D {
        width,
        height,
        depth: 1,
    };
    let image_info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .extent(extent)
        .mip_levels(1)
        .array_layers(2)
        .format(vk::Format::R8G8B8A8_SRGB)
        .tiling(vk::ImageTiling::OPTIMAL)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
        .samples(SampleCountFlags::TYPE_1)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);

    let image = unsafe { ctx.device.create_image(&image_info, None)? };
    let mem_reqs = unsafe { ctx.device.get_image_memory_requirements(image) };
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
    unsafe { ctx.device.bind_image_memory(image, image_memory, 0)? };

    let all_layers = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 2,
    };

    let cmd = ctx.begin_single_time_commands(command_pool);
    unsafe {
        ctx.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TOP_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .image(image)
                .subresource_range(all_layers)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)],
        );

        for i in 0..2u32 {
            let region = vk::BufferImageCopy::default()
                .buffer_offset(i as vk::DeviceSize * layer_bytes)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: i,
                    layer_count: 1,
                })
                .image_extent(extent);
            ctx.device.cmd_copy_buffer_to_image(
                cmd,
                staging_buffer,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        ctx.device.cmd_pipeline_barrier(
            cmd,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[vk::ImageMemoryBarrier::default()
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image(image)
                .subresource_range(all_layers)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)],
        );
    }
    ctx.end_single_time_commands(
        cmd,
        ctx.queues[&ctx.queue_families.transfer],
        command_pool,
    );

    unsafe {
        ctx.device.destroy_buffer(staging_buffer, None);
        ctx.device.free_memory(staging_memory, None);
    }

    let image_view = unsafe {
        ctx.device.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D_ARRAY)
                .format(vk::Format::R8G8B8A8_SRGB)
                .subresource_range(all_layers),
            None,
        )?
    };

    // Wrap horizontally (longitude seam), clamp vertically (poles).
    let sampler = unsafe {
        ctx.device.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
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

    Ok(SkyboxTextures {
        image,
        image_memory,
        image_view,
        sampler,
        descriptor_set,
    })
}

/// Builds the unit UV sphere drawn by the skybox pass. Positions double as the
/// view directions; tex_coord carries the equirectangular UVs.
pub fn build_skybox_sphere_mesh(
    context: &Arc<VulkanRenderingContext>,
    command_pool: vk::CommandPool,
) -> Result<Mesh> {
    const RINGS: u32 = 16;
    const SEGMENTS: u32 = 32;

    let mut vertices: Vec<Vertex> = Vec::with_capacity(((RINGS + 1) * (SEGMENTS + 1)) as usize);
    for ring in 0..=RINGS {
        let v = ring as f32 / RINGS as f32;
        let phi = v * std::f32::consts::PI; // 0 (top) → π (bottom)
        for segment in 0..=SEGMENTS {
            let u = segment as f32 / SEGMENTS as f32;
            let theta = u * std::f32::consts::TAU;
            let position = [
                phi.sin() * theta.cos(),
                phi.cos(),
                phi.sin() * theta.sin(),
            ];
            vertices.push(Vertex {
                position,
                normal: position,
                tex_coord: [u, v],
                weights: [0.0; 32],
                color: [1.0, 1.0, 1.0],
            });
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity((RINGS * SEGMENTS * 6) as usize);
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let a = ring * (SEGMENTS + 1) + segment;
            let b = a + SEGMENTS + 1;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    let (vertex_buffer, vertex_buffer_memory) =
        context.create_vertex_buffer(&vertices, command_pool)?;
    let (index_buffer, index_buffer_memory) =
        context.create_index_buffer(&indices, command_pool)?;

    Ok(Mesh {
        vertex_buffer,
        vertex_buffer_memory,
        index_buffer,
        index_buffer_memory,
        index_count: indices.len() as u32,
        material_name: String::new(),
        material: None,
    })
}
