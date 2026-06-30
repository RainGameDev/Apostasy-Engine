use apostasy_core::{
    anyhow::Result,
    cgmath::{InnerSpace, One, Quaternion, Vector3},
    ecs::{
        components::transform::Transform,
        resources::{
            cursor_manager::{CursorLockMode, CursorManager},
            input_manager::{InputManager, KeyAction, KeyBind},
            window_manager::WindowManager,
        },
        systems::DeltaTime,
        world::World,
    },
    egui::{self, Color32, LayerId, Rect},
    init_core,
    packages::{Packages, project_package::load_startup_scene},
    rendering::RenderingBackend,
    start,
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
    winit::keyboard::{KeyCode, PhysicalKey},
};
use apostasy_macros::{Resource, Tag};

#[derive(Tag, Clone)]
pub struct FlyCamera;

#[derive(Resource, Clone)]
pub struct Paused;

const MOVE_SPEED: f32 = 8.0;
const SPRINT_MULTIPLIER: f32 = 3.0;
const MOUSE_SENSITIVITY: f32 = 0.12;

fn main() {
    init_core(RenderingBackend::Vulkan, vec![Packages::Project]).unwrap();
}


#[start(priority = 50)]
pub fn load_scene(world: &mut World) -> Result<()> {
    load_startup_scene(world, "default", &["FlyCamera"])
}

#[start]
pub fn setup_input(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>()?;

    let holds = [
        ("Forwards", KeyCode::KeyW),
        ("Backwards", KeyCode::KeyS),
        ("Left", KeyCode::KeyA),
        ("Right", KeyCode::KeyD),
        ("Up", KeyCode::Space),
        ("Down", KeyCode::ControlLeft),
        ("Sprint", KeyCode::ShiftLeft),
    ];
    for (name, code) in holds {
        inputs
            .register_default_keybind(name, KeyBind::new(PhysicalKey::Code(code), KeyAction::Hold));
    }

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
    {
        let cursor = world.get_resource_mut::<CursorManager>()?;
        cursor.set_mode(if paused {
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
pub fn fly_camera(world: &mut World) -> Result<()> {
    if world.has_resource::<Paused>() {
        return Ok(());
    }

    let dt = world.get_resource::<DeltaTime>()?.0;
    let inputs = world.get_resource::<InputManager>()?.clone();
    let cam = world.get_entity_with_tag::<FlyCamera>()?;

    let (dx, dy) = inputs.mouse_delta;
    if let Some(t) = world.get_component_mut::<Transform>(cam) {
        t.local_euler_angles.y -= dx as f32 * MOUSE_SENSITIVITY;
        t.local_euler_angles.x -= dy as f32 * MOUSE_SENSITIVITY;
        t.local_euler_angles.x = t.local_euler_angles.x.clamp(-89.0, 89.0);
    }

    let move_input = inputs.input_vector_3d("Right", "Left", "Up", "Down", "Backwards", "Forwards");
    let rotation = world
        .get_component::<Transform>(cam)
        .map(|t| t.global_rotation)
        .unwrap_or_else(Quaternion::one);

    let look_relative = rotation * Vector3::new(move_input.x, 0.0, move_input.z);
    let mut wish = look_relative + Vector3::new(0.0, move_input.y, 0.0);
    if wish.magnitude2() > 0.0 {
        wish = wish.normalize();
    }

    let speed = MOVE_SPEED
        * if inputs.is_keybind_active("Sprint") {
            SPRINT_MULTIPLIER
        } else {
            1.0
        };

    if let Some(t) = world.get_component_mut::<Transform>(cam) {
        t.local_position += wish * speed * dt;
    }

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
