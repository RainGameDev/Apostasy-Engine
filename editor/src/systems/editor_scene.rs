use apostasy_core::{
    anyhow::Result,
    cgmath::{SquareMatrix, Vector3, Zero},
    objects::{
        Object,
        components::transform::Transform,
        resources::input_manager::{InputManager, KeyAction, KeyBind, MouseBind},
        tags::Player,
        world::World,
    },
    physics::{
        Gravity,
        collider::{Collider, ColliderShape},
        raycast::{Ray, build_collider_snapshot, raycast_colliders_raw, unproject},
        velocity::Velocity,
    },
    rendering::components::{
        camera::{ActiveCamera, Camera, EditorCamera, get_perspective_projection, get_view_matrix},
        model_renderer::ModelRenderer,
    },
    start,
    ui::ui_context::ViewportSize,
    update,
    winit::{
        event::MouseButton,
        keyboard::{KeyCode, PhysicalKey},
    },
};

use crate::ui::{cell_panel::CellSearchState, viewport_panel::ViewportInfo};

#[start(mode = "editor")]
pub fn editor_scene_setup(world: &mut World) -> Result<()> {
    let camera = Object::new()
        .set_name("Camera")
        .add_component(Camera::default())
        .add_component(Transform {
            local_position: Vector3::new(0.0, 2.0, 20.0),
            ..Default::default()
        })
        .add_component(Velocity::default())
        .add_tag(ActiveCamera)
        .add_tag(EditorCamera);

    world.add_object(camera);

    let floor = Object::new()
        .set_name("Floor")
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
        .set_name("Cube")
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
        .set_name("Cube")
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
        .set_name("Sphere")
        .add_component(Transform {
            local_position: Vector3::new(1.0, 8.0, 0.0),
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

    world.add_object(sphere);

    let sphere = Object::new()
        .set_name("Sphere")
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

    world.add_object(sphere);

    let inputs = world.get_resource_mut::<InputManager>().unwrap();

    inputs.register_mousebind(
        "MouseClick",
        MouseBind::new(MouseButton::Left, KeyAction::Press),
    )?;
    inputs.register_mousebind(
        "RightMouseClick",
        MouseBind::new(MouseButton::Right, KeyAction::Hold),
    )?;
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

    Ok(())
}

#[update(mode = "editor")]
pub fn editor_raycasting(world: &mut World) -> Result<()> {
    if !world.has_resource::<ViewportInfo>() {
        world.insert_resource(ViewportInfo::default());
    }
    let viewport_info = world.get_resource::<ViewportInfo>()?;

    if !viewport_info.is_hovered {
        return Ok(());
    }

    let inputs = world.get_resource::<InputManager>().unwrap();

    if inputs.is_mousebind_active("MouseClick") {
        let camera_transform = world
            .get_objects_with_component::<Camera>()
            .first()
            .unwrap()
            .get_component::<Transform>()
            .unwrap()
            .clone();

        let camera_view = world
            .get_objects_with_component::<Camera>()
            .first()
            .unwrap()
            .get_component::<Camera>()
            .unwrap()
            .clone();
        let viewport_size = world.get_resource::<ViewportSize>().unwrap();

        let aspect = viewport_size.logical_width / viewport_size.logical_height;
        let perspective = get_perspective_projection(&camera_view, aspect);
        let view = get_view_matrix(&camera_transform);

        let mouse_position = world.get_resource::<InputManager>().unwrap().mouse_position;

        let relative_x = mouse_position.x - viewport_size.logical_x as f64;
        let relative_y = mouse_position.y - viewport_size.logical_y as f64;

        if relative_x >= 0.0
            && relative_x <= viewport_size.logical_width as f64
            && relative_y >= 0.0
            && relative_y <= viewport_size.logical_height as f64
        {
            let ndc_x = (relative_x / viewport_size.logical_width as f64) * 2.0 - 1.0;
            let ndc_y = (relative_y / viewport_size.logical_height as f64) * 2.0 - 1.0;

            let direction = unproject(
                ndc_x as f32,
                ndc_y as f32,
                &(perspective * view).invert().unwrap(),
                camera_transform.global_position,
            );
            let ray = Ray::new(camera_transform.global_position, direction);

            let snapshots = build_collider_snapshot(world);
            let hit = raycast_colliders_raw(&ray, 1000.0, &snapshots, None);

            if let Ok(cell_search_state) = world.get_resource_mut::<CellSearchState>() {
                if let Some(hit) = hit {
                    cell_search_state.selected_obj = Some(hit.object_id);
                } else {
                    cell_search_state.selected_obj = None;
                }
            }
        }
    }
    Ok(())
}
