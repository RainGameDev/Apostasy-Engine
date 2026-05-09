use std::{
    path::Path,
    sync::{Arc, RwLock},
};

use crate::{
    assets::{asset_manager::AssetManager, loaders::item_loader::ItemLoader},
    items::ItemRegistry,
    log,
    objects::world::World,
};

pub(crate) fn add_item_system_package(world: &mut World) {
    log!("Implimanting item system package");

    let item_registry = Arc::new(RwLock::new(ItemRegistry::default()));

    {
        {
            let mut asset_manager = AssetManager::new();
            asset_manager.register_loader(ItemLoader {
                registry: Arc::clone(&item_registry),
            });

            asset_manager
                .load_directory(Path::new(&format!(
                    "{}/{}",
                    env!("CARGO_MANIFEST_DIR"),
                    "res/"
                )))
                .unwrap();

            asset_manager.load_directory(Path::new("res/")).unwrap();
        }

        let item_registry = Arc::try_unwrap(item_registry)
            .expect("ItmeRegistry still has multiple owners")
            .into_inner()
            .expect("ItemRegistry RwLock poisoned");

        world.insert_resource(item_registry);
    }
}
