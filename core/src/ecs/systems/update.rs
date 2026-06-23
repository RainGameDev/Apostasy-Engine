use anyhow::Result;

use crate::EngineMode;
use crate::ecs::world::World;
use crate::ecs::systems::{HasMode, HasPriority};

/// A system that runs every frame.
#[derive(Clone, Copy)]
pub struct UpdateSystem {
    pub name: &'static str,
    pub func: fn(&mut World) -> Result<()>,
    pub priority: u32,
    pub mode: EngineMode,
}
inventory::collect!(UpdateSystem);

impl HasPriority for UpdateSystem {
    fn priority(&self) -> u32 {
        self.priority
    }
}

impl HasMode for UpdateSystem {
    fn mode(&self) -> EngineMode {
        self.mode
    }
}
