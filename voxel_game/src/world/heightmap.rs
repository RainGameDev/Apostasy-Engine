use apostasy_core::{noise::Perlin, voxels::biome::BiomeRegistry};

use crate::world::cache::NoiseColumnCache;

// Upsampling cell size per LOD
pub fn upsample_cell_size(lod: u8) -> usize {
    match lod {
        1 => 4,
        2 => 8,
        3 => 16,
        _ => 8,
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Upsample {
    X1,
    X2,
    X4,
    X8,
    X16,
    X32,
}

pub struct UpsampledHeightmap {
    coarse: Vec<f64>,
    grid_size: usize,
    cell_size: usize,
}

impl UpsampledHeightmap {
    pub fn new(
        world_x: f64,
        world_z: f64,
        upsample: Upsample,
        noise: &Perlin,
        biome_registry: &BiomeRegistry,
        cache: &mut NoiseColumnCache,
        lod: u8,
        seed: u32,
        temp_noise: &Perlin,
        humid_noise: &Perlin,
        continental_noise: &Perlin,
    ) -> Self {
        let cell_size = match upsample {
            Upsample::X1 => 1,
            Upsample::X2 => 2,
            Upsample::X4 => 4,
            Upsample::X8 => 8,
            Upsample::X16 => 16,
            Upsample::X32 => 32,
        };

        let grid_size = 32 / cell_size;
        let corner_count = grid_size + 1;
        let mut coarse = Vec::with_capacity(corner_count * corner_count);

        for gz in 0..corner_count {
            for gx in 0..corner_count {
                let wx = world_x as i32 + (gx * cell_size) as i32;
                let wz = world_z as i32 + (gz * cell_size) as i32;
                let col = cache.get_or_insert(
                    wx,
                    wz,
                    noise,
                    biome_registry,
                    lod,
                    seed,
                    temp_noise,
                    humid_noise,
                    continental_noise,
                );
                coarse.push(col.height as f64);
            }
        }

        Self {
            coarse,
            grid_size,
            cell_size,
        }
    }

    pub fn sample(&self, lx: usize, lz: usize) -> i32 {
        let cs = self.cell_size;
        let gx = lx / cs;
        let gz = lz / cs;
        let fx = (lx % cs) as f64 / cs as f64;
        let fz = (lz % cs) as f64 / cs as f64;
        let stride = self.grid_size + 1;

        let h00 = self.coarse[gz * stride + gx];
        let h10 = self.coarse[gz * stride + gx + 1];
        let h01 = self.coarse[(gz + 1) * stride + gx];
        let h11 = self.coarse[(gz + 1) * stride + gx + 1];

        (h00 * (1.0 - fx) * (1.0 - fz)
            + h10 * fx * (1.0 - fz)
            + h01 * (1.0 - fx) * fz
            + h11 * fx * fz) as i32
    }
}
