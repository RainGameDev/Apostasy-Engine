use crate::{
    objects::world::World,
    terrain::{TerrainChunkMap, TerrainSettings},
};

pub(crate) fn add_terrain_package(world: &mut World) {
    let settings = TerrainSettings::default();

    world.insert_resource(settings);
    world.insert_resource(TerrainChunkMap::default());
}
