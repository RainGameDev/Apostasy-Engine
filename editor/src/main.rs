use apostasy_core::{init_core, packages::Packages, rendering::RenderingBackend};

pub mod objects;
pub mod ui;
pub mod systems;

fn main() {
    init_core(RenderingBackend::Vulkan, vec![Packages::ItemSystem]).unwrap();
}
