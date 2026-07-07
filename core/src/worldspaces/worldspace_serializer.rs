use anyhow::Result;
use std::path::Path;

use cgmath::Vector3;
use hashbrown::HashMap;

use crate::{
    ecs::{
        cell::{CellCoord, EntityId},
        components::get_component_registration,
        tag::get_tag_registration,
        tags::{Player, skips_serilization::SkipsSerilization},
        world::World,
        worldspace_streaming::WorldspaceStreaming,
    },
    rendering::components::camera::{ActiveCamera, EditorCamera},
    terrain::chunk::{NeedsTerrainRebuild, TerrainChunk},
};

fn serialize_entity(world: &World, id: EntityId) -> Option<serde_yaml::Value> {
    // Skip editor/internal entities
    if world.has_tag::<EditorCamera>(id) || world.has_tag::<SkipsSerilization>(id) {
        return None;
    }

    let name = world.get_name(id).unwrap_or("Entity").to_string();
    let children_ids = world.get_children_ids(id);

    let mut components_data: serde_yaml::Sequence = Vec::new();
    for reg in inventory::iter::<crate::ecs::components::ComponentRegistration>() {
        if let Some(v) = (reg.read)(world, id) {
            components_data.push(v);
        }
    }

    // Serialize known persistent tag types (excluding editor/transient tags)
    let mut tags_data: serde_yaml::Sequence = Vec::new();
    if world.has_tag::<Player>(id) {
        tags_data.push(serde_yaml::Value::String("Player".to_string()));
    }

    if world.has_tag::<ActiveCamera>(id) {
        tags_data.push(serde_yaml::Value::String("ActiveCamera".to_string()));
    }

    let (name, children_ids, components_data, tags_data) =
        (name, children_ids, components_data, tags_data);

    let children: serde_yaml::Sequence = children_ids
        .iter()
        .filter_map(|&child_id| serialize_entity(world, child_id))
        .collect();

    let mut map = serde_yaml::Mapping::new();
    map.insert("name".into(), name.into());
    map.insert(
        "components".into(),
        serde_yaml::Value::Sequence(components_data),
    );
    map.insert("tags".into(), serde_yaml::Value::Sequence(tags_data));
    map.insert("children".into(), serde_yaml::Value::Sequence(children));

    Some(serde_yaml::Value::Mapping(map))
}

fn coord_key(coord: CellCoord) -> String {
    format!("{},{},{}", coord.x, coord.y, coord.z)
}

fn parse_coord_key(key: &str) -> Option<CellCoord> {
    let mut parts = key.split(',').map(|p| p.trim().parse::<i32>());
    let x = parts.next()?.ok()?;
    let y = parts.next()?.ok()?;
    let z = parts.next()?.ok()?;
    Some(Vector3::new(x, y, z))
}

/// Serializes a single loaded cell into its on-disk `{ name, entities }` mapping.
/// Returns `None` when the cell is not currently loaded.
pub fn serialize_cell(world: &World, coord: CellCoord) -> Option<serde_yaml::Value> {
    let cell = world.worldspace().get_cell(coord)?;
    let root_ids = cell.get_root_ids();
    let cell_name = cell.name.clone();
    let mut entities = serde_yaml::Sequence::new();
    for id in root_ids {
        if let Some(entity_value) = serialize_entity(world, id) {
            entities.push(entity_value);
        }
    }
    let mut cell_map = serde_yaml::Mapping::new();
    cell_map.insert("name".into(), cell_name.into());
    cell_map.insert("entities".into(), serde_yaml::Value::Sequence(entities));
    Some(serde_yaml::Value::Mapping(cell_map))
}

/// Loads a single cell's `{ name, entities }` mapping (as produced by
/// [`serialize_cell`]) into `coord`. Used by the streaming system to bring cells
/// back into memory once they re-enter render distance.
pub fn load_cell(world: &mut World, coord: CellCoord, value: &serde_yaml::Value) -> Result<()> {
    let (name, entities) = match value {
        serde_yaml::Value::Mapping(_) => (
            value.get("name").and_then(|v| v.as_str()).unwrap_or(""),
            value.get("entities").and_then(|v| v.as_sequence()),
        ),
        _ => ("", value.as_sequence()),
    };
    if !name.is_empty() {
        world.worldspace_mut().set_cell_name(coord, name);
    }
    if let Some(entities) = entities {
        for entity_value in entities {
            load_entity(world, entity_value, None, coord)?;
        }
    }
    Ok(())
}

/// Saves the active worldspace to a single YAML file
pub fn save_worldspace(world: &World, name: &str, namespace: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cells = serde_yaml::Mapping::new();
    let cell_coords: Vec<CellCoord> = world.worldspace().loaded_cell_coords();
    for coord in cell_coords {
        let cell = match world.worldspace().get_cell(coord) {
            Some(c) => c,
            None => continue,
        };
        let root_ids = cell.get_root_ids();
        let cell_name = cell.name.clone();
        let cell_coord = cell.coord;
        let mut entities = serde_yaml::Sequence::new();
        for id in root_ids {
            if let Some(entity_value) = serialize_entity(world, id) {
                entities.push(entity_value);
            }
        }
        // Persist a cell if it has serialized entities or a user-assigned name.
        if entities.is_empty() && cell_name.is_empty() {
            continue;
        }
        let mut cell_map = serde_yaml::Mapping::new();
        cell_map.insert("name".into(), cell_name.into());
        cell_map.insert("entities".into(), serde_yaml::Value::Sequence(entities));
        cells.insert(
            coord_key(cell_coord).into(),
            serde_yaml::Value::Mapping(cell_map),
        );
    }

    // Include cells that have been streamed out of memory but still belong to this
    // worldspace, so unloaded regions are not lost on save.
    if let Ok(streaming) = world.get_resource::<WorldspaceStreaming>() {
        for (coord, value) in &streaming.source {
            let key: serde_yaml::Value = coord_key(*coord).into();
            if cells.contains_key(&key) {
                continue; // a currently-loaded cell already covers this coord
            }
            let has_entities = value
                .get("entities")
                .and_then(|v| v.as_sequence())
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            let has_name = value
                .get("name")
                .and_then(|v| v.as_str())
                .map(|n| !n.is_empty())
                .unwrap_or(false);
            if has_entities || has_name {
                cells.insert(key, value.clone());
            }
        }
    }

    let mut doc = serde_yaml::Mapping::new();
    doc.insert("name".into(), name.into());
    doc.insert("namespace".into(), namespace.into());
    doc.insert("class".into(), "worldspace".into());
    doc.insert("is_interior".into(), world.worldspace().is_interior.into());
    doc.insert("cells".into(), serde_yaml::Value::Mapping(cells));

    let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(doc))?;
    std::fs::write(path, yaml)?;

    Ok(())
}

fn populate_entity(world: &mut World, id: EntityId, value: &serde_yaml::Value) {
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Entity");
    world.set_name(id, name);

    if let Some(components) = value.get("components").and_then(|v| v.as_sequence()) {
        for comp_value in components {
            let type_name = comp_value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(reg) = get_component_registration(type_name) {
                let mut component = (reg.create)();
                let _ = (reg.deserialize)(&mut component, comp_value);
                (reg.add_to_world)(world, id, component);
            }
        }
    }

    if let Some(tags) = value.get("tags").and_then(|v| v.as_sequence()) {
        for tag_value in tags {
            if let Some(tag_name) = tag_value.as_str() {
                if let Some(reg) = get_tag_registration(tag_name) {
                    (reg.add_to_world)(world, id);
                }
            }
        }
    }

    if world.has_component::<TerrainChunk>(id) {
        world.add_tag::<NeedsTerrainRebuild>(id);
    }
}

/// Loads an entity, and its children, into `cell_coord`,
/// Root entities are placed directly in the given cell so the on-disk layout is preserved exactly
/// Children follow their parent's cell
fn load_entity(
    world: &mut World,
    value: &serde_yaml::Value,
    parent_id: Option<EntityId>,
    cell_coord: CellCoord,
) -> Result<()> {
    let entity_id = world.spawn_in_cell(cell_coord).id();
    populate_entity(world, entity_id, value);

    if let Some(pid) = parent_id {
        world.set_parent(entity_id, Some(pid))?;
    }

    if let Some(children) = value.get("children").and_then(|v| v.as_sequence()) {
        for child_value in children {
            load_entity(world, child_value, Some(entity_id), cell_coord)?;
        }
    }

    Ok(())
}

pub fn load_worldspace(
    world: &mut World,
    value: &serde_yaml::Value,
    keep_tags: &[&str],
) -> Result<()> {
    let root_ids = world.get_root_ids();

    // Resolve keep_tags names → TypeIds once before the loop.
    let keep_type_ids: Vec<std::any::TypeId> = keep_tags
        .iter()
        .filter_map(|name| get_tag_registration(name))
        .map(|reg| (reg.type_id)())
        .collect();

    for id in root_ids {
        let entity_tag_ids = world.worldspace().get_entity_tag_type_ids(id);
        let should_keep = keep_type_ids
            .iter()
            .any(|keep_id| entity_tag_ids.contains(keep_id));

        if !should_keep {
            world.despawn(id);
        }
    }

    let ws_name = value
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("default");
    world.insert_resource(crate::worldspaces::CurrentWorldspace(ws_name.to_string()));

    if let Some(v) = value.get("is_interior").and_then(|v| v.as_bool()) {
        world.worldspace_mut().is_interior = v;
    }

    if let Some(cells) = value.get("cells").and_then(|v| v.as_mapping()) {
        for (coord_value, cell_value) in cells {
            let Some(coord) = coord_value.as_str().and_then(parse_coord_key) else {
                continue;
            };

            // New format: { name, entities }. Legacy: a bare entities sequence.
            let (name, entities) = match cell_value {
                serde_yaml::Value::Mapping(_) => (
                    cell_value
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    cell_value.get("entities").and_then(|v| v.as_sequence()),
                ),
                _ => ("", cell_value.as_sequence()),
            };

            if !name.is_empty() {
                world.worldspace_mut().set_cell_name(coord, name);
            }
            if let Some(entities) = entities {
                for entity_value in entities {
                    load_entity(world, entity_value, None, coord)?;
                }
            }
        }
    } else if let Some(entities) = value.get("entities").and_then(|v| v.as_sequence()) {
        // Legacy flat scene format: place each entity by its transform position.
        for entity_value in entities {
            load_legacy_entity(world, entity_value, None)?;
        }
    }

    // Snapshot every loaded cell so the streaming system can unload far-away cells
    // and rebuild them when they re-enter render distance. Preserve any existing
    // render-distance configuration across worldspace switches.
    let mut source: HashMap<CellCoord, serde_yaml::Value> = HashMap::new();
    for coord in world.worldspace().loaded_cell_coords() {
        if let Some(snapshot) = serialize_cell(world, coord) {
            source.insert(coord, snapshot);
        }
    }
    let mut streaming = world
        .get_resource::<WorldspaceStreaming>()
        .cloned()
        .unwrap_or_default();
    // Every snapshotted cell is materialized in the world right now, so mark them
    // all as streamed-in; the streaming system unloads the far ones from here.
    streaming.loaded = source.keys().copied().collect();
    streaming.source = source;
    world.insert_resource(streaming);

    Ok(())
}

/// Legacy loader for the old flat `entities` scene format. Root entities are placed
/// in cell (0,0,0) by default (or the position-inferred cell for root entities).
fn load_legacy_entity(
    world: &mut World,
    value: &serde_yaml::Value,
    parent_id: Option<EntityId>,
) -> Result<()> {
    let entity_id = world.spawn().id();
    populate_entity(world, entity_id, value);

    if let Some(pid) = parent_id {
        world.set_parent(entity_id, Some(pid))?;
    }

    if let Some(children) = value.get("children").and_then(|v| v.as_sequence()) {
        for child_value in children {
            load_legacy_entity(world, child_value, Some(entity_id))?;
        }
    }

    Ok(())
}
