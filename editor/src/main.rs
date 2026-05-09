use apostasy_core::{init_core, packages::Packages, rendering::RenderingBackend};

pub mod editor_camera;
pub mod input;

fn main() {
    init_core(
        RenderingBackend::Vulkan,
        vec![Packages::Voxel, Packages::ItemSystem],
    )
    .unwrap();
}
