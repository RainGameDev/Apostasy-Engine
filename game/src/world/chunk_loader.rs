use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use apostasy_core::objects::components::transform::FORWARD;
use apostasy_core::rand::{RngExt, rng};
use apostasy_core::voxels::chunk::{ChunkGenQueue, GeneratedChunkData};
use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    log,
    objects::{components::transform::Transform, scene::ObjectId, tags::Player, world::World},
    physics::velocity::Velocity,
    voxels::{
        VoxelTransform, biome::BiomeRegistry, chunk::Chunk, meshes::NeedsRemeshing,
        structure::StructureRegistry, voxel::VoxelRegistry,
    },
};
use apostasy_macros::{Resource, fixed_update, start};

use crate::states::{GetNewSeed, HasInitGeneration};
use crate::world::{generation::generate_chunk_data, loading_state::LoadingState};

#[derive(Resource, Clone)]
pub struct ChunkLoader {
    pub last_chunk_position: Vector3<i32>,
    pub load_radius: i32,
    pub chunk_lod_distances: Vec<u32>,
    pub frame_counter: u32,
    pub seed: u32,
}

impl Default for ChunkLoader {
    fn default() -> Self {
        Self {
            last_chunk_position: Vector3::new(i32::MAX, i32::MAX, i32::MAX),
            load_radius: 8,
            chunk_lod_distances: vec![8, 12, 14, 16],
            frame_counter: 0,
            seed: 1,
        }
    }
}

fn build_chunk_maps(
    world: &World,
) -> (
    HashMap<Vector3<i32>, ObjectId>,
    HashMap<Vector3<i32>, u8>, // position , current lod
) {
    let objects = world.get_objects_with_component_with_ids::<VoxelTransform>();
    let mut position_to_id = HashMap::with_capacity(objects.len());
    let mut position_to_lod = HashMap::with_capacity(objects.len());

    for (id, obj) in objects {
        let Ok(t) = obj.get_component::<VoxelTransform>() else {
            continue;
        };
        position_to_id.insert(t.position, id);
        if let Ok(chunk) = obj.get_component::<Chunk>() {
            position_to_lod.insert(t.position, chunk.lod);
        }
    }

    (position_to_id, position_to_lod)
}

const NEIGHBOUR_OFFSETS: [Vector3<i32>; 6] = [
    Vector3::new(1, 0, 0),
    Vector3::new(-1, 0, 0),
    Vector3::new(0, 1, 0),
    Vector3::new(0, -1, 0),
    Vector3::new(0, 0, 1),
    Vector3::new(0, 0, -1),
];

#[start]
pub fn update_chunks_init(world: &mut World) -> Result<()> {
    dispatch_chunk_jobs(world, 0.0)?;
    std::thread::sleep(std::time::Duration::from_millis(50));
    receive_chunks(world, 0.0)
}

#[fixed_update]
pub fn dispatch_chunk_jobs(world: &mut World, _delta: f32) -> Result<()> {
    if !world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }
    if world.get_resource::<GetNewSeed>().is_ok() {
        let mut seed = rng();
        let seed = seed.random::<u32>();

        world.get_resource_mut::<ChunkLoader>()?.seed = seed;
        world.remove_resource::<GetNewSeed>();
        return Ok(());
    }

    let player = world.get_object_with_tag::<Player>()?;
    let player_transform = player.get_component::<Transform>()?;
    let player_velocity = player.get_component::<Velocity>()?;

    let player_chunk_pos = Vector3::new(
        (player_transform.global_position.x / 32.0).floor() as i32,
        (player_transform.global_position.y / 32.0).floor() as i32,
        (player_transform.global_position.z / 32.0).floor() as i32,
    );

    let player_forward_chunk = player_transform.global_rotation * FORWARD;

    let vel = player_velocity.linear_velocity;

    let (last_chunk_pos, load_radius, lod_distances, frame_counter) = {
        let loader = world.get_resource_mut::<ChunkLoader>()?;
        loader.frame_counter += 1;
        (
            loader.last_chunk_position,
            loader.load_radius,
            loader.chunk_lod_distances.clone(),
            loader.frame_counter,
        )
    };

    // Only dispatch every 10 frames to reduce load, unless initial load
    let is_initial_load = last_chunk_pos == Vector3::new(i32::MAX, i32::MAX, i32::MAX);
    if !is_initial_load && frame_counter % 10 != 0 {
        return Ok(());
    }

    if last_chunk_pos == player_chunk_pos {
        return Ok(());
    }

    let delta = player_chunk_pos - last_chunk_pos;
    let vx = vel.x.signum() as i32;
    let vz = vel.z.signum() as i32;
    let moving_toward =
        (delta.x != 0 && delta.x.signum() == vx) || (delta.z != 0 && delta.z.signum() == vz);

    if !moving_toward && last_chunk_pos != Vector3::new(i32::MAX, i32::MAX, i32::MAX) {
        return Ok(());
    }

    log!("Entered new chunk at {:?}", player_chunk_pos);
    world.get_resource_mut::<ChunkLoader>()?.last_chunk_position = player_chunk_pos;

    // Initialize loading state on initial load
    if is_initial_load {
        let loading_state = LoadingState::new(player_chunk_pos, load_radius);
        world.insert_resource(loading_state);
    }

    let (position_to_id, position_to_lod) = build_chunk_maps(world);

    let unload_ids: Vec<ObjectId> = position_to_id
        .iter()
        .filter_map(|(pos, &id)| {
            let dx = (pos.x - player_chunk_pos.x).abs();
            let dy = (pos.y - player_chunk_pos.y).abs();
            let dz = (pos.z - player_chunk_pos.z).abs();
            if dx > load_radius || dy > load_radius || dz > load_radius {
                Some(id)
            } else {
                None
            }
        })
        .collect();

    for id in unload_ids {
        world.unregister_chunk(id);
        world.remove_object(id);
    }

    world
        .get_resource_mut::<ChunkGenQueue>()?
        .in_flight
        .retain(|pos| {
            let dx = (pos.x - player_chunk_pos.x).abs();
            let dy = (pos.y - player_chunk_pos.y).abs();
            let dz = (pos.z - player_chunk_pos.z).abs();
            dx <= load_radius && dy <= load_radius && dz <= load_radius
        });

    let mut candidates: Vec<(Vector3<i32>, usize)> = Vec::new();

    for x in (player_chunk_pos.x - load_radius)..=(player_chunk_pos.x + load_radius) {
        for y in (player_chunk_pos.y - load_radius)..=(player_chunk_pos.y + load_radius) {
            for z in (player_chunk_pos.z - load_radius)..=(player_chunk_pos.z + load_radius) {
                let pos = Vector3::new(x, y, z);
                let dx = (x - player_chunk_pos.x) as f32;
                let dy = (y - player_chunk_pos.y) as f32;
                let dz = (z - player_chunk_pos.z) as f32;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                let target_lod = lod_distances.iter().position(|&d| dist < d as f32);

                if let Some(lod) = target_lod {
                    candidates.push((pos, lod));
                }
            }
        }
    }

    candidates.sort_unstable_by(|a, b| {
        let score = |pos: Vector3<i32>| -> i32 {
            let d = pos - player_chunk_pos;
            let dist_sq = d.x * d.x + d.y * d.y + d.z * d.z;

            let forward = player_forward_chunk;
            let dot = (d.x as f32 * forward.x + d.z as f32 * forward.z).max(0.0) as i32;

            dist_sq - dot * 4
        };
        score(a.0).cmp(&score(b.0))
    });

    // For initial load, dispatch all candidates, otherwise limit per frame
    let is_initial_load = last_chunk_pos == Vector3::new(i32::MAX, i32::MAX, i32::MAX);
    if !is_initial_load {
        candidates.truncate(MAX_GEN_JOBS_PER_FRAME);
    }

    let registry = Arc::new(world.get_resource::<VoxelRegistry>()?.clone());
    let biome_registry = Arc::new(world.get_resource::<BiomeRegistry>()?.clone());
    let structure_registry = Arc::new(world.get_resource::<StructureRegistry>()?.clone());

    let pool_arc = world.get_resource::<ChunkGenQueue>()?.pool.clone();
    let pool = pool_arc.lock().unwrap();
    let sender = world.get_resource::<ChunkGenQueue>()?.sender.clone();

    let mut new_positions: Vec<Vector3<i32>> = Vec::new();
    let in_flight = &world.get_resource::<ChunkGenQueue>()?.in_flight.clone();

    for (pos, target_lod) in candidates {
        let lod = (target_lod + 1) as u8;

        if let Some(&current_lod) = position_to_lod.get(&pos) {
            if current_lod != lod {
                if let Some(&id) = position_to_id.get(&pos) {
                    if let Some(obj) = world.get_object_mut(id) {
                        if let Ok(chunk) = obj.get_component_mut::<Chunk>() {
                            chunk.lod = lod;
                        }
                        obj.add_tag(NeedsRemeshing);
                    }
                }
            }
            new_positions.push(pos);
            continue;
        }

        if in_flight.contains(&pos) {
            continue;
        }

        // spawn generation job
        let sender = sender.clone();
        let reg = Arc::clone(&registry);
        let biome_reg = Arc::clone(&biome_registry);
        let structure_reg = Arc::clone(&structure_registry);

        world
            .get_resource_mut::<ChunkGenQueue>()?
            .in_flight
            .insert(pos);

        let seed = world.get_resource_mut::<ChunkLoader>()?.seed;
        pool.spawn(move || {
            let data = generate_chunk_data(pos, &reg, &biome_reg, &structure_reg, seed, lod);
            let _ = sender.send(data);
        });

        new_positions.push(pos);
    }

    drop(pool);

    let new_pos_set: HashSet<Vector3<i32>> = new_positions.iter().cloned().collect();
    let mut remesh_ids: Vec<ObjectId> = Vec::new();

    for pos in &new_pos_set {
        for offset in &NEIGHBOUR_OFFSETS {
            let neighbour = pos + offset;
            if !new_pos_set.contains(&neighbour) {
                if let Some(&id) = position_to_id.get(&neighbour) {
                    remesh_ids.push(id);
                }
            }
        }
    }

    remesh_ids.dedup();
    for id in remesh_ids {
        if let Some(obj) = world.get_object_mut(id) {
            obj.add_tag(NeedsRemeshing);
        }
    }

    Ok(())
}

const MAX_CHUNKS_PER_FRAME: usize = 512;
const MAX_GEN_JOBS_PER_FRAME: usize = 512;
#[fixed_update]
pub fn receive_chunks(world: &mut World, _delta: f32) -> Result<()> {
    let completed: Vec<GeneratedChunkData> = {
        let queue = world.get_resource::<ChunkGenQueue>()?;
        queue
            .receiver
            .try_iter()
            .take(MAX_CHUNKS_PER_FRAME)
            .collect()
    };

    if completed.is_empty() {
        return Ok(());
    }

    let mut added_positions: Vec<Vector3<i32>> = Vec::with_capacity(completed.len());

    for data in completed {
        world
            .get_resource_mut::<ChunkGenQueue>()?
            .in_flight
            .remove(&data.position);

        let mut object = apostasy_core::objects::Object::new();
        object.set_name("Chunk".to_string());
        object.add_component(VoxelTransform {
            position: data.position,
        });
        object.add_component(Chunk {
            voxels: data.voxels,
            lod: data.lod,
            biome: data.biome,
        });
        object.add_tag(NeedsRemeshing);
        let id = world.add_object(object);

        world.register_chunk(id);
        added_positions.push(data.position);
    }

    let position_to_id: HashMap<Vector3<i32>, ObjectId> = world
        .get_objects_with_component_with_ids::<VoxelTransform>()
        .into_iter()
        .filter_map(|(id, obj)| {
            let pos = obj.get_component::<VoxelTransform>().ok()?.position;
            Some((pos, id))
        })
        .collect();

    let mut remesh_ids: Vec<ObjectId> = Vec::new();
    let added_set: HashSet<Vector3<i32>> = added_positions.iter().cloned().collect();

    for pos in &added_positions {
        for offset in &NEIGHBOUR_OFFSETS {
            let neighbour = pos + offset;
            if !added_set.contains(&neighbour) {
                if let Some(&id) = position_to_id.get(&neighbour) {
                    remesh_ids.push(id);
                }
            }
        }
    }

    remesh_ids.sort_unstable();
    remesh_ids.dedup();

    for id in remesh_ids {
        if let Some(obj) = world.get_object_mut(id) {
            obj.add_tag(NeedsRemeshing);
        }
    }

    // Update loading state with current chunk count
    {
        let chunk_count = world.get_objects_with_component::<Chunk>().len();
        if let Ok(mut loading_state) = world.get_resource_mut::<LoadingState>() {
            if !loading_state.is_complete {
                loading_state.chunks_loaded = chunk_count;
            }
        }
    }

    Ok(())
}
