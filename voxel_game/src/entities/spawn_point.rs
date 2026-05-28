use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    objects::{components::transform::Transform, world::World},
    update,
    voxels::voxel_raycast::{Direction, voxel_raycast},
};
use apostasy_macros::Tag;

use crate::entities::loading_gate::LoadingGate;

#[derive(Tag, Clone)]
pub struct NeedsSpawnPoint;

#[update]
pub fn find_spawn_point(world: &mut World) -> Result<()> {
    let object_ids: Vec<_> = world
        .get_objects_with_tag_with_ids::<NeedsSpawnPoint>()
        .into_iter()
        .filter(|(id, _)| {
            // Only process spawn points for entities that don't have the loading gate
            world
                .get_object(id.clone())
                .map(|obj| !obj.has_tag::<LoadingGate>())
                .unwrap_or(false)
        })
        .map(|(id, _)| id)
        .collect();

    let transform = Transform {
        local_position: Vector3::new(0.0, 500.0, 0.0),
        global_position: Vector3::new(0.0, 500.0, 0.0),
        ..Default::default()
    };

    for id in object_ids {
        if let Some(hit) = voxel_raycast(world, &transform, 1500.0, Direction::Down) {
            let spawn = Vector3::new(
                hit.voxel_pos.x as f32,
                hit.voxel_pos.y as f32 + 5.0,
                hit.voxel_pos.z as f32,
            );

            let object = world.get_object_mut(id.clone()).unwrap();
            let t = object.get_component_mut::<Transform>()?;
            t.local_position = spawn;
            t.global_position = spawn;
            object.remove_tag::<NeedsSpawnPoint>();
        }
    }

    Ok(())
}
