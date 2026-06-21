use anyhow::Result;
use apostasy_macros::Resource;

use crate::EngineMode;
use crate::ecs::world::World;
use crate::ecs::systems::{HasMode, HasPriority};

/// A system that runs at a fixed timestep (20 Hz by default).
#[derive(Clone, Copy)]
pub struct FixedUpdateSystem {
    pub name: &'static str,
    pub func: fn(&mut World, delta: f32) -> Result<()>,
    pub priority: u32,
    pub mode: EngineMode,
}
inventory::collect!(FixedUpdateSystem);

impl HasPriority for FixedUpdateSystem {
    fn priority(&self) -> u32 {
        self.priority
    }
}

impl HasMode for FixedUpdateSystem {
    fn mode(&self) -> EngineMode {
        self.mode
    }
}

#[derive(Resource, Clone, Default)]
pub struct DeltaTime(pub f32);

#[derive(Resource, Clone, Default)]
pub struct EngineTimer(pub f32);

#[derive(Resource, Clone, Default)]
pub struct FixedUpdateTimer {
    pub accumulator: f32,
    /// Target seconds per fixed tick (default: 1/20 = 0.05s).
    pub fixed_timestep: f32,
    pub last_time: Option<std::time::Instant>,
}
