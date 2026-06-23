use crate::{
    ecs::world::World,
    packages::{item_system_package::add_item_system_package, terrain_package::add_terrain_package, voxel_package::add_voxel_package},
};

pub mod item_system_package;
pub mod terrain_package;
pub mod voxel_package;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Packages {
    Voxel,
    ItemSystem,
    Terrain,
}

pub fn add_package(world: &mut World, package: Packages) {
    match package {
        Packages::Voxel => {
            add_voxel_package(world);
        }
        Packages::ItemSystem => {
            add_item_system_package(world);
        }
        Packages::Terrain => {
            add_terrain_package(world);
        }
    }
}
