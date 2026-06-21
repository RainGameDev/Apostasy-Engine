use anyhow::Result;

use crate::EngineMode;
use crate::ecs::world::World;
use crate::ecs::systems::{HasMode, HasPriority};

/// A system that runs before each frame's render pass.
#[derive(Clone, Copy)]
pub struct PreRenderSystem {
    pub name: &'static str,
    pub func: fn(&mut World) -> Result<()>,
    pub priority: u32,
    pub mode: EngineMode,
}
inventory::collect!(PreRenderSystem);

impl HasPriority for PreRenderSystem {
    fn priority(&self) -> u32 {
        self.priority
    }
}

impl HasMode for PreRenderSystem {
    fn mode(&self) -> EngineMode {
        self.mode
    }
}
