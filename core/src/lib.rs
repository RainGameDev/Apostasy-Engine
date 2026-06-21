extern crate self as apostasy_core;
pub use apostasy_macros::Component;
use apostasy_macros::Resource;
pub use apostasy_macros::fixed_update;
pub use apostasy_macros::late_update;
pub use apostasy_macros::start;
pub use apostasy_macros::update;

use winit::event::DeviceEvent;
use winit::event::DeviceId;
use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

use std::path::Path;
use std::sync::RwLock;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

use crate::assets::asset_manager::AssetManager;
use crate::assets::gltf::ModelLoader;
use crate::assets::gltf::ModelRegistry;
use crate::ecs::component::InspectorRegistry;
use crate::ecs::components::transform::Transform;
use crate::ecs::resources::cursor_manager::CursorManager;
use crate::ecs::resources::input_manager::InputManager;
use crate::ecs::resources::input_manager::KeyAction;
use crate::ecs::resources::input_manager::KeyBind;
use crate::ecs::resources::window_manager::WindowManager;
use crate::ecs::systems::EngineTimer;
use crate::packages::Packages;
use crate::packages::add_package;
use crate::rendering::WindowInfo;
use crate::rendering::components::camera::ActiveCamera;
use crate::rendering::components::camera::Camera;
use crate::rendering::components::camera::EditorCamera;
use crate::rendering::components::camera::get_perspective_projection;
use crate::rendering::components::camera::get_view_matrix;
use crate::rendering::components::lighting::{Light, LightType};
use crate::rendering::components::model_renderer::ModelRenderer;
use crate::rendering::lighting::gpu_light::{
    GpuLight, PointShadowData, ShadowData, compute_csm_matrices, compute_light_space_matrix,
    compute_point_shadow_matrices,
};
use crate::rendering::shared::UpdateRenderer;
use crate::rendering::shared::anti_alisaing::AntiAliasing;
use crate::rendering::shared::frustrum::Frustum;
use crate::rendering::shared::frustrum::ObjectsDrawing;
use crate::rendering::shared::material::GpuMaterial;
use crate::rendering::shared::push_constants::ModelPushConstants;
use crate::rendering::shared::push_constants::{
    PushConstants, ShadowModelPushConstants, ShadowPointModelPushConstants,
    ShadowPointVoxelPushConstants, ShadowVoxelPushConstants, VoxelPushConstants,
};
use crate::rendering::shared::shadow_settings::ShadowDistance;
use crate::states::ShouldExit;
use crate::terrain::chunk::{NeedsTerrainRebuild, TerrainChunk, TerrainMesh};
use crate::terrain::rebuild::rebuild_dirty_terrain;
use crate::terrain::texture_atlas::TerrainTextureAtlas;
use crate::terrain::{TerrainAtlasNeedsRebuild, TerrainSettings};
use crate::ui::FontRegistry;
use crate::ui::ui_context::{EguiContext, ViewportSize, ViewportTexture};
use crate::utils::profiler::{FrameSample, Profiler, SystemTiming};
use crate::voxels::VoxelTransform;
use crate::voxels::meshes::NeedsRemeshing;
use crate::voxels::meshes::VoxelChunkMesh;
use crate::voxels::meshes::WaterMesh;
use crate::voxels::meshes::{dispatch_remesh_jobs, receive_meshes};
use crate::voxels::texture_atlas::PendingAtlas;
use crate::voxels::texture_atlas::VoxelTextureAtlas;
use crate::voxels::texture_atlas::upload_atlas;
use crate::{
    ecs::world::World,
    rendering::{RenderingBackend, RenderingInfo},
};
use winit::application::ApplicationHandler;

pub mod assets;
pub mod items;
pub mod ecs;
pub mod packages;
pub mod physics;
pub mod rendering;
pub mod scripting;
pub mod states;
pub mod terrain;
pub mod ui;
pub mod utils;
pub mod voxels;
pub mod worldspaces;

#[derive(Clone, Resource, Default)]
pub struct ReloadShadersRequest(pub bool);

pub use anyhow;
pub use cgmath;
use cgmath::{InnerSpace, Vector3};
pub use crossbeam_channel;
pub use egui;
pub use egui_extras;
pub use epaint;
pub use lru;
pub use noise;
pub use num_cpus;
pub use rand;
pub use rayon;
pub use serde;
pub use serde_yaml;
pub use slotmap;
pub use winit;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineMode {
    All,
    Game,
    Editor,
}

impl Default for EngineMode {
    fn default() -> Self {
        Self::Game
    }
}

impl EngineMode {
    pub fn matches(&self, mode: EngineMode) -> bool {
        *self == EngineMode::All || *self == mode
    }
}

pub struct Core {
    pub rendering_api: RenderingBackend,
    pub rendering_info: Option<Arc<Mutex<RenderingInfo>>>,
    pub world: Arc<Mutex<World>>,
    pub asset_loader: AssetManager,
    pub packages: Vec<Packages>,
}

impl Core {
    pub fn new(rendering_api: RenderingBackend, packages: Vec<Packages>) -> Self {
        Self::new_with_mode(rendering_api, packages, EngineMode::Game)
    }

    pub fn new_with_mode(
        rendering_api: RenderingBackend,
        packages: Vec<Packages>,
        engine_mode: EngineMode,
    ) -> Self {
        let mut world = World::default();
        world.insert_resource(engine_mode);
        {
            let mut input_manager = InputManager::default();
            let keybinds_path = format!("{}/res/.editor/keybinds.yaml", env!("CARGO_MANIFEST_DIR"));
            input_manager.load_or_init_keybinds(keybinds_path);
            world.insert_resource(input_manager);
        }
        world.insert_resource(CursorManager::default());
        world.insert_resource(WindowManager::default());
        world.insert_resource(AntiAliasing::default());
        world.insert_resource(ShadowDistance::default());
        world.insert_resource(InspectorRegistry::build());
        world.insert_resource(WindowInfo::default());

        world.insert_resource(PushConstants::default());
        world.insert_resource(ModelPushConstants::default());
        world.insert_resource(ObjectsDrawing(0));
        world.insert_resource(EngineTimer(0.0));
        world.insert_resource(Profiler::default());

        for package in packages.clone() {
            add_package(&mut world, package);
        }

        world.build_systems();
        Self {
            rendering_api,
            rendering_info: None,
            world: Arc::new(Mutex::new(world)),
            asset_loader: AssetManager::new(),
            packages,
        }
    }

    pub fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(rendering_info) = &mut self.rendering_info {
            let mut rendering_info = rendering_info.lock().unwrap();

            if let Some(renderer) = &mut rendering_info.renderer {
                let _ = renderer.handle_ui_event(&event.clone());
            }

            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::Resized(_) => {
                    if let Some(renderer) = &mut rendering_info.renderer
                        && let Err(e) = renderer.resize()
                    {
                        log_error!("Failed to resize renderer: {}", e);
                    }
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(renderer) = &mut rendering_info.renderer
                        && let Err(e) = renderer.resize()
                    {
                        log_error!("Failed to resize renderer: {}", e);
                    }
                }
                WindowEvent::RedrawRequested => {
                    let frame_start = std::time::Instant::now();
                    let mut objects_dawn = 0;
                    let mut world = self.world.lock().unwrap();
                    let asset_manager = world.get_resource::<AssetManager>().unwrap().clone();

                    let model_registry = asset_manager.model_loader.registry.read().unwrap();

                    world.get_resource_mut::<WindowInfo>().unwrap().window_size =
                        rendering_info.window.outer_size().into();

                    let aa_amount = world.get_resource::<AntiAliasing>().unwrap().amount;

                    if world.get_resource::<ShouldExit>().is_ok() {
                        log!("Recieved ShouldExit resource, closing");
                        event_loop.exit();
                    }

                    let context = Arc::new(rendering_info.context.clone());

                    // Clone out the push constants immediately so the mutable borrows are dropped
                    let mut push_constants =
                        world.get_resource_mut::<PushConstants>().unwrap().clone();
                    let model_push_constants = world
                        .get_resource_mut::<ModelPushConstants>()
                        .unwrap()
                        .clone();

                    let Some(renderer) = &mut rendering_info.renderer else {
                        log_error!("No renderer found!");
                        return;
                    };

                    // If an EditorCamera exists, only render from an object that has
                    // both ActiveCamera + EditorCamera. If no EditorCamera is in the
                    // world (game / standalone mode), fall back to any ActiveCamera.
                    let has_editor_cam = !world.get_entities_with_tag::<EditorCamera>().is_empty();
                    let active_ids = world.get_entities_with_tag::<ActiveCamera>();
                    let camera_id = if has_editor_cam {
                        active_ids
                            .iter()
                            .find(|&&id| world.has_tag::<EditorCamera>(id))
                            .copied()
                    } else {
                        active_ids.first().copied()
                    };
                    let Some(camera_id) = camera_id else {
                        // No camera yet. Still run a minimal frame so the window is presented
                        // (required on Wayland — a window that never calls present stays invisible).
                        world.prerender();
                        if renderer.begin_frame().is_ok() {
                            renderer.begin_ui();
                            world.update();
                            world.fixed_update();
                            let _ = renderer.end_ui();
                            let _ = renderer.end_frame();
                            world.late_update();
                        }
                        return;
                    };
                    let Some(camera_transform) = world.get_component::<Transform>(camera_id).cloned() else {
                        return;
                    };
                    let Some(camera_comp) = world.get_component::<Camera>(camera_id).cloned() else {
                        return;
                    };
                    let camera_pos = camera_transform.global_position;
                    let camera_near = camera_comp.near;
                    let camera_far = camera_comp.far;
                    let view = get_view_matrix(&camera_transform);

                    let aspect = if let Ok(viewport_size) = world.get_resource::<ViewportSize>() {
                        viewport_size.aspect_logical()
                    } else {
                        renderer.get_aspect()
                    };
                    let proj = get_perspective_projection(&camera_comp, aspect);

                    let view_proj = proj * view;

                    push_constants.set_camera_constants(&camera_transform, &camera_comp, aspect);

                    if !world
                        .get_entities_with_tag::<NeedsRemeshing>()
                        .is_empty()
                    {
                        dispatch_remesh_jobs(&mut world).expect("Failed to dispatch remesh jobs");

                        if let Ok(command_pool) = renderer.get_command_pool() {
                            receive_meshes(
                                &mut world,
                                &context,
                                command_pool,
                                renderer.get_buffer_graveyard(),
                            )
                            .expect("Failed to receive meshes");
                        }
                    }

                    if !world
                        .get_entities_with_tag::<NeedsTerrainRebuild>()
                        .is_empty()
                    {
                        if let Ok(command_pool) = renderer.get_command_pool() {
                            rebuild_dirty_terrain(
                                &mut world,
                                &context,
                                command_pool,
                                renderer.get_buffer_graveyard(),
                            )
                            .expect("Failed to rebuild terrain meshes");
                        }
                    }

                    if world.has_resource::<UpdateRenderer>() {
                        world.remove_resource::<UpdateRenderer>();

                        renderer.reload_shaders().unwrap();

                        if let Ok(viewport_size) = world.get_resource::<ViewportSize>() {
                            let w = viewport_size.pixel_width as u32;
                            let h = viewport_size.pixel_height as u32;
                            renderer.resize_viewport(w, h, aa_amount).unwrap();
                            world.insert_resource(ViewportTexture(
                                renderer.get_viewport_texture_id().unwrap(),
                            ));
                        }
                    }

                    // Collect active lights and upload to GPU, preserving order for shadow indexing.
                    let light_ids = world.get_entities_with_component::<Light>();
                    let emitting_lights: Vec<_> = light_ids
                        .iter()
                        .filter_map(|&id| {
                            let light = world.get_component::<Light>(id)?;
                            let transform = world.get_component::<Transform>(id)?;
                            if light.is_emitting {
                                Some((light.clone(), transform.clone()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    let gpu_lights: Vec<GpuLight> = emitting_lights
                        .iter()
                        .map(|(l, t)| GpuLight::from_component(l, t))
                        .collect();

                    let (
                        shadow_dist,
                        csm_cascade_count,
                        shadow_bias_constant,
                        shadow_bias_slope,
                        shadow_map_size,
                    ) = world
                        .get_resource::<ShadowDistance>()
                        .map(|r| {
                            (
                                r.distance,
                                r.cascade_count,
                                r.bias_constant,
                                r.bias_slope,
                                r.shadow_map_size,
                            )
                        })
                        .unwrap_or((128.0, 4, 2.0, 2.0, 2048));

                    if let Err(e) = renderer.rebuild_shadow_map(shadow_map_size) {
                        log_error!("Failed to rebuild shadow map: {}", e);
                    }
                    if let Err(e) = renderer.rebuild_point_shadow_map(shadow_map_size) {
                        log_error!("Failed to rebuild point shadow map: {}", e);
                    }

                    let camera_forward = camera_transform.calculate_global_forward();

                    enum ShadowCastData {
                        Directional {
                            matrices: [cgmath::Matrix4<f32>; 4],
                            splits: [f32; 4],
                            light_index: u32,
                        },
                        Spot {
                            matrix: [[f32; 4]; 4],
                            light_index: u32,
                        },
                    }

                    // Find first directional or spot light for shadow casting.
                    let shadow_result: Option<ShadowCastData> = emitting_lights
                        .iter()
                        .enumerate()
                        .find_map(|(idx, (light, transform))| match light.light_type {
                            LightType::Directional => {
                                let (matrices, splits) = compute_csm_matrices(
                                    light,
                                    transform,
                                    camera_pos,
                                    camera_comp.fov_y,
                                    aspect,
                                    camera_near,
                                    camera_far,
                                    shadow_dist,
                                    csm_cascade_count,
                                    shadow_map_size,
                                );
                                Some(ShadowCastData::Directional {
                                    matrices,
                                    splits,
                                    light_index: idx as u32,
                                })
                            }
                            LightType::Spot { .. } => {
                                let m = compute_light_space_matrix(
                                    light,
                                    transform,
                                    camera_pos,
                                    shadow_dist,
                                );
                                Some(ShadowCastData::Spot {
                                    matrix: *m.as_ref(),
                                    light_index: idx as u32,
                                })
                            }
                            _ => None,
                        });

                    let shadow_data = shadow_result.as_ref().map(|sr| match sr {
                        ShadowCastData::Directional {
                            matrices,
                            splits,
                            light_index,
                        } => ShadowData {
                            matrices: matrices.iter().map(|m| *m.as_ref()).collect(),
                            splits: *splits,
                            cascade_count: csm_cascade_count as u32,
                            shadow_light_index: *light_index,
                        },
                        ShadowCastData::Spot {
                            matrix,
                            light_index,
                        } => ShadowData {
                            matrices: vec![*matrix; 4],
                            splits: [shadow_dist; 4],
                            cascade_count: 1,
                            shadow_light_index: *light_index,
                        },
                    });


                    // Find first point light for omnidirectional shadow casting.
                    let point_shadow_data: Option<PointShadowData> = emitting_lights
                        .iter()
                        .enumerate()
                        .find_map(|(idx, (light, transform))| {
                            if let LightType::Point { radius } = light.light_type {
                                let pos = transform.global_position;
                                let face_mats = compute_point_shadow_matrices(pos, radius);
                                Some(PointShadowData {
                                    light_index: idx as u32,
                                    far: radius,
                                    face_matrices: face_mats.map(|m| *m.as_ref()),
                                })
                            } else {
                                None
                            }
                        });

                    let prerender_start = std::time::Instant::now();
                    let prerender_timings = world.prerender();
                    let prerender_ns = prerender_start.elapsed().as_nanos() as u64;

                    let mut render_other_timings: Vec<(&'static str, u64)> = Vec::new();
                    let render_start = std::time::Instant::now();
                    let render_step = std::time::Instant::now();
                    if let Err(e) = renderer.begin_frame() {
                        log_error!("Failed to begin frame: {}", e);
                        return;
                    }
                    let begin_frame_ns = render_step.elapsed().as_nanos() as u64;
                    let fence_wait_ns = renderer.last_fence_wait_ns().min(begin_frame_ns);
                    render_other_timings.push(("fence_wait", fence_wait_ns));
                    render_other_timings.push(("begin_frame", begin_frame_ns - fence_wait_ns));

                    let render_step = std::time::Instant::now();
                    renderer.set_lights(
                        &gpu_lights,
                        shadow_data,
                        point_shadow_data.as_ref().map(|d| PointShadowData {
                            light_index: d.light_index,
                            far: d.far,
                            face_matrices: d.face_matrices,
                        }),
                        shadow_dist,
                        [camera_pos.x, camera_pos.y, camera_pos.z],
                        [camera_forward.x, camera_forward.y, camera_forward.z],
                    );
                    render_other_timings.push(("set_lights", render_step.elapsed().as_nanos() as u64));

                    let shadow_start = std::time::Instant::now();
                    // Shadow pre-pass - one pass per cascade (csm_cascade_count for directional, 1 for spot).
                    let cascade_count = match &shadow_result {
                        Some(ShadowCastData::Directional { .. }) => csm_cascade_count,
                        Some(ShadowCastData::Spot { .. }) => 1,
                        None => 0,
                    };

                    // Pre-collect model IDs once to avoid re-borrowing per cascade.
                    let shadow_model_ids: Vec<_> = if cascade_count > 0 {
                        world.get_entities_with_component::<ModelRenderer>()
                    } else {
                        vec![]
                    };

                    for cascade_idx in 0..cascade_count {
                        let cascade_matrix: [[f32; 4]; 4] = match &shadow_result {
                            Some(ShadowCastData::Directional { matrices, .. }) => {
                                *matrices[cascade_idx].as_ref()
                            }
                            Some(ShadowCastData::Spot { matrix, .. }) => *matrix,
                            None => unreachable!(),
                        };

                        if let Err(e) = renderer.begin_shadow_pass(
                            cascade_idx,
                            shadow_bias_constant,
                            shadow_bias_slope,
                        ) {
                            log_error!("Failed to begin shadow pass {}: {}", cascade_idx, e);
                            return;
                        }

                        // Shadow models
                        for &id in &shadow_model_ids {
                            let Some(model_renderer) = world.get_component::<ModelRenderer>(id) else { continue };
                            let Some(model) = model_renderer.model.as_ref() else { continue };
                            let model = model.clone();
                            let Some(transform) = world.get_component::<Transform>(id) else { continue };
                            let pc = ShadowModelPushConstants::new(
                                cascade_matrix,
                                transform.global_position.into(),
                                transform.global_scale.into(),
                                [
                                    transform.global_rotation.v.x,
                                    transform.global_rotation.v.y,
                                    transform.global_rotation.v.z,
                                    transform.global_rotation.s,
                                ],
                            );
                            for mesh in &model.meshes {
                                if let Err(e) = renderer
                                    .shadow_model_render(Box::new(mesh.clone()), &pc)
                                {
                                    log_error!("Failed shadow model render: {}", e);
                                }
                            }
                        }

                        // Shadow terrain
                        if self.packages.contains(&Packages::Terrain) {
                            let terrain_ids = world.get_entities_with_component::<TerrainMesh>();
                            for id in terrain_ids {
                                let Some(terrain_mesh) = world.get_component::<TerrainMesh>(id) else { continue };
                                if terrain_mesh.index_count == 0 {
                                    continue;
                                }
                                let terrain_mesh = terrain_mesh.clone();
                                let pc = ShadowModelPushConstants::new(
                                    cascade_matrix,
                                    [0.0, 0.0, 0.0],
                                    [1.0, 1.0, 1.0],
                                    [0.0, 0.0, 0.0, 1.0],
                                );
                                if let Err(e) = renderer
                                    .shadow_model_render(Box::new(terrain_mesh), &pc)
                                {
                                    log_error!("Failed shadow terrain render: {}", e);
                                }
                            }
                        }

                        // Shadow voxels
                        if self.packages.contains(&Packages::Voxel) {
                            let voxel_ids = world.get_entities_with_component::<VoxelChunkMesh>();
                            for id in voxel_ids {
                                let Some(transform) = world.get_component::<VoxelTransform>(id) else { continue };
                                let Some(voxel_mesh) = world.get_component::<VoxelChunkMesh>(id) else { continue };
                                let pc = ShadowVoxelPushConstants::new(
                                    cascade_matrix,
                                    [
                                        transform.position.x * 32,
                                        transform.position.y * 32,
                                        transform.position.z * 32,
                                    ],
                                );
                                let voxel_mesh = voxel_mesh.clone();
                                if let Err(e) =
                                    renderer.shadow_voxel_render(Box::new(voxel_mesh), &pc)
                                {
                                    log_error!("Failed shadow voxel render: {}", e);
                                }
                            }
                        }

                        if let Err(e) = renderer.end_shadow_pass(cascade_idx) {
                            log_error!("Failed to end shadow pass {}: {}", cascade_idx, e);
                            return;
                        }
                    }

                    // Point light shadow pre-pass - 6 faces for the cube shadow map.
                    if let Some(ref ps) = point_shadow_data {
                        let point_model_ids = world.get_entities_with_component::<ModelRenderer>();

                        for face in 0..6usize {
                            let face_matrix = ps.face_matrices[face];

                            if let Err(e) = renderer.begin_point_shadow_pass(
                                face,
                                shadow_bias_constant,
                                shadow_bias_slope,
                            ) {
                                log_error!("Failed to begin point shadow pass {}: {}", face, e);
                                return;
                            }

                            for &id in &point_model_ids {
                                let Some(model_renderer) = world.get_component::<ModelRenderer>(id) else { continue };
                                let Some(model) = model_renderer.model.as_ref() else { continue };
                                let model = model.clone();
                                let Some(transform) = world.get_component::<Transform>(id) else { continue };
                                let pc = ShadowPointModelPushConstants::new(
                                    [
                                        gpu_lights[ps.light_index as usize].position[0],
                                        gpu_lights[ps.light_index as usize].position[1],
                                        gpu_lights[ps.light_index as usize].position[2],
                                    ],
                                    ps.far,
                                    face_matrix,
                                    transform.global_position.into(),
                                    transform.global_scale.into(),
                                    [
                                        transform.global_rotation.v.x,
                                        transform.global_rotation.v.y,
                                        transform.global_rotation.v.z,
                                        transform.global_rotation.s,
                                    ],
                                );
                                for mesh in &model.meshes {
                                    if let Err(e) = renderer.shadow_point_model_render(
                                        Box::new(mesh.clone()),
                                        &pc,
                                    ) {
                                        log_error!(
                                            "Failed point shadow model render: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            if self.packages.contains(&Packages::Voxel) {
                                let voxel_ids = world.get_entities_with_component::<VoxelChunkMesh>();
                                for id in voxel_ids {
                                    let Some(transform) = world.get_component::<VoxelTransform>(id) else { continue };
                                    let Some(voxel_mesh) = world.get_component::<VoxelChunkMesh>(id) else { continue };
                                    let pc = ShadowPointVoxelPushConstants::new(
                                        [
                                            gpu_lights[ps.light_index as usize].position[0],
                                            gpu_lights[ps.light_index as usize].position[1],
                                            gpu_lights[ps.light_index as usize].position[2],
                                        ],
                                        ps.far,
                                        face_matrix,
                                        [
                                            transform.position.x * 32,
                                            transform.position.y * 32,
                                            transform.position.z * 32,
                                        ],
                                    );
                                    let voxel_mesh = voxel_mesh.clone();
                                    if let Err(e) = renderer.shadow_point_voxel_render(
                                        Box::new(voxel_mesh),
                                        &pc,
                                    ) {
                                        log_error!("Failed point shadow voxel render: {}", e);
                                    }
                                }
                            }

                            if let Err(e) = renderer.end_point_shadow_pass(face) {
                                log_error!("Failed to end point shadow pass {}: {}", face, e);
                                return;
                            }
                        }
                    }

                    let shadow_ns = shadow_start.elapsed().as_nanos() as u64;

                    let render_step = std::time::Instant::now();
                    if let Ok(viewport_size) = world.get_resource::<ViewportSize>() {
                        let w = viewport_size.pixel_width as u32;
                        let h = viewport_size.pixel_height as u32;
                        renderer.resize_viewport(w, h, aa_amount).unwrap();

                        world.insert_resource(ViewportTexture(
                            renderer.get_viewport_texture_id().unwrap(),
                        ));
                    }
                    renderer.begin_ui();
                    render_other_timings.push(("begin_ui", render_step.elapsed().as_nanos() as u64));

                    let world_update_start = std::time::Instant::now();
                    let update_timings = world.update();
                    let world_update_ns = world_update_start.elapsed().as_nanos() as u64;

                    let world_fixed_update_start = std::time::Instant::now();
                    let fixed_timings = world.fixed_update();
                    let world_fixed_update_ns =
                        world_fixed_update_start.elapsed().as_nanos() as u64;

                    let viewport_render_start = std::time::Instant::now();
                    if let Err(e) = renderer.begin_viewport_render() {
                        log_error!("Failed to begin viewport render: {}", e);
                    }

                    // Build a name → GpuMaterial lookup for material_override on ModelRenderer.
                    let gpu_mat_by_name: std::collections::HashMap<String, GpuMaterial> = {
                        let mut map = std::collections::HashMap::new();
                        for model in model_registry.paths.values() {
                            for mesh in &model.meshes {
                                if let Some(ref mat) = mesh.material {
                                    map.entry(mesh.material_name.clone())
                                        .or_insert_with(|| mat.clone());
                                }
                            }
                        }
                        map
                    };

                    // Snapshot shader_path values from YAML materials so overrides on
                    // standalone materials (not embedded in any glTF mesh) can apply shaders.
                    let yaml_shader_by_name: std::collections::HashMap<String, String> = world
                        .get_resource::<AssetManager>()
                        .ok()
                        .and_then(|am| am.get_loader::<crate::assets::loaders::material_loader::MaterialLoader>())
                        .map(|loader| {
                            loader.registry.read().unwrap()
                                .materials
                                .iter()
                                .filter_map(|(k, mat)| {
                                    mat.shader_path.as_ref().map(|s| (k.clone(), s.clone()))
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let object_ids = world.get_entities_with_component::<ModelRenderer>();

                    for id in object_ids {
                        // Lazily load model if needed
                        if world.get_component::<ModelRenderer>(id).map(|mr| mr.model.is_none()).unwrap_or(false) {
                            let model_path = match world.get_component::<ModelRenderer>(id) {
                                Some(mr) => mr.model_path.clone(),
                                None => continue,
                            };
                            let Some(model) = model_registry.paths.get(&model_path) else {
                                continue;
                            };
                            let model = model.clone();
                            if let Some(mr) = world.get_component_mut::<ModelRenderer>(id) {
                                mr.model = Some(Box::new(model));
                            }
                        }

                        let model_renderer = match world.get_component::<ModelRenderer>(id) {
                            Some(mr) => mr.clone(),
                            None => continue,
                        };
                        let Some(model) = model_renderer.model.clone() else {
                            continue;
                        };

                        let Some(transform) = world.get_component::<Transform>(id) else { continue };
                        let transform = transform.clone();

                        let mut model_push = model_push_constants.clone();
                        model_push.world_position = transform.global_position;
                        model_push.world_scale = transform.global_scale;
                        model_push.world_rotation = transform.global_rotation;

                        let override_gpu_mat = model_renderer
                            .material_override
                            .as_ref()
                            .and_then(|name| gpu_mat_by_name.get(name));

                        for mesh in &model.meshes {
                            let effective_mat: Option<&GpuMaterial> =
                                override_gpu_mat.or(mesh.material.as_ref());
                            let albedo_ds = effective_mat
                                .and_then(|m| m.albedo.as_ref())
                                .map(|t| t.descriptor_set);
                            let shader_override: Option<&str> =
                                effective_mat.and_then(|m| m.shader.as_deref()).or_else(|| {
                                    model_renderer
                                        .material_override
                                        .as_deref()
                                        .and_then(|name| yaml_shader_by_name.get(name))
                                        .map(|s| s.as_str())
                                });
                            let mut mesh_push = model_push.clone();
                            if let Some(mat) = effective_mat {
                                mesh_push.color_modifier = mat.color;
                            }
                            if model_renderer.is_wireframe {
                                if let Err(e) = renderer.wireframe_render(
                                    Box::new(mesh.clone()),
                                    push_constants.clone(),
                                    &mesh_push,
                                    albedo_ds,
                                    shader_override,
                                ) {
                                    log_error!("Failed to render wireframe: {}", e);
                                }
                            } else {
                                if let Err(e) = renderer.render(
                                    Box::new(mesh.clone()),
                                    push_constants.clone(),
                                    &mesh_push,
                                    albedo_ds,
                                    shader_override,
                                ) {
                                    log_error!("Failed to render model: {}", e);
                                }
                            }
                        }
                    }

                    // Render terrain chunks
                    if self.packages.contains(&Packages::Terrain) {
                        // Rebuild atlas if textures changed
                        if world.has_resource::<TerrainAtlasNeedsRebuild>() {
                            world.remove_resource::<TerrainTextureAtlas>();
                            world.remove_resource::<TerrainAtlasNeedsRebuild>();
                        }

                        // Lazily upload the terrain texture atlas
                        if !world.has_resource::<TerrainTextureAtlas>() {
                            let tex_settings = world
                                .get_resource::<TerrainSettings>()
                                .ok()
                                .map(|s| s.texture_layers.clone())
                                .unwrap_or_default();
                            if let Ok(cmd_pool) = renderer.get_command_pool() {
                                let dp = renderer.get_descriptor_pool();
                                let dsl = renderer.get_voxel_descriptor_set_layout();
                                if let Ok(atlas) =
                                    crate::terrain::texture_atlas::upload_terrain_textures(
                                        &context,
                                        cmd_pool,
                                        dp,
                                        dsl,
                                        &tex_settings,
                                        256,
                                    )
                                {
                                    world.insert_resource(atlas);
                                }
                            }
                        }

                        let terrain_atlas_ds = world
                            .get_resource::<TerrainTextureAtlas>()
                            .ok()
                            .map(|a| a.descriptor_set);

                        let terrain_ids = world.get_entities_with_component::<TerrainMesh>();
                        for id in terrain_ids {
                            let Some(terrain_mesh) = world.get_component::<TerrainMesh>(id) else { continue };
                            if terrain_mesh.index_count == 0 {
                                continue;
                            }
                            let terrain_mesh = terrain_mesh.clone();
                            let mut terrain_push = model_push_constants.clone();
                            terrain_push.world_position = Vector3::new(0.0, 0.0, 0.0);
                            terrain_push.world_scale = Vector3::new(1.0, 1.0, 1.0);
                            terrain_push.world_rotation =
                                cgmath::Quaternion::new(1.0, 0.0, 0.0, 0.0);
                            // Fill in per-chunk active layer IDs for the splatting shader.
                            if let Some(chunk) = world.get_component::<TerrainChunk>(id) {
                                terrain_push.active_layer_ids_packed = ModelPushConstants::pack_layer_ids(
                                    &chunk.active_layer_ids,
                                    chunk.active_layer_count,
                                );
                                terrain_push.layer_count = chunk.active_layer_count as u32;
                            }
                            if let Err(e) = renderer.render(
                                Box::new(terrain_mesh),
                                push_constants.clone(),
                                &terrain_push,
                                terrain_atlas_ds,
                                Some("sdr_default_terrain"),
                            ) {
                                log_error!("Failed to render terrain: {}", e);
                            }
                        }
                    }

                    // TODO: make this a render function so its not in the main render
                    if self.packages.contains(&Packages::Voxel) {
                        let voxel_push_constants =
                            world.get_resource::<VoxelPushConstants>().unwrap();
                        let texture_atlas = world.get_resource::<VoxelTextureAtlas>().unwrap();
                        let frustum = Frustum::from_view_proj(&view_proj);
                        let mut water_draws: Vec<(
                            f32,
                            Box<dyn crate::rendering::shared::model::GpuMesh>,
                            PushConstants,
                            VoxelPushConstants,
                        )> = Vec::new();
                        let voxel_chunk_ids = world.get_entities_with_component::<VoxelChunkMesh>();
                        for id in voxel_chunk_ids {
                            let Some(transform) = world.get_component::<VoxelTransform>(id) else { continue };
                            let world_pos = Vector3::new(
                                transform.position.x as f32 * 32.0,
                                transform.position.y as f32 * 32.0,
                                transform.position.z as f32 * 32.0,
                            );
                            let transform_pos = transform.position;

                            if !frustum.contains_aabb(
                                world_pos,
                                world_pos + Vector3::new(32.0, 32.0, 32.0),
                            ) {
                                continue;
                            }
                            objects_dawn += 1;
                            let Some(voxel_mesh) = world.get_component::<VoxelChunkMesh>(id) else { continue };
                            let voxel_mesh = voxel_mesh.clone();
                            let water_mesh = world.get_component::<WaterMesh>(id).cloned();

                            let delta = world.get_resource::<EngineTimer>().unwrap();

                            let chunk_push = push_constants.clone();
                            let mut voxel_chunk_push = voxel_push_constants.clone();

                            voxel_chunk_push.time = delta.0;
                            voxel_chunk_push.set_position(Vector3::new(
                                transform_pos.x * 32,
                                transform_pos.y * 32,
                                transform_pos.z * 32,
                            ));

                            if let Err(e) = renderer.voxel_render(
                                Box::new(voxel_mesh),
                                texture_atlas,
                                &chunk_push,
                                &voxel_chunk_push,
                            ) {
                                log_error!("Failed to render voxel: {}", e);
                            }

                            if let Some(water_mesh) = water_mesh {
                                let chunk_center = world_pos + Vector3::new(16.0, 16.0, 16.0);
                                let distance = (chunk_center - camera_pos).magnitude2();
                                water_draws.push((
                                    distance,
                                    Box::new(water_mesh),
                                    chunk_push.clone(),
                                    voxel_chunk_push.clone(),
                                ));
                            }
                        }

                        water_draws.sort_by(|a, b| {
                            b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
                        });
                        for (_, mesh, chunk_push, voxel_chunk_push) in water_draws {
                            if let Err(e) = renderer.water_render(
                                mesh,
                                texture_atlas,
                                &chunk_push,
                                &voxel_chunk_push,
                            ) {
                                log_error!("Failed to render water: {}", e);
                            }
                        }
                    }

                    world.get_resource_mut::<ObjectsDrawing>().unwrap().0 = objects_dawn;
                    if let Err(e) = renderer.end_viewport_render() {
                        log_error!("Failed to end viewport render: {}", e);
                    }
                    let viewport_render_ns = viewport_render_start.elapsed().as_nanos() as u64;

                    let render_step = std::time::Instant::now();
                    if let Err(e) = renderer.begin_swapchain_render() {
                        log_error!("Failed to begin swapchain render: {}", e);
                    }
                    render_other_timings.push(("begin_swapchain", render_step.elapsed().as_nanos() as u64));

                    let render_step = std::time::Instant::now();
                    if let Err(e) = renderer.end_ui() {
                        log_error!("Failed to end UI: {}", e);
                    }
                    render_other_timings.push(("end_ui", render_step.elapsed().as_nanos() as u64));

                    let render_step = std::time::Instant::now();
                    if let Err(e) = renderer.end_frame() {
                        log_error!("Failed to end frame: {}", e);
                    }
                    render_other_timings.push(("end_frame", render_step.elapsed().as_nanos() as u64));
                    let render_total_ns = render_start.elapsed().as_nanos() as u64;

                    let world_late_update_start = std::time::Instant::now();
                    let late_timings = world.late_update();
                    let world_late_update_ns = world_late_update_start.elapsed().as_nanos() as u64;

                    let frame_time_ns = frame_start.elapsed().as_nanos() as u64;

                    if let Ok(profiler) = world.get_resource_mut::<Profiler>() {
                        let system_timings = {
                            let mut v = Vec::new();
                            for (name, ns) in prerender_timings {
                                v.push(SystemTiming { phase: "PreRender", name, elapsed_ns: ns });
                            }
                            for (name, ns) in update_timings {
                                v.push(SystemTiming { phase: "Update", name, elapsed_ns: ns });
                            }
                            for (name, ns) in fixed_timings {
                                v.push(SystemTiming { phase: "FixedUpdate", name, elapsed_ns: ns });
                            }
                            for (name, ns) in late_timings {
                                v.push(SystemTiming { phase: "LateUpdate", name, elapsed_ns: ns });
                            }
                            for (name, ns) in render_other_timings {
                                v.push(SystemTiming { phase: "RenderOther", name, elapsed_ns: ns });
                            }
                            v
                        };
                        profiler.push_sample(
                            FrameSample {
                                frame_ns: frame_time_ns,
                                render_total_ns,
                                shadow_pass_ns: shadow_ns,
                                viewport_render_ns,
                                world_prerender_ns: prerender_ns,
                                world_update_ns,
                                world_fixed_update_ns,
                                world_late_update_ns,
                            },
                            system_timings,
                        );
                    }
                }

                _ => {}
            }
            let mut world = self.world.lock().unwrap();
            let input_manager = world.get_resource_mut::<InputManager>().unwrap();
            input_manager.handle_input_event(event.clone());
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let mut world = self.world.lock().unwrap();
        let input_manager = world.get_resource_mut::<InputManager>().unwrap();

        match event {
            DeviceEvent::MouseMotion { delta } => {
                input_manager.handle_mouse_motion(delta);
            }

            _ => (),
        }
    }
}

impl ApplicationHandler for Core {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut world = self.world.lock().unwrap();

        let aa_amount = world.get_resource::<AntiAliasing>().unwrap().amount;

        let rendering_info = Some(RenderingInfo::new(
            event_loop,
            self.rendering_api,
            aa_amount,
        ));

        {
            let ri = rendering_info.as_ref().unwrap();
            let locked = ri.lock().unwrap();

            world
                .get_resource_mut::<AntiAliasing>()
                .unwrap()
                .available_options = locked.context.get_supported_sample_counts();

            let window_id = locked.window.id();
            let window = locked.window.clone();

            let window_manager = world.get_resource_mut::<WindowManager>().unwrap();
            window_manager.windows.insert(window_id, window);
            window_manager.primary_window_id = window_id;
        }

        self.rendering_info = rendering_info;
        let context = self
            .rendering_info
            .clone()
            .unwrap()
            .lock()
            .unwrap()
            .context
            .clone();

        let (command_pool, descriptor_pool, descriptor_set_layout, egui_context) = {
            let ri = self.rendering_info.as_ref().unwrap().lock().unwrap();
            let renderer = ri.renderer.as_ref().unwrap();
            (
                renderer.get_command_pool().unwrap(),
                renderer.get_descriptor_pool(),
                renderer.get_voxel_descriptor_set_layout(),
                renderer.get_egui_context(),
            )
        };

        let model_registry = ModelRegistry::default();
        let model_loader = ModelLoader {
            registry: Arc::new(RwLock::new(model_registry)),
        };

        // TODO: clean this up
        {
            if !world.has_resource::<AssetManager>() {
                world.insert_resource(AssetManager::new());
            }
            let asset_manager = world.get_resource_mut::<AssetManager>().unwrap();
            asset_manager.model_loader = model_loader;

            asset_manager
                .load_models(
                    Path::new("res/"),
                    Arc::new(context.clone()),
                    command_pool,
                    descriptor_pool,
                    descriptor_set_layout,
                )
                .unwrap();

            asset_manager
                .load_models(
                    Path::new(&format!("{}/{}", env!("CARGO_MANIFEST_DIR"), "res/")),
                    Arc::new(context),
                    command_pool,
                    descriptor_pool,
                    descriptor_set_layout,
                )
                .unwrap();

            // world.insert_resource(b);
        }

        let context = self
            .rendering_info
            .clone()
            .unwrap()
            .lock()
            .unwrap()
            .context
            .clone();

        if let Ok(pending) = world.get_resource::<PendingAtlas>() {
            let atlas = upload_atlas(
                &context,
                command_pool,
                descriptor_pool,
                descriptor_set_layout,
                &pending.image,
                pending.tiles,
            )
            .expect("Failed to upload voxel atlas");
            world.insert_resource(atlas);
        }

        let viewport_texture_id = self
            .rendering_info
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .renderer
            .as_ref()
            .unwrap()
            .get_viewport_texture_id()
            .expect("Viewport texture id missing");

        let mut font_registry = FontRegistry::default();
        // Core fonts (path baked in at compile time).
        font_registry.load_dir(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/res/fonts"
        )));
        // App fonts (relative to working directory at runtime).
        font_registry.load_dir(std::path::Path::new("res/fonts"));
        font_registry.apply_if_needed(&egui_context);

        world.insert_resource(EguiContext(egui_context));
        world.insert_resource(font_registry);
        world.insert_resource(ViewportTexture(viewport_texture_id));
        world.insert_resource(ViewportSize::default());
        world.insert_resource(context);

        world.start();
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.window_event(event_loop, window_id, event);
    }

    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.device_event(event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let mut world = self.world.lock().unwrap();

        if world.has_resource::<ReloadShadersRequest>() {
            world.remove_resource::<ReloadShadersRequest>();

            if let Some(render_info) = &self.rendering_info {
                let mut render_info = render_info.lock().unwrap();

                if let Some(renderer) = &mut render_info.renderer {
                    renderer.reload_shaders().unwrap();
                }
            }
        }

        if let Some(render_info) = &self.rendering_info {
            let render_info = render_info.lock().unwrap();
            render_info.window.request_redraw();
        }
    }
}

/// Initializes the core of the application
/// Note: nothing can run in main after this
/// Note: automatically runs all start systems
pub fn init_core(rendering_api: RenderingBackend, packages: Vec<Packages>) -> Result<()> {
    let mut core = Core::new(rendering_api, packages);

    // run all start systems
    {
        let mut world = core.world.lock().unwrap();

        world.build_systems();
    }

    // begin event loop
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut core)?;

    Ok(())
}

pub fn init_core_with_mode(
    rendering_api: RenderingBackend,
    packages: Vec<Packages>,
    engine_mode: EngineMode,
) -> Result<()> {
    let mut core = Core::new_with_mode(rendering_api, packages, engine_mode);

    // run all start systems
    {
        let mut world = core.world.lock().unwrap();

        world.build_systems();
    }

    // begin event loop
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut core)?;

    Ok(())
}

#[start(mode = "editor")]
pub fn editor_start(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>()?;

    inputs.register_default_keybind(
        "Reload Shaders",
        KeyBind::new(PhysicalKey::Code(KeyCode::F5), KeyAction::Press),
    );
    Ok(())
}

#[update(mode = "editor")]
pub fn editor_update(world: &mut World) -> Result<()> {
    let inputs = world.get_resource::<InputManager>()?;
    if inputs.is_keybind_active("ReloadShaders") {
        world.insert_resource(ReloadShadersRequest(true));
    }
    Ok(())
}
