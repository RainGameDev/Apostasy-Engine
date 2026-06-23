use apostasy_macros::Resource;
use cgmath::Vector3;
use hashbrown::HashMap;

use crate::ecs::cell::EntityId;

#[derive(Resource, Clone)]
pub struct ChunkLoadBounds {
    pub player_chunk_pos: Vector3<i32>,
    pub load_radius: i32,
    pub v_load_radius: i32,
}

#[derive(Resource, Clone, Default)]
pub struct ChunkPositionMap {
    pub position_to_id: HashMap<Vector3<i32>, EntityId>,
    pub position_to_lod: HashMap<Vector3<i32>, u8>,
}

impl ChunkPositionMap {
    pub fn on_chunk_added(&mut self, id: EntityId, position: Vector3<i32>, lod: u8) {
        self.position_to_id.insert(position, id);
        self.position_to_lod.insert(position, lod);
    }

    pub fn on_chunk_lod_changed(&mut self, position: Vector3<i32>, new_lod: u8) {
        self.position_to_lod.insert(position, new_lod);
    }

    pub fn on_chunk_removed(&mut self, position: Vector3<i32>) {
        self.position_to_id.remove(&position);
        self.position_to_lod.remove(&position);
    }
}
