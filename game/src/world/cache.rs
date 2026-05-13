use apostasy_core::{
    egui::ahash::{HashMap, HashMapExt},
    noise::Perlin,
    voxels::biome::BiomeRegistry,
};

use crate::world::helpers::compute_column;

#[derive(Clone)]
pub struct CachedColumn {
    pub height: i32,
    pub biome: u16,
}

pub struct NoiseColumnCache {
    pub entries: HashMap<(i32, i32), CachedColumn>,
}

impl NoiseColumnCache {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(cap),
        }
    }

    pub fn get_or_insert(
        &mut self,
        wx: i32,
        wz: i32,
        noise: &Perlin,
        biome_registry: &BiomeRegistry,
        lod: u8,
        seed: u32,
    ) -> &CachedColumn {
        self.entries.entry((wx, wz)).or_insert_with(|| {
            let (height, biome) = compute_column(wx as f64, wz as f64, noise, biome_registry, lod);
            CachedColumn { height, biome }
        })
    }
}
