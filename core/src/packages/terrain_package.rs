use crate::{
    objects::world::World,
    terrain::{TerrainChunkMap, TerrainSettings},
};

pub(crate) fn add_terrain_package(world: &mut World) {
    world.insert_resource(TerrainSettings::default());
    world.insert_resource(TerrainChunkMap::default());
}
