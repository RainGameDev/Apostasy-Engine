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
    X2,
    X4,
    X8,
    X16,
}

// Upsampled heightmap
pub struct UpsampledHeightmap {
    coarse: Vec<f64>,
    grid_size: usize,
    cell_size: usize,
}

impl UpsampledHeightmap {
    pub fn new(
        world_x: f64,
        world_z: f64,
        cell_size: usize,
        noise: &Perlin,
        biome_registry: &BiomeRegistry,
        cache: &mut NoiseColumnCache,
        lod: u8,
        seed: u32,
    ) -> Self {
        debug_assert!(32 % cell_size == 0, "cell_size must divide 32 evenly");

        let grid_size = 32 / cell_size;
        let corner_count = grid_size + 1;
        let mut coarse = Vec::with_capacity(corner_count * corner_count);

        for gz in 0..corner_count {
            for gx in 0..corner_count {
                let wx = world_x as i32 + (gx * cell_size) as i32;
                let wz = world_z as i32 + (gz * cell_size) as i32;

                // Reuse the cache so corners shared with adjacent chunks are only ever computed once per generation session
                let col = cache.get_or_insert(wx, wz, noise, biome_registry, lod, seed);
                coarse.push(col.height as f64);
            }
        }

        Self {
            coarse,
            grid_size,
            cell_size,
        }
    }

    /// interpolat height for local chunk coordinates (lx, lz)
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

        let h = h00 * (1.0 - fx) * (1.0 - fz)
            + h10 * fx * (1.0 - fz)
            + h01 * (1.0 - fx) * fz
            + h11 * fx * fz;

        h as i32
    }
}
