use std::sync::OnceLock;

use anyhow::{Error, Result};
use apostasy_macros::Resource;
use hashbrown::HashMap;
use noise::{NoiseFn, Perlin};

pub type BiomeId = u16;

/// A global registry of loaded biome definitions.
///
/// `defs` contains all available biomes, and the hash maps allow lookup by
/// biome name or id.
#[derive(Resource, Default, Clone, Debug)]
pub struct BiomeRegistry {
    pub defs: Vec<BiomeDefinition>,
    pub name_to_id: HashMap<String, BiomeId>,
    pub id_to_name: HashMap<BiomeId, String>,
}

impl BiomeRegistry {
    pub fn get_def(&self, id: BiomeId) -> Result<&BiomeDefinition> {
        let msg = format!("Biome {} not found", id);
        self.defs.get(id as usize).ok_or(Error::msg(msg))
    }
}

pub static NOISE: OnceLock<Perlin> = OnceLock::new();
pub static TEMPERATURE_NOISE: OnceLock<Perlin> = OnceLock::new();
pub static HUMIDITY_NOISE: OnceLock<Perlin> = OnceLock::new();
pub static CONTINENTAL_NOISE: OnceLock<Perlin> = OnceLock::new();

/// A structure entry inside a biome definition.
///
/// This describes how a biome should spawn reusable structures or data-driven
/// features during world generation.
#[derive(Clone, Debug)]
pub struct StructureDefinition {
    /// Feature type, e.g. `tree`, `boulder`, or a custom asset-backed type.
    pub structure_type: String,
    /// Relative density across the feature grid.
    pub density: f64,
    /// Optional external structure asset name to instantiate.
    pub asset: Option<String>,
    /// Named voxel references used by the structure generator.
    pub voxels: HashMap<String, String>,
    /// Additional custom parameters for the generator or asset placement.
    pub parameters: HashMap<String, serde_yaml::Value>,
}

/// Per-biome terrain shaping parameters.
#[derive(Clone, Debug)]
pub struct TerrainShaping {
    /// How strongly ridge noise lifts terrain. 0.0 = flat, 1.0 = sharp ridgelines.
    pub ridge_strength: f64,

    /// How strongly peak noise adds bumpy high points. 0.0 = smooth, 1.0 = peaky.
    pub peak_strength: f64,

    /// How strongly valley erosion carves downward. 0.0 = none, 1.0 = deep valleys.
    pub valley_strength: f64,

    /// How far the continentalness noise shifts this biome's base height.
    pub continental_scale: f64,

    /// Exponent applied to the base FBM noise before scaling by amplitude.
    pub height_curve: f64,
}

impl Default for TerrainShaping {
    fn default() -> Self {
        Self {
            ridge_strength: 0.0,
            peak_strength: 0.0,
            valley_strength: 0.0,
            continental_scale: 15.0,
            height_curve: 1.0,
        }
    }
}

impl TerrainShaping {
    /// Preset for flat biomes: plains, beaches, tundra.
    pub fn flat() -> Self {
        Self {
            ridge_strength: 0.03,
            peak_strength: 0.03,
            valley_strength: 0.0,
            continental_scale: 10.0,
            height_curve: 1.0,
        }
    }

    /// Preset for gently rolling terrain: forests, savannas.
    pub fn rolling() -> Self {
        Self {
            ridge_strength: 0.15,
            peak_strength: 0.12,
            valley_strength: 0.08,
            continental_scale: 18.0,
            height_curve: 1.2,
        }
    }

    /// Preset for hilly terrain: shrublands, taiga, jungles.
    pub fn hilly() -> Self {
        Self {
            ridge_strength: 0.4,
            peak_strength: 0.3,
            valley_strength: 0.2,
            continental_scale: 25.0,
            height_curve: 1.4,
        }
    }

    /// Preset for mountainous terrain.
    pub fn mountainous() -> Self {
        Self {
            ridge_strength: 1.0,
            peak_strength: 0.8,
            valley_strength: 0.6,
            continental_scale: 45.0,
            height_curve: 1.6,
        }
    }

    /// Preset for ocean/deep water biomes: as flat as possible.
    pub fn ocean() -> Self {
        Self {
            ridge_strength: 0.0,
            peak_strength: 0.0,
            valley_strength: 0.0,
            continental_scale: 5.0,
            height_curve: 1.0,
        }
    }
}

/// A biome definition used during terrain generation.
///
/// This contains voxel rules, procedural noise properties, climate values, and
/// any structures that should spawn inside the biome.
#[derive(Clone, Debug)]
pub struct BiomeDefinition {
    pub name: String,
    pub namespace: String,
    pub class: String,

    /// Voxels used for the terrain surface.
    pub surface_voxels: Vec<String>,
    /// Voxels used for the shallow subsurface layer.
    pub subsurface_voxels: Vec<String>,
    /// Voxels used for deeper underground layers.
    pub underground_voxels: Vec<String>,

    /// Height amplitude multiplier for this biome's base FBM noise.
    pub amplitude: f64,
    /// Noise frequency used to shape biome terrain.
    pub frequency: f64,
    /// Number of noise octaves for terrain detail.
    pub octaves: u32,

    /// Target biome temperature for climate sampling.
    pub temperature: f64,
    /// Target biome humidity for climate sampling.
    pub humidity: f64,
    /// Structures or feature definitions that spawn in this biome.
    pub structures: Vec<StructureDefinition>,

    /// RGB color for water tinting.
    pub water_color: (u8, u8, u8),
    /// RGB color for foliage tinting.
    pub foliage_color: (u8, u8, u8),

    /// Per-biome terrain shaping parameters. Controls how much ridge,
    /// peak, valley, and continental noise influence this biome's height.
    pub terrain_shaping: TerrainShaping,
}

impl Default for BiomeDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            namespace: String::new(),
            class: String::new(),
            surface_voxels: vec![],
            subsurface_voxels: vec![],
            underground_voxels: vec![],
            amplitude: 20.0,
            frequency: 0.005,
            octaves: 5,
            temperature: 0.5,
            humidity: 0.5,
            structures: vec![],
            water_color: (63, 118, 228),
            foliage_color: (77, 140, 61),
            terrain_shaping: TerrainShaping::default(),
        }
    }
}

pub struct ClimateCache {
    pub temp: [[f64; 5]; 5],
    pub humid: [[f64; 5]; 5],
    pub continentalness: [[f64; 5]; 5],
    pub climate_scale: usize,
}

impl ClimateCache {
    pub fn new(world_x: f64, world_z: f64, seed: u32) -> Self {
        let climate_scale = 8usize;
        let grid = (32 / climate_scale) + 1; // 5x5

        let temp_noise = TEMPERATURE_NOISE.get_or_init(|| Perlin::new(seed));
        let humid_noise = HUMIDITY_NOISE.get_or_init(|| Perlin::new(seed.wrapping_add(1)));
        let continental_noise = CONTINENTAL_NOISE.get_or_init(|| Perlin::new(seed.wrapping_add(2)));

        let mut temp = [[0.0f64; 5]; 5];
        let mut humid = [[0.0f64; 5]; 5];
        let mut continentalness = [[0.0f64; 5]; 5];

        for cz in 0..grid {
            for cx in 0..grid {
                let sx = world_x + (cx * climate_scale) as f64;
                let sz = world_z + (cz * climate_scale) as f64;
                temp[cz][cx] = (temp_noise.get([sx * 0.001, sz * 0.001]) + 1.0) * 0.5;
                humid[cz][cx] = (humid_noise.get([sx * 0.001, sz * 0.001]) + 1.0) * 0.5;
                continentalness[cz][cx] =
                    (continental_noise.get([sx * 0.00035, sz * 0.00035]) + 1.0) * 0.5;
            }
        }

        Self {
            temp,
            humid,
            continentalness,
            climate_scale,
        }
    }

    /// local_x/local_z are column offsets within the chunk (0..32)
    pub fn sample(&self, local_x: f64, local_z: f64) -> (f64, f64, f64) {
        let t = bilinear_interpolation(&self.temp, local_x, local_z, self.climate_scale);
        let h = bilinear_interpolation(&self.humid, local_x, local_z, self.climate_scale);
        let c = bilinear_interpolation(&self.continentalness, local_x, local_z, self.climate_scale);
        (t, h, c)
    }
}

pub fn select_biome_at_climate(
    temperature: f64,
    humidity: f64,
    registry: &BiomeRegistry,
) -> BiomeId {
    registry
        .defs
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let dist_a = (a.temperature - temperature).powi(2) + (a.humidity - humidity).powi(2);
            let dist_b = (b.temperature - temperature).powi(2) + (b.humidity - humidity).powi(2);
            dist_a.partial_cmp(&dist_b).unwrap()
        })
        .map(|(i, _)| i as BiomeId)
        .unwrap_or(0)
}

fn biome_climate_weights(
    temperature: f64,
    humidity: f64,
    registry: &BiomeRegistry,
    blend_distance: f64,
) -> Vec<(BiomeId, f64)> {
    if blend_distance <= 0.0 {
        let closest = registry
            .defs
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let dist_a =
                    (a.temperature - temperature).powi(2) + (a.humidity - humidity).powi(2);
                let dist_b =
                    (b.temperature - temperature).powi(2) + (b.humidity - humidity).powi(2);
                dist_a.partial_cmp(&dist_b).unwrap()
            })
            .map(|(i, _)| i as BiomeId)
            .unwrap_or(0);

        return vec![(closest, 1.0)];
    }

    let sigma = blend_distance * 0.5;
    let two_sigma_sq = 2.0 * sigma * sigma;
    let mut weights: Vec<(BiomeId, f64)> = registry
        .defs
        .iter()
        .enumerate()
        .map(|(i, def)| {
            let dist_sq =
                (def.temperature - temperature).powi(2) + (def.humidity - humidity).powi(2);
            let weight = (-dist_sq / two_sigma_sq).exp();
            (i as BiomeId, weight)
        })
        .collect();

    weights.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let total_weight: f64 = weights.iter().map(|(_, weight)| *weight).sum();

    if total_weight <= 0.0 {
        return vec![(weights[0].0, 1.0)];
    }

    weights
        .into_iter()
        .filter_map(|(id, weight)| {
            let normalized = weight / total_weight;
            if normalized > 1e-4 {
                Some((id, normalized))
            } else {
                None
            }
        })
        .collect()
}

pub fn sample_biome_weights_at_climate(
    temperature: f64,
    humidity: f64,
    registry: &BiomeRegistry,
    blend_distance: f64,
) -> Vec<(BiomeId, f64)> {
    biome_climate_weights(temperature, humidity, registry, blend_distance)
}

fn bilinear_interpolation(cache: &[[f64; 5]; 5], cx: f64, cz: f64, scale: usize) -> f64 {
    let gx = cx / scale as f64;
    let gz = cz / scale as f64;
    let x0 = gx.floor() as usize;
    let z0 = gz.floor() as usize;
    let x1 = (x0 + 1).min(4);
    let z1 = (z0 + 1).min(4);
    let tx = gx.fract();
    let tz = gz.fract();
    let top = cache[z0][x0] * (1.0 - tx) + cache[z0][x1] * tx;
    let bot = cache[z1][x0] * (1.0 - tx) + cache[z1][x1] * tx;
    top * (1.0 - tz) + bot * tz
}

pub fn select_biome(world_x: f64, world_z: f64, registry: &BiomeRegistry, seed: u32) -> BiomeId {
    let temp_noise = Perlin::new(seed);
    let humid_noise = Perlin::new(seed.wrapping_add(1));

    let temperature = (temp_noise.get([world_x * 0.001, world_z * 0.001]) + 1.0) * 0.5;
    let humidity = (humid_noise.get([world_x * 0.001, world_z * 0.001]) + 1.0) * 0.5;

    registry
        .defs
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let dist_a = (a.temperature - temperature).powi(2) + (a.humidity - humidity).powi(2);
            let dist_b = (b.temperature - temperature).powi(2) + (b.humidity - humidity).powi(2);
            dist_a.partial_cmp(&dist_b).unwrap()
        })
        .map(|(i, _)| i as BiomeId)
        .unwrap_or(0)
}

pub fn sample_biome_weights(
    world_x: f64,
    world_z: f64,
    registry: &BiomeRegistry,
    seed: u32,
    blend_distance: f64,
) -> Vec<(BiomeId, f64)> {
    let temp_noise = TEMPERATURE_NOISE.get_or_init(|| Perlin::new(seed));
    let humid_noise = HUMIDITY_NOISE.get_or_init(|| Perlin::new(seed.wrapping_add(1)));

    let temperature = (temp_noise.get([world_x * 0.001, world_z * 0.001]) + 1.0) * 0.5;
    let humidity = (humid_noise.get([world_x * 0.001, world_z * 0.001]) + 1.0) * 0.5;

    biome_climate_weights(temperature, humidity, registry, blend_distance)
}
