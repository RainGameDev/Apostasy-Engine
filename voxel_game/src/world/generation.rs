use apostasy_core::{
    cgmath::Vector3,
    log,
    noise::Perlin,
    utils::flatten::flatten,
    voxels::{
        biome::{BiomeRegistry, CONTINENTAL_NOISE, HUMIDITY_NOISE, NOISE, TEMPERATURE_NOISE},
        chunk::GeneratedChunkData,
        structure::StructureRegistry,
        voxel::{VoxelId, VoxelRegistry},
    },
};
use std::time::Instant;

use crate::world::{
    cache::NoiseColumnCache,
    consts::SEA_LEVEL,
    heightmap::{Upsample, UpsampledHeightmap},
    noise::is_carved_out,
};

pub fn generate_chunk_data(
    position: Vector3<i32>,
    registry: &VoxelRegistry,
    biome_registry: &BiomeRegistry,
    _structure_registry: &StructureRegistry,
    seed: u32,
    lod: u8,
) -> GeneratedChunkData {
    let full = Instant::now();

    let noise = NOISE.read().unwrap().unwrap();
    let cavern_noise = Perlin::new(seed.wrapping_add(10));
    let tunnel_noise = Perlin::new(seed.wrapping_add(11));

    let temp_noise = TEMPERATURE_NOISE.read().unwrap().unwrap();
    let humid_noise = HUMIDITY_NOISE.read().unwrap().unwrap();
    let continental_noise = CONTINENTAL_NOISE.read().unwrap().unwrap();

    let world_x = position.x as f64 * 32.0;
    let world_z = position.z as f64 * 32.0;
    let chunk_world_x = position.x * 32;
    let chunk_world_y = position.y * 32;
    let chunk_world_z = position.z * 32;

    let feature_radius = 4i32;
    let overhang = (feature_radius * 2 + 32) as usize;
    let mut col_cache = NoiseColumnCache::with_capacity(overhang * overhang);

    let heightmap_sampler = UpsampledHeightmap::new(
        world_x,
        world_z,
        Upsample::X2,
        &noise,
        biome_registry,
        &mut col_cache,
        lod,
        seed,
        &temp_noise,
        &humid_noise,
        &continental_noise, // pass through
    );

    let mut heightmap = [0i32; 32 * 32];
    let mut column_biome = [0u16; 32 * 32];

    for z in 0..32usize {
        for x in 0..32usize {
            heightmap[z * 32 + x] = heightmap_sampler.sample(x, z);
            let wx = chunk_world_x + x as i32;
            let wz = chunk_world_z + z as i32;
            let col = col_cache.get_or_insert(
                wx,
                wz,
                &noise,
                biome_registry,
                lod,
                seed,
                &temp_noise,
                &humid_noise,
                &continental_noise,
            );
            column_biome[z * 32 + x] = col.biome;
        }
    }

    // Early chunk skip: if entire chunk is above surface, all air
    let max_surface = *heightmap.iter().max().unwrap();

    if chunk_world_y > max_surface.max(SEA_LEVEL) {
        let voxels = Box::new([0u16; 32 * 32 * 32]);
        let center_biome = column_biome[16 * 32 + 16];
        return GeneratedChunkData {
            position,
            voxels,
            lod,
            biome: center_biome,
        };
    }

    let water_voxel = registry
        .name_to_id
        .get("Apostasy:Voxel:Water")
        .copied()
        .unwrap_or(0);

    // Precompute biome voxel IDs per column
    let mut col_surface_voxel = [0u16; 32 * 32];
    let mut col_subsurface_voxel = [0u16; 32 * 32];
    let mut col_underground_voxel = [0u16; 32 * 32];

    for z in 0..32usize {
        for x in 0..32usize {
            let biome_id = column_biome[z * 32 + x];
            let biome = &biome_registry.defs[biome_id as usize];
            col_surface_voxel[z * 32 + x] = *registry
                .name_to_id
                .get(biome.surface_voxels.first().unwrap())
                .expect("surface voxel");
            col_subsurface_voxel[z * 32 + x] = *registry
                .name_to_id
                .get(biome.subsurface_voxels.first().unwrap())
                .expect("subsurface voxel");
            col_underground_voxel[z * 32 + x] = *registry
                .name_to_id
                .get(biome.underground_voxels.first().unwrap())
                .expect("underground voxel");
        }
    }

    let mut voxels = vec![0u16; 32 * 32 * 32].into_boxed_slice();

    // Determine carving bounds

    for z in 0..32usize {
        for x in 0..32usize {
            let surface_y = heightmap[z * 32 + x];
            let surface_voxel = col_surface_voxel[z * 32 + x];
            let subsurface_voxel = col_subsurface_voxel[z * 32 + x];
            let underground_voxel = col_underground_voxel[z * 32 + x];

            let wx_f = world_x + x as f64;
            let wz_f = world_z + z as f64;

            for y in 0..32usize {
                let wy = chunk_world_y + y as i32;
                let depth = surface_y - wy;

                let id = if wy > surface_y {
                    // Above surface: water or air no carving needed
                    if water_voxel != 0 && wy <= SEA_LEVEL {
                        water_voxel
                    } else {
                        0
                    }
                } else {
                    // Below or at surface: determine base voxel
                    let base = if depth == 0 {
                        surface_voxel
                    } else if depth < 4 {
                        subsurface_voxel
                    } else {
                        underground_voxel
                    };

                    if is_carved_out(wx_f, wy as f64, wz_f, depth, &cavern_noise, &tunnel_noise) {
                        0
                    } else {
                        base
                    }
                };

                voxels[flatten(x as u32, y as u32, z as u32, 32)] = id;
            }
        }
    }
    // Finalise
    let voxels: Box<[VoxelId; 32 * 32 * 32]> =
        voxels.try_into().expect("voxel array size mismatch");

    let center_biome = column_biome[16 * 32 + 16];

    GeneratedChunkData {
        position,
        voxels,
        lod,
        biome: center_biome,
    }
}
