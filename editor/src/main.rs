use apostasy_core::{init_core_with_mode, packages::Packages, rendering::RenderingBackend, EngineMode};

pub mod ecs;
pub mod terrain;
pub mod ui;
pub mod systems;

fn main() {
    init_core_with_mode(
        RenderingBackend::Vulkan,
        vec![Packages::ItemSystem, Packages::Terrain],
        EngineMode::Editor,
    )
    .unwrap();
}
