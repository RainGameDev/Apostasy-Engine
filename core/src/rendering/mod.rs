use std::sync::{Arc, Mutex};

use anyhow::Result;
use ash::vk::{self, CommandPool};
use egui::Context;
use winit::event::WindowEvent;
use winit::{event_loop::ActiveEventLoop, window::Window};

use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ModelPushConstants, PushConstants, VoxelPushConstants,
};
use crate::rendering::{
    shared::rendering_settings::RenderingSettings,
    vulkan::{
        VulkanRenderer,
        queue_family::queue_family_picker,
        rendering_context::{RenderingContextAttributes, VulkanRenderingContext},
    },
};
use crate::voxels::texture_atlas::VoxelTextureAtlas;

pub mod components;
pub mod opengl;
pub mod shared;
pub mod vulkan;

#[derive(Clone, Copy)]
pub enum RenderingBackend {
    Vulkan,
    OpenGl,
}

pub struct RenderingInfo {
    /// TODO: change this to a basic rendering context
    pub context: VulkanRenderingContext,
    pub window: Arc<Window>,
    pub settings: RenderingSettings,
    pub renderer: Option<Box<dyn RenderingAPI>>,
    pub push_constants: PushConstants,
    pub model_push_constants: ModelPushConstants,
    pub voxel_push_constants: VoxelPushConstants,
}

/// A trait assigned to any Rendering API
/// Used for Vulkan and Opengl
pub trait RenderingAPI {
    fn begin_frame(&mut self, push_constants: PushConstants) -> Result<()>;
    fn end_frame(&mut self) -> Result<()>;

    fn render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
    ) -> Result<()>;
    fn wireframe_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
    ) -> Result<()>;

    fn voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
    ) -> Result<()>;

    fn begin_ui(&mut self);
    fn end_ui(&mut self) -> Result<()>;
    fn handle_ui_event(&mut self, event: &WindowEvent) -> bool;
    fn get_egui_context(&self) -> Context;

    fn resize(&mut self) -> Result<()>;
    fn update_command_buffer(&mut self);
    fn recreate_swapchain(&mut self);

    fn get_command_pool(&self) -> Result<CommandPool>;
    fn get_aspect(&self) -> f32;
    fn get_descriptor_pool(&self) -> vk::DescriptorPool;
    fn get_voxel_descriptor_set_layout(&self) -> vk::DescriptorSetLayout;
    /// Assigns the rendering_info's renderer the the value created via this
    fn new(rendering_info: Arc<Mutex<RenderingInfo>>, window: Arc<Window>) -> Result<()>
    where
        Self: Sized;
}

impl RenderingInfo {
    pub fn new(event_loop: &ActiveEventLoop, rendering_api: RenderingBackend) -> Arc<Mutex<Self>> {
        let window = Arc::new(event_loop.create_window(Default::default()).unwrap());

        let rendering_info = Arc::new(Mutex::new(RenderingInfo {
            context: VulkanRenderingContext::new(RenderingContextAttributes {
                compatability_window: &window,
                queue_family_picker: queue_family_picker::single_queue_family,
            })
            .unwrap(),
            window: window.clone(),
            settings: RenderingSettings::default(),
            renderer: None,
            push_constants: PushConstants::default(),
            voxel_push_constants: VoxelPushConstants::default(),
            model_push_constants: ModelPushConstants::default(),
        }));

        match rendering_api {
            RenderingBackend::Vulkan => {
                VulkanRenderer::new(rendering_info.clone(), window).unwrap();
            }
            RenderingBackend::OpenGl => {
                println!("Opengl is not supported at the moment");
            }
        }

        rendering_info
    }
}
