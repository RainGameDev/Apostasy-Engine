use anyhow::Result;
use apostasy_macros::{Resource, late_update};
use hashbrown::{HashMap, HashSet};

use crate::{
    objects::{
        cell::{CellCoord, world_to_cell},
        components::transform::Transform,
        world::World,
        worldspace_serializer::{load_cell, serialize_cell},
    },
    rendering::components::camera::ActiveCamera,
};

/// Smallest allowed render distance, in cells.
pub const MIN_RENDER_DISTANCE: i32 = 1;
/// Largest allowed render distance, in cells.
pub const MAX_RENDER_DISTANCE: i32 = 999;

/// Controls cell streaming around the active camera. Cells whose grid distance from
/// the camera's cell exceeds `render_distance` are serialized into `source` and
/// dropped from memory; cells that come back within range are rebuilt from `source`.
///
/// `source` holds the last-known `{ name, objects }` snapshot of every cell that
/// belongs to the active worldspace, including ones not currently loaded. It is
/// (re)populated whenever a worldspace is loaded.
///
/// `loaded` tracks which `source` cells are currently materialized in the world,
/// so we can tell "this cell's content is streamed in" apart from "a cell object
/// exists here for some other reason" (e.g. the camera stands in it, or an object
/// migrated into it). Relying on cell existence alone would leave a cell's saved
/// content unloaded whenever the camera enters an otherwise-empty coordinate.
#[derive(Resource, Clone)]
pub struct WorldspaceStreaming {
    pub enabled: bool,
    pub render_distance: i32,
    pub source: HashMap<CellCoord, serde_yaml::Value>,
    pub loaded: HashSet<CellCoord>,
}

impl Default for WorldspaceStreaming {
    fn default() -> Self {
        Self {
            enabled: true,
            render_distance: 8,
            source: HashMap::new(),
            loaded: HashSet::new(),
        }
    }
}

impl WorldspaceStreaming {
    /// Sets the render distance, clamped to the supported range.
    pub fn set_render_distance(&mut self, distance: i32) {
        self.render_distance = distance.clamp(MIN_RENDER_DISTANCE, MAX_RENDER_DISTANCE);
    }
}

/// Streams cells in and out around the active camera so that only cells within
/// `render_distance` (Chebyshev distance, in cells) stay loaded. Cells leaving
/// range are snapshotted back into [`WorldspaceStreaming::source`] before being
/// dropped, and cells re-entering range are rebuilt from that snapshot.
#[late_update(mode = "all")]
pub fn worldspace_streaming_system(world: &mut World) -> Result<()> {
    let render_distance = match world.get_resource::<WorldspaceStreaming>() {
        Ok(s) if s.enabled => s.render_distance,
        _ => return Ok(()),
    };

    // The active camera's cell is the centre of the loaded region.
    let Ok(camera) = world.get_object_with_tag::<ActiveCamera>() else {
        return Ok(());
    };
    let Ok(transform) = camera.get_component::<Transform>() else {
        return Ok(());
    };
    let center = world_to_cell(transform.global_position);

    let in_range = |c: CellCoord| {
        (c.x - center.x).abs() <= render_distance && (c.z - center.z).abs() <= render_distance
    };

    // Unload any loaded cell that has moved out of range, snapshotting it first so
    // changes made while loaded (including objects that migrated in) survive and can
    // be restored later.
    for coord in world.worldspace().loaded_cell_coords() {
        if in_range(coord) {
            continue;
        }
        if let Some(snapshot) = serialize_cell(world, coord) {
            world
                .get_resource_mut::<WorldspaceStreaming>()?
                .source
                .insert(coord, snapshot);
        }
        world.worldspace_mut().unload_cell(coord);
        world
            .get_resource_mut::<WorldspaceStreaming>()?
            .loaded
            .remove(&coord);
    }

    // Stream in source cells that are back in range but whose content isn't loaded
    // yet. This is tracked by `loaded` rather than cell existence, so a cell still
    // streams its content in even if the camera is already standing in it.
    let to_load: Vec<CellCoord> = {
        let streaming = world.get_resource::<WorldspaceStreaming>()?;
        streaming
            .source
            .keys()
            .copied()
            .filter(|&c| in_range(c) && !streaming.loaded.contains(&c))
            .collect()
    };
    for coord in to_load {
        let value = world
            .get_resource::<WorldspaceStreaming>()?
            .source
            .get(&coord)
            .cloned();
        if let Some(value) = value {
            let _ = load_cell(world, coord, &value);
            world
                .get_resource_mut::<WorldspaceStreaming>()?
                .loaded
                .insert(coord);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::cell_streaming::cell_streaming_system;
    use crate::objects::{Object, components::transform::Transform};
    use crate::rendering::components::camera::ActiveCamera;
    use cgmath::{Vector3, Zero};

    fn count_named(world: &World, name: &str) -> usize {
        world
            .worldspace()
            .get_all_objects()
            .iter()
            .filter(|(_, o)| o.name == name)
            .count()
    }

    fn camera_at(world: &mut World, pos: Vector3<f32>) {
        let cams: Vec<_> = world
            .get_objects_with_tag_with_ids::<ActiveCamera>()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        for id in cams {
            world.remove_object(id);
        }
        let mut cam = Object::new();
        cam.name = "Camera".into();
        cam.add_component(Transform {
            global_position: pos,
            local_position: pos,
            ..Default::default()
        });
        cam.add_tag(ActiveCamera);
        world.add_object(cam);
    }

    fn far_cell_object_count(world: &World) -> usize {
        // Count only the streamed content, not the camera that may share the cell.
        count_named(world, "FarObject")
    }

    #[test]
    fn reload_does_not_duplicate_objects() {
        let mut world = World::default();

        // A populated far cell at (5,0,5).
        let far = Vector3::new(5, 0, 5);
        let mut obj = Object::new();
        obj.name = "FarObject".into();
        obj.add_component(Transform {
            global_position: Vector3::new(5.0 * 128.0 + 1.0, 0.0, 5.0 * 128.0 + 1.0),
            local_position: Vector3::new(5.0 * 128.0 + 1.0, 0.0, 5.0 * 128.0 + 1.0),
            ..Default::default()
        });
        world.worldspace_mut().get_or_create_cell(far).add_object(obj);
        assert_eq!(far_cell_object_count(&world), 1);

        // Snapshot it into the streaming source, as load_worldspace would.
        let snapshot = serialize_cell(&world, far).unwrap();
        let mut streaming = WorldspaceStreaming::default();
        streaming.render_distance = 1;
        streaming.source.insert(far, snapshot);
        world.insert_resource(streaming);

        // Camera far away -> the far cell unloads.
        camera_at(&mut world, Vector3::zero());
        worldspace_streaming_system(&mut world).unwrap();
        assert_eq!(far_cell_object_count(&world), 0, "far cell should unload");

        // Camera near the far cell -> it reloads, with exactly one object.
        camera_at(&mut world, Vector3::new(5.0 * 128.0, 0.0, 5.0 * 128.0));
        for _ in 0..5 {
            worldspace_streaming_system(&mut world).unwrap();
        }
        assert_eq!(
            far_cell_object_count(&world),
            1,
            "reload must not duplicate objects"
        );
    }

    #[test]
    fn reload_with_migration_does_not_duplicate() {
        let mut world = World::default();

        // Source snapshot for a far cell, built so reload goes through the real
        // deserialize path (where the stale-global-position bug lived).
        let far = Vector3::new(5, 0, 5);
        let cell_yaml: serde_yaml::Value = serde_yaml::from_str(
            r#"
name: ""
objects:
  - name: FarObject
    components:
      - type: Transform
        local_position: [641.0, 0.0, 641.0]
        local_euler_angles: [0.0, 0.0, 0.0]
        local_scale: [1.0, 1.0, 1.0]
    tags: []
    children: []
"#,
        )
        .unwrap();

        let mut streaming = WorldspaceStreaming::default();
        streaming.render_distance = 1;
        streaming.source.insert(far, cell_yaml);
        world.insert_resource(streaming);

        let origin = Vector3::zero();
        let near_far = Vector3::new(640.0, 0.0, 640.0);

        // Oscillate the camera in and out of the far cell, running both the cell
        // streaming (object migration) and worldspace streaming systems each frame,
        // exactly as the engine does in late_update.
        for _ in 0..6 {
            camera_at(&mut world, origin);
            cell_streaming_system(&mut world).unwrap();
            worldspace_streaming_system(&mut world).unwrap();

            camera_at(&mut world, near_far);
            for _ in 0..3 {
                cell_streaming_system(&mut world).unwrap();
                worldspace_streaming_system(&mut world).unwrap();
            }
        }

        assert_eq!(
            count_named(&world, "FarObject"),
            1,
            "object must not be duplicated across reload + migration cycles"
        );
    }
}
