use apostasy_core::{
    cgmath::Vector3,
    log,
    noise::{NoiseFn, Perlin},
    utils::flatten::flatten,
    voxels::{
        biome::{
            BiomeDefinition, BiomeRegistry, CONTINENTAL_NOISE, ClimateCache, HUMIDITY_NOISE, NOISE,
            StructureDefinition, TEMPERATURE_NOISE, sample_biome_weights,
            sample_biome_weights_at_climate,
        },
        chunk::GeneratedChunkData,
        structure::StructureRegistry,
        voxel::{VoxelId, VoxelRegistry},
    },
};

// Noise helpers
fn fractal_brownian_motion(
    noise: &Perlin,
    x: f64,
    z: f64,
    octaves: u32,
    lacunarity: f64,
    gain: f64,
) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        value += noise.get([x * frequency, z * frequency]) * amplitude;
        max_value += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }

    value / max_value // normalised to [-1, 1]
}

/// Sample 3D noise for cave generation at world coordinates.
/// Returns [-1, 1] noise value.
fn sample_3d_noise(noise: &Perlin, wx: f64, wy: f64, wz: f64, scale: f64) -> f64 {
    noise.get([wx * scale, wy * scale, wz * scale])
}

/// Layered 3D noise for caverns .
fn sample_cavern_noise(noise: &Perlin, wx: f64, wy: f64, wz: f64) -> f64 {
    let cavern_low = sample_3d_noise(noise, wx, wy, wz, 0.00005);
    let cavern_detail = sample_3d_noise(noise, wx, wy, wz, 0.0002).abs() * 0.3;

    (cavern_low + cavern_detail) / 1.3
}

/// Layered 3D noise for tunnels (high frequency = narrow paths).
fn sample_tunnel_noise(noise: &Perlin, wx: f64, wy: f64, wz: f64) -> f64 {
    let tunnel_winding = sample_3d_noise(noise, wx, wy, wz, 0.05);
    (1.0 - tunnel_winding.abs() * 0.7).max(-1.0)
}

/// Determines if a voxel position is inside a cave.
fn is_carved_out(
    wx: f64,
    wy: f64,
    wz: f64,
    depth: i32,
    cavern_noise: &Perlin,
    tunnel_noise: &Perlin,
) -> bool {
    // Don't carve above surface
    if depth < 0 {
        return false;
    }

    let cavern = sample_cavern_noise(cavern_noise, wx, wy, wz);
    let tunnel = sample_tunnel_noise(tunnel_noise, wx, wy, wz);

    // Surface openings check for strong cave signal
    if depth == 0 {
        return cavern < -0.3;
    }

    // Near-surface allow caves to start forming, creating entrance connections
    if depth < 3 {
        let cavern_carve = cavern < -0.55; // Caves propagate upward
        let tunnel_carve = tunnel < -0.2 && cavern < -0.4;
        return cavern_carve || tunnel_carve;
    }

    // Deep caves normal carving thresholds
    let cavern_carve = cavern < -0.4;
    let tunnel_carve = tunnel < -0.3 && cavern > -0.7;

    cavern_carve || tunnel_carve
}

fn ridged_fbm(noise: &Perlin, x: f64, z: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    let mut weight = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        let signal = 1.0 - noise.get([x * frequency, z * frequency]).abs();
        value += signal * amplitude * weight;
        max_value += amplitude * weight;
        weight = (signal * 2.0).clamp(0.0, 1.0);
        amplitude *= gain;
        frequency *= lacunarity;
    }

    (value / max_value).clamp(0.0, 1.0)
}

// Per-biome base detail

fn lod_octaves(biome_octaves: u32, lod: u8) -> u32 {
    match lod {
        1 => biome_octaves,
        2 => (biome_octaves - 1).max(2),
        3 => (biome_octaves - 2).max(2),
        _ => 2,
    }
}

/// Apply the biome's height-curve exponent to the raw FBM value.
/// Values above zero are exaggerated; negative values are left linear so
/// that below-average terrain doesn't get artificially raised.
fn apply_height_curve(val: f64, curve: f64) -> f64 {
    if val > 0.0 { val.powf(curve) } else { val }
}

/// Returns the base FBM contribution for a single biome at (world_x, world_z).
/// Does NOT include ridge, peak, valley, or continental terms.
fn compute_biome_base_detail(
    noise: &Perlin,
    world_x: f64,
    world_z: f64,
    biome: &BiomeDefinition,
    lod: u8,
) -> f64 {
    let nx = world_x * biome.frequency;
    let nz = world_z * biome.frequency;
    let octaves = lod_octaves(biome.octaves, lod);
    let raw = fractal_brownian_motion(noise, nx, nz, octaves, 2.0, 0.5);
    apply_height_curve(raw, biome.terrain_shaping.height_curve) * biome.amplitude
}

// Global noise layers (evaluated once per column, shared by all biomes)

struct GlobalNoiseLayers {
    /// ridged_fbm result, [0, 1]
    ridge: f64,
    /// FBM peak bump, [-1, 1]
    peak: f64,
    /// absolute FBM valley carve, [0, ~1]
    valley: f64,
}

impl GlobalNoiseLayers {
    fn sample(noise: &Perlin, world_x: f64, world_z: f64) -> Self {
        Self {
            ridge: ridged_fbm(noise, world_x * 0.006, world_z * 0.006, 4, 2.0, 0.55),
            peak: fractal_brownian_motion(noise, world_x * 0.02, world_z * 0.02, 3, 2.0, 0.45),
            valley: fractal_brownian_motion(noise, world_x * 0.012, world_z * 0.012, 3, 2.0, 0.5)
                .abs(),
        }
    }

    /// Compute this layer's height contribution for a single biome, weighted
    /// by that biome's terrain-shaping parameters and the blending weight.
    fn weighted_contribution(&self, biome: &BiomeDefinition, biome_weight: f64) -> f64 {
        let s = &biome.terrain_shaping;
        (self.ridge * 35.0 * s.ridge_strength + self.peak * 28.0 * s.peak_strength
            - self.valley * 12.0 * s.valley_strength)
            * biome_weight
    }
}

// Height + dominant-biome computation (shared between heightmap build and
// the structure-placement height probe)

const SEA_LEVEL: i32 = 58;
const BASE_HEIGHT: f64 = 60.0;
const BIOME_BLEND_DISTANCE: f64 = 0.12;

/// Returns `(surface_y, dominant_biome_id)` for a world-space column.
fn sample_height_and_biome(
    world_x: f64,
    world_z: f64,
    noise: &Perlin,
    biome_registry: &BiomeRegistry,
    lod: u8,
    seed: u32,
) -> (i32, u16) {
    let temp_noise = TEMPERATURE_NOISE.get_or_init(|| Perlin::new(seed));
    let humid_noise = HUMIDITY_NOISE.get_or_init(|| Perlin::new(seed.wrapping_add(1)));
    let continental_noise = CONTINENTAL_NOISE.get_or_init(|| Perlin::new(seed.wrapping_add(2)));

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

const FEATURE_GRID_SIZE: i32 = 8;
const FEATURE_CELLS_PER_CHUNK: f64 = ((32 / FEATURE_GRID_SIZE) * (32 / FEATURE_GRID_SIZE)) as f64;

fn div_floor(value: i32, divisor: i32) -> i32 {
    if value >= 0 {
        value / divisor
    } else {
        (value - divisor + 1) / divisor
    }
}

fn hash_column(x: i32, z: i32, seed: u32) -> u32 {
    let mut h = seed;
    h ^= (x as u32).wrapping_mul(0x9e3779b9);
    h = h.wrapping_mul(0x517cc1b727220a95u64 as u32);
    h ^= h >> 17;
    h ^= (z as u32).wrapping_mul(0x6c62272e07bb0142u64 as u32);
    h = h.wrapping_mul(0xbf58476d1ce4e5b9u64 as u32);
    h ^= h >> 31;
    h
}

fn random_range(x: i32, z: i32, seed: u32, min: u32, max: u32) -> u32 {
    let h = hash_column(x, z, seed);
    min + (h % (max - min + 1))
}

fn set_voxel_global(
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
    voxels[index] = voxel_id;
}

fn set_voxel_global_non_floating(
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

    // Skip over empty spaces and water voxels
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
            let index = flatten(lx as u32, above_y as u32, lz as u32, 32);
            voxels[index] = voxel_id;
        }
    }
}

fn set_voxel_global_if_empty(
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
    water_voxel: u16,
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

    // Don't spawn trees underwater (at or below sea level)
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
            let index = flatten(lx as u32, ly as u32, lz as u32, 32);
            voxels[index] = trunk_id;
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
    let boulder_id = structure
        .voxels
        .get("boulder")
        .and_then(|name| registry.name_to_id.get(name).copied());

    let boulder_id = match boulder_id {
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
    let noise = NOISE.get_or_init(|| Perlin::new(seed));

    let cavern_noise = Perlin::new(seed.wrapping_add(10)); // Different seed for variety
    let tunnel_noise = Perlin::new(seed.wrapping_add(11));

    let world_x = position.x as f64 * 32.0;
    let world_y = position.y as f64 * 32.0;
    let world_z = position.z as f64 * 32.0;

    let climate = ClimateCache::new(world_x, world_z, seed);

    let continental_noise = CONTINENTAL_NOISE.get_or_init(|| Perlin::new(seed.wrapping_add(2)));

    let mut heightmap = [0i32; 32 * 32];
    let mut column_biome = [0u16; 32 * 32];

    for z in 0..32usize {
        for x in 0..32usize {
            let wx = world_x + x as f64;
            let wz = world_z + z as f64;

            let (temp, humid, continental) = climate.sample(x as f64, z as f64);
            let climate_temp = (temp * 0.7 + continental * 0.25 + 0.05).clamp(0.0, 1.0);
            let climate_humid = (humid * 0.6 + (1.0 - continental) * 0.3 + 0.05).clamp(0.0, 1.0);

            let weights = sample_biome_weights_at_climate(
                climate_temp,
                climate_humid,
                biome_registry,
                BIOME_BLEND_DISTANCE,
            );

            // Sample global noise layers once per column — shared across all
            // biome contributions so there are no seams between biome regions.
            let layers = GlobalNoiseLayers::sample(noise, wx, wz);

            let mut weighted_base = 0.0f64;
            let mut weighted_global = 0.0f64;
            let mut weighted_continental_scale = 0.0f64;
            let mut dominant_biome = 0u16;
            let mut dominant_weight = 0.0f64;

            for &(biome_id, weight) in &weights {
                let biome = &biome_registry.defs[biome_id as usize];

                // Base FBM shaped by this biome's own frequency / amplitude /
                // height curve — the only part that varies per-biome.
                weighted_base += compute_biome_base_detail(noise, wx, wz, biome, lod) * weight;

                // Global ridge / peak / valley contributions, gated by the
                // biome's shaping parameters so flat biomes stay flat.
                weighted_global += layers.weighted_contribution(biome, weight);

                // Continental scale: how much the coast-to-inland gradient
                // lifts or lowers this biome.
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

            heightmap[z * 32 + x] = blended_height as i32;
            column_biome[z * 32 + x] = dominant_biome;
        }
    }
    // Fill voxels

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
                let wy = world_y as i32 + y as i32;
                let depth = surface_y - wy;

                // World coordinates for 3D noise
                let wx = world_x + x as f64;
                let wy_f64 = wy as f64;
                let wz = world_z + z as f64;

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
                if is_carved_out(wx, wy_f64, wz, depth, &cavern_noise, &tunnel_noise) {
                    id = 0;
                }
                voxels[flatten(x as u32, y as u32, z as u32, 32)] = id;
            }
        }
    }

    // Structure placement

    let chunk_world_x = position.x * 32;
    let chunk_world_y = position.y * 32;
    let chunk_world_z = position.z * 32;

    let feature_radius = 4;
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

            let (feature_surface_y, feature_biome_id) = sample_height_and_biome(
                feature_x as f64,
                feature_z as f64,
                noise,
                biome_registry,
                lod,
                seed,
            );
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
