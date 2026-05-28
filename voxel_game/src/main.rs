use apostasy_core::{
    anyhow::Result,
    init_core,
    objects::{
        resources::input_manager::{InputManager, KeyAction, KeyBind, MouseBind},
        world::World,
    },
    packages::Packages,
    rendering::RenderingBackend,
    start,
    voxels::chunk::ChunkGenQueue,
    winit::{
        event::MouseButton,
        keyboard::{KeyCode, PhysicalKey},
    },
};
use apostasy_macros::prerender;

use crate::world::chunk_loader::ChunkLoader;
use crate::world::loading_state::LoadingState;
pub mod entities;
pub mod states;
pub mod ui;
pub mod world;

fn main() {
    init_core(
        RenderingBackend::Vulkan,
        vec![Packages::Voxel, Packages::ItemSystem],
    )
    .unwrap();
}

#[start]
pub fn start(world: &mut World) -> Result<()> {
    world.insert_resource(ChunkLoader::default());
    world.insert_resource(ChunkGenQueue::default());
    world.insert_resource(LoadingState::default());

    Ok(())
}

#[start]
pub fn input_init(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>()?;

    // Movement
    inputs.register_keybind(
        "Left",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyA), KeyAction::Hold),
    )?;
    inputs.register_keybind(
        "Right",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyD), KeyAction::Hold),
    )?;
    inputs.register_keybind(
        "Forwards",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyW), KeyAction::Hold),
    )?;
    inputs.register_keybind(
        "Backwards",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyS), KeyAction::Hold),
    )?;
    inputs.register_keybind(
        "Downwards",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyQ), KeyAction::Hold),
    )?;
    inputs.register_keybind(
        "Jump",
        KeyBind::new(PhysicalKey::Code(KeyCode::Space), KeyAction::Press),
    )?;

    // Structure tools
    inputs.register_keybind(
        "SetStructureStart",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyT), KeyAction::Press),
    )?;
    inputs.register_keybind(
        "SetStructureEnd",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyY), KeyAction::Press),
    )?;
    inputs.register_keybind(
        "SaveStructure",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyH), KeyAction::Press),
    )?;
    inputs.register_keybind(
        "ToggleStructureSelection",
        KeyBind::new(PhysicalKey::Code(KeyCode::F6), KeyAction::Press),
    )?;

    // Debug
    inputs.register_keybind(
        "ReloadShaders",
        KeyBind::new(PhysicalKey::Code(KeyCode::F1), KeyAction::Press),
    )?;

    // Hotbar
    for (name, code) in [
        ("Hotbar1", KeyCode::Digit1),
        ("Hotbar2", KeyCode::Digit2),
        ("Hotbar3", KeyCode::Digit3),
        ("Hotbar4", KeyCode::Digit4),
        ("Hotbar5", KeyCode::Digit5),
        ("Hotbar6", KeyCode::Digit6),
        ("Hotbar7", KeyCode::Digit7),
        ("Hotbar8", KeyCode::Digit8),
        ("Hotbar9", KeyCode::Digit9),
    ] {
        inputs.register_keybind(
            name,
            KeyBind::new(PhysicalKey::Code(code), KeyAction::Press),
        )?;
    }

    // Misc
    inputs.register_keybind(
        "Pause",
        KeyBind::new(PhysicalKey::Code(KeyCode::Escape), KeyAction::Press),
    )?;

    // Mouse
    inputs.register_mousebind("Break", MouseBind::new(MouseButton::Left, KeyAction::Hold))?;
    inputs.register_mousebind("Place", MouseBind::new(MouseButton::Right, KeyAction::Hold))?;

    Ok(())
}

#[prerender]
pub fn voxel_prerender(world: &mut World) -> Result<()> {
    Ok(())
}
