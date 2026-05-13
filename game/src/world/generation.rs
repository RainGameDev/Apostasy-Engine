use apostasy_core::{
    cgmath::Vector3,
    log,
    noise::Perlin,
    utils::flatten::flatten,
    voxels::{
        biome::{BiomeRegistry, NOISE, StructureDefinition},
        chunk::GeneratedChunkData,
        structure::StructureRegistry,
        voxel::{VoxelId, VoxelRegistry},
    },
};
use std::time::Instant;

use crate::world::{
    cache::NoiseColumnCache,
    consts::{FEATURE_CELLS_PER_CHUNK, FEATURE_GRID_SIZE, SEA_LEVEL},
    heightmap::{UpsampledHeightmap, upsample_cell_size},
    helpers::{
        div_floor, hash_column, random_range, set_voxel_global, set_voxel_global_if_empty,
        set_voxel_global_non_floating,
    },
    noise::is_carved_out,
};

// Column cache

// Structure placement
fn place_tree_data_driven(
    voxels: &mut [u16],
    center_x: i32,
    base_y: i32,
    center_z: i32,
    chunk_world_x: i32,
    chunk_world_y: i32,
    chunk_world_z: i32,
    structure: &StructureDefinition,
    registry: &VoxelRegistry,
    seed: u32,
    _water_voxel: u16,
) {
    let trunk_id = structure
        .voxels
        .get("trunk")
        .and_then(|name| registry.name_to_id.get(name).copied());
    let canopy_id = structure
        .voxels
        .get("canopy")
        .and_then(|name| registry.name_to_id.get(name).copied());

    let (trunk_id, canopy_id) = match (trunk_id, canopy_id) {
        (Some(t), Some(c)) => (t, c),
        _ => return,
    };

    if base_y <= SEA_LEVEL {
        return;
    }

    let min_height = structure
        .parameters
        .get("min_height")
        .and_then(|v| v.as_u64())
        .unwrap_or(6) as i32;
    let max_height = structure
        .parameters
        .get("max_height")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as i32;
    let canopy_radius_base = structure
        .parameters
        .get("canopy_radius")
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as i32;

    let trunk_height = random_range(
        center_x,
        center_z,
        seed,
        min_height as u32,
        max_height as u32,
    ) as i32;
    let shape_seed = hash_column(center_x, center_z, seed.wrapping_add(1));
    let canopy_radius = canopy_radius_base + ((shape_seed & 1) as i32);
    let max_y = 32;

    for level in 0..trunk_height {
        let y = base_y + level;
        if y >= chunk_world_y + max_y {
            break;
        }

        let lx = center_x - chunk_world_x;
        let ly = y - chunk_world_y;
        let lz = center_z - chunk_world_z;
        if (0..32).contains(&lx) && (0..32).contains(&ly) && (0..32).contains(&lz) {
            voxels[flatten(lx as u32, ly as u32, lz as u32, 32)] = trunk_id;
        }

        if level > trunk_height / 2 && (shape_seed >> (level as u32)) & 1 == 1 {
            let branch_x = center_x
                + if (shape_seed >> (level as u32 + 1)) & 1 == 0 {
                    1
                } else {
                    -1
                };
            let branch_z = center_z
                + if (shape_seed >> (level as u32 + 2)) & 1 == 0 {
                    1
                } else {
                    -1
                };
            set_voxel_global(
                voxels,
                branch_x,
                y,
                branch_z,
                chunk_world_x,
                chunk_world_y,
                chunk_world_z,
                trunk_id,
            );
        }
    }

    let canopy_center = base_y + trunk_height;
    for dy in -2..=3 {
        let layer_y = canopy_center + dy;
        if layer_y < chunk_world_y || layer_y >= chunk_world_y + max_y {
            continue;
        }

        let layer_radius = canopy_radius - (dy.abs() / 2);
        let mut layer_threshold = canopy_radius as i32 * canopy_radius as i32;
        if dy == 3 {
            layer_threshold = 1;
        }
        if dy == -2 {
            layer_threshold = 2;
        }

        for dz in -layer_radius..=layer_radius {
            for dx in -layer_radius..=layer_radius {
                let dist_sq = dx * dx + dz * dz;
                if dist_sq > layer_threshold {
                    continue;
                }

                let noise_factor =
                    ((hash_column(center_x + dx, center_z + dz, seed.wrapping_add(dy as u32)) & 7)
                        as i32)
                        - 2;
                if dist_sq > layer_radius * layer_radius - noise_factor {
                    continue;
                }

                set_voxel_global_if_empty(
                    voxels,
                    center_x + dx,
                    layer_y,
                    center_z + dz,
                    chunk_world_x,
                    chunk_world_y,
                    chunk_world_z,
                    canopy_id,
                );
            }
        }
    }

    let extra_leaf_base = canopy_center - 1;
    for dz in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dz == 0 {
                continue;
            }
            set_voxel_global_if_empty(
                voxels,
                center_x + dx,
                extra_leaf_base,
                center_z + dz,
                chunk_world_x,
                chunk_world_y,
                chunk_world_z,
                canopy_id,
            );
        }
    }
}

fn place_boulder_data_driven(
    voxels: &mut [u16],
    center_x: i32,
    base_y: i32,
    center_z: i32,
    chunk_world_x: i32,
    chunk_world_y: i32,
    chunk_world_z: i32,
    structure: &StructureDefinition,
    registry: &VoxelRegistry,
    seed: u32,
    water_voxel: u16,
) {
    let boulder_id = match structure
        .voxels
        .get("boulder")
        .and_then(|name| registry.name_to_id.get(name).copied())
    {
        Some(id) => id,
        None => return,
    };

    let min_radius = structure
        .parameters
        .get("min_radius")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as i32;
    let max_radius = structure
        .parameters
        .get("max_radius")
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as i32;

    let radius = (random_range(
        center_x,
        center_z,
        seed,
        min_radius as u32,
        max_radius as u32,
    ) + 1) as i32;
    let center_y = base_y + 1;

    for dz in -radius..=radius {
        for dx in -radius..=radius {
            for dy in 0..=radius {
                let dy = if voxels[(dx + (dy - 1) + 32 + dz + 32 + 32) as usize] == 0 {
                    dy - 1
                } else {
                    dy
                };
                let dist_sq = dx * dx + dy * dy + dz * dz;
                if dist_sq > radius * radius {
                    continue;
                }
                set_voxel_global_non_floating(
                    voxels,
                    center_x + dx,
                    center_y + dy,
                    center_z + dz,
                    chunk_world_x,
                    chunk_world_y,
                    chunk_world_z,
                    boulder_id,
                    water_voxel,
                );
            }
        }
    }
}

fn place_structure_asset(
    voxels: &mut [u16],
    feature_x: i32,
    feature_surface_y: i32,
    feature_z: i32,
    chunk_world_x: i32,
    chunk_world_y: i32,
    chunk_world_z: i32,
    structure: &StructureDefinition,
    registry: &VoxelRegistry,
    structure_registry: &StructureRegistry,
    water_voxel: u16,
) {
    let asset_name = match &structure.asset {
        Some(name) => name,
        None => return,
    };
    let asset_id = match structure_registry.name_to_id.get(asset_name) {
        Some(id) => *id,
        None => return,
    };
    let asset = &structure_registry.defs[asset_id as usize];
    let y_offset = structure
        .parameters
        .get("y_offset")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    for block in asset.blocks.iter() {
        if let Some(voxel_id) = registry.name_to_id.get(&block.voxel).copied() {
            set_voxel_global_non_floating(
                voxels,
                feature_x + block.position[0],
                feature_surface_y + block.position[1] + y_offset,
                feature_z + block.position[2],
                chunk_world_x,
                chunk_world_y,
                chunk_world_z,
                voxel_id,
                water_voxel,
            );
        }
    }
}

fn place_structure_data_driven(
    voxels: &mut [u16],
    feature_x: i32,
    feature_surface_y: i32,
    feature_z: i32,
    chunk_world_x: i32,
    chunk_world_y: i32,
    chunk_world_z: i32,
    structure: &StructureDefinition,
    registry: &VoxelRegistry,
    structure_registry: &StructureRegistry,
    seed: u32,
    water_voxel: u16,
) {
    if structure.asset.is_some() {
        place_structure_asset(
            voxels,
            feature_x,
            feature_surface_y,
            feature_z,
            chunk_world_x,
            chunk_world_y,
            chunk_world_z,
            structure,
            registry,
            structure_registry,
            water_voxel,
        );
        return;
    }

    match structure.structure_type.as_str() {
        "tree" => place_tree_data_driven(
            voxels,
            feature_x,
            feature_surface_y,
            feature_z,
            chunk_world_x,
            chunk_world_y,
            chunk_world_z,
            structure,
            registry,
            seed,
            water_voxel,
        ),
        "boulder" => place_boulder_data_driven(
            voxels,
            feature_x,
            feature_surface_y,
            feature_z,
            chunk_world_x,
            chunk_world_y,
            chunk_world_z,
            structure,
            registry,
            seed,
            water_voxel,
        ),
        _ => {}
    }
}

pub fn generate_chunk_data(
    position: Vector3<i32>,
    registry: &VoxelRegistry,
    biome_registry: &BiomeRegistry,
    structure_registry: &StructureRegistry,
    seed: u32,
    lod: u8,
) -> GeneratedChunkData {
    let now = Instant::now();
    let noise = NOISE.read().unwrap().unwrap();
    let cavern_noise = Perlin::new(seed.wrapping_add(10));
    let tunnel_noise = Perlin::new(seed.wrapping_add(11));

    let world_x = position.x as f64 * 32.0;
    let world_z = position.z as f64 * 32.0;

    let chunk_world_x = position.x * 32;
    let chunk_world_y = position.y * 32;
    let chunk_world_z = position.z * 32;

    //  Column cache covers in-chunk columns + structure overhang
    let feature_radius = 4i32;
    let overhang = (feature_radius * 2 + 32) as usize; // ~40
    let mut col_cache = NoiseColumnCache::with_capacity(overhang * overhang);

    let cell_size = upsample_cell_size(lod);
    let heightmap_sampler = UpsampledHeightmap::new(
        world_x,
        world_z,
        cell_size,
        &noise,
        biome_registry,
        &mut col_cache,
        lod,
        seed,
    );

    // Per colum biome selection
    let mut heightmap = [0i32; 32 * 32];
    let mut column_biome = [0u16; 32 * 32];

    for z in 0..32usize {
        for x in 0..32usize {
            heightmap[z * 32 + x] = heightmap_sampler.sample(x, z);

            let wx = chunk_world_x + x as i32;
            let wz = chunk_world_z + z as i32;
            let col = col_cache.get_or_insert(wx, wz, &noise, biome_registry, lod, seed);
            column_biome[z * 32 + x] = col.biome;
        }
    }

    //  Voxel fill
    let mut voxels = vec![0u16; 32 * 32 * 32].into_boxed_slice();
    let water_voxel = registry
        .name_to_id
        .get("Apostasy:Voxel:Water")
        .copied()
        .unwrap_or(0);

    for z in 0..32usize {
        for x in 0..32usize {
            let surface_y = heightmap[z * 32 + x];
            let biome_id = column_biome[z * 32 + x];
            let biome = &biome_registry.defs[biome_id as usize];

            let surface_voxel = *registry
                .name_to_id
                .get(biome.surface_voxels.first().unwrap())
                .expect("surface voxel not found");
            let subsurface_voxel = *registry
                .name_to_id
                .get(biome.subsurface_voxels.first().unwrap())
                .expect("subsurface voxel not found");
            let underground_voxel = *registry
                .name_to_id
                .get(biome.underground_voxels.first().unwrap())
                .expect("underground voxel not found");

            for y in 0..32usize {
                let wy = chunk_world_y + y as i32;
                let depth = surface_y - wy;
                let wx_f = world_x + x as f64;
                let wy_f = wy as f64;
                let wz_f = world_z + z as f64;

                let mut id = if wy > surface_y {
                    if water_voxel != 0 && wy <= SEA_LEVEL {
                        water_voxel
                    } else {
                        0
                    }
                } else if depth == 0 {
                    surface_voxel
                } else if depth < 4 {
                    subsurface_voxel
                } else {
                    underground_voxel
                };

                if is_carved_out(wx_f, wy_f, wz_f, depth, &cavern_noise, &tunnel_noise) {
                    id = 0;
                }

                voxels[flatten(x as u32, y as u32, z as u32, 32)] = id;
            }
        }
    }

    let min_cell_x = div_floor(chunk_world_x - feature_radius, FEATURE_GRID_SIZE);
    let max_cell_x = div_floor(chunk_world_x + 31 + feature_radius, FEATURE_GRID_SIZE);
    let min_cell_z = div_floor(chunk_world_z - feature_radius, FEATURE_GRID_SIZE);
    let max_cell_z = div_floor(chunk_world_z + 31 + feature_radius, FEATURE_GRID_SIZE);

    for cell_z in min_cell_z..=max_cell_z {
        for cell_x in min_cell_x..=max_cell_x {
            let cell_hash = hash_column(cell_x, cell_z, seed.wrapping_add(0x9e3779b9));
            let offset_x = (cell_hash & 0x7) as i32;
            let offset_z = ((cell_hash >> 3) & 0x7) as i32;
            let feature_x = cell_x * FEATURE_GRID_SIZE + offset_x;
            let feature_z = cell_z * FEATURE_GRID_SIZE + offset_z;

            let col =
                col_cache.get_or_insert(feature_x, feature_z, &noise, biome_registry, lod, seed);
            let feature_surface_y = col.height;
            let feature_biome_id = col.biome;

            let biome = &biome_registry.defs[feature_biome_id as usize];

            for (structure_idx, structure) in biome.structures.iter().enumerate() {
                let structure_probability =
                    (structure.density / FEATURE_CELLS_PER_CHUNK).clamp(0.0, 1.0);

                let structure_hash = hash_column(
                    feature_x,
                    feature_z,
                    seed.wrapping_add(1 + structure_idx as u32),
                );
                let structure_chance = ((structure_hash & 0xffff) as f64) / 65535.0;

                if structure_chance < structure_probability {
                    place_structure_data_driven(
                        &mut voxels,
                        feature_x,
                        feature_surface_y,
                        feature_z,
                        chunk_world_x,
                        chunk_world_y,
                        chunk_world_z,
                        structure,
                        registry,
                        structure_registry,
                        seed.wrapping_add(2 + structure_idx as u32),
                        water_voxel,
                    );
                }
            }
        }
    }

    // Finalise
    let voxels: Box<[VoxelId; 32 * 32 * 32]> =
        voxels.try_into().expect("voxel array size mismatch");

    let center_biome = column_biome[16 * 32 + 16];

    let elapsed = now.elapsed();
    log!("Chunk took: {:.2?}s to load", elapsed);

    GeneratedChunkData {
        position,
        voxels,
        lod,
        biome: center_biome,
    }
}
