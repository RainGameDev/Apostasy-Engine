use std::collections::HashSet;
use std::sync::Arc;

use apostasy_core::noise::Perlin;
use apostasy_core::ecs::components::transform::FORWARD;
use apostasy_core::rand::{RngExt, rng};
use apostasy_core::voxels::biome::{CONTINENTAL_NOISE, HUMIDITY_NOISE, NOISE, TEMPERATURE_NOISE};
use apostasy_core::voxels::chunk::{ChunkGenQueue, GeneratedChunkData};
use apostasy_core::voxels::chunk_loader::{ChunkLoadBounds, ChunkPositionMap};
use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    log,
    ecs::{cell::EntityId, components::transform::Transform, tags::Player, world::World},
    voxels::{
        VoxelTransform, biome::BiomeRegistry, chunk::Chunk, meshes::NeedsRemeshing,
        structure::StructureRegistry, voxel::VoxelRegistry,
    },
};
use apostasy_macros::{Resource, fixed_update, start};

use crate::states::{GetNewSeed, HasInitGeneration};
use crate::world::{generation::generate_chunk_data, loading_state::LoadingState};

/// A resource that stores the information related to chunk loading
#[derive(Resource, Clone)]
pub struct ChunkLoader {
    /// the last position the player was in
    pub last_chunk_position: Vector3<i32>,
    /// the horizontal (x, z) chunk load distance
    pub load_radius: i32,
    /// the vertical (y) chunk load distance
    pub v_load_radius: i32,
    /// the distances used for each lod
    pub chunk_lod_distances: Vec<u32>,
    /// current frame counter (unused)
    pub frame_counter: u32,
    /// current world seed
    pub seed: u32,
}

impl Default for ChunkLoader {
    fn default() -> Self {
        Self {
            last_chunk_position: Vector3::new(i32::MAX, i32::MAX, i32::MAX),
            load_radius: 32,
            v_load_radius: 8,
            chunk_lod_distances: vec![16, 32, 64, 128],
            frame_counter: 0,
            seed: 1,
        }
    }
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
    receive_chunks(world, 0.0)
}

#[fixed_update]
pub fn dispatch_chunk_jobs(world: &mut World, _delta: f32) -> Result<()> {
    if !world.has_resource::<HasInitGeneration>() {
        return Ok(());
    }

    if world.has_resource::<GetNewSeed>() {
        let seed = rng().random::<u32>();
        world.get_resource_mut::<ChunkLoader>()?.seed = seed;
        world.remove_resource::<GetNewSeed>();

        *NOISE.write().unwrap() = Some(Perlin::new(seed));
        *TEMPERATURE_NOISE.write().unwrap() = Some(Perlin::new(seed.wrapping_add(1)));
        *HUMIDITY_NOISE.write().unwrap() = Some(Perlin::new(seed.wrapping_add(10)));
        *CONTINENTAL_NOISE.write().unwrap() = Some(Perlin::new(seed.wrapping_add(2)));
        return Ok(());
    }

    let player_id_pre = world.get_entity_with_tag::<Player>()?;
    let (player_chunk_pos, player_forward_chunk) = {
        let t = world.get_component::<Transform>(player_id_pre)
            .ok_or_else(|| apostasy_core::anyhow::anyhow!("Player has no Transform"))?;
        let pos = Vector3::new(
            (t.global_position.x / 32.0).floor() as i32,
            (t.global_position.y / 32.0).floor() as i32,
            (t.global_position.z / 32.0).floor() as i32,
        );
        let fwd = t.global_rotation * FORWARD;
        (pos, fwd)
    };

    let (last_chunk_pos, load_radius, v_load_radius, lod_distances) = {
        let loader = world.get_resource_mut::<ChunkLoader>()?;
        loader.frame_counter += 1;
        (
            loader.last_chunk_position,
            loader.load_radius,
            loader.v_load_radius,
            loader.chunk_lod_distances.clone(),
        )
    };

    if !world.has_resource::<ChunkLoadBounds>() {
        world.insert_resource(ChunkLoadBounds {
            player_chunk_pos,
            load_radius,
            v_load_radius,
        });
    } else {
        let bounds = world.get_resource_mut::<ChunkLoadBounds>()?;
        bounds.player_chunk_pos = player_chunk_pos;
        bounds.load_radius = load_radius;
        bounds.v_load_radius = v_load_radius;
    }

    let is_initial_load = last_chunk_pos == Vector3::new(i32::MAX, i32::MAX, i32::MAX);

    if last_chunk_pos == player_chunk_pos {
        return Ok(());
    }

    log!("Entered new chunk at {:?}", player_chunk_pos);
    world.get_resource_mut::<ChunkLoader>()?.last_chunk_position = player_chunk_pos;

    if is_initial_load {
        world.insert_resource(LoadingState::new(
            player_chunk_pos,
            load_radius,
            v_load_radius,
        ));
    }

    // unload out of range chunks
    let unload_ids: Vec<EntityId> = {
        let map = world.get_resource::<ChunkPositionMap>()?;
        map.position_to_id
            .iter()
            .filter_map(|(pos, &id)| {
                let dx = (pos.x - player_chunk_pos.x).abs();
                let dy = (pos.y - player_chunk_pos.y).abs();
                let dz = (pos.z - player_chunk_pos.z).abs();
                if dx > load_radius || dy > v_load_radius || dz > load_radius {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    };
    for id in unload_ids {
        let position = world.get_component::<VoxelTransform>(id).map(|t| t.position);

        if let Some(pos) = position {
            let map = world.get_resource_mut::<ChunkPositionMap>()?;
            map.position_to_id.remove(&pos);
            map.position_to_lod.remove(&pos);
        }

        world.unregister_chunk(id);
        world.despawn(id);
    }
    world
        .get_resource_mut::<ChunkGenQueue>()?
        .in_flight
        .retain(|pos| {
            let dx = (pos.x - player_chunk_pos.x).abs();
            let dy = (pos.y - player_chunk_pos.y).abs();
            let dz = (pos.z - player_chunk_pos.z).abs();
            dx <= load_radius && dy <= v_load_radius && dz <= load_radius
        });

    // build candidate list
    let mut candidates: Vec<(Vector3<i32>, usize)> = Vec::new();

    for x in (player_chunk_pos.x - load_radius)..=(player_chunk_pos.x + load_radius) {
        for y in (player_chunk_pos.y - v_load_radius)..=(player_chunk_pos.y + v_load_radius) {
            for z in (player_chunk_pos.z - load_radius)..=(player_chunk_pos.z + load_radius) {
                let pos = Vector3::new(x, y, z);
                let dx = (x - player_chunk_pos.x) as f32;
                let dy = (y - player_chunk_pos.y) as f32;
                let dz = (z - player_chunk_pos.z) as f32;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                if let Some(lod) = lod_distances.iter().position(|&d| dist < d as f32) {
                    candidates.push((pos, lod));
                }
            }
        }
    }

    candidates.sort_unstable_by(|a, b| {
        let score = |pos: Vector3<i32>| -> i32 {
            let d = pos - player_chunk_pos;
            let dist_sq = d.x * d.x + d.y * d.y + d.z * d.z;
            let dot = (d.x as f32 * player_forward_chunk.x + d.z as f32 * player_forward_chunk.z)
                .max(0.0) as i32;
            dist_sq - dot * 4
        };
        score(a.0).cmp(&score(b.0))
    });

    if !is_initial_load {
        candidates.truncate(MAX_GEN_JOBS_PER_FRAME);
    }

    // dispatch generation jobs
    let registry = Arc::new(world.get_resource::<VoxelRegistry>()?.clone());
    let biome_registry = Arc::new(world.get_resource::<BiomeRegistry>()?.clone());
    let structure_registry = Arc::new(world.get_resource::<StructureRegistry>()?.clone());

    let pool_arc = world.get_resource::<ChunkGenQueue>()?.pool.clone();
    let pool = pool_arc.lock().unwrap();
    let sender = world.get_resource::<ChunkGenQueue>()?.sender.clone();
    let in_flight = world.get_resource::<ChunkGenQueue>()?.in_flight.clone();

    let mut new_positions: Vec<Vector3<i32>> = Vec::new();

    for (pos, target_lod) in candidates {
        let lod = (target_lod + 1) as u8;

        // read from persistent map, drop borrow before any mut access
        let current_lod = world
            .get_resource::<ChunkPositionMap>()?
            .position_to_lod
            .get(&pos)
            .copied();
        let current_id = world
            .get_resource::<ChunkPositionMap>()?
            .position_to_id
            .get(&pos)
            .copied();

        if let Some(current_lod) = current_lod {
            if current_lod != lod {
                if let Some(id) = current_id {
                    if let Some(chunk) = world.get_component_mut::<Chunk>(id) {
                        chunk.lod = lod;
                    }
                    world.add_tag::<NeedsRemeshing>(id);
                }
                world
                    .get_resource_mut::<ChunkPositionMap>()?
                    .position_to_lod
                    .insert(pos, lod);
            }
            new_positions.push(pos);
            continue;
        }

        if in_flight.contains(&pos) {
            continue;
        }

        let sender = sender.clone();
        let reg = Arc::clone(&registry);
        let biome_reg = Arc::clone(&biome_registry);
        let structure_reg = Arc::clone(&structure_registry);
        let seed = world.get_resource::<ChunkLoader>()?.seed;

        world
            .get_resource_mut::<ChunkGenQueue>()?
            .in_flight
            .insert(pos);

        pool.spawn(move || {
            let data = generate_chunk_data(pos, &reg, &biome_reg, &structure_reg, seed, lod);
            let _ = sender.send(data);
        });

        new_positions.push(pos);
    }

    drop(pool);

    // --- remesh neighbours of updated positions ---
    let new_pos_set: HashSet<Vector3<i32>> = new_positions.iter().cloned().collect();
    let mut remesh_ids: HashSet<EntityId> = HashSet::new();

    {
        let map = world.get_resource::<ChunkPositionMap>()?;
        for pos in &new_pos_set {
            for offset in &NEIGHBOUR_OFFSETS {
                let neighbour = pos + offset;
                if !new_pos_set.contains(&neighbour)
                    && let Some(&id) = map.position_to_id.get(&neighbour)
                {
                    remesh_ids.insert(id);
                }
            }
        }
    }

    for id in remesh_ids {
        world.add_tag::<NeedsRemeshing>(id);
    }

    Ok(())
}

const MAX_CHUNKS_PER_FRAME: usize = 512;
const MAX_GEN_JOBS_PER_FRAME: usize = 512;

#[fixed_update]
pub fn receive_chunks(world: &mut World, _delta: f32) -> Result<()> {
    // get all the chunks that have finished generating
    let completed: Vec<GeneratedChunkData> = {
        let queue = world.get_resource::<ChunkGenQueue>()?;
        queue
            .receiver
            .try_iter()
            .take(MAX_CHUNKS_PER_FRAME)
            .collect()
    };

    // return if no chunks are finished generating
    if completed.is_empty() {
        return Ok(());
    }

    let mut added_positions: Vec<Vector3<i32>> = Vec::with_capacity(completed.len());

    // for each new chunk
    for data in completed {
        // remove from in-flight
        world
            .get_resource_mut::<ChunkGenQueue>()?
            .in_flight
            .remove(&data.position);

        let id = world.spawn().id();
        world.set_name(id, "Chunk");
        world.add_component(id, VoxelTransform { position: data.position });
        world.add_component(id, Chunk {
            voxels: data.voxels,
            lod: data.lod,
            biome: data.biome,
        });
        world.add_tag::<NeedsRemeshing>(id);
        world.register_chunk(id);

        // update the persistent map
        let map = world.get_resource_mut::<ChunkPositionMap>()?;
        map.position_to_id.insert(data.position, id);
        map.position_to_lod.insert(data.position, data.lod);

        added_positions.push(data.position);
    }

    // read directly from the persistent map no allocation, no scan
    let map = world.get_resource::<ChunkPositionMap>()?;

    let mut remesh_ids: HashSet<EntityId> = HashSet::new();

    for pos in &added_positions {
        for offset in &NEIGHBOUR_OFFSETS {
            let neighbour = pos + offset;
            // skip neighbours that were just added (already tagged NeedsRemeshing)
            if !added_positions.contains(&neighbour)
                && let Some(&id) = map.position_to_id.get(&neighbour)
            {
                remesh_ids.insert(id);
            }
        }
    }

    for id in remesh_ids {
        world.add_tag::<NeedsRemeshing>(id);
    }

    Ok(())
}

#[fixed_update]
pub fn chunk_amount_update(world: &mut World, _delta: f32) -> Result<()> {
    // Update loading state with current chunk count
    let chunk_count = world.get_entities_with_component::<Chunk>().len();
    if let Ok(loading_state) = world.get_resource_mut::<LoadingState>()
        && !loading_state.is_complete
    {
        loading_state.chunks_loaded = chunk_count;
    }

    Ok(())
}
