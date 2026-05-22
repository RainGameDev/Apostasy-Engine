use apostasy_core::{init_core, packages::Packages, rendering::RenderingBackend};

pub mod objects;

fn main() {
    init_core(RenderingBackend::Vulkan, vec![]).unwrap();
}
