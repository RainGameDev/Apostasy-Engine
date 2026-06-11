use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::Path;

use cgmath::Vector3;

use crate::{
    objects::{
        Object,
        component::{BoxedComponent, get_component_registration},
        components::transform::Transform,
        object_store::ObjectId,
        tag::get_tag_registration,
        world::World,
        worldspace::{ActiveWorldspace, CellData, CellKey, WorldspaceKind},
    },
    physics::{
        Gravity,
        collider::{Collider, ColliderShape},
        velocity::Velocity,
    },
    rendering::components::{camera::Camera, model_renderer::ModelRenderer},
};

fn vec3_to_yaml(v: Vector3<f32>) -> serde_yaml::Value {
    serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Number(serde_yaml::Number::from(v.x as f64)),
        serde_yaml::Value::Number(serde_yaml::Number::from(v.y as f64)),
        serde_yaml::Value::Number(serde_yaml::Number::from(v.z as f64)),
    ])
}

fn serialize_component(component: &BoxedComponent) -> Option<serde_yaml::Value> {
    let type_name = component.type_name();
    let short = type_name.rsplit("::").next()?;

    let mut map = serde_yaml::Mapping::new();
    map.insert("type".into(), short.into());

    match short {
        "Transform" => {
            let t = component.as_any().downcast_ref::<Transform>()?;
            map.insert("local_position".into(), vec3_to_yaml(t.local_position));
            map.insert(
                "local_euler_angles".into(),
                vec3_to_yaml(t.local_euler_angles),
            );
            map.insert("local_scale".into(), vec3_to_yaml(t.local_scale));
        }
        "ModelRenderer" => {
            let mr = component.as_any().downcast_ref::<ModelRenderer>()?;
            map.insert("model_path".into(), mr.model_path.clone().into());
            map.insert("is_wireframe".into(), mr.is_wireframe.into());
        }
        "Camera" => {
            let c = component.as_any().downcast_ref::<Camera>()?;
            map.insert("fov_y".into(), (c.fov_y as f64).into());
            map.insert("near".into(), (c.near as f64).into());
            map.insert("far".into(), (c.far as f64).into());
            map.insert("is_main".into(), c.is_main.into());
        }
        "Collider" => {
            let col = component.as_any().downcast_ref::<Collider>()?;
            match &col.shape {
                ColliderShape::Cuboid { size } => {
                    map.insert("shape".into(), "Cuboid".into());
                    map.insert("size".into(), vec3_to_yaml(*size));
                }
                ColliderShape::Sphere { radius } => {
                    map.insert("shape".into(), "Sphere".into());
                    map.insert("radius".into(), (*radius as f64).into());
                }
                ColliderShape::Capsule { radius, height } => {
                    map.insert("shape".into(), "Capsule".into());
                    map.insert("radius".into(), (*radius as f64).into());
                    map.insert("height".into(), (*height as f64).into());
                }
                ColliderShape::Cylinder { radius, height } => {
                    map.insert("shape".into(), "Cylinder".into());
                    map.insert("radius".into(), (*radius as f64).into());
                    map.insert("height".into(), (*height as f64).into());
                }
            }
            map.insert("offset".into(), vec3_to_yaml(col.offset));
            map.insert("is_static".into(), col.is_static.into());
            map.insert("is_area".into(), col.is_area.into());
        }
        "Velocity" => {
            let v = component.as_any().downcast_ref::<Velocity>()?;
            map.insert("mass".into(), (v.mass as f64).into());
            map.insert("process".into(), v.process.into());
            map.insert("mu_static".into(), (v.mu_static as f64).into());
            map.insert("mu_kinetic".into(), (v.mu_kinetic as f64).into());
            map.insert("restitution".into(), (v.restitution as f64).into());
            map.insert("linear_damping".into(), (v.linear_damping as f64).into());
            map.insert("angular_damping".into(), (v.angular_damping as f64).into());
        }
        "Gravity" => {
            let g = component.as_any().downcast_ref::<Gravity>()?;
            map.insert("strength".into(), (g.strength as f64).into());
        }
        _ => return None,
    }

    Some(serde_yaml::Value::Mapping(map))
}

pub fn serialize_object(world: &World, id: ObjectId) -> Option<serde_yaml::Value> {
    let (name, children_ids, components_data, tags_data) = {
        let obj = world.get_object(id)?;

        let should_skip = obj.tags.iter().any(|t| {
            let tn = t.type_name();
            tn.ends_with("EditorCamera") || tn.ends_with("SkipsSerilization")
        });
        if should_skip {
            return None;
        }

        let name = obj.name.clone();
        let children_ids: Vec<ObjectId> = obj.children.clone();

        let components_data: serde_yaml::Sequence = obj
            .components
            .iter()
            .filter_map(serialize_component)
            .collect();

        let tags_data: serde_yaml::Sequence = obj
            .tags
            .iter()
            .filter_map(|t| {
                let n = t.type_name().rsplit("::").next()?.to_string();
                if matches!(n.as_str(), "EditorCamera" | "ActiveCamera") {
                    return None;
                }
                Some(serde_yaml::Value::String(n))
            })
            .collect();

        (name, children_ids, components_data, tags_data)
    };

    let children: serde_yaml::Sequence = children_ids
        .iter()
        .filter_map(|&child_id| serialize_object(world, child_id))
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

/// Instantiates an object (and its children) from yaml. Returns the created id.
pub fn load_object(
    world: &mut World,
    value: &serde_yaml::Value,
    parent_id: Option<ObjectId>,
) -> Result<ObjectId> {
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Object");

    let mut obj = Object::new();
    obj.name = name.to_string();

    if let Some(components) = value.get("components").and_then(|v| v.as_sequence()) {
        for comp_value in components {
            let type_name = comp_value
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Some(reg) = get_component_registration(type_name) {
                let mut component = (reg.create)();
                let _ = (reg.deserialize)(&mut component, comp_value);
                obj.components.push(component);
            }
        }
    }

    if let Some(tags) = value.get("tags").and_then(|v| v.as_sequence()) {
        for tag_value in tags {
            if let Some(tag_name) = tag_value.as_str() {
                if let Some(reg) = get_tag_registration(tag_name) {
                    obj.tags.push((reg.create)());
                }
            }
        }
    }

    let obj_id = match parent_id {
        Some(pid) => world.store.add_child_object(pid, obj)?,
        None => world.add_object(obj),
    };

    if let Some(children) = value.get("children").and_then(|v| v.as_sequence()) {
        for child_value in children {
            load_object(world, child_value, Some(obj_id))?;
        }
    }

    Ok(obj_id)
}

/// True for objects that live outside the cell system (player, cameras, editor-only objects).
/// These are never despawned when their cell unloads.
fn is_cell_independent(obj: &Object) -> bool {
    obj.tags.iter().any(|t| {
        let tn = t.type_name();
        tn.ends_with("Player")
            || tn.ends_with("EditorCamera")
            || tn.ends_with("GameCamera")
            || tn.ends_with("ActiveCamera")
            || tn.ends_with("SkipsSerilization")
    })
}

/// Removes all root objects not tagged with any of `keep_tags`.
pub fn clear_world(world: &mut World, keep_tags: &[&str]) {
    let root_ids: Vec<ObjectId> = world
        .get_root_objects()
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    for id in root_ids {
        let should_keep = world
            .get_object(id)
            .map(|obj| {
                obj.tags.iter().any(|t| {
                    let short = t.type_name().rsplit("::").next().unwrap_or("");
                    keep_tags.contains(&short)
                })
            })
            .unwrap_or(false);

        if !should_keep {
            world.remove_object(id);
        }
    }
}

/// Which cell a root object belongs to, based on its current position.
fn cell_for_object(world: &World, ws: &ActiveWorldspace, id: ObjectId) -> CellKey {
    match ws.kind {
        WorldspaceKind::Exterior => {
            let pos = world
                .get_object(id)
                .and_then(|o| o.get_component::<Transform>().ok())
                .map(|t| t.global_position)
                .unwrap_or(Vector3::new(0.0, 0.0, 0.0));
            CellKey::from_position(pos)
        }
        WorldspaceKind::Interior => ws
            .active_cell
            .clone()
            .unwrap_or(CellKey::Named("main".to_string())),
    }
}

// ========== ========== Worldspace loading ========== ==========

fn parse_cells(value: &serde_yaml::Value) -> HashMap<CellKey, CellData> {
    let mut cells = HashMap::new();
    if let Some(seq) = value.get("cells").and_then(|v| v.as_sequence()) {
        for cell_value in seq {
            let key = if let Some(coord) =
                cell_value.get("coord").and_then(|v| v.as_sequence())
            {
                CellKey::Grid(
                    coord.first().and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                    coord.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                )
            } else if let Some(name) = cell_value.get("name").and_then(|v| v.as_str()) {
                CellKey::Named(name.to_string())
            } else {
                continue;
            };

            let objects = cell_value
                .get("objects")
                .and_then(|v| v.as_sequence())
                .cloned()
                .unwrap_or_default();

            cells
                .entry(key)
                .or_insert_with(CellData::default)
                .objects
                .extend(objects);
        }
    }
    cells
}

/// Replaces the world content with a worldspace parsed from yaml.
/// Exterior worldspaces load all cells when `load_all` is set (editor) and rely on
/// the streaming system otherwise (game). Interior worldspaces load their first cell.
pub fn open_worldspace(
    world: &mut World,
    value: &serde_yaml::Value,
    keep_tags: &[&str],
    load_all: bool,
) -> Result<()> {
    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Missing 'name' in worldspace"))?
        .to_string();
    let kind = WorldspaceKind::from_str(value.get("kind").and_then(|v| v.as_str()).unwrap_or(""));

    clear_world(world, keep_tags);

    let mut ws = ActiveWorldspace::new(&name, kind);
    ws.cells = parse_cells(value);
    world.insert_resource(ws);

    match kind {
        WorldspaceKind::Exterior => {
            if load_all {
                let keys: Vec<CellKey> = world
                    .get_resource::<ActiveWorldspace>()?
                    .cells
                    .keys()
                    .cloned()
                    .collect();
                for key in keys {
                    load_cell(world, &key)?;
                }
            }
        }
        WorldspaceKind::Interior => {
            let first = world
                .get_resource::<ActiveWorldspace>()?
                .all_cell_keys()
                .into_iter()
                .next()
                .unwrap_or(CellKey::Named("main".to_string()));
            enter_cell(world, &first)?;
        }
    }

    Ok(())
}

/// Instantiates an unloaded cell's objects into the world.
pub fn load_cell(world: &mut World, key: &CellKey) -> Result<()> {
    let data = {
        let ws = world.get_resource_mut::<ActiveWorldspace>()?;
        ws.cells.remove(key)
    };

    let Some(data) = data else {
        // Nothing stored for this cell; still mark it as loaded so new objects can join it.
        let ws = world.get_resource_mut::<ActiveWorldspace>()?;
        ws.loaded.entry(key.clone()).or_default();
        return Ok(());
    };

    let mut root_ids = Vec::new();
    for obj_value in &data.objects {
        root_ids.push(load_object(world, obj_value, None)?);
    }

    let ws = world.get_resource_mut::<ActiveWorldspace>()?;
    ws.loaded.entry(key.clone()).or_default().extend(root_ids);
    Ok(())
}

/// Serializes a loaded cell's objects back into the worldspace data and despawns them.
/// Exterior objects that drifted into another loaded cell migrate there instead.
pub fn unload_cell(world: &mut World, key: &CellKey) -> Result<()> {
    let ids = {
        let ws = world.get_resource_mut::<ActiveWorldspace>()?;
        ws.loaded.remove(key).unwrap_or_default()
    };

    for id in ids {
        let Some(obj) = world.get_object(id) else {
            continue;
        };
        if is_cell_independent(obj) {
            continue;
        }

        let target = {
            let ws = world.get_resource::<ActiveWorldspace>()?;
            cell_for_object(world, ws, id)
        };

        let still_loaded = {
            let ws = world.get_resource::<ActiveWorldspace>()?;
            target != *key && ws.loaded.contains_key(&target)
        };

        if still_loaded {
            let ws = world.get_resource_mut::<ActiveWorldspace>()?;
            ws.loaded.entry(target).or_default().push(id);
            continue;
        }

        let serialized = serialize_object(world, id);
        world.remove_object(id);
        if let Some(value) = serialized {
            let ws = world.get_resource_mut::<ActiveWorldspace>()?;
            ws.cells.entry(target).or_default().objects.push(value);
        }
    }

    Ok(())
}

/// Switches the active cell of an interior worldspace (also used to load a
/// specific exterior cell on demand).
pub fn enter_cell(world: &mut World, key: &CellKey) -> Result<()> {
    let (kind, loaded_keys) = {
        let ws = world.get_resource::<ActiveWorldspace>()?;
        (ws.kind, ws.loaded.keys().cloned().collect::<Vec<_>>())
    };

    if kind == WorldspaceKind::Interior {
        for loaded in loaded_keys {
            unload_cell(world, &loaded)?;
        }
        world.get_resource_mut::<ActiveWorldspace>()?.active_cell = Some(key.clone());
    }

    load_cell(world, key)
}

/// Re-assigns every live root object to the cell its position falls in.
/// Run before saving so moved and newly created objects land in the right cell.
pub fn sync_loaded_cells(world: &mut World) -> Result<()> {
    let kind = world.get_resource::<ActiveWorldspace>()?.kind;

    let root_ids: Vec<ObjectId> = world
        .get_root_objects()
        .into_iter()
        .filter(|(_, obj)| !is_cell_independent(obj))
        .map(|(id, _)| id)
        .collect();

    let mut buckets: HashMap<CellKey, Vec<ObjectId>> = HashMap::new();
    for id in root_ids {
        let key = {
            let ws = world.get_resource::<ActiveWorldspace>()?;
            cell_for_object(world, ws, id)
        };
        buckets.entry(key).or_default().push(id);
    }

    let ws = world.get_resource_mut::<ActiveWorldspace>()?;
    // Keep previously loaded (possibly now empty) cells alive as loaded entries.
    for ids in ws.loaded.values_mut() {
        ids.clear();
    }
    for (key, ids) in buckets {
        ws.loaded.entry(key).or_default().extend(ids);
    }

    // Interior worldspaces only ever have the active cell loaded.
    if kind == WorldspaceKind::Interior {
        let active = ws
            .active_cell
            .clone()
            .unwrap_or(CellKey::Named("main".to_string()));
        let all: Vec<ObjectId> = ws.loaded.drain().flat_map(|(_, ids)| ids).collect();
        ws.loaded.insert(active, all);
    }

    Ok(())
}

// ========== ========== Saving ========== ==========

fn cell_to_yaml(key: &CellKey, objects: serde_yaml::Sequence) -> serde_yaml::Value {
    let mut map = serde_yaml::Mapping::new();
    match key {
        CellKey::Grid(x, y) => {
            map.insert(
                "coord".into(),
                serde_yaml::Value::Sequence(vec![(*x as i64).into(), (*y as i64).into()]),
            );
        }
        CellKey::Named(name) => {
            map.insert("name".into(), name.clone().into());
        }
    }
    map.insert("objects".into(), serde_yaml::Value::Sequence(objects));
    serde_yaml::Value::Mapping(map)
}

/// Serializes the whole active worldspace (loaded and unloaded cells) to yaml.
pub fn worldspace_to_yaml(world: &mut World, name: &str, namespace: &str) -> Result<serde_yaml::Value> {
    sync_loaded_cells(world)?;

    let (kind, keys) = {
        let ws = world.get_resource::<ActiveWorldspace>()?;
        (ws.kind, ws.all_cell_keys())
    };

    let mut cells = serde_yaml::Sequence::new();
    for key in keys {
        let mut objects = serde_yaml::Sequence::new();

        let loaded_ids = {
            let ws = world.get_resource::<ActiveWorldspace>()?;
            ws.loaded.get(&key).cloned().unwrap_or_default()
        };
        for id in loaded_ids {
            if let Some(value) = serialize_object(world, id) {
                objects.push(value);
            }
        }

        if let Ok(ws) = world.get_resource::<ActiveWorldspace>() {
            if let Some(data) = ws.cells.get(&key) {
                objects.extend(data.objects.iter().cloned());
            }
        }

        if !objects.is_empty() {
            cells.push(cell_to_yaml(&key, objects));
        }
    }

    let mut doc = serde_yaml::Mapping::new();
    doc.insert("name".into(), name.into());
    doc.insert("namespace".into(), namespace.into());
    doc.insert("class".into(), "worldspace".into());
    doc.insert("kind".into(), kind.as_str().into());
    doc.insert("cells".into(), serde_yaml::Value::Sequence(cells));

    Ok(serde_yaml::Value::Mapping(doc))
}

/// Saves the active worldspace to a yaml file (authored data, edited by the editor).
pub fn save_worldspace(world: &mut World, name: &str, namespace: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    world.get_resource_mut::<ActiveWorldspace>()?.name = name.to_string();
    let doc = worldspace_to_yaml(world, name, namespace)?;
    std::fs::write(path, serde_yaml::to_string(&doc)?)?;
    Ok(())
}

// ========== ========== Saved state (runtime persistence) ========== ==========

/// Writes the runtime state of the active worldspace (all cell data, including
/// changes made during play) to a save file. Authored worldspace files stay untouched.
pub fn save_world_state(world: &mut World, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let name = world.get_resource::<ActiveWorldspace>()?.name.clone();
    let mut doc = worldspace_to_yaml(world, &name, "save")?;
    if let serde_yaml::Value::Mapping(ref mut map) = doc {
        map.insert("class".into(), "worldstate".into());
        if let Ok(ws) = world.get_resource::<ActiveWorldspace>() {
            if let Some(CellKey::Named(active)) = &ws.active_cell {
                map.insert("active_cell".into(), active.clone().into());
            }
        }
    }
    std::fs::write(path, serde_yaml::to_string(&doc)?)?;
    Ok(())
}

/// Restores a previously saved world state. The saved cell data fully overrides
/// the authored worldspace cells.
pub fn load_world_state(world: &mut World, path: &Path, keep_tags: &[&str]) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let value: serde_yaml::Value = serde_yaml::from_str(&content)?;

    open_worldspace(world, &value, keep_tags, false)?;

    if let Some(active) = value.get("active_cell").and_then(|v| v.as_str()) {
        enter_cell(world, &CellKey::Named(active.to_string()))?;
    }

    Ok(())
}
