use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use apostasy_macros::Resource;
use parking_lot::RwLock;

use crate::{
    assets::{
        asset_manager::AssetManager,
        loaders::{
            material_loader::MaterialLoader,
            worldspace_loader::{WorldspaceLoader, WorldspaceRegistry},
        },
    },
    ecs::world::World,
    log, log_warn, project_dir,
    terrain::persistence::load_terrain_cells,
    worldspaces::worldspace_serializer::load_worldspace,
};

#[derive(Resource, Clone)]
pub struct ProjectRoot(pub PathBuf);

pub(crate) fn add_project_package(world: &mut World) {
    log!("Implimenting project package");

    if !world.has_resource::<AssetManager>() {
        world.insert_resource(AssetManager::new());
    }

    let worldspace_registry = Arc::new(RwLock::new(WorldspaceRegistry::default()));

    {
        let asset_manager = world.get_resource_mut::<AssetManager>().unwrap();
        asset_manager.register_loader(WorldspaceLoader {
            registry: Arc::clone(&worldspace_registry),
        });
        asset_manager.register_loader(MaterialLoader::new());

        let core_res = Path::new(env!("CARGO_MANIFEST_DIR")).join("res");
        if core_res.is_dir() {
            if let Err(e) = asset_manager.load_directory(&core_res) {
                log_warn!("Failed to load core res {:?}: {}", core_res, e);
            }
        }

        let project = project_dir();
        if project.is_dir() {
            if let Err(e) = asset_manager.load_directory(&project) {
                log_warn!("Failed to load project dir {:?}: {}", project, e);
            }
        }
    }

    world.insert_resource(ProjectRoot(project_dir()));
}

pub fn load_startup_worldspace(
    world: &mut World,
    worldspace_name: &str,
    retain_tags: &[&str],
) -> Result<()> {
    let worldspace_value = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .and_then(|loader| {
            let registry = loader.registry.read();
            registry
                .worldspaces
                .get(worldspace_name)
                .or_else(|| registry.worldspaces.get("default"))
                .cloned()
        });

    if let Some(value) = worldspace_value {
        load_worldspace(world, &value, retain_tags)?;
        log!("Loaded startup worldspace '{}'", worldspace_name);
    } else {
        log_warn!(
            "No startup worldspace '{}' found in project",
            worldspace_name
        );
    }

    let terrain_dir = project_dir().join("terrain");
    if terrain_dir.is_dir() {
        let _ = load_terrain_cells(world, &terrain_dir);
    }

    Ok(())
}
