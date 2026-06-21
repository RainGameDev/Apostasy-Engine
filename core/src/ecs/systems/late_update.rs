use anyhow::Result;

use crate::EngineMode;
use crate::ecs::world::World;
use crate::ecs::systems::{HasMode, HasPriority};

/// A system that runs at the end of every frame.
#[derive(Clone, Copy)]
pub struct LateUpdateSystem {
    pub name: &'static str,
    pub func: fn(&mut World) -> Result<()>,
    pub priority: u32,
    pub mode: EngineMode,
}
inventory::collect!(LateUpdateSystem);

impl HasPriority for LateUpdateSystem {
    fn priority(&self) -> u32 {
        self.priority
    }
}

impl HasMode for LateUpdateSystem {
    fn mode(&self) -> EngineMode {
        self.mode
    }
}
