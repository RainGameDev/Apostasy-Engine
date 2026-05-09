use std::fmt::Debug;

use apostasy_macros::Component;
use cgmath::Vector2;

use crate::log;

#[derive(Component, Clone, Debug)]
pub struct Container {
    pub items: Vec<ContainerItem>,
    pub size: Vector2<u32>,
    pub selected_item: u32,
}

impl Default for Container {
    fn default() -> Self {
        Container {
            items: Vec::new(),
            size: Vector2::new(9, 4),
            selected_item: 0,
        }
    }
}

impl Container {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn add_item(&mut self, item: ContainerItem) -> &mut Self {
        if self.items.len() < (self.size.x * self.size.y) as usize {
            self.items.push(item);
        }
        self
    }

    pub fn remove_item_index(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.amount -= 1;
            if item.amount <= 0 {
                self.items.remove(index);
            }
        }
    }
}

#[derive(Clone)]
pub struct ContainerItem {
    pub item: String,
    pub amount: u32,
}

impl Debug for ContainerItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerItem")
            .field("Item", &self.item)
            .field("Amount", &self.amount)
            .finish()
    }
}
