use std::sync::Arc;

use anyhow::Result;
use ash::vk;
use cgmath::{Vector3, Zero};

use crate::{
    ecs::{components::transform::Transform, world::World},
    physics::{
        collider::{Collider, ColliderShape},
        velocity::Velocity,
    },
    rendering::{shared::model::Bvh, vulkan::rendering_context::VulkanRenderingContext},
    terrain::{TerrainChunkMap, mesh_builder::build_terrain_collider},
};

use super::{
    chunk::{NeedsTerrainRebuild, TerrainChunk, TerrainMesh},
    mesh_builder::{NeighborBorders, build_terrain_mesh},
};

/// Uploads or updates GPU meshes for all terrain chunks tagged with `NeedsTerrainRebuild`.
///
/// Always creates fresh vertex / index buffers (the terrain switched to per-triangle
/// vertices so the old shared-vertex buffers are incompatible).  Previous buffers are
/// pushed onto `_graveyard` for deferred destruction.
pub fn rebuild_dirty_terrain(
    world: &mut World,
    context: &VulkanRenderingContext,
    command_pool: vk::CommandPool,
    _graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>,
) -> Result<()> {
    let ids = world.get_entities_with_tag::<NeedsTerrainRebuild>();

    for id in ids {
        let chunk = match world.get_component::<TerrainChunk>(id).cloned() {
            Some(c) => c,
            None => continue,
        };

        let neighbors = gather_neighbors(world, &chunk);
        let new_mesh = build_terrain_mesh(&chunk, &neighbors, context, command_pool)?;

        let terrain_triangles = build_terrain_collider(&chunk);
        let bvh = Bvh::build(terrain_triangles.clone());
        let terrain_collider = Collider::new_static(
            ColliderShape::Mesh {
                triangles: Arc::new(terrain_triangles),
                bvh: Arc::new(bvh),
                model_path: String::new(),
            },
            Vector3::zero(),
        );

        world.remove_component::<Collider>(id);
        world.add_component(id, terrain_collider);

        if !world.has_component::<Transform>(id) {
            world.add_component(
                id,
                Transform {
                    ..Default::default()
                },
            );
        }

        if !world.has_component::<Velocity>(id) {
            world.add_component(id, Velocity::static_entity());
        }

        // Queue old gpu buffers for deferred destruction.
        if let Some(old) = world.get_component::<TerrainMesh>(id) {
            queue_old_buffers(_graveyard, old);
        }

        if world.has_component::<TerrainMesh>(id) {
            if let Some(mesh) = world.get_component_mut::<TerrainMesh>(id) {
                *mesh = new_mesh;
            }
        } else {
            world.add_component(id, new_mesh);
        }

        world.remove_tag::<NeedsTerrainRebuild>(id);
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
    let left_col = map
        .0
        .get(&Vector3::new(coord.x - 1, 0, coord.z))
        .and_then(|&id| world.get_component::<TerrainChunk>(id))
        .map(|c| {
            (0..side)
                .map(|z| c.heights[r.saturating_sub(1) + z * side])
                .collect::<Vec<_>>()
        });

    let right_col = map
        .0
        .get(&Vector3::new(coord.x + 1, 0, coord.z))
        .and_then(|&id| world.get_component::<TerrainChunk>(id))
        .map(|c| {
            (0..side)
                .map(|z| c.heights[1.min(r) + z * side])
                .collect::<Vec<_>>()
        });

    let top_row = map
        .0
        .get(&Vector3::new(coord.x, 0, coord.z - 1))
        .and_then(|&id| world.get_component::<TerrainChunk>(id))
        .map(|c| {
            (0..side)
                .map(|x| c.heights[x + r.saturating_sub(1) * side])
                .collect::<Vec<_>>()
        });

    let bottom_row = map
        .0
        .get(&Vector3::new(coord.x, 0, coord.z + 1))
        .and_then(|&id| world.get_component::<TerrainChunk>(id))
        .map(|c| {
            (0..side)
                .map(|x| c.heights[x + 1.min(r) * side])
                .collect::<Vec<_>>()
        });

    NeighborBorders {
        left_col,
        right_col,
        top_row,
        bottom_row,
    }
}

fn queue_old_buffers(graveyard: &mut Vec<(vk::Buffer, vk::DeviceMemory)>, mesh: &TerrainMesh) {
    if mesh.vertex_buffer != vk::Buffer::null() {
        graveyard.push((mesh.vertex_buffer, mesh.vertex_buffer_memory));
    }
    if mesh.index_buffer != vk::Buffer::null() {
        graveyard.push((mesh.index_buffer, mesh.index_buffer_memory));
    }
}
