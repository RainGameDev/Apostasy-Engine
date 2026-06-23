use anyhow::Result;
use apostasy_macros::{Resource, late_update};
use hashbrown::HashMap;

use crate::ecs::{
    cell::{CellCoord, ObjectId, world_to_cell},
    components::transform::Transform,
    world::World,
};

/// Records `old id -> new id` for objects whose `ObjectId` changed because they
/// crossed a cell boundary during the most recent late update. Populated by
/// [`cell_streaming_system`] (and replaced each frame). Consumers that cache
/// `ObjectId`s (e.g. editor selection) can use this to keep their references
/// pointing at the migrated object.
#[derive(Resource, Default, Clone)]
pub struct CellMigrations {
    pub remap: HashMap<ObjectId, ObjectId>,
}

/// Migrates objects that have crossed a cell boundary into the cell that now
/// contains them, creating that cell if it does not yet exist.
#[late_update(mode = "all")]
pub fn cell_streaming_system(world: &mut World) -> Result<()> {
    // Collect first
    let mut migrations: Vec<(ObjectId, CellCoord)> = Vec::new();
    for id in world.get_root_ids() {
        if let Some(transform) = world.get_component::<Transform>(id) {
            let target = world_to_cell(transform.global_position);
            if target != id.cell {
                migrations.push((id, target));
            }
        }
    }

    let mut remap: HashMap<ObjectId, ObjectId> = HashMap::new();
    for (id, target) in migrations {
        if let Some(new_id) = world.worldspace_mut().move_subtree_to_cell(id, target) {
            remap.insert(id, new_id);
        }
    }

    // The tag_index embeds ObjectIds, so any migration invalidates it.
    if !remap.is_empty() {
        world.rebuild_tag_index();
    }

    // Publish the remap, replacing last frames
    if world.has_resource::<CellMigrations>() {
        world.get_resource_mut::<CellMigrations>()?.remap = remap;
    } else {
        world.insert_resource(CellMigrations { remap });
    }

    Ok(())
}
