use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    fixed_update,
    items::{
        ItemRegistry,
        container::{Container, ContainerItem},
    },
    log, log_error,
    objects::{
        resources::input_manager::InputManager, scene::ObjectId, tags::Player, world::World,
    },
    utils::flatten::flatten,
    voxels::{
        VoxelTransform,
        chunk::{Chunk, VoxelBreakProgress},
        meshes::NeedsRemeshing,
        voxel::{Voxel, VoxelRegistry},
        voxel_components::{break_ticks::BreakTicks, drops::Drops},
        voxel_raycast::RaycastHit,
    },
};

#[fixed_update]
pub fn check_voxel_raycast(world: &mut World, _delta: f32) -> Result<()> {
    let inputs = world.get_resource::<InputManager>()?;
    let is_breaking = inputs.is_mousebind_active("Break");

    let Ok(raycast_hit) = world.get_resource::<RaycastHit>() else {
        if let Ok(progress) = world.get_resource_mut::<VoxelBreakProgress>() {
            progress.progress.clear();
        }
        return Ok(());
    };
    let raycast_hit = raycast_hit.clone();

    let Some(set_to) = raycast_hit.set_to else {
        world.remove_resource::<RaycastHit>();
        return Ok(());
    };

    // breaking a voxel
    if set_to == 0 {
        if !is_breaking {
            world.remove_resource::<RaycastHit>();
            return Ok(());
        }
        let hit_world_pos = (
            raycast_hit.chunk_pos.x * 32 + raycast_hit.local_pos.x,
            raycast_hit.chunk_pos.y * 32 + raycast_hit.local_pos.y,
            raycast_hit.chunk_pos.z * 32 + raycast_hit.local_pos.z,
        );

        // get the voxel's break ticks requirement
        let registry = world.get_resource::<VoxelRegistry>()?.clone();

        // find the voxel id at the hit position
        let voxel_id = world
            .get_objects_with_component::<VoxelTransform>()
            .iter()
            .find_map(|obj| {
                let t = obj.get_component::<VoxelTransform>().ok()?;
                if t.position != raycast_hit.chunk_pos {
                    return None;
                }
                let chunk = obj.get_component::<Chunk>().ok()?;
                Some(
                    chunk.voxels[flatten(
                        raycast_hit.local_pos.x as u32,
                        raycast_hit.local_pos.y as u32,
                        raycast_hit.local_pos.z as u32,
                        32,
                    )],
                )
            });

        let Some(voxel_id) = voxel_id else {
            world.remove_resource::<RaycastHit>();
            return Ok(());
        };

        let def = &registry.defs[voxel_id as usize];

        // get required break ticks if no BreakTicks component, voxel is unbreakable
        let Ok(break_ticks) = def.get_component::<BreakTicks>() else {
            world.remove_resource::<RaycastHit>();
            return Ok(());
        };
        let required_ticks = break_ticks.0;

        // increment progress for this voxel
        let current_ticks = {
            let progress = world.get_resource_mut::<VoxelBreakProgress>().unwrap();

            // clear progress on voxels that are no longer being targeted
            progress.progress.retain(|pos, _| *pos == hit_world_pos);

            let ticks = progress.progress.entry(hit_world_pos).or_insert(0);
            *ticks += 1;
            *ticks
        };

        if current_ticks >= required_ticks {
            // voxel is fully broken

            if let Ok(drops) = def.get_component::<Drops>() {
                if let Ok(item_registry) = world.get_resource::<ItemRegistry>() {
                    if let Some(_) = item_registry.name_to_id.get(&drops.0) {
                        let player_id = world
                            .get_objects_with_tag_with_ids::<Player>()
                            .first()
                            .map(|o| o.0);

                        if let Some(pid) = player_id {
                            let player = world.get_object_mut(pid).unwrap();

                            if let Ok(container) = player.get_component_mut::<Container>() {
                                // check if item already exists in container
                                if let Some(existing) =
                                    container.items.iter_mut().find(|i| i.item == drops.0)
                                {
                                    existing.amount += 1;
                                } else {
                                    container.add_item(ContainerItem {
                                        item: drops.0.clone(),
                                        amount: 1,
                                    });
                                }
                            } else {
                                log_error!("Player has no Container component");
                            }
                        }
                    } else {
                        log_error!(
                            "Drops expected for voxel: {}:{}:{} but there is no item registered as {}",
                            def.name,
                            def.class,
                            def.name,
                            drops.0
                        );
                    }
                } else {
                    log_error!(
                        "Drops expected for voxel: {}:{}:{} but there is no registered ItemRegistry",
                        def.name,
                        def.class,
                        def.name
                    );
                    log!("A regsitry can be added via the package system 'ItemSystemPackage'");
                }
            }
            world
                .get_resource_mut::<VoxelBreakProgress>()
                .unwrap()
                .progress
                .remove(&hit_world_pos);

            // find and update the chunk
            let mut chunks_to_update: Vec<ObjectId> = Vec::new();
            for (id, obj) in world.get_objects_with_component_with_ids::<VoxelTransform>() {
                if let Ok(t) = obj.get_component::<VoxelTransform>() {
                    if t.position == raycast_hit.chunk_pos {
                        chunks_to_update.push(id);
                    }
                }
            }

            world.remove_resource::<RaycastHit>();
            for id in chunks_to_update {
                let obj = world.get_object_mut(id).unwrap();
                obj.get_component_mut::<Chunk>()?.set(
                    raycast_hit.local_pos.x as u32,
                    raycast_hit.local_pos.y as u32,
                    raycast_hit.local_pos.z as u32,
                    Voxel { id: 0 },
                );
                obj.add_tag(NeedsRemeshing);
            }
        }

        return Ok(());
    }

    // placing a voxel  existing placement code unchanged
    let (target_chunk_pos, target_local_pos) = {
        let offset = match raycast_hit.face {
            0 => Vector3::new(1, 0, 0),
            1 => Vector3::new(-1, 0, 0),
            2 => Vector3::new(0, 1, 0),
            3 => Vector3::new(0, -1, 0),
            4 => Vector3::new(0, 0, 1),
            5 => Vector3::new(0, 0, -1),
            _ => Vector3::new(0, 0, 0),
        };

        let world_voxel = Vector3::new(
            raycast_hit.chunk_pos.x * 32 + raycast_hit.local_pos.x + offset.x,
            raycast_hit.chunk_pos.y * 32 + raycast_hit.local_pos.y + offset.y,
            raycast_hit.chunk_pos.z * 32 + raycast_hit.local_pos.z + offset.z,
        );

        (
            Vector3::new(
                world_voxel.x.div_euclid(32),
                world_voxel.y.div_euclid(32),
                world_voxel.z.div_euclid(32),
            ),
            Vector3::new(
                world_voxel.x.rem_euclid(32),
                world_voxel.y.rem_euclid(32),
                world_voxel.z.rem_euclid(32),
            ),
        )
    };

    let mut chunks_to_update: Vec<ObjectId> = Vec::new();
    for (id, obj) in world.get_objects_with_component_with_ids::<VoxelTransform>() {
        if let Ok(t) = obj.get_component::<VoxelTransform>() {
            if t.position == target_chunk_pos {
                chunks_to_update.push(id);
            }
        }
    }

    for id in chunks_to_update {
        let obj = world.get_object_mut(id).unwrap();
        obj.get_component_mut::<Chunk>()?.set_if_empty(
            target_local_pos.x as u32,
            target_local_pos.y as u32,
            target_local_pos.z as u32,
            Voxel { id: set_to },
        );
        obj.add_tag(NeedsRemeshing);
        break;
    }

    world.remove_resource::<RaycastHit>();
    Ok(())
}
