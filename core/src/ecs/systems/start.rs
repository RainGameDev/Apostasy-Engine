use anyhow::Result;

use crate::EngineMode;
use crate::ecs::world::World;
use crate::ecs::systems::{HasMode, HasPriority};

/// A system that runs once at startup.
#[derive(Clone, Copy)]
pub struct StartSystem {
    pub name: &'static str,
    pub func: fn(&mut World) -> Result<()>,
    pub priority: u32,
    pub mode: EngineMode,
}
inventory::collect!(StartSystem);

impl HasPriority for StartSystem {
    fn priority(&self) -> u32 {
        self.priority
    }
}

impl HasMode for StartSystem {
    fn mode(&self) -> EngineMode {
        self.mode
    }
}
