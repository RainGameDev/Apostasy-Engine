use std::path::{Path, PathBuf};

use anyhow::Result;
use apostasy_macros::{Resource, update};

use crate::{
    ecs::{cell::EntityId, world::World},
    project_dir,
    terrain::{
        TerrainAtlasNeedsRebuild, TerrainChunkMap, TerrainSettings,
        chunk::{NeedsTerrainRebuild, TerrainChunk},
        persistence::{
            cell_filename, load_terrain_settings, parse_cell_filename, read_terrain_cell,
        },
    },
    worldspaces::CurrentWorldspace,
};

/// Terrain directory for a worldspace: `{project}/terrain/{worldspace}`.
pub fn terrain_dir(worldspace: &str) -> PathBuf {
    project_dir().join("terrain").join(worldspace)
}

/// Which worldspace's terrain settings/chunk map are currently in effect.
#[derive(Resource, Clone, Default)]
struct TerrainLoaderState {
    worldspace: Option<String>,
}

fn current_worldspace_name(world: &World) -> String {
    world
        .get_resource::<CurrentWorldspace>()
        .ok()
        .map(|w| w.0.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

/// Loads heightmap data for terrain chunk entities from the current worldspace's terrain directory.
#[update(mode = "all")]
pub fn terrain_data_system(world: &mut World) -> Result<()> {
    if !world.has_resource::<TerrainSettings>() {
        return Ok(());
    }

    let ws = current_worldspace_name(world);
    let dir = terrain_dir(&ws);

    let loaded_ws = world
        .get_resource::<TerrainLoaderState>()
        .ok()
        .and_then(|s| s.worldspace.clone());
    if loaded_ws.as_deref() != Some(ws.as_str()) {
        enter_worldspace(world, &dir);
        world.insert_resource(TerrainLoaderState {
            worldspace: Some(ws.clone()),
        });
    }

    let pending: Vec<EntityId> = world
        .get_entities_with_component::<TerrainChunk>()
        .into_iter()
        .filter(|&id| {
            world
                .get_component::<TerrainChunk>(id)
                .is_some_and(|c| !c.data_loaded)
        })
        .collect();

    for id in pending {
        let coord = match world.get_component::<TerrainChunk>(id) {
            Some(c) => c.cell_coord,
            None => continue,
        };

        let path = dir.join(cell_filename(coord));
        if path.is_file() {
            match read_terrain_cell(&path, coord) {
                Ok((mut chunk, tex_table)) => {
                    remap_layers(world, &mut chunk, &tex_table);
                    if let Some(c) = world.get_component_mut::<TerrainChunk>(id) {
                        *c = chunk;
                    }
                }
                Err(e) => {
                    crate::log_warn!("Failed to load terrain {}: {}", path.display(), e);
                    if let Some(c) = world.get_component_mut::<TerrainChunk>(id) {
                        c.data_loaded = true;
                    }
                }
            }
        } else if let Some(c) = world.get_component_mut::<TerrainChunk>(id) {
            // No file yet — keep the flat default chunk.
            c.data_loaded = true;
        }

        world.add_tag::<NeedsTerrainRebuild>(id);
        if let Ok(map) = world.get_resource_mut::<TerrainChunkMap>() {
            map.0.insert(coord, id);
        }
    }

    Ok(())
}

/// Remaps a freshly read chunk's layer ids from its file texture table
/// to the global `TerrainSettings` list, appending any missing paths.
fn remap_layers(world: &mut World, chunk: &mut TerrainChunk, table: &[String]) {
    if table.is_empty() {
        return;
    }
    let mut appended = false;
    if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
        for slot in 0..chunk.active_layer_count as usize {
            let local_id = chunk.active_layer_ids[slot];
            let Some(path) = table.get(local_id as usize) else {
                continue;
            };
            let global = match settings.texture_layers.iter().position(|p| p == path) {
                Some(i) => i,
                None => {
                    settings.texture_layers.push(path.clone());
                    appended = true;
                    settings.texture_layers.len() - 1
                }
            };
            chunk.active_layer_ids[slot] = global as u32;
        }
    }
    if appended {
        world.insert_resource(TerrainAtlasNeedsRebuild);
    }
}

/// Switches terrain state to a new worldspace,
/// replaces the texture layer list,
/// rebuilds the chunk map,
/// and spawns chunk entities for any `.terrain` files that have no entity yet.
fn enter_worldspace(world: &mut World, dir: &Path) {
    let saved_layers = load_terrain_settings(dir);
    if !saved_layers.is_empty() {
        let mut changed = false;
        if let Ok(settings) = world.get_resource_mut::<TerrainSettings>()
            && settings.texture_layers != saved_layers
        {
            settings.texture_layers = saved_layers;
            changed = true;
        }
        if changed {
            world.insert_resource(TerrainAtlasNeedsRebuild);
        }
    }

    let mut map = TerrainChunkMap::default();
    for id in world.get_entities_with_component::<TerrainChunk>() {
        if let Some(c) = world.get_component::<TerrainChunk>(id) {
            map.0.insert(c.cell_coord, id);
        }
    }

    let resolution = world
        .get_resource::<TerrainSettings>()
        .map(|s| s.resolution)
        .unwrap_or(32);

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("terrain") {
                continue;
            }
            let Some(coord) = parse_cell_filename(&path) else {
                continue;
            };
            if map.0.contains_key(&coord) {
                continue;
            }
            let id = world.spawn_in_cell(coord).id();
            world.set_name(id, &format!("Terrain ({},{})", coord.x, coord.z));
            let mut chunk = TerrainChunk::new(coord, resolution);
            chunk.data_loaded = false;
            world.add_component(id, chunk);
            map.0.insert(coord, id);
        }
    }

    world.insert_resource(map);
}
