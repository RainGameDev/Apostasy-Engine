use apostasy_macros::Resource;
use hashbrown::HashMap;

use crate::objects::cell::{CellCoord, ObjectId};

pub mod chunk;
pub mod mesh_builder;
pub mod persistence;
pub mod rebuild;
pub mod texture_atlas;

pub use texture_atlas::{TerrainAtlasNeedsRebuild, TerrainTextureAtlas, discover_terrain_textures, load_terrain_texture};

/// Global terrain configuration.
#[derive(Resource, Clone)]
pub struct TerrainSettings {
    /// Vertex grid resolution per cell side (default 128). Must be >= 2.
    pub resolution: u32,
    /// Texture layer paths for the terrain shader (up to 64).
    pub texture_layers: Vec<String>,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            resolution: 32,
            texture_layers: Vec::new(),
        }
    }
}

/// Maps cell coordinates to the ObjectId of the terrain object in that cell.
#[derive(Resource, Clone, Default)]
pub struct TerrainChunkMap(pub HashMap<CellCoord, ObjectId>);
