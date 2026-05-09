use crate::{
    objects::world::World,
    packages::{item_system_package::add_item_system_package, voxel_package::add_voxel_package},
};

pub mod item_system_package;
pub mod voxel_package;

#[derive(Clone, Copy)]
pub enum Packages {
    Voxel,
    ItemSystem,
}

pub fn add_package(world: &mut World, package: Packages) {
    match package {
        Packages::Voxel => {
            add_voxel_package(world);
        }
        Packages::ItemSystem => {
            add_item_system_package(world);
        }
    }
}
