use anyhow::Result;
use ash::vk;
use cgmath::Vector3;

use crate::{
    objects::world::World,
    rendering::vulkan::rendering_context::VulkanRenderingContext,
    terrain::TerrainChunkMap,
};

use super::{
    chunk::{NeedsTerrainRebuild, TerrainChunk, TerrainMesh},
    mesh_builder::{ExistingMesh, NeighborBorders, build_terrain_mesh},
};

/// Uploads or updates GPU meshes for all terrain chunks tagged with `NeedsTerrainRebuild`.
/// On the first build: allocates HOST_VISIBLE vertex buffer + DEVICE_LOCAL index buffer.
/// On subsequent rebuilds: writes vertex data directly into the mapped buffer — no staging
/// buffer, no GPU queue submission, no pipeline stall.
pub fn rebuild_dirty_terrain(
    world: &mut World,
    context: &VulkanRenderingContext,
    command_pool: vk::CommandPool,
    _graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>,
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

        // Reuse existing buffers if they are host-visible (created by a previous rebuild).
        // Old DEVICE_LOCAL meshes (host_visible = false) fall through to the allocation path;
        // their buffers are queued for deferred cleanup below.
        let existing = world
            .get_object(id)
            .and_then(|o| o.get_component::<TerrainMesh>().ok())
            .filter(|m| m.host_visible && m.vertex_buffer != vk::Buffer::null())
            .map(|m| ExistingMesh {
                vertex_buffer: m.vertex_buffer,
                vertex_buffer_memory: m.vertex_buffer_memory,
                index_buffer: m.index_buffer,
                index_buffer_memory: m.index_buffer_memory,
            });

        let is_in_place = existing.is_some();
        let neighbors = gather_neighbors(world, &chunk);
        let new_mesh = build_terrain_mesh(&chunk, &neighbors, existing, context, command_pool)?;

        if let Some(obj) = world.get_object_mut(id) {
            if !is_in_place {
                // Old mesh had DEVICE_LOCAL buffers — queue them for deferred cleanup.
                if let Ok(old) = obj.get_component::<TerrainMesh>() {
                    queue_old_buffers(_graveyard, old);
                }
            }
            // In-place: same Vulkan handles, struct just replaced with updated data.

            if obj.has_component::<TerrainMesh>() {
                *obj.get_component_mut::<TerrainMesh>().unwrap() = new_mesh;
            } else {
                obj.add_component(new_mesh);
            }

            obj.remove_tag::<NeedsTerrainRebuild>();
        }
    }

    Ok(())
}

fn gather_neighbors(world: &World, chunk: &TerrainChunk) -> NeighborBorders {
    let map = match world.get_resource::<TerrainChunkMap>() {
        Ok(m) => m,
        Err(_) => return NeighborBorders::default(),
    };

    let coord = chunk.cell_coord;
    let r = chunk.resolution as usize;
    let side = r + 1;

    // Sample one step *inside* each neighbor so both chunks produce the same central
    // difference gradient at their shared border vertex. See NeighborBorders doc.
    let left_col = map.0.get(&Vector3::new(coord.x - 1, 0, coord.z))
        .and_then(|&id| world.get_object(id))
        .and_then(|o| o.get_component::<TerrainChunk>().ok())
        .map(|c| (0..side).map(|z| c.heights[r.saturating_sub(1) + z * side]).collect::<Vec<_>>());

    let right_col = map.0.get(&Vector3::new(coord.x + 1, 0, coord.z))
        .and_then(|&id| world.get_object(id))
        .and_then(|o| o.get_component::<TerrainChunk>().ok())
        .map(|c| (0..side).map(|z| c.heights[1.min(r) + z * side]).collect::<Vec<_>>());

    let top_row = map.0.get(&Vector3::new(coord.x, 0, coord.z - 1))
        .and_then(|&id| world.get_object(id))
        .and_then(|o| o.get_component::<TerrainChunk>().ok())
        .map(|c| (0..side).map(|x| c.heights[x + r.saturating_sub(1) * side]).collect::<Vec<_>>());

    let bottom_row = map.0.get(&Vector3::new(coord.x, 0, coord.z + 1))
        .and_then(|&id| world.get_object(id))
        .and_then(|o| o.get_component::<TerrainChunk>().ok())
        .map(|c| (0..side).map(|x| c.heights[x + 1.min(r) * side]).collect::<Vec<_>>());

    NeighborBorders { left_col, right_col, top_row, bottom_row }
}

fn queue_old_buffers(graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>, mesh: &TerrainMesh) {
    if mesh.vertex_buffer != vk::Buffer::null() {
        graveyard.push((mesh.vertex_buffer, mesh.vertex_buffer_memory));
    }
    if mesh.index_buffer != vk::Buffer::null() {
        graveyard.push((mesh.index_buffer, mesh.index_buffer_memory));
    }
}
