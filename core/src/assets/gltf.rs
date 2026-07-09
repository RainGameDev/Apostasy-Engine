use std::{path::Path, sync::Arc};

use anyhow::Result;
use ash::vk::{self, CommandPool, SampleCountFlags};
use cgmath::{InnerSpace, Vector3};
use hashbrown::HashMap;
use parking_lot::RwLock;
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
    pub version: u64,
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

    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("model path is not valid UTF-8: {:?}", path))?;
    let (gltf, buffers, images) = gltf::import(path_str)?;

    let mut meshes = Vec::new();

    let mut all_triangles: Vec<[Vector3<f32>; 3]> = Vec::new();
    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            // Positions are the only attribute we can't synthesise; skip a
            // primitive that lacks them rather than crashing the whole load.
            let Some(positions) = reader.read_positions().map(|p| p.collect::<Vec<_>>()) else {
                log_error!("Skipping primitive in {:?}: no position data", path);
                continue;
            };

            // Indices: fall back to a sequential list for non-indexed primitives.
            let indices = match reader.read_indices() {
                Some(i) => i.into_u32().collect::<Vec<_>>(),
                None => (0..positions.len() as u32).collect::<Vec<_>>(),
            };

            // Normals: compute flat normals from the geometry when absent.
            let normals = match reader.read_normals() {
                Some(n) => n.collect::<Vec<_>>(),
                None => compute_normals(&positions, &indices),
            };

            // UVs: default to (0, 0) when the mesh has none.
            let tex_coords = match reader.read_tex_coords(0) {
                Some(t) => t.into_f32().collect::<Vec<_>>(),
                None => vec![[0.0, 0.0]; positions.len()],
            };

            let tangents = match reader.read_tangents() {
                Some(t) => t.collect::<Vec<_>>(),
                None => compute_tangents(&positions, &normals, &tex_coords, &indices),
            };

            dbg!(&tangents);

            let vertices: Vec<Vertex> = positions
                .iter()
                .zip(normals.iter())
                .zip(tex_coords.iter())
                .zip(tangents.iter())
                .map(|(((pos, norm), tex), tan)| Vertex {
                    position: *pos,
                    normal: *norm,
                    tex_coord: *tex,
                    weights: [0.0; 32],
                    color: [1.0, 1.0, 1.0],
                    tangent: *tan,
                })
                .collect();

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

/// Computes per-vertex normals by accumulating face normals over the index list.
/// Used as a fallback when a glTF primitive ships without a NORMAL attribute.
fn compute_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![Vector3::new(0.0f32, 0.0, 0.0); positions.len()];

    for tri in indices.chunks_exact(3) {
        let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if a >= positions.len() || b >= positions.len() || c >= positions.len() {
            continue;
        }
        let pa = Vector3::from(positions[a]);
        let pb = Vector3::from(positions[b]);
        let pc = Vector3::from(positions[c]);
        let face = (pb - pa).cross(pc - pa);
        normals[a] += face;
        normals[b] += face;
        normals[c] += face;
    }

    normals
        .into_iter()
        .map(|n| {
            if n.magnitude2() > 1e-12 {
                let n = n.normalize();
                [n.x, n.y, n.z]
            } else {
                [0.0, 1.0, 0.0]
            }
        })
        .collect()
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
        let albedo = yaml_mat.albedo_path.as_deref().and_then(|path| {
            upload_texture_from_path(
                path,
                material_name,
                vk::Format::R8G8B8A8_SRGB,
                context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
            )
        });
        // Normal maps store vectors, not color — upload as linear (UNORM).
        let normal = yaml_mat.normal_path.as_deref().and_then(|path| {
            upload_texture_from_path(
                path,
                material_name,
                vk::Format::R8G8B8A8_UNORM,
                context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
            )
        });
        return Some(build_material_set(
            albedo,
            normal,
            yaml_mat.color,
            yaml_mat.shader_path.clone(),
            context,
            command_pool,
            descriptor_pool,
            descriptor_set_layout,
        ));
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
    let name = material.name().unwrap_or("gltf_texture");

    let albedo = pbr.base_color_texture().and_then(|info| {
        let idx = info.texture().source().index();
        images.get(idx).and_then(|img| {
            upload_texture_from_pixels(
                &to_rgba8(img),
                img.width,
                img.height,
                name,
                vk::Format::R8G8B8A8_SRGB,
                context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
            )
            .ok()
        })
    });

    let normal = material.normal_texture().and_then(|info| {
        let idx = info.texture().source().index();
        images.get(idx).and_then(|img| {
            upload_texture_from_pixels(
                &to_rgba8(img),
                img.width,
                img.height,
                name,
                vk::Format::R8G8B8A8_UNORM,
                context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
            )
            .ok()
        })
    });

    if albedo.is_some() || normal.is_some() || color != [1.0, 1.0, 1.0, 1.0] {
        Some(build_material_set(
            albedo,
            normal,
            color,
            None,
            context,
            command_pool,
            descriptor_pool,
            descriptor_set_layout,
        ))
    } else {
        None
    }
}

/// Fills missing albedo/normal slots with default textures and builds the
/// combined per-material descriptor set (binding 0 = albedo, 1 = normal).
fn build_material_set(
    albedo: Option<GpuTexture>,
    normal: Option<GpuTexture>,
    color: [f32; 4],
    shader: Option<String>,
    context: &Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> GpuMaterial {
    let albedo = albedo.or_else(|| {
        solid_texture(
            [255, 255, 255, 255],
            vk::Format::R8G8B8A8_SRGB,
            "mat_default_albedo",
            context,
            command_pool,
            descriptor_pool,
            descriptor_set_layout,
        )
    });
    let normal = normal.or_else(|| {
        solid_texture(
            [128, 128, 255, 255],
            vk::Format::R8G8B8A8_UNORM,
            "mat_default_normal",
            context,
            command_pool,
            descriptor_pool,
            descriptor_set_layout,
        )
    });

    let descriptor_set = match (&albedo, &normal) {
        (Some(a), Some(n)) => context.create_material_descriptor_set(
            descriptor_pool,
            descriptor_set_layout,
            a.image_view,
            a.sampler,
            n.image_view,
            n.sampler,
        ),
        // If either upload failed, leave null so the draw path falls back to
        // the shared default material set.
        _ => vk::DescriptorSet::null(),
    };

    GpuMaterial {
        albedo,
        normal,
        color,
        shader,
        descriptor_set,
    }
}

fn solid_texture(
    rgba: [u8; 4],
    format: vk::Format,
    name: &str,
    context: &Arc<VulkanRenderingContext>,
    command_pool: CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
) -> Option<GpuTexture> {
    upload_texture_from_pixels(
        &rgba,
        1,
        1,
        name,
        format,
        context,
        command_pool,
        descriptor_pool,
        descriptor_set_layout,
    )
    .ok()
}

fn upload_texture_from_path(
    path: &str,
    name: &str,
    format: vk::Format,
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
        format,
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

pub(crate) fn upload_texture_from_pixels(
    pixels: &[u8],
    width: u32,
    height: u32,
    name: &str,
    format: vk::Format,
    context: &VulkanRenderingContext,
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
        format,
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

    let image_view = context.create_image_view(image, format, vk::ImageAspectFlags::COLOR)?;

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

fn compute_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    tex_coords: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    let mut tan = vec![Vector3::new(0.0f32, 0.0, 0.0); positions.len()];
    let mut bitan = vec![Vector3::new(0.0f32, 0.0, 0.0); positions.len()];

    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            continue;
        }

        let p0 = Vector3::from(positions[i0]);
        let p1 = Vector3::from(positions[i1]);
        let p2 = Vector3::from(positions[i2]);

        let uv0 = tex_coords[i0];
        let uv1 = tex_coords[i1];
        let uv2 = tex_coords[i2];

        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let duv1 = [uv1[0] - uv0[0], uv1[1] - uv0[1]];
        let duv2 = [uv2[0] - uv0[0], uv2[1] - uv0[1]];

        let denom = duv1[0] * duv2[1] - duv2[0] * duv1[1];
        if denom.abs() < 1e-12 {
            continue; // degenerate UVs on this triangle; let neighbors carry it
        }
        let f = 1.0 / denom;

        let t = (edge1 * duv2[1] - edge2 * duv1[1]) * f;
        let b = (edge2 * duv1[0] - edge1 * duv2[0]) * f;

        for i in [i0, i1, i2] {
            tan[i] += t;
            bitan[i] += b;
        }
    }

    positions
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let n = Vector3::from(normals[i]);
            let mut t = tan[i];

            t -= n * n.dot(t);

            let t = if t.magnitude2() > 1e-12 {
                t.normalize()
            } else {
                let fallback = if n.x.abs() < 0.9 {
                    Vector3::new(1.0, 0.0, 0.0)
                } else {
                    Vector3::new(0.0, 1.0, 0.0)
                };
                (fallback - n * n.dot(fallback)).normalize()
            };

            let handedness = if n.cross(t).dot(bitan[i]) < 0.0 {
                -1.0
            } else {
                1.0
            };
            [t.x, t.y, t.z, handedness]
        })
        .collect()
}
