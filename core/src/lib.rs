extern crate self as apostasy_core;
pub use apostasy_macros::Component;
pub use apostasy_macros::fixed_update;
pub use apostasy_macros::late_update;
pub use apostasy_macros::start;
pub use apostasy_macros::update;

use winit::event::DeviceEvent;
use winit::event::DeviceId;

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use winit::{
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

use crate::assets::asset_manager::AssetManager;
use crate::assets::gltf::load_model;
use crate::objects::components::transform::Transform;
use crate::objects::resources::cursor_manager::CursorManager;
use crate::objects::resources::input_manager::InputManager;
use crate::objects::resources::window_manager::WindowManager;
use crate::packages::Packages;
use crate::packages::add_package;
use crate::rendering::components::camera::ActiveCamera;
use crate::rendering::components::camera::Camera;
use crate::rendering::components::camera::get_perspective_projection;
use crate::rendering::components::camera::get_view_matrix;
use crate::rendering::components::model_renderer::ModelRenderer;
use crate::rendering::shared::frustrum::Frustum;
use crate::rendering::shared::frustrum::ObjectsDrawing;
use crate::states::ShouldExit;
use crate::ui::ui_context::EguiContext;
use crate::voxels::VoxelTransform;
use crate::voxels::meshes::NeedsRemeshing;
use crate::voxels::meshes::VoxelChunkMesh;
use crate::voxels::meshes::{dispatch_remesh_jobs, receive_meshes};
use crate::voxels::texture_atlas::PendingAtlas;
use crate::voxels::texture_atlas::VoxelTextureAtlas;
use crate::voxels::texture_atlas::upload_atlas;
use crate::{
    objects::world::World,
    rendering::{RenderingBackend, RenderingInfo},
};
use winit::application::ApplicationHandler;

pub mod assets;
pub mod items;
pub mod objects;
pub mod packages;
pub mod physics;
pub mod rendering;
pub mod states;
pub mod ui;
pub mod utils;
pub mod voxels;

pub use anyhow;
pub use cgmath;
use cgmath::Vector3;
pub use crossbeam_channel;
pub use egui;
pub use epaint;
pub use noise;
pub use num_cpus;
pub use rand;
pub use rayon;
pub use serde;
pub use serde_yaml;
pub use winit;

pub struct Core {
    pub rendering_api: RenderingBackend,
    pub rendering_info: Option<Arc<Mutex<RenderingInfo>>>,
    pub world: Arc<Mutex<World>>,
    pub asset_loader: AssetManager,
}

impl Core {
    pub fn new(rendering_api: RenderingBackend, packages: Vec<Packages>) -> Self {
        let mut world = World::default();
        world.insert_resource(InputManager::default());
        world.insert_resource(CursorManager::default());
        world.insert_resource(WindowManager::default());
        world.insert_resource(ObjectsDrawing(0));

        for package in packages {
            add_package(&mut world, package);
        }

        Self {
            rendering_api,
            rendering_info: None,
            world: Arc::new(Mutex::new(world)),
            asset_loader: AssetManager::new(),
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
                    if let Some(renderer) = &mut rendering_info.renderer {
                        if let Err(e) = renderer.resize() {
                            log_error!("Failed to resize renderer: {}", e);
                        }
                    }
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(renderer) = &mut rendering_info.renderer {
                        if let Err(e) = renderer.resize() {
                            log_error!("Failed to resize renderer: {}", e);
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let mut objects_dawn = 0;
                    let mut world = self.world.lock().unwrap();

                    if world.get_resource::<ShouldExit>().is_ok() {
                        log!("Recieved ShouldExit resource, closing");
                        event_loop.exit();
                    }

                    let context = Arc::new(rendering_info.context.clone());
                    let push_constants = rendering_info.push_constants.clone();
                    let mut voxel_push_constants = rendering_info.voxel_push_constants.clone();
                    let model_push = rendering_info.model_push_constants.clone();

                    if let Some(atlas) = world.get_resource::<VoxelTextureAtlas>().ok() {
                        voxel_push_constants.set_atlas_tiles(atlas.atlas_size);
                    }

                    let Some(renderer) = &mut rendering_info.renderer else {
                        log_error!("No renderer found!");
                        return;
                    };

                    let camera = world.get_object_with_tag::<ActiveCamera>().unwrap();
                    let view = get_view_matrix(camera.get_component::<Transform>().unwrap());

                    let aspect = renderer.get_aspect();
                    let proj = get_perspective_projection(
                        camera.get_component::<Camera>().unwrap(),
                        aspect,
                    );

                    let view_proj = proj * view;

                    let mut push_constants = push_constants;
                    push_constants.set_camera_constants(camera.to_owned(), aspect);

                    if !world
                        .get_objects_with_tag_with_ids::<NeedsRemeshing>()
                        .is_empty()
                    {
                        dispatch_remesh_jobs(&mut world).expect("Failed to dispatch remesh jobs");
                    }

                    if let Ok(command_pool) = renderer.get_command_pool() {
                        receive_meshes(&mut world, &context, command_pool)
                            .expect("Failed to receive meshes");
                    }
                    
                    // Begin frame - if device is lost, skip rendering this frame
                    if let Err(e) = renderer.begin_frame(push_constants.clone()) {
                        log_error!("Failed to begin frame: {}", e);
                        return;
                    }

                    renderer.begin_ui();
                    world.update();
                    world.fixed_update();
                    let object_ids: Vec<_> = world
                        .get_objects_with_component_with_ids::<ModelRenderer>()
                        .iter()
                        .map(|o| o.0)
                        .collect();

                    for id in object_ids {
                        let object = world.get_object_mut(id).unwrap();

                        if object
                            .get_component::<ModelRenderer>()
                            .unwrap()
                            .model
                            .is_none()
                        {
                            let model_path = object
                                .get_component::<ModelRenderer>()
                                .unwrap()
                                .model_path
                                .clone();

                            let Some(command_pool) = renderer.get_command_pool().ok() else {
                                continue;
                            };

                            let model =
                                load_model(Path::new(&model_path), context.clone(), command_pool)
                                    .unwrap();

                            object.get_component_mut::<ModelRenderer>().unwrap().model =
                                Some(Box::new(model));
                        }

                        let model_renderer = object.get_component::<ModelRenderer>().unwrap();
                        let model = object
                            .get_component::<ModelRenderer>()
                            .unwrap()
                            .model
                            .clone()
                            .unwrap();

                        let transform = object.get_component::<Transform>().unwrap();

                        let mut frame_model_push = model_push.clone();
                        frame_model_push.world_position = transform.global_position;
                        frame_model_push.world_scale = transform.global_scale;
                        frame_model_push.world_rotation = transform.global_rotation;

                        for mesh in &model.meshes {
                            if model_renderer.is_wireframe {
                                if let Err(e) = renderer
                                    .wireframe_render(
                                        Box::new(mesh.clone()),
                                        push_constants.clone(),
                                        &frame_model_push,
                                    )
                                {
                                    log_error!("Failed to render wireframe: {}", e);
                                }
                            } else {
                                if let Err(e) = renderer
                                    .render(
                                        Box::new(mesh.clone()),
                                        push_constants.clone(),
                                        &frame_model_push,
                                    )
                                {
                                    log_error!("Failed to render model: {}", e);
                                }
                            }
                        }
                    }

                    if let Some(texture_atlas) = world.get_resource::<VoxelTextureAtlas>().ok() {
                        let frustum = Frustum::from_view_proj(&view_proj);

                        for object in world.get_objects_with_component::<VoxelChunkMesh>() {
                            let transform = object.get_component::<VoxelTransform>().unwrap();
                            let world_pos = Vector3::new(
                                transform.position.x as f32 * 32.0,
                                transform.position.y as f32 * 32.0,
                                transform.position.z as f32 * 32.0,
                            );

                            if !frustum.contains_aabb(
                                world_pos,
                                world_pos + Vector3::new(32.0, 32.0, 32.0),
                            ) {
                                continue;
                            }
                            objects_dawn += 1;
                            let voxel_mesh = object.get_component::<VoxelChunkMesh>().unwrap();

                            let chunk_push = push_constants.clone();
                            let mut voxel_chunk_push = voxel_push_constants.clone();

                            voxel_chunk_push.set_position(Vector3::new(
                                transform.position.x * 32,
                                transform.position.y * 32,
                                transform.position.z * 32,
                            ));

                            if let Err(e) = renderer
                                .voxel_render(
                                    Box::new(voxel_mesh.clone()),
                                    &texture_atlas,
                                    &chunk_push,
                                    &voxel_chunk_push,
                                )
                            {
                                log_error!("Failed to render voxel: {}", e);
                            }
                        }
                    }

                    world.get_resource_mut::<ObjectsDrawing>().unwrap().0 = objects_dawn;
                    if let Err(e) = renderer.end_ui() {
                        log_error!("Failed to end UI: {}", e);
                    }
                    if let Err(e) = renderer.end_frame() {
                        log_error!("Failed to end frame: {}", e);
                    }
                    world.late_update();
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
        input_manager.handle_device_event(event.clone());
    }
}
impl ApplicationHandler for Core {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let rendering_info = Some(RenderingInfo::new(&event_loop, self.rendering_api));
        let mut world = self.world.lock().unwrap();
        {
            let ri = rendering_info.as_ref().unwrap();
            let locked = ri.lock().unwrap();

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

        let pending = world.get_resource::<PendingAtlas>().unwrap().clone();

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

        let atlas = upload_atlas(
            &context,
            command_pool,
            descriptor_pool,
            descriptor_set_layout,
            &pending.image,
            pending.tiles,
        )
        .expect("Failed to upload voxel atlas");

        world.insert_resource(EguiContext(egui_context));
        world.insert_resource(context);
        world.insert_resource(atlas);

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
        if let Some(render_info) = &self.rendering_info {
            render_info.lock().unwrap().window.request_redraw();
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
