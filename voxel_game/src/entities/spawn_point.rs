use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    ecs::{components::transform::Transform, world::World},
    physics::raycast::Direction,
    update,
    voxels::voxel_raycast::voxel_raycast,
};
use apostasy_macros::Tag;

use crate::entities::loading_gate::LoadingGate;

#[derive(Tag, Clone)]
pub struct NeedsSpawnPoint;

#[update]
pub fn find_spawn_point(world: &mut World) -> Result<()> {
    let object_ids: Vec<_> = world
        .get_entities_with_tag::<NeedsSpawnPoint>()
        .into_iter()
        .filter(|&id| !world.has_tag::<LoadingGate>(id))
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

            if let Some(t) = world.get_component_mut::<Transform>(id) {
                t.local_position = spawn;
                t.global_position = spawn;
            }
            world.remove_tag::<NeedsSpawnPoint>(id);
        }
    }

    Ok(())
}
