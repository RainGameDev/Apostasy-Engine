use std::sync::{Arc, Mutex};

use anyhow::Result;
use apostasy_macros::Resource;
use ash::vk::{self, CommandPool, Extent2D};
use egui::{Context, TextureId};
use winit::event::WindowEvent;
use winit::{event_loop::ActiveEventLoop, window::Window};

use crate::rendering::lighting::gpu_light::{GpuLight, PointShadowData, ShadowData};
use crate::rendering::shared::anti_aliasing::AntiAliasingAmount;
use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ModelPushConstants, PushConstants, ShadowModelPushConstants, ShadowPointModelPushConstants,
    ShadowPointVoxelPushConstants, ShadowVoxelPushConstants, VoxelPushConstants,
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
pub mod lighting;
pub mod opengl;
pub mod shared;
pub mod vulkan;

#[derive(Clone, Copy)]
pub enum RenderingBackend {
    Vulkan,
    OpenGl,
}

#[derive(Clone, Resource, Default)]
pub struct WindowInfo {
    pub window_size: (f32, f32),
}

pub struct RenderingInfo {
    /// TODO: change this to a basic rendering context
    pub context: VulkanRenderingContext,
    pub window: Arc<Window>,
    pub settings: RenderingSettings,
    pub renderer: Option<Box<dyn RenderingAPI>>,
}

/// Common interface for graphics backends (Vulkan, OpenGL).
///
/// Each frame follows the sequence: `begin_frame` → viewport render → swapchain render → `end_frame`.
pub trait RenderingAPI {
    /// Acquires the next swapchain image and prepares per-frame state.
    fn begin_frame(&mut self) -> Result<()>;
    /// Returns the fence wait time (ns) from the last begin_frame call.
    fn last_fence_wait_ns(&self) -> u64;
    /// Begins the offscreen viewport render pass.
    fn begin_viewport_render(&mut self) -> Result<()>;
    /// Ends the offscreen viewport render pass.
    fn end_viewport_render(&mut self) -> Result<()>;
    /// Begins the final swapchain render pass.
    fn begin_swapchain_render(&mut self) -> Result<()>;
    /// Submits the frame and presents it to the surface.
    fn end_frame(&mut self) -> Result<()>;

    /// Draws a model mesh with the given push constants.
    /// `shader_override`: name of a custom shader pair (e.g. `"sdr_default_water"` -> `sdr_default_water.vert`/`sdr_default_water.frag`),
    /// or `None` to use the default model shader.
    fn render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        albedo_descriptor_set: Option<vk::DescriptorSet>,
        shader_override: Option<&str>,
    ) -> Result<()>;
    /// Draws a model mesh in wireframe mode.
    fn wireframe_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        albedo_descriptor_set: Option<vk::DescriptorSet>,
        shader_override: Option<&str>,
    ) -> Result<()>;
    /// Draws a collider debug wireframe: depth test disabled (always renders above the
    /// mesh) and blended so the outline color is the inverse of whatever is underneath.
    fn collider_debug_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
    ) -> Result<()>;

    /// Draws a voxel mesh using the provided texture atlas. `wireframe` selects
    /// the line-rasterized voxel pipeline instead of the filled one.
    fn voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
        wireframe: bool,
    ) -> Result<()>;

    /// Draws a water voxel mesh using the provided texture atlas.
    fn water_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
    ) -> Result<()>;

    /// Draws one sky layer on the sky sphere; call once per layer, back to front.
    /// `color_modifier.a` is the layer opacity; `additive` selects the additive
    /// blend pipeline (glow layers) instead of alpha-over.
    fn skybox_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        sky_descriptor_set: vk::DescriptorSet,
        additive: bool,
    ) -> Result<()>;

    /// Begins the egui UI render pass for this frame.
    fn begin_ui(&mut self);
    /// Ends the egui UI render pass and submits draw commands.
    fn end_ui(&mut self) -> Result<()>;
    /// Forwards a window event to the egui input handler. Returns `true` if egui consumed it.
    fn handle_ui_event(&mut self, event: &WindowEvent) -> bool;
    fn get_egui_context(&self) -> Context;
    /// Returns the egui `TextureId` for the offscreen viewport, if one exists.
    fn get_viewport_texture_id(&self) -> Option<TextureId>;

    /// Responds to a window resize by updating surface-dependent state.
    fn resize(&mut self) -> Result<()>;
    fn update_command_buffer(&mut self);
    /// Destroys and recreates the swapchain, typically after a resize or surface loss.
    fn recreate_swapchain(&mut self);
    /// Hot-reloads shaders from disk. Returns `true` if any shaders were reloaded.
    fn reload_shaders(&mut self) -> Result<bool>;

    fn get_buffer_graveyard(&mut self) -> &mut Vec<(vk::Buffer, vk::DeviceMemory)>;
    fn get_command_pool(&self) -> Result<CommandPool>;
    fn get_aspect(&self) -> f32;
    fn get_descriptor_pool(&self) -> vk::DescriptorPool;
    fn get_voxel_descriptor_set_layout(&self) -> vk::DescriptorSetLayout;

    /// Uploads the active lights and shadow data. `shadow_data: None` disables shadow sampling.
    fn set_lights(
        &mut self,
        lights: &[GpuLight],
        shadow_data: Option<ShadowData>,
        point_shadow_data: Option<PointShadowData>,
        shadow_distance: f32,
        camera_pos: [f32; 3],
        camera_dir: [f32; 3],
    );

    /// Destroys and recreates the directional/spot shadow map texture array at the given size.
    fn rebuild_shadow_map(&mut self, size: u32) -> Result<()>;
    /// Destroys and recreates the point light cubemap shadow texture at the given size.
    fn rebuild_point_shadow_map(&mut self, size: u32) -> Result<()>;

    /// Begins the depth-only shadow pre-pass for the given cascade index (directional/spot).
    fn begin_shadow_pass(
        &mut self,
        cascade_index: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()>;
    /// Ends the shadow pre-pass for the given cascade index.
    fn end_shadow_pass(&mut self, cascade_index: usize) -> Result<()>;
    /// Renders a model mesh into the directional/spot shadow map.
    fn shadow_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowModelPushConstants,
    ) -> Result<()>;
    /// Renders a voxel mesh into the directional/spot shadow map.
    fn shadow_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowVoxelPushConstants,
    ) -> Result<()>;

    /// Begins the depth-only shadow pre-pass for the given cubemap face (point light).
    fn begin_point_shadow_pass(
        &mut self,
        face: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()>;
    /// Ends the point light shadow pass for the given face.
    fn end_point_shadow_pass(&mut self, face: usize) -> Result<()>;
    /// Renders a model mesh into the point light cubemap shadow map.
    fn shadow_point_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointModelPushConstants,
    ) -> Result<()>;
    /// Renders a voxel mesh into the point light cubemap shadow map.
    fn shadow_point_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointVoxelPushConstants,
    ) -> Result<()>;

    /// Assigns the rendering_info's renderer the the value created via this
    fn new(
        rendering_info: Arc<Mutex<RenderingInfo>>,
        window: Arc<Window>,
        aa_amount: AntiAliasingAmount,
    ) -> Result<()>
    where
        Self: Sized;

    fn resize_viewport(
        &mut self,
        width: u32,
        height: u32,
        aa_amount: AntiAliasingAmount,
    ) -> Result<()>;
    fn get_viewport_extent(&mut self) -> Extent2D;
}

impl RenderingInfo {
    pub fn new(
        event_loop: &ActiveEventLoop,
        rendering_api: RenderingBackend,
        aa_amount: AntiAliasingAmount,
    ) -> Arc<Mutex<Self>> {
        let window = Arc::new(
            event_loop
                .create_window(
                    winit::window::WindowAttributes::default()
                        .with_inner_size(winit::dpi::LogicalSize::new(3840.0, 2160.0)),
                )
                .unwrap(),
        );

        let rendering_info = Arc::new(Mutex::new(RenderingInfo {
            context: VulkanRenderingContext::new(RenderingContextAttributes {
                compatability_window: &window,
                queue_family_picker: queue_family_picker::single_queue_family,
            })
            .unwrap(),
            window: window.clone(),
            settings: RenderingSettings::default(),
            renderer: None,
        }));

        match rendering_api {
            RenderingBackend::Vulkan => {
                VulkanRenderer::new(rendering_info.clone(), window, aa_amount).unwrap();
            }
            RenderingBackend::OpenGl => {
                println!("Opengl is not supported at the moment");
            }
        }

        rendering_info
    }
}
