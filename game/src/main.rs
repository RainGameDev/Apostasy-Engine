use apostasy_core::{
    anyhow::Result,
    ecs::{
        resources::{
            cursor_manager::{CursorLockMode, CursorManager},
            input_manager::{InputManager, KeyAction, KeyBind},
            window_manager::WindowManager,
        },
        world::World,
    },
    egui::{self, Color32, LayerId, Rect},
    init_core,
    packages::{Packages, project_package::load_startup_worldspace},
    rendering::RenderingBackend,
    start,
    ui::Console,
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
    winit::keyboard::{KeyCode, PhysicalKey},
};
use apostasy_macros::Resource;

#[derive(Resource, Clone)]
pub struct Paused;

fn main() {
    init_core(RenderingBackend::Vulkan, vec![Packages::Project, Packages::Terrain]).unwrap();
}

// Player movement, mouse-look, and jumping are driven from Lua (game/res/scripts/main.lua),
// which reads/writes the player entity's Velocity and Transform components directly.
#[start(priority = 50)]
pub fn load_worldspace(world: &mut World) -> Result<()> {
    load_startup_worldspace(world, "default", &["Player"])
}

#[start]
pub fn setup_input(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>()?;

    let holds = [
        ("Forwards", KeyCode::KeyW),
        ("Backwards", KeyCode::KeyS),
        ("Left", KeyCode::KeyA),
        ("Right", KeyCode::KeyD),
        ("Sprint", KeyCode::ShiftLeft),
    ];
    for (name, code) in holds {
        inputs
            .register_default_keybind(name, KeyBind::new(PhysicalKey::Code(code), KeyAction::Hold));
    }

    inputs.register_default_keybind(
        "Jump",
        KeyBind::new(PhysicalKey::Code(KeyCode::Space), KeyAction::Press),
    );
    inputs.register_default_keybind(
        "Pause",
        KeyBind::new(PhysicalKey::Code(KeyCode::Escape), KeyAction::Press),
    );

    Ok(())
}

#[update(priority = 10)]
pub fn cursor_and_pause(world: &mut World) -> Result<()> {
    if world
        .get_resource::<InputManager>()?
        .is_keybind_active("Pause")
    {
        if world.has_resource::<Paused>() {
            world.remove_resource::<Paused>();
        } else {
            world.insert_resource(Paused);
        }
    }

    let paused = world.has_resource::<Paused>();
    let console_open = world.get_resource::<Console>().is_ok_and(|c| c.open);
    {
        let cursor = world.get_resource_mut::<CursorManager>()?;
        cursor.set_mode(if paused || console_open {
            CursorLockMode::NoneVisible
        } else {
            CursorLockMode::ConfinedHidden
        });
    }

    let cursor = world.get_resource::<CursorManager>()?.clone();
    let windows = world.get_resource_mut::<WindowManager>()?;
    cursor.update_cursor(windows);
    Ok(())
}

#[update]
pub fn viewport(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let viewport_texture = world.get_resource::<ViewportTexture>().ok().map(|r| r.0);
    let viewport_size = world.get_resource_mut::<ViewportSize>().unwrap();

    let screen_rect = ctx.input(|i| i.viewport_rect());
    let pixels_per_point = ctx.pixels_per_point();
    let ss = viewport_size.supersample;

    viewport_size.logical_x = screen_rect.min.x;
    viewport_size.logical_y = screen_rect.min.y;
    viewport_size.logical_width = screen_rect.width();
    viewport_size.logical_height = screen_rect.height();
    viewport_size.pixel_width = (screen_rect.width() * pixels_per_point * ss)
        .ceil()
        .clamp(1.0, 8192.0);
    viewport_size.pixel_height = (screen_rect.height() * pixels_per_point * ss)
        .ceil()
        .clamp(1.0, 8192.0);

    if let Some(texture_id) = viewport_texture {
        let painter = ctx.layer_painter(LayerId::background());
        painter.image(
            texture_id,
            screen_rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    Ok(())
}
