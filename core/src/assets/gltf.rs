use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use anyhow::Result;
use ash::vk::{self, CommandPool, SampleCountFlags};
use cgmath::Vector3;
use hashbrown::HashMap;
use walkdir::WalkDir;

use crate::{
    assets::loaders::material_loader::MaterialRegistry,
    log, log_error,
    rendering::{
        shared::{
            material::GpuMaterial,
            model::{Bvh, GpuModel, Mesh},
            texture::GpuTexture,
            vertex::Vertex,
        },
        vulkan::rendering_context::VulkanRenderingContext,
    },
};

#[derive(Default, Clone, Debug)]
pub struct ModelRegistry {
    pub paths: HashMap<String, GpuModel>,
}

#[derive(Default, Clone)]
pub struct ModelLoader {
    pub registry: Arc<RwLock<ModelRegistry>>,
}

impl ModelLoader {
    pub fn load_all_models(
        dir_path: &Path,
        context: Arc<VulkanRenderingContext>,
        command_pool: CommandPool,
        descriptor_pool: vk::DescriptorPool,
        descriptor_set_layout: vk::DescriptorSetLayout,
        material_registry: &MaterialRegistry,
    ) -> Result<HashMap<String, GpuModel>> {
        let mut models = HashMap::new();

        for entry in WalkDir::new(dir_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path().is_file()
                    && matches!(
                        e.path().extension().and_then(|s| s.to_str()),
                        Some("gltf") | Some("glb")
                    )
            })
        {
            let path = entry.path();

            match load_model(
                path,
                Arc::clone(&context),
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
                material_registry,
            ) {
                Ok(model) => {
                    log!("Loaded model: {} ({:?})", model.name, path);
                    models.insert(model.name.clone(), model);
                }
                Err(e) => {
                    log_error!("Failed to load model {:?}: {}", path, e);
                }
            }
        }

        Ok(models)
    }
}

pub fn load_model(
    path: &Path,
    context: Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    material_registry: &MaterialRegistry,
) -> Result<GpuModel> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model")
        .to_string();

    let (gltf, buffers, images) = gltf::import(path.to_str().unwrap())?;

    let mut meshes = Vec::new();

    let mut all_triangles: Vec<[Vector3<f32>; 3]> = Vec::new();
    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions = reader.read_positions().unwrap().collect::<Vec<_>>();
            let normals = reader.read_normals().unwrap().collect::<Vec<_>>();
            let tex_coords = reader
                .read_tex_coords(0)
                .unwrap()
                .into_f32()
                .collect::<Vec<_>>();

            let vertices: Vec<Vertex> = positions
                .iter()
                .zip(normals.iter())
                .zip(tex_coords.iter())
                .map(|((pos, norm), tex)| Vertex {
                    position: *pos,
                    normal: *norm,
                    tex_coord: *tex,
                    tex_layer: 0.0,
                })
                .collect();

            let indices = reader
                .read_indices()
                .unwrap()
                .into_u32()
                .collect::<Vec<_>>();

            let vertex_buffer = context.create_vertex_buffer(vertices.as_slice(), command_pool)?;
            let index_buffer = context.create_index_buffer(&indices, command_pool)?;

            let material_name = primitive
                .material()
                .name()
                .unwrap_or("material")
                .to_string();

            let material = resolve_material(
                &material_name,
                &primitive.material(),
                &images,
                &context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
                material_registry,
            );

            for chunk in indices.chunks(3) {
                if let [i0, i1, i2] = *chunk {
                    let p = |i: u32| {
                        let p = positions[i as usize];
                        Vector3::new(p[0], p[1], p[2])
                    };
                    all_triangles.push([p(i0), p(i1), p(i2)]);
                }
            }

            meshes.push(Mesh {
                vertex_buffer: vertex_buffer.0,
                vertex_buffer_memory: vertex_buffer.1,
                index_buffer: index_buffer.0,
                index_buffer_memory: index_buffer.1,
                index_count: indices.len() as u32,
                material_name,
                material,
            });
        }
    }

    let collision_bvh = if all_triangles.is_empty() {
        None
    } else {
        Some(Arc::new(Bvh::build(all_triangles)))
    };

    Ok(GpuModel {
        name,
        meshes,
        collision_bvh,
    })
}

/// Resolves a mesh material: prefers a YAML-defined material by name, falls back
/// to the embedded glTF PBR data.
fn resolve_material(
    material_name: &str,
    gltf_mat: &gltf::Material,
    images: &[gltf::image::Data],
    context: &Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    material_registry: &MaterialRegistry,
) -> Option<GpuMaterial> {
    // YAML-defined material takes priority
    if let Some(yaml_mat) = material_registry.materials.get(material_name) {
        let texture = yaml_mat.albedo_path.as_deref().and_then(|path| {
            upload_texture_from_path(
                path,
                material_name,
                context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
            )
        });
        return Some(GpuMaterial {
            albedo: texture,
            color: yaml_mat.color,
            shader: yaml_mat.shader_path.clone(),
        });
    }

    // Fall back to glTF embedded material
    from_gltf_material(
        gltf_mat,
        images,
        context,
        command_pool,
        descriptor_pool,
        descriptor_set_layout,
    )
}

fn from_gltf_material(
    material: &gltf::Material,
    images: &[gltf::image::Data],
    context: &Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Option<GpuMaterial> {
    let pbr = material.pbr_metallic_roughness();
    let color = pbr.base_color_factor();

    let albedo = pbr.base_color_texture().and_then(|info| {
        let idx = info.texture().source().index();
        images.get(idx).and_then(|img| {
            let name = material.name().unwrap_or("gltf_texture");
            upload_texture_from_pixels(
                &to_rgba8(img),
                img.width,
                img.height,
                name,
                context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
            )
            .ok()
        })
    });

    if albedo.is_some() || color != [1.0, 1.0, 1.0, 1.0] {
        Some(GpuMaterial { albedo, color, shader: None })
    } else {
        None
    }
}

fn upload_texture_from_path(
    path: &str,
    name: &str,
    context: &Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Option<GpuTexture> {
    let candidates = [
        Path::new("res").join(path),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("res").join(path),
    ];
    let full_path = candidates.iter().find(|p| p.exists())?;

    let img = image::open(full_path).ok()?;
    let rgba = img.to_rgba8();
    upload_texture_from_pixels(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
        name,
        context,
        command_pool,
        descriptor_pool,
        descriptor_set_layout,
    )
    .ok()
}

fn to_rgba8(img: &gltf::image::Data) -> Vec<u8> {
    match img.format {
        gltf::image::Format::R8G8B8A8 => img.pixels.clone(),
        gltf::image::Format::R8G8B8 => img
            .pixels
            .chunks(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        _ => {
            if img.pixels.len() == (img.width * img.height * 4) as usize {
                img.pixels.clone()
            } else {
                vec![255u8; (img.width * img.height * 4) as usize]
            }
        }
    }
}

fn upload_texture_from_pixels(
    pixels: &[u8],
    width: u32,
    height: u32,
    name: &str,
    context: &Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Result<GpuTexture> {
    let size = pixels.len() as vk::DeviceSize;

    let (staging_buffer, staging_memory) = context.create_buffer(
        size,
        vk::BufferUsageFlags::TRANSFER_SRC,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    unsafe {
        let ptr = context
            .device
            .map_memory(staging_memory, 0, size, vk::MemoryMapFlags::empty())?
            as *mut u8;
        ptr.copy_from_nonoverlapping(pixels.as_ptr(), pixels.len());
        context.device.unmap_memory(staging_memory);
    }

    let (image, memory) = context.create_image(
        vk::Extent2D { width, height },
        vk::Format::R8G8B8A8_SRGB,
        vk::ImageTiling::OPTIMAL,
        vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        SampleCountFlags::TYPE_1,
    )?;

    let sub = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    };

    let cmd = context.begin_single_time_commands(command_pool);
    unsafe {
        context.device.cmd_pipeline_barrier(
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
                .subresource_range(sub)
                .src_access_mask(vk::AccessFlags::empty())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)],
        );

        context.device.cmd_copy_buffer_to_image(
            cmd,
            staging_buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[vk::BufferImageCopy::default()
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })],
        );

        context.device.cmd_pipeline_barrier(
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
                .subresource_range(sub)
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)],
        );
    }
    let queue = context.queues[&context.queue_families.transfer];
    context.end_single_time_commands(cmd, queue, command_pool);

    unsafe {
        context.device.destroy_buffer(staging_buffer, None);
        context.device.free_memory(staging_memory, None);
    }

    let image_view = context.create_image_view(
        image,
        vk::Format::R8G8B8A8_SRGB,
        vk::ImageAspectFlags::COLOR,
    )?;

    let sampler = unsafe {
        context.device.create_sampler(
            &vk::SamplerCreateInfo::default()
                .mag_filter(vk::Filter::LINEAR)
                .min_filter(vk::Filter::LINEAR)
                .address_mode_u(vk::SamplerAddressMode::REPEAT)
                .address_mode_v(vk::SamplerAddressMode::REPEAT)
                .address_mode_w(vk::SamplerAddressMode::REPEAT)
                .anisotropy_enable(false)
                .max_anisotropy(1.0),
            None,
        )?
    };

    let descriptor_set = context.create_texture_descriptor_set(
        descriptor_pool,
        descriptor_set_layout,
        image_view,
        sampler,
    );

    Ok(GpuTexture {
        name: name.to_string(),
        image,
        image_view,
        memory,
        sampler,
        descriptor_set,
    })
}
