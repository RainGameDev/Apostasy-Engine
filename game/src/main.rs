use apostasy_core::{
    anyhow::Result,
    cgmath::{Vector3, Zero},
    egui::{CentralPanel, Color32, Frame, Image, Label, RichText, Sense},
    init_core,
    objects::{
        Object, components::transform::Transform, resources::input_manager::InputManager,
        systems::DeltaTime, tags::Player, world::World,
    },
    packages::Packages,
    physics::{
        Gravity,
        collider::{Collider, ColliderShape},
        velocity::Velocity,
    },
    rendering::{
        RenderingBackend,
        components::{
            camera::{ActiveCamera, Camera, GameCamera},
            model_renderer::ModelRenderer,
        },
    },
    start,
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
    winit::keyboard::{KeyCode, PhysicalKey},
};
use apostasy_macros::Resource;

#[derive(Resource, Clone)]
pub struct CoyoteTime(pub f32);

const COYOTE_TIME_WINDOW: f32 = 0.15;
const JUMP_VELOCITY: f32 = 8.0;
const SIDE_SPEED: f32 = 5.0;

fn main() {
    init_core(
        RenderingBackend::Vulkan,
        vec![Packages::Voxel, Packages::ItemSystem],
    )
    .unwrap();
}

#[start]
pub fn start(world: &mut World) -> Result<()> {
    // world.insert_resource(ChunkLoader::default());
    // world.insert_resource(ChunkGenQueue::default());
    // world.insert_resource(LoadingState::default());
    //
    let camera = Object::new()
        .add_component(Camera::default())
        .add_component(Transform {
            local_position: Vector3::new(0.0, 2.0, 20.0),
            ..Default::default()
        })
        .add_tag(ActiveCamera)
        .add_tag(GameCamera);

    let camera = world.add_object(camera);

    let floor = Object::new()
        .add_component(Transform {
            local_scale: Vector3::new(15.0, 1.0, 15.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::static_object())
        .add_component(Collider::new_static(
            ColliderShape::Cuboid {
                size: Vector3::new(1.0, 1.0, 1.0),
            },
            Vector3::zero(),
        ));
    world.add_object(floor);

    let cube = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(4.0, 10.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::default());

    world.add_object(cube);

    let cube = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(-4.0, 15.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::default())
        .add_component(Velocity::default())
        .add_component(Gravity::default())
        .add_component(Collider::default());

    world.add_object(cube);

    let sphere = Object::new()
        .add_component(Transform {
            local_position: Vector3::new(0.0, 8.0, 0.0),
            ..Default::default()
        })
        .add_component(ModelRenderer::from_path("sphere"))
        .add_component(Velocity::default_sphere())
        .add_component(Gravity::default())
        .add_component(Collider::new(
            ColliderShape::Sphere { radius: 1.0 },
            Vector3::zero(),
        ))
        .add_tag(Player);

    let sphere = world.add_object(sphere);
    let _ = world.set_parent(camera, Some(sphere));
    world.insert_resource(CoyoteTime(0.0));

    Ok(())
}

#[update(priority = 25)]
pub fn sphere_player_input(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;
    let inputs = world.get_resource::<InputManager>()?.clone();

    let move_left = inputs.keys_held.contains(&PhysicalKey::Code(KeyCode::KeyA));
    let move_right = inputs.keys_held.contains(&PhysicalKey::Code(KeyCode::KeyD));
    let jump_pressed = inputs
        .keys_pressed
        .contains(&PhysicalKey::Code(KeyCode::Space));

    let is_grounded = {
        let player = world.get_object_with_tag::<Player>()?;
        let velocity = player.get_component::<Velocity>()?;
        velocity.is_grounded
    };

    let coyote_time_value = {
        let coyote_time = world.get_resource_mut::<CoyoteTime>()?;
        if is_grounded {
            coyote_time.0 = COYOTE_TIME_WINDOW;
        } else {
            coyote_time.0 = (coyote_time.0 - delta).max(0.0);
        }
        coyote_time.0
    };

    let should_jump = jump_pressed && coyote_time_value > 0.0;

    {
        let player = world.get_object_with_tag_mut::<Player>()?;
        let velocity = player.get_component_mut::<Velocity>()?;

        if move_left && !move_right {
            velocity.linear_velocity.x = -SIDE_SPEED;
        } else if move_right && !move_left {
            velocity.linear_velocity.x = SIDE_SPEED;
        }

        if should_jump {
            velocity.linear_velocity.y = JUMP_VELOCITY;
        }
    }

    if should_jump {
        let coyote_time = world.get_resource_mut::<CoyoteTime>()?;
        coyote_time.0 = 0.0;
    }

    Ok(())
}

#[update]
pub fn viewport(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let viewport_texture = world.get_resource::<ViewportTexture>().ok().map(|r| r.0);
    let viewport_size = world.get_resource_mut::<ViewportSize>().unwrap();

    let frame = Frame::none();

    CentralPanel::default().frame(frame).show(&ctx, |ui| {
        let available_size = ui.available_size();
        if available_size.x <= 0.0 || available_size.y <= 0.0 {
            return;
        }

        let (frame_rect, _) = ui.allocate_exact_size(available_size, Sense::hover());

        if let Some(texture_id) = viewport_texture {
            let image = Image::new((texture_id, available_size));
            ui.put(frame_rect, image);
        } else {
            let label = Label::new(RichText::new("Viewport initializing...").color(Color32::WHITE));
            ui.put(frame_rect, label);
        }

        viewport_size.logical_width = available_size.x;
        viewport_size.logical_height = available_size.y;

        let pixels_per_point = ctx.pixels_per_point();
        let ss = viewport_size.supersample;
        let pixel_w = (available_size.x * pixels_per_point * ss)
            .ceil()
            .clamp(1.0, 8192.0);
        let pixel_h = (available_size.y * pixels_per_point * ss)
            .ceil()
            .clamp(1.0, 8192.0);

        viewport_size.pixel_width = pixel_w;
        viewport_size.pixel_height = pixel_h;
    });

    Ok(())
}
