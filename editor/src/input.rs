use apostasy_core::{
    anyhow::Result,
    objects::{
        resources::input_manager::{InputManager, KeyAction, KeyBind, MouseBind},
        world::World,
    },
    start,
    winit::{
        event::MouseButton,
        keyboard::{KeyCode, PhysicalKey},
    },
};

#[start]
pub fn start(world: &mut World) -> Result<()> {
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
        PhysicalKey::Code(KeyCode::KeyE),
        KeyAction::Hold,
        "Upwards",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::KeyQ),
        KeyAction::Hold,
        "Downwards",
    ));

    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::ArrowLeft),
        KeyAction::Hold,
        "LookLeft",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::ArrowRight),
        KeyAction::Hold,
        "LookRight",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::ArrowUp),
        KeyAction::Hold,
        "LookUp",
    ));
    inputs.register_keybind(KeyBind::new(
        PhysicalKey::Code(KeyCode::ArrowDown),
        KeyAction::Hold,
        "LookDown",
    ));

    inputs.register_mousebind(MouseBind::new(MouseButton::Left, KeyAction::Hold, "Break"));
    inputs.register_mousebind(MouseBind::new(MouseButton::Right, KeyAction::Hold, "Place"));

    Ok(())
}
