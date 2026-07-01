use apostasy_core::parking_lot::RwLock;
use apostasy_core::{
    anyhow::Result,
    assets::{
        asset_manager::AssetManager,
        loaders::{
            biome_loader::BiomeLoader, structure_loader::StructureLoader,
            voxel_loader::VoxelLoader, worldspace_loader::WorldspaceLoader,
        },
    },
    cgmath::{SquareMatrix, Vector3, Zero},
    ecs::{
        components::transform::Transform,
        resources::input_manager::{InputManager, KeyAction, KeyBind, MouseBind},
        tags::Player,
        world::World,
        worldspace_serializer::load_worldspace,
        worldspace_streaming::WorldspaceStreaming,
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
    terrain::{chunk::TerrainChunk, persistence::load_terrain_cells},
    ui::ui_context::ViewportSize,
    update,
    voxels::{
        biome::BiomeRegistry, structure::StructureRegistry, texture_atlas::AtlasBuilder,
        voxel::VoxelRegistry,
    },
    winit::{
        event::MouseButton,
        keyboard::{KeyCode, PhysicalKey},
    },
};
use std::sync::Arc;

use crate::ui::{
    cell_panel::CellSearchState,
    inspector_panel::InspectorPanelState,
    preferences_panel::EditorPreferences,
    viewport_panel::{ViewportContextMenu, ViewportInfo},
};

/// Registers all asset loaders and loads yaml definitions from the game res directory.
#[start(mode = "editor", priority = 1)]
pub fn editor_data_loader_setup(world: &mut World) -> Result<()> {
    if !world.has_resource::<AssetManager>() {
        world.insert_resource(AssetManager::new());
    }

    let biome_registry = Arc::new(RwLock::new(BiomeRegistry::default()));
    let voxel_registry = Arc::new(RwLock::new(VoxelRegistry::default()));
    let structure_registry = Arc::new(RwLock::new(StructureRegistry::default()));
    let atlas_builder = Arc::new(RwLock::new(AtlasBuilder::new(16)));

    {
        let asset_manager = world.get_resource_mut::<AssetManager>().unwrap();
        asset_manager.register_loader(BiomeLoader {
            registry: Arc::clone(&biome_registry),
        });
        asset_manager.register_loader(VoxelLoader {
            registry: Arc::clone(&voxel_registry),
            atlas_builder: Arc::clone(&atlas_builder),
        });
        asset_manager.register_loader(StructureLoader {
            registry: Arc::clone(&structure_registry),
        });

        // Worldspace + Material loaders and the project data are registered/loaded
        // by Packages::Project. Load the project dir again here so the voxel/biome/
        // structure loaders registered above pick up their definitions.
        let project = apostasy_core::project_dir();
        if project.is_dir() {
            let _ = asset_manager.load_directory(&project);
        }
    }

    Ok(())
}

#[start(mode = "editor")]
pub fn editor_scene_setup(world: &mut World) -> Result<()> {
    world
        .spawn()
        .with_name("Camera")
        .add_component(Camera::default())
        .add_component(Transform {
            local_position: Vector3::new(0.0, 2.0, 20.0),
            ..Default::default()
        })
        .add_component(Velocity::default())
        .add_tag::<ActiveCamera>()
        .add_tag::<EditorCamera>();

    // Seed cell streaming with the saved render distance so far cells unload/reload
    // around the camera. load_worldspace preserves this setting when it runs.
    let prefs = EditorPreferences::load();
    let mut streaming = WorldspaceStreaming::default();
    streaming.set_render_distance(prefs.render_distance);
    world.insert_resource(streaming);

    // Try loading: last opened scene → "default" scene → hard-coded test entities.
    let last_scene_name = prefs.last_scene;
    let startup_scene: Option<(String, serde_yaml::Value)> = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .and_then(|l| {
            let reg = l.registry.read();
            if !last_scene_name.is_empty() {
                if let Some(v) = reg.worldspaces.get(last_scene_name.as_str()).cloned() {
                    return Some((last_scene_name.clone(), v));
                }
            }
            reg.worldspaces
                .get("default")
                .map(|v| ("default".to_string(), v.clone()))
        });

    let terrain_dir = apostasy_core::project_dir().join("terrain");

    if let Some((scene_name, scene_value)) = startup_scene {
        load_worldspace(world, &scene_value, &["EditorCamera"])?;
        let _ = load_terrain_cells(world, &terrain_dir);
        EditorPreferences::save_last_scene(&scene_name);
    } else {
        let floor_id = world.spawn().id();
        world.set_name(floor_id, "Floor");
        world.add_component(
            floor_id,
            Transform {
                local_scale: Vector3::new(15.0, 1.0, 15.0),
                ..Default::default()
            },
        );
        world.add_component(floor_id, ModelRenderer::default());
        world.add_component(floor_id, Velocity::static_entity());
        world.add_component(
            floor_id,
            Collider::new_static(
                ColliderShape::Cuboid {
                    size: Vector3::new(1.0, 1.0, 1.0),
                },
                Vector3::zero(),
            ),
        );

        let cube_id = world.spawn().id();
        world.set_name(cube_id, "Cube");
        world.add_component(
            cube_id,
            Transform {
                local_position: Vector3::new(4.0, 10.0, 0.0),
                ..Default::default()
            },
        );
        world.add_component(cube_id, ModelRenderer::default());
        world.add_component(cube_id, Velocity::default());
        world.add_component(cube_id, Gravity::default());
        world.add_component(cube_id, Collider::default());

        let cube2_id = world.spawn().id();
        world.set_name(cube2_id, "Cube");
        world.add_component(
            cube2_id,
            Transform {
                local_position: Vector3::new(-4.0, 15.0, 0.0),
                ..Default::default()
            },
        );
        world.add_component(cube2_id, ModelRenderer::default());
        world.add_component(cube2_id, Velocity::default());
        world.add_component(cube2_id, Gravity::default());
        world.add_component(cube2_id, Collider::default());

        let sphere_id = world.spawn().id();
        world.set_name(sphere_id, "Sphere");
        world.add_component(
            sphere_id,
            Transform {
                local_position: Vector3::new(1.0, 8.0, 0.0),
                ..Default::default()
            },
        );
        world.add_component(sphere_id, ModelRenderer::from_path("m_default_sphere"));
        world.add_component(sphere_id, Velocity::default_sphere());
        world.add_component(sphere_id, Gravity::default());
        world.add_component(
            sphere_id,
            Collider::new(ColliderShape::Sphere { radius: 1.0 }, Vector3::zero()),
        );
        world.add_tag::<Player>(sphere_id);

        let sphere2_id = world.spawn().id();
        world.set_name(sphere2_id, "Sphere");
        world.add_component(
            sphere2_id,
            Transform {
                local_position: Vector3::new(0.0, 8.0, 0.0),
                ..Default::default()
            },
        );
        world.add_component(sphere2_id, ModelRenderer::from_path("m_default_sphere"));
        world.add_component(sphere2_id, Velocity::default_sphere());
        world.add_component(sphere2_id, Gravity::default());
        world.add_component(
            sphere2_id,
            Collider::new(ColliderShape::Sphere { radius: 1.0 }, Vector3::zero()),
        );
        world.add_tag::<Player>(sphere2_id);

        let _ = load_terrain_cells(world, &terrain_dir);
    }

    let inputs = world.get_resource_mut::<InputManager>().unwrap();

    inputs.register_default_mousebind(
        "MouseClick",
        MouseBind::new(MouseButton::Left, KeyAction::Press),
    );
    inputs.register_default_mousebind(
        "MiddleMouseClick",
        MouseBind::new(MouseButton::Middle, KeyAction::Hold),
    );

    inputs.register_default_mousebind(
        "RightMouseClick",
        MouseBind::new(MouseButton::Right, KeyAction::Hold),
    );
    inputs.register_default_mousebind(
        "RightMousePress",
        MouseBind::new(MouseButton::Right, KeyAction::Press),
    );
    inputs.register_default_keybind(
        "Left",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyA), KeyAction::Hold),
    );
    inputs.register_default_keybind(
        "Right",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyD), KeyAction::Hold),
    );
    inputs.register_default_keybind(
        "Forwards",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyW), KeyAction::Hold),
    );
    inputs.register_default_keybind(
        "Backwards",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyS), KeyAction::Hold),
    );
    inputs.register_default_keybind(
        "Downwards",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyQ), KeyAction::Hold),
    );
    inputs.register_default_keybind(
        "Jump",
        KeyBind::new(PhysicalKey::Code(KeyCode::Space), KeyAction::Press),
    );

    inputs.register_default_keybind(
        "FocusEntity",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyC), KeyAction::Press),
    );

    inputs.register_default_keybind(
        "TerrainMode",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyH), KeyAction::Press),
    );

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
    let left_click = inputs.is_mousebind_active("MouseClick");
    let right_click = inputs.is_mousebind_active("RightMousePress");

    if left_click {
        let cam_ids = world.get_entities_with_component::<Camera>();
        let first_cam = cam_ids.first().copied();
        let camera_transform = first_cam
            .and_then(|id| world.get_component::<Transform>(id).cloned())
            .unwrap();
        let camera_view = first_cam
            .and_then(|id| world.get_component::<Camera>(id).cloned())
            .unwrap();
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

            let gizmo_consuming = world
                .get_resource::<crate::ui::gizmo::GizmoState>()
                .map(|g| g.consuming)
                .unwrap_or(false);

            if !gizmo_consuming {
                if let Some(ref hit) = hit {
                    if world.has_component::<TerrainChunk>(hit.entity_id) {
                        return Ok(());
                    }
                }
                if let Ok(cell_search_state) = world.get_resource_mut::<CellSearchState>() {
                    if let Some(hit) = hit {
                        cell_search_state.selected_entity = Some(hit.entity_id);
                        if let Ok(inspector_state) = world.get_resource_mut::<InspectorPanelState>()
                        {
                            inspector_state.visible = true;
                        }
                    } else {
                        cell_search_state.selected_entity = None;
                    }
                }
            }
        }
    }

    if right_click {
        if !world.has_resource::<ViewportContextMenu>() {
            world.insert_resource(ViewportContextMenu::default());
        }

        let cam_ids2 = world.get_entities_with_component::<Camera>();
        let first_cam2 = cam_ids2.first().copied();
        let camera_transform = first_cam2
            .and_then(|id| world.get_component::<Transform>(id).cloned())
            .unwrap();
        let camera_view = first_cam2
            .and_then(|id| world.get_component::<Camera>(id).cloned())
            .unwrap();

        let (vp_x, vp_y, vp_w, vp_h) = {
            let vp = world.get_resource::<ViewportSize>().unwrap();
            (
                vp.logical_x,
                vp.logical_y,
                vp.logical_width,
                vp.logical_height,
            )
        };
        let mouse_position = world.get_resource::<InputManager>().unwrap().mouse_position;

        let relative_x = mouse_position.x - vp_x as f64;
        let relative_y = mouse_position.y - vp_y as f64;

        let hit_id = if relative_x >= 0.0
            && relative_x <= vp_w as f64
            && relative_y >= 0.0
            && relative_y <= vp_h as f64
        {
            let aspect = vp_w / vp_h;
            let perspective = get_perspective_projection(&camera_view, aspect);
            let view = get_view_matrix(&camera_transform);

            let ndc_x = (relative_x / vp_w as f64) * 2.0 - 1.0;
            let ndc_y = (relative_y / vp_h as f64) * 2.0 - 1.0;

            let direction = unproject(
                ndc_x as f32,
                ndc_y as f32,
                &(perspective * view).invert().unwrap(),
                camera_transform.global_position,
            );
            let ray = Ray::new(camera_transform.global_position, direction);

            let snapshots = build_collider_snapshot(world);
            raycast_colliders_raw(&ray, 1000.0, &snapshots, None).map(|h| h.entity_id)
        } else {
            None
        };

        if let Ok(ctx_menu) = world.get_resource_mut::<ViewportContextMenu>() {
            ctx_menu.hit_entity = hit_id;
        }
    }

    Ok(())
}
