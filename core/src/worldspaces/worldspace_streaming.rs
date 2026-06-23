use anyhow::Result;
use apostasy_macros::{Resource, late_update};
use hashbrown::{HashMap, HashSet};

use crate::{
    ecs::{
        cell::{CellCoord, world_to_cell},
        components::transform::Transform,
        world::World,
        worldspace_serializer::{load_cell, serialize_cell},
    },
    rendering::components::{camera::ActiveCamera, lighting::Light},
};

/// Smallest allowed render distance, in cells.
pub const MIN_RENDER_DISTANCE: i32 = 1;
/// Largest allowed render distance, in cells.
pub const MAX_RENDER_DISTANCE: i32 = 999;

/// Controls cell streaming around the active camera. Cells whose grid distance from
/// the camera's cell exceeds `render_distance` are serialized into `source` and
/// dropped from memory; cells that come back within range are rebuilt from `source`.
///
/// `source` holds the last-known `{ name, entities }` snapshot of every cell that
/// belongs to the active worldspace, including ones not currently loaded
///
/// `loaded` tracks which `source` cells are currently shown in the world
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

#[late_update(mode = "all")]
pub fn worldspace_streaming_system(world: &mut World) -> Result<()> {
    let render_distance = match world.get_resource::<WorldspaceStreaming>() {
        Ok(s) if s.enabled => s.render_distance,
        _ => return Ok(()),
    };

    // The active camera's cell is the centre of the loaded region.
    let Ok(camera_id) = world.get_entity_with_tag::<ActiveCamera>() else {
        return Ok(());
    };
    let Some(global_position) = world.get_component::<Transform>(camera_id).map(|t| t.global_position) else {
        return Ok(());
    };
    let center = world_to_cell(global_position);

    let in_range = |c: CellCoord| {
        (c.x - center.x).abs() <= render_distance && (c.z - center.z).abs() <= render_distance
    };

    // Pre-collect all cells that contain emitting lights so we never evict them.
    // Directional and global lights should illuminate the whole world regardless of
    // where the camera is, so their host cell must stay resident.
    let light_cells: std::collections::HashSet<CellCoord> = world
        .get_entities_with_component::<Light>()
        .into_iter()
        .filter(|&id| {
            world.get_component::<Light>(id).map(|l| l.is_emitting).unwrap_or(false)
        })
        .map(|id| id.cell)
        .collect();

    // Unload any loaded cell that has moved out of range, snapshotting it first so
    // changes made while loaded (including entities that migrated in) survive and can
    // be restored later.
    for coord in world.worldspace().loaded_cell_coords() {
        if in_range(coord) {
            continue;
        }
        // Never evict a cell that hosts an emitting light.
        if light_cells.contains(&coord) {
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
    use crate::worldspaces::cell_streaming::cell_streaming_system;
    use crate::ecs::components::transform::Transform;
    use crate::rendering::components::camera::ActiveCamera;
    use cgmath::{Vector3, Zero};

    fn count_named(world: &World, name: &str) -> usize {
        world
            .get_all_ids()
            .iter()
            .filter(|&&id| world.get_name(id).map(|n| n == name).unwrap_or(false))
            .count()
    }

    fn camera_at(world: &mut World, pos: Vector3<f32>) {
        let cams = world.get_entities_with_tag::<ActiveCamera>();
        for id in cams {
            world.despawn(id);
        }
        let cam_id = world.spawn().id();
        world.set_name(cam_id, "Camera");
        world.add_component(cam_id, Transform {
            global_position: pos,
            local_position: pos,
            ..Default::default()
        });
        world.add_tag::<ActiveCamera>(cam_id);
    }

    fn far_cell_entity_count(world: &World) -> usize {
        // Count only the streamed content, not the camera that may share the cell.
        count_named(world, "FarEntity")
    }

    #[test]
    fn reload_does_not_duplicate_entities() {
        let mut world = World::default();

        // A populated far cell at (5,0,5).
        let far = Vector3::new(5, 0, 5);
        let far_id = world.spawn_in_cell(far).id();
        world.set_name(far_id, "FarEntity");
        world.add_component(far_id, Transform {
            global_position: Vector3::new(5.0 * 128.0 + 1.0, 0.0, 5.0 * 128.0 + 1.0),
            local_position: Vector3::new(5.0 * 128.0 + 1.0, 0.0, 5.0 * 128.0 + 1.0),
            ..Default::default()
        });
        assert_eq!(far_cell_entity_count(&world), 1);

        // Snapshot it into the streaming source, as load_worldspace would.
        let snapshot = serialize_cell(&world, far).unwrap();
        let mut streaming = WorldspaceStreaming::default();
        streaming.render_distance = 1;
        streaming.source.insert(far, snapshot);
        world.insert_resource(streaming);

        // Camera far away -> the far cell unloads.
        camera_at(&mut world, Vector3::zero());
        worldspace_streaming_system(&mut world).unwrap();
        assert_eq!(far_cell_entity_count(&world), 0, "far cell should unload");

        // Camera near the far cell -> it reloads, with exactly one entity.
        camera_at(&mut world, Vector3::new(5.0 * 128.0, 0.0, 5.0 * 128.0));
        for _ in 0..5 {
            worldspace_streaming_system(&mut world).unwrap();
        }
        assert_eq!(
            far_cell_entity_count(&world),
            1,
            "reload must not duplicate entities"
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
entities:
  - name: FarEntity
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
            count_named(&world, "FarEntity"),
            1,
            "entity must not be duplicated across reload + migration cycles"
        );
    }
}
