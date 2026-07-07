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

    migrate_legacy_terrain_dir();
}

/// Terrain used to live directly in `{project}/terrain/`; it is now stored
/// per worldspace. Move any legacy files into `terrain/default/` once.
fn migrate_legacy_terrain_dir() {
    let root = crate::project_dir().join("terrain");
    if !root.is_dir() {
        return;
    }
    let default_dir = root.join("default");

    let entries: Vec<_> = match std::fs::read_dir(&root) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && matches!(
                        p.extension().and_then(|s| s.to_str()),
                        Some("terrain") | Some("yaml")
                    )
            })
            .collect(),
        Err(_) => return,
    };
    if entries.is_empty() {
        return;
    }

    if std::fs::create_dir_all(&default_dir).is_err() {
        return;
    }
    for path in &entries {
        if let Some(name) = path.file_name() {
            let _ = std::fs::rename(path, default_dir.join(name));
        }
    }
    crate::log!(
        "Migrated {} legacy terrain files into {:?}",
        entries.len(),
        default_dir
    );
}

use std::path::Path;
