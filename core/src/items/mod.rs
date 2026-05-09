use std::fmt::Debug;

use apostasy_macros::Resource;
use hashbrown::HashMap;

use crate::objects::component::BoxedComponent;

pub mod container;

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub id: ItemId,
}

pub type ItemId = u16;
#[derive(Clone)]
pub struct ItemDefinition {
    pub name: String,
    pub namespace: String,
    pub class: String,
    pub components: Vec<BoxedComponent>,
    // pub icon: VoxelTextures,
}

impl Debug for ItemDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemDefinition")
            .field("name", &self.name)
            .field("namespace", &self.namespace)
            .field("class", &self.class)
            .field("component_count", &self.components.len())
            .finish()
    }
}

#[derive(Resource, Default, Clone, Debug)]
pub struct ItemRegistry {
    pub defs: Vec<ItemDefinition>,
    pub name_to_id: HashMap<String, ItemId>,
    pub id_to_name: HashMap<ItemId, String>,
}
