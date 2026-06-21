use crate::{
    ecs::world::World,
    terrain::{TerrainChunkMap, TerrainSettings, discover_terrain_textures},
};

pub(crate) fn add_terrain_package(world: &mut World) {
    let mut settings = TerrainSettings::default();

    // Try to auto-discover textures in well-known locations.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        manifest.join("../editor/res/textures/terrain"),
        manifest.join("../game/res/textures"),
        manifest.join("res/textures"),
    ];

    for dir in &candidates {
        let found = discover_terrain_textures(dir);
        if !found.is_empty() {
            settings.texture_layers = found;
            break;
        }
    }

    // Fallback hardcoded defaults.
    if settings.texture_layers.is_empty() {
        settings.texture_layers = vec![
            "textures/grass.png".to_string(),
            "textures/dirt.png".to_string(),
            "textures/stone.png".to_string(),
            "textures/sand.png".to_string(),
        ];
    }

    world.insert_resource(settings);
    world.insert_resource(TerrainChunkMap::default());
}

use std::path::Path;
