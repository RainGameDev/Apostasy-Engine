use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use crate::{
    assets::{
        asset_manager::AssetManager,
        loaders::{biome_loader::BiomeLoader, structure_loader::StructureLoader, voxel_loader::VoxelLoader},
    },
    log,
    objects::world::World,
    voxels::{
        biome::BiomeRegistry,
        chunk::VoxelBreakProgress,
        structure::StructureRegistry,
        texture_atlas::{AtlasBuilder, PendingAtlas},
        voxel::VoxelRegistry,
    },
};

pub(crate) fn add_voxel_package(world: &mut World) {
    log!("Implimanting voxel package");

    let voxel_registry = Arc::new(RwLock::new(VoxelRegistry::default()));
    let biome_registry = Arc::new(RwLock::new(BiomeRegistry::default()));
    let structure_registry = Arc::new(RwLock::new(StructureRegistry::default()));
    let atlas_builder = Arc::new(RwLock::new(AtlasBuilder::new(16)));

    {
        let mut asset_manager = AssetManager::new();
        asset_manager.register_loader(VoxelLoader {
            registry: Arc::clone(&voxel_registry),
            atlas_builder: Arc::clone(&atlas_builder),
        });
        asset_manager.register_loader(BiomeLoader {
            registry: Arc::clone(&biome_registry),
        });
        asset_manager.register_loader(StructureLoader {
            registry: Arc::clone(&structure_registry),
        });

        asset_manager
            .load_directory(Path::new(&format!(
                "{}/{}",
                env!("CARGO_MANIFEST_DIR"),
                "res/"
            )))
            .unwrap();

        asset_manager.load_directory(Path::new("res/"
        ))
        .unwrap();
    }

    let registry = Arc::try_unwrap(voxel_registry)
        .expect("VoxelRegistry still has multiple owners")
        .into_inner()
        .expect("VoxelRegistry RwLock poisoned");

    let biome_registry = Arc::try_unwrap(biome_registry)
        .expect("BiomeRegistry still has multiple owners")
        .into_inner()
        .expect("BiomeRegistry RwLock poisoned");

    let structure_registry = Arc::try_unwrap(structure_registry)
        .expect("StructureRegistry still has multiple owners")
        .into_inner()
        .expect("StructureRegistry RwLock poisoned");

    let atlas_builder = Arc::try_unwrap(atlas_builder)
        .unwrap()
        .into_inner()
        .unwrap();

    let (atlas_image, atlas_tiles) = atlas_builder.build();

    world.insert_resource(registry);
    world.insert_resource(biome_registry);
    world.insert_resource(structure_registry);
    world.insert_resource(VoxelBreakProgress::default());
    world.insert_resource(PendingAtlas {
        image: atlas_image,
        tiles: atlas_tiles,
    });
}
