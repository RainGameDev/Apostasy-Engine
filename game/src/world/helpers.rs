use apostasy_core::{
    noise::{NoiseFn, Perlin},
    utils::flatten::flatten,
    voxels::biome::{BiomeRegistry, sample_biome_weights_at_climate},
};

use crate::world::{
    consts::{BASE_HEIGHT, BIOME_BLEND_DISTANCE},
    noise::{GlobalNoiseLayers, compute_biome_base_detail},
};

/// Pure computation for a single column
#[allow(clippy::too_many_arguments)]
pub fn compute_column(
    world_x: f64,
    world_z: f64,
    noise: &Perlin,
    biome_registry: &BiomeRegistry,
    lod: u8,
    temp_noise: &Perlin,
    humid_noise: &Perlin,
    continental_noise: &Perlin,
) -> (i32, u16) {
    let temperature = (temp_noise.get([world_x * 0.001, world_z * 0.001]) + 1.0) * 0.5;
    let humidity = (humid_noise.get([world_x * 0.001, world_z * 0.001]) + 1.0) * 0.5;
    let continental = (continental_noise.get([world_x * 0.00035, world_z * 0.00035]) + 1.0) * 0.5;

    let climate_temp = (temperature * 0.7 + continental * 0.25 + 0.05).clamp(0.0, 1.0);
    let climate_humid = (humidity * 0.6 + (1.0 - continental) * 0.3 + 0.05).clamp(0.0, 1.0);

    let weights = sample_biome_weights_at_climate(
        climate_temp,
        climate_humid,
        biome_registry,
        BIOME_BLEND_DISTANCE,
    );

    let layers = GlobalNoiseLayers::sample(noise, world_x, world_z);

    let mut weighted_base = 0.0f64;
    let mut weighted_global = 0.0f64;
    let mut weighted_continental_scale = 0.0f64;
    let mut dominant_biome = 0u16;
    let mut dominant_weight = 0.0f64;

    for &(biome_id, weight) in &weights {
        let biome = &biome_registry.defs[biome_id as usize];

        weighted_base += compute_biome_base_detail(noise, world_x, world_z, biome, lod) * weight;
        weighted_global += layers.weighted_contribution(biome, weight);
        weighted_continental_scale += biome.terrain_shaping.continental_scale * weight;

        if weight > dominant_weight {
            dominant_weight = weight;
            dominant_biome = biome_id;
        }
    }

    let blended_height = BASE_HEIGHT
        + weighted_base * 0.6
        + weighted_global
        + (continental - 0.5) * weighted_continental_scale;

    (blended_height as i32, dominant_biome)
}

pub fn div_floor(value: i32, divisor: i32) -> i32 {
    if value >= 0 {
        value / divisor
    } else {
        (value - divisor + 1) / divisor
    }
}

pub fn hash_column(x: i32, z: i32, seed: u32) -> u32 {
    let mut h = seed;
    h ^= (x as u32).wrapping_mul(0x9e3779b9);
    h = h.wrapping_mul(0x517cc1b727220a95u64 as u32);
    h ^= h >> 17;
    h ^= (z as u32).wrapping_mul(0x6c62272e07bb0142u64 as u32);
    h = h.wrapping_mul(0xbf58476d1ce4e5b9u64 as u32);
    h ^= h >> 31;
    h
}

pub fn random_range(x: i32, z: i32, seed: u32, min: u32, max: u32) -> u32 {
    let h = hash_column(x, z, seed);
    min + (h % (max - min + 1))
}

#[allow(clippy::too_many_arguments)]
pub fn set_voxel_global(
    voxels: &mut [u16],
    global_x: i32,
    global_y: i32,
    global_z: i32,
    chunk_world_x: i32,
    chunk_world_y: i32,
    chunk_world_z: i32,
    voxel_id: u16,
) {
    let lx = global_x - chunk_world_x;
    let ly = global_y - chunk_world_y;
    let lz = global_z - chunk_world_z;
    if !(0..32).contains(&lx) || !(0..32).contains(&ly) || !(0..32).contains(&lz) {
        return;
    }
    voxels[flatten(lx as u32, ly as u32, lz as u32, 32)] = voxel_id;
}

#[allow(clippy::too_many_arguments)]
pub fn set_voxel_global_non_floating(
    voxels: &mut [u16],
    global_x: i32,
    global_y: i32,
    global_z: i32,
    chunk_world_x: i32,
    chunk_world_y: i32,
    chunk_world_z: i32,
    voxel_id: u16,
    water_voxel: u16,
) {
    let lx = global_x - chunk_world_x;
    let mut ly = global_y - chunk_world_y;
    let lz = global_z - chunk_world_z;
    if !(0..32).contains(&lx) || !(0..32).contains(&ly) || !(0..32).contains(&lz) {
        return;
    }

    while ly > 0
        && (voxels[(lx + ly * 32 + lz * 32 * 32) as usize] == 0
            || voxels[(lx + ly * 32 + lz * 32 * 32) as usize] == water_voxel)
    {
        ly -= 1;
    }

    let current_voxel = voxels[(lx + ly * 32 + lz * 32 * 32) as usize];
    if current_voxel != 0 && current_voxel != water_voxel {
        let above_y = ly + 1;
        if above_y < 32 {
            voxels[flatten(lx as u32, above_y as u32, lz as u32, 32)] = voxel_id;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn set_voxel_global_if_empty(
    voxels: &mut [u16],
    global_x: i32,
    global_y: i32,
    global_z: i32,
    chunk_world_x: i32,
    chunk_world_y: i32,
    chunk_world_z: i32,
    voxel_id: u16,
) {
    let lx = global_x - chunk_world_x;
    let ly = global_y - chunk_world_y;
    let lz = global_z - chunk_world_z;
    if !(0..32).contains(&lx) || !(0..32).contains(&ly) || !(0..32).contains(&lz) {
        return;
    }
    let index = flatten(lx as u32, ly as u32, lz as u32, 32);
    if voxels[index] == 0 {
        voxels[index] = voxel_id;
    }
}
