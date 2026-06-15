use anyhow::Result;
use ash::vk;

use crate::{objects::world::World, rendering::vulkan::rendering_context::VulkanRenderingContext};

use super::{
    chunk::{NeedsTerrainRebuild, TerrainChunk, TerrainMesh},
    mesh_builder::build_terrain_mesh,
};

/// Uploads GPU meshes for all terrain chunks tagged with `NeedsTerrainRebuild`.
/// Call this from the main render loop where the context and command pool are available.
pub fn rebuild_dirty_terrain(
    world: &mut World,
    context: &VulkanRenderingContext,
    command_pool: vk::CommandPool,
    graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>,
) -> Result<()> {
    let ids: Vec<_> = world
        .get_objects_with_tag_with_ids::<NeedsTerrainRebuild>()
        .iter()
        .map(|(id, _)| *id)
        .collect();

    for id in ids {
        let chunk = match world
            .get_object(id)
            .and_then(|o| o.get_component::<TerrainChunk>().ok().cloned())
        {
            Some(c) => c,
            None => continue,
        };

        let new_mesh = build_terrain_mesh(&chunk, context, command_pool)?;

        if let Some(obj) = world.get_object_mut(id) {
            // Queue old buffers for deferred cleanup
            if let Ok(old) = obj.get_component::<TerrainMesh>() {
                queue_old_buffers(graveyard, old);
            }

            if obj.has_component::<TerrainMesh>() {
                let mesh = obj.get_component_mut::<TerrainMesh>().unwrap();
                *mesh = new_mesh;
            } else {
                obj.add_component(new_mesh);
            }

            obj.remove_tag::<NeedsTerrainRebuild>();
        }
    }

    Ok(())
}

fn queue_old_buffers(graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>, mesh: &TerrainMesh) {
    if mesh.vertex_buffer != vk::Buffer::null() {
        graveyard.push((mesh.vertex_buffer, mesh.vertex_buffer_memory));
    }
    if mesh.index_buffer != vk::Buffer::null() {
        graveyard.push((mesh.index_buffer, mesh.index_buffer_memory));
    }
}
