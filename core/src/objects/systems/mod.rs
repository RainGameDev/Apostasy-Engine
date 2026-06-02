use anyhow::Result;
use apostasy_macros::Resource;

use crate::{EngineMode, objects::world::World};

pub trait HasPriority {
    fn priority(&self) -> u32;
}

pub trait HasMode {
    fn mode(&self) -> EngineMode;
}

/// A system that happens every frame
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

/// A system that happens once at the start of the application
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

/// A system that happens x amount of times per second
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
    pub fixed_timestep: f32, // 1.0 / 20.0 = 0.05s for 20 tps
    pub last_time: Option<std::time::Instant>,
}

/// A system that happens at the end over every frame
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

/// A system that happens before each frame is rendererd
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
