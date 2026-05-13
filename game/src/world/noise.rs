use apostasy_core::{
    noise::{NoiseFn, Perlin},
    voxels::biome::BiomeDefinition,
};

// Global noise layers
#[derive(Clone)]
pub struct GlobalNoiseLayers {
    ridge: f64,
    peak: f64,
    valley: f64,
}

impl GlobalNoiseLayers {
    pub fn sample(noise: &Perlin, world_x: f64, world_z: f64) -> Self {
        Self {
            ridge: ridged_fbm(noise, world_x * 0.006, world_z * 0.006, 4, 2.0, 0.55),
            peak: fractal_brownian_motion(noise, world_x * 0.02, world_z * 0.02, 3, 2.0, 0.45),
            valley: fractal_brownian_motion(noise, world_x * 0.012, world_z * 0.012, 3, 2.0, 0.5)
                .abs(),
        }
    }

    pub fn weighted_contribution(&self, biome: &BiomeDefinition, biome_weight: f64) -> f64 {
        let s = &biome.terrain_shaping;
        (self.ridge * 35.0 * s.ridge_strength + self.peak * 28.0 * s.peak_strength
            - self.valley * 12.0 * s.valley_strength)
            * biome_weight
    }
}

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

    value / max_value
}

pub fn sample_3d_noise(noise: &Perlin, wx: f64, wy: f64, wz: f64, scale: f64) -> f64 {
    noise.get([wx * scale, wy * scale, wz * scale])
}

pub fn sample_cavern_noise(noise: &Perlin, wx: f64, wy: f64, wz: f64) -> f64 {
    let cavern_low = sample_3d_noise(noise, wx, wy, wz, 0.00005);
    let cavern_detail = sample_3d_noise(noise, wx, wy, wz, 0.0002).abs() * 0.3;
    (cavern_low + cavern_detail) / 1.3
}

pub fn sample_tunnel_noise(noise: &Perlin, wx: f64, wy: f64, wz: f64) -> f64 {
    let tunnel_winding = sample_3d_noise(noise, wx, wy, wz, 0.05);
    (1.0 - tunnel_winding.abs() * 0.7).max(-1.0)
}

pub fn is_carved_out(
    wx: f64,
    wy: f64,
    wz: f64,
    depth: i32,
    cavern_noise: &Perlin,
    tunnel_noise: &Perlin,
) -> bool {
    if depth < 0 {
        return false;
    }

    let cavern = sample_cavern_noise(cavern_noise, wx, wy, wz);
    let tunnel = sample_tunnel_noise(tunnel_noise, wx, wy, wz);

    if depth == 0 {
        return cavern < -0.3;
    }

    if depth < 3 {
        let cavern_carve = cavern < -0.55;
        let tunnel_carve = tunnel < -0.2 && cavern < -0.4;
        return cavern_carve || tunnel_carve;
    }

    let cavern_carve = cavern < -0.4;
    let tunnel_carve = tunnel < -0.3 && cavern > -0.7;

    cavern_carve || tunnel_carve
}

pub fn ridged_fbm(noise: &Perlin, x: f64, z: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
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

// Per-biome detail
pub fn lod_octaves(biome_octaves: u32, lod: u8) -> u32 {
    match lod {
        1 => biome_octaves,
        2 => (biome_octaves - 1).max(2),
        3 => (biome_octaves - 2).max(2),
        _ => 2,
    }
}

pub fn apply_height_curve(val: f64, curve: f64) -> f64 {
    if val > 0.0 { val.powf(curve) } else { val }
}

pub fn compute_biome_base_detail(
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
