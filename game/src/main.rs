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

    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyA),
        KeyAction::Hold,
        "Left",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyD),
        KeyAction::Hold,
        "Right",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyW),
        KeyAction::Hold,
        "Forwards",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyS),
        KeyAction::Hold,
        "Backwards",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Space),
        KeyAction::Press,
        "Jump",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyQ),
        KeyAction::Hold,
        "Downwards",
    ));

    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyY),
        KeyAction::Press,
        "SetStructureEnd",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyT),
        KeyAction::Press,
        "SetStructureStart",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyH),
        KeyAction::Press,
        "SaveStructure",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::F6),
        KeyAction::Press,
        "ToggleStructureSelection",
    ));

    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::F1),
        KeyAction::Press,
        "ReloadShaders",
    ));

    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit1),
        KeyAction::Press,
        "Hotbar1",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit2),
        KeyAction::Press,
        "Hotbar2",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit3),
        KeyAction::Press,
        "Hotbar3",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit4),
        KeyAction::Press,
        "Hotbar4",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit5),
        KeyAction::Press,
        "Hotbar5",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit6),
        KeyAction::Press,
        "Hotbar6",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit7),
        KeyAction::Press,
        "Hotbar7",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit8),
        KeyAction::Press,
        "Hotbar8",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Digit9),
        KeyAction::Press,
        "Hotbar9",
    ));

    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::Escape),
        KeyAction::Press,
        "Pause",
    ));

    inputs.register_mousebind(MouseBind::new(MouseButton::Left, KeyAction::Hold, "Break"));
    inputs.register_mousebind(MouseBind::new(MouseButton::Right, KeyAction::Hold, "Place"));

    Ok(())
}
