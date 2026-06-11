use anyhow::Result;
use std::path::Path;

use cgmath::Vector3;

use crate::{
    objects::{
        Object,
        cell::{CellCoord, ObjectId},
        component::{BoxedComponent, get_component_registration},
        components::transform::Transform,
        tag::get_tag_registration,
        world::World,
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
            map.insert("local_euler_angles".into(), vec3_to_yaml(t.local_euler_angles));
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

fn serialize_object(world: &World, id: ObjectId) -> Option<serde_yaml::Value> {
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
    map.insert("components".into(), serde_yaml::Value::Sequence(components_data));
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

/// Saves the active worldspace to a single YAML file. Each loaded, non-empty
/// cell becomes one entry under `cells`, keyed by its `"x,y,z"` coordinate, with
/// the cell's root object tree serialized in the existing per-object format.
pub fn save_worldspace(world: &World, name: &str, namespace: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut cells = serde_yaml::Mapping::new();
    for cell in world.worldspace().loaded_cells() {
        let mut objects = serde_yaml::Sequence::new();
        for (id, _) in cell.get_root_objects() {
            if let Some(obj_value) = serialize_object(world, id) {
                objects.push(obj_value);
            }
        }
        // Persist a cell if it has serialized objects or a user-assigned name.
        if objects.is_empty() && cell.name.is_empty() {
            continue;
        }
        let mut cell_map = serde_yaml::Mapping::new();
        cell_map.insert("name".into(), cell.name.clone().into());
        cell_map.insert("objects".into(), serde_yaml::Value::Sequence(objects));
        cells.insert(coord_key(cell.coord).into(), serde_yaml::Value::Mapping(cell_map));
    }

    let mut doc = serde_yaml::Mapping::new();
    doc.insert("name".into(), name.into());
    doc.insert("namespace".into(), namespace.into());
    doc.insert("class".into(), "worldspace".into());
    doc.insert("cells".into(), serde_yaml::Value::Mapping(cells));

    let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(doc))?;
    std::fs::write(path, yaml)?;

    Ok(())
}

fn build_object(value: &serde_yaml::Value) -> Object {
    let name = value.get("name").and_then(|v| v.as_str()).unwrap_or("Object");

    let mut obj = Object::new();
    obj.name = name.to_string();

    if let Some(components) = value.get("components").and_then(|v| v.as_sequence()) {
        for comp_value in components {
            let type_name = comp_value.get("type").and_then(|v| v.as_str()).unwrap_or("");
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

    obj
}

/// Loads an object (and its children) into `cell_coord`. Root objects are placed
/// directly in the given cell so the on-disk layout is preserved exactly;
/// children follow their parent's cell.
fn load_object(
    world: &mut World,
    value: &serde_yaml::Value,
    parent_id: Option<ObjectId>,
    cell_coord: CellCoord,
) -> Result<()> {
    let obj = build_object(value);

    let obj_id = match parent_id {
        Some(pid) => world.add_child_object(pid, obj)?,
        None => world.add_object_to_cell(cell_coord, obj),
    };

    if let Some(children) = value.get("children").and_then(|v| v.as_sequence()) {
        for child_value in children {
            load_object(world, child_value, Some(obj_id), cell_coord)?;
        }
    }

    Ok(())
}

/// Removes all root objects not tagged with any of `keep_tags`, then loads the
/// worldspace described by `value`. Supports both the new `cells` map format and
/// the legacy flat `objects` list (objects are auto-placed by position).
pub fn load_worldspace(world: &mut World, value: &serde_yaml::Value, keep_tags: &[&str]) -> Result<()> {
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

    if let Some(cells) = value.get("cells").and_then(|v| v.as_mapping()) {
        for (coord_value, cell_value) in cells {
            let Some(coord) = coord_value.as_str().and_then(parse_coord_key) else {
                continue;
            };

            // New format: { name, objects }. Legacy: a bare objects sequence.
            let (name, objects) = match cell_value {
                serde_yaml::Value::Mapping(_) => (
                    cell_value.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    cell_value.get("objects").and_then(|v| v.as_sequence()),
                ),
                _ => ("", cell_value.as_sequence()),
            };

            if !name.is_empty() {
                world.worldspace_mut().set_cell_name(coord, name);
            }
            if let Some(objects) = objects {
                for obj_value in objects {
                    load_object(world, obj_value, None, coord)?;
                }
            }
        }
    } else if let Some(objects) = value.get("objects").and_then(|v| v.as_sequence()) {
        // Legacy flat scene format: place each object by its transform position.
        for obj_value in objects {
            load_legacy_object(world, obj_value, None)?;
        }
    }

    Ok(())
}

/// Legacy loader for the old flat `objects` scene format. Root objects are added
/// via `add_object` (auto-placed into the cell containing their transform).
fn load_legacy_object(
    world: &mut World,
    value: &serde_yaml::Value,
    parent_id: Option<ObjectId>,
) -> Result<()> {
    let obj = build_object(value);

    let obj_id = match parent_id {
        Some(pid) => world.add_child_object(pid, obj)?,
        None => world.add_object(obj),
    };

    if let Some(children) = value.get("children").and_then(|v| v.as_sequence()) {
        for child_value in children {
            load_legacy_object(world, child_value, Some(obj_id))?;
        }
    }

    Ok(())
}
