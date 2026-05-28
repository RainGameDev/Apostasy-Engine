use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use anyhow::Result;
use apostasy_macros::Resource;
use ash::vk::CommandPool;
use hashbrown::HashMap;
use walkdir::WalkDir;

use crate::{
    log, log_error,
    rendering::{
        shared::{
            model::{GpuModel, Mesh},
            vertex::Vertex,
        },
        vulkan::rendering_context::VulkanRenderingContext,
    },
};

#[derive(Resource, Default, Clone, Debug)]
pub struct ModelRegistry {
    pub paths: HashMap<String, GpuModel>, // name to gltf
}

#[derive(Default)]
pub struct ModelLoader {
    pub registry: Arc<RwLock<ModelRegistry>>,
}

impl ModelLoader {
    pub fn load_all_models(
        dir_path: &Path,
        context: Arc<VulkanRenderingContext>,
        command_pool: CommandPool,
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

            match load_model(path, Arc::clone(&context), command_pool) {
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
) -> Result<GpuModel> {
    let path_str = path.to_str();

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model")
        .to_string();

    let (gltf, buffers, _images) = gltf::import(path_str.unwrap())?;

    let mut meshes = Vec::new();

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

            meshes.push(Mesh {
                vertex_buffer: vertex_buffer.0,
                vertex_buffer_memory: vertex_buffer.1,
                index_buffer: index_buffer.0,
                index_buffer_memory: index_buffer.1,
                index_count: indices.len() as u32,
                material_name,
            });
        }
    }

    Ok(GpuModel { name, meshes })
}
