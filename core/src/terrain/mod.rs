use apostasy_macros::Resource;
use hashbrown::HashMap;

use crate::objects::cell::{CellCoord, ObjectId};

pub mod chunk;
pub mod mesh_builder;
pub mod persistence;
pub mod rebuild;

pub use mesh_builder::build_terrain_mesh;
pub use persistence::{load_terrain_cells, save_terrain_cells};
pub use rebuild::rebuild_dirty_terrain;

/// Global terrain configuration.
#[derive(Resource, Clone)]
pub struct TerrainSettings {
    /// Vertex grid resolution per cell side (default 128). Must be >= 2.
    pub resolution: u32,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            resolution: 32,
        }
    }
}

/// Maps cell coordinates to the ObjectId of the terrain object in that cell.
#[derive(Resource, Clone, Default)]
pub struct TerrainChunkMap(pub HashMap<CellCoord, ObjectId>);
