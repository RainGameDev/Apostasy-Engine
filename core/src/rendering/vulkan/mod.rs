use std::sync::{Arc, Mutex};

use anyhow::Result;
use ash::vk::{self, CommandPool, Extent2D, PipelineLayout, SampleCountFlags};
use egui::{ClippedPrimitive, Context, TextureId};
use winit::event::WindowEvent;
use winit::window::Window;

use crate::rendering::lighting::gpu_light::{
    CSM_CASCADE_COUNT, GpuLight, PointShadowData, ShadowData,
};
use crate::rendering::shared::anti_aliasing::AntiAliasingAmount;
use crate::rendering::shared::material::GpuMaterial;
use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ModelPushConstants, PushConstants, ShadowModelPushConstants, ShadowPointModelPushConstants,
    ShadowPointVoxelPushConstants, ShadowVoxelPushConstants, VoxelPushConstants,
};
use crate::rendering::vulkan::image_layout::ImageLayouts;
use crate::rendering::vulkan::pipeline_manager::PipelineManager;
use crate::rendering::vulkan::rendering_context::VulkanRenderingContext;
use crate::rendering::vulkan::{frame::VulkanFrame, swapchain::VulkanSwapchain};
use crate::rendering::{RenderingAPI, RenderingInfo};
use crate::ui::UIRenderer;
use crate::voxels::texture_atlas::VoxelTextureAtlas;

pub mod device;
pub mod draw;
pub mod frame;
pub mod image_layout;
pub mod init;
pub mod lights;
pub mod pipeline_manager;
pub mod pipelines;
pub mod point_shadow;
pub mod queue_family;
pub mod rendering_context;
pub mod shadow;
pub mod surface;
pub mod swapchain;
pub mod ui;
pub mod viewport;

/// A container for a UBO
#[derive(Clone)]
pub struct Ubo {
    pub buffer: vk::Buffer,
    pub memory: vk::DeviceMemory,
}

pub(crate) fn aa_sample_count(aa_amount: AntiAliasingAmount) -> SampleCountFlags {
    match aa_amount {
        AntiAliasingAmount::X0 => SampleCountFlags::TYPE_1,
        AntiAliasingAmount::X2 => SampleCountFlags::TYPE_2,
        AntiAliasingAmount::X4 => SampleCountFlags::TYPE_4,
        AntiAliasingAmount::X8 => SampleCountFlags::TYPE_8,
    }
}

pub struct VulkanRenderer {
    pub current_image_index: u32,
    pub in_flight_frames_count: usize,
    pub swapchain: VulkanSwapchain,
    pub frames: Vec<VulkanFrame>,
    pub current_frame: usize,
    pub image_layouts: ImageLayouts,

    pub pipeline_layout: PipelineLayout,
    pub voxel_pipeline_layout: PipelineLayout,
    pub water_pipeline_layout: PipelineLayout,
    pub voxel_descriptor_pool: vk::DescriptorPool,
    pub voxel_descriptor_set_layout: vk::DescriptorSetLayout,

    pub default_white_material: GpuMaterial,

    pub ui_renderer: UIRenderer,
    pub buffer_graveyard: Vec<(vk::Buffer, vk::DeviceMemory)>,

    pub viewport_image: vk::Image,
    pub viewport_image_memory: vk::DeviceMemory,
    pub viewport_image_view: vk::ImageView,
    // MSAA color buffer
    pub msaa_color_image: vk::Image,
    pub msaa_color_memory: vk::DeviceMemory,
    pub msaa_color_view: vk::ImageView,
    // MSAA depth buffer
    pub viewport_depth_image: vk::Image,
    pub viewport_depth_memory: vk::DeviceMemory,
    pub viewport_depth_view: vk::ImageView,

    pub viewport_sampler: vk::Sampler,
    pub viewport_descriptor_set: vk::DescriptorSet,
    pub viewport_texture_id: TextureId,
    pub viewport_extent: vk::Extent2D,
    pub viewport_target_initialized: bool,
    pub viewport_depth_initialized: bool,
    pub last_fence_wait_ns: u64,

    pub light_ssbo: vk::Buffer,
    pub light_ssbo_memory: vk::DeviceMemory,
    pub light_descriptor_pool: vk::DescriptorPool,
    pub light_descriptor_set_layout: vk::DescriptorSetLayout,
    pub light_descriptor_set: vk::DescriptorSet,

    pub shadow_image: vk::Image,
    pub shadow_image_memory: vk::DeviceMemory,
    pub shadow_image_view: vk::ImageView,
    pub shadow_cascade_views: [vk::ImageView; CSM_CASCADE_COUNT],
    pub shadow_sampler: vk::Sampler,
    pub shadow_map_size: u32,
    pub shadow_model_pipeline_layout: PipelineLayout,
    pub shadow_voxel_pipeline_layout: PipelineLayout,
    pub shadow_model_vertex_shader: String,
    pub shadow_voxel_vertex_shader: String,
    pub shadow_fragment_shader: String,

    pub point_shadow_image: vk::Image,
    pub point_shadow_image_memory: vk::DeviceMemory,
    pub point_shadow_cube_view: vk::ImageView,
    pub point_shadow_face_views: [vk::ImageView; 6],
    pub point_shadow_sampler: vk::Sampler,
    pub point_shadow_map_size: u32,
    pub shadow_point_model_pipeline_layout: PipelineLayout,
    pub shadow_point_voxel_pipeline_layout: PipelineLayout,
    pub shadow_point_model_vertex_shader: String,
    pub shadow_point_voxel_vertex_shader: String,
    pub shadow_point_fragment_shader: String,

    pub ubo: Ubo,
    pub pipeline_manager: PipelineManager,
    pub default_vertex_shader: String,
    pub default_fragment_shader: String,
    pub voxel_vertex_shader: String,
    pub voxel_fragment_shader: String,
    pub water_vertex_shader: String,
    pub water_fragment_shader: String,
    context: Arc<VulkanRenderingContext>,

    pub aa_amount: AntiAliasingAmount,
    pub ui_cached_primitives: Vec<ClippedPrimitive>,
    pub ui_pending_texture_frees: Vec<TextureId>,
}

impl VulkanRenderer {
    /// Command buffer of the frame currently being recorded.
    pub(crate) fn cmd(&self) -> vk::CommandBuffer {
        self.frames[self.current_frame].command_buffer
    }
}

impl RenderingAPI for VulkanRenderer {
    fn new(
        rendering_info: Arc<Mutex<RenderingInfo>>,
        window: Arc<Window>,
        aa_amount: AntiAliasingAmount,
    ) -> Result<()> {
        Self::initialize(rendering_info, window, aa_amount)
    }

    fn begin_frame(&mut self) -> Result<()> {
        self.begin_frame()
    }

    fn begin_viewport_render(&mut self) -> Result<()> {
        self.begin_viewport_render()
    }

    fn end_viewport_render(&mut self) -> Result<()> {
        self.end_viewport_render()
    }

    fn begin_swapchain_render(&mut self) -> Result<()> {
        self.begin_swapchain_render()
    }

    fn end_frame(&mut self) -> Result<()> {
        self.end_frame()
    }

    fn render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        albedo_descriptor_set: Option<vk::DescriptorSet>,
        shader_override: Option<&str>,
    ) -> Result<()> {
        self.model_render(
            mesh,
            push_constants,
            model_push_constants,
            albedo_descriptor_set,
            shader_override,
            false,
        )
    }

    fn wireframe_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        albedo_descriptor_set: Option<vk::DescriptorSet>,
        shader_override: Option<&str>,
    ) -> Result<()> {
        self.model_render(
            mesh,
            push_constants,
            model_push_constants,
            albedo_descriptor_set,
            shader_override,
            true,
        )
    }

    fn collider_debug_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
    ) -> Result<()> {
        self.collider_debug_render(mesh, push_constants, model_push_constants)
    }

    fn skybox_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        sky_descriptor_set: vk::DescriptorSet,
        additive: bool,
    ) -> Result<()> {
        self.skybox_render(
            mesh,
            push_constants,
            model_push_constants,
            sky_descriptor_set,
            additive,
        )
    }

    fn voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
        wireframe: bool,
    ) -> Result<()> {
        self.voxel_render(mesh, atlas, push_constants, voxel_push_constants, wireframe)
    }

    fn water_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
    ) -> Result<()> {
        self.water_render(mesh, atlas, push_constants, voxel_push_constants)
    }

    fn begin_ui(&mut self) {
        self.begin_ui()
    }

    fn end_ui(&mut self) -> Result<()> {
        self.end_ui()
    }

    fn handle_ui_event(&mut self, event: &WindowEvent) -> bool {
        self.handle_ui_event(event)
    }

    fn get_egui_context(&self) -> Context {
        self.get_egui_context()
    }

    fn set_lights(
        &mut self,
        lights: &[GpuLight],
        shadow_data: Option<ShadowData>,
        point_shadow_data: Option<PointShadowData>,
        shadow_distance: f32,
        camera_pos: [f32; 3],
        camera_dir: [f32; 3],
    ) {
        self.set_lights(
            lights,
            shadow_data,
            point_shadow_data,
            shadow_distance,
            camera_pos,
            camera_dir,
        )
    }

    fn rebuild_shadow_map(&mut self, size: u32) -> Result<()> {
        self.rebuild_shadow_map(size)
    }

    fn begin_shadow_pass(
        &mut self,
        cascade_index: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()> {
        self.begin_shadow_pass(cascade_index, bias_constant, bias_slope)
    }

    fn end_shadow_pass(&mut self, cascade_index: usize) -> Result<()> {
        self.end_shadow_pass(cascade_index)
    }

    fn shadow_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowModelPushConstants,
    ) -> Result<()> {
        self.shadow_model_render(mesh, pc)
    }

    fn shadow_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowVoxelPushConstants,
    ) -> Result<()> {
        self.shadow_voxel_render(mesh, pc)
    }

    fn rebuild_point_shadow_map(&mut self, size: u32) -> Result<()> {
        self.rebuild_point_shadow_map(size)
    }

    fn begin_point_shadow_pass(
        &mut self,
        face: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()> {
        self.begin_point_shadow_pass(face, bias_constant, bias_slope)
    }

    fn end_point_shadow_pass(&mut self, face: usize) -> Result<()> {
        self.end_point_shadow_pass(face)
    }

    fn shadow_point_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointModelPushConstants,
    ) -> Result<()> {
        self.shadow_point_model_render(mesh, pc)
    }

    fn shadow_point_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointVoxelPushConstants,
    ) -> Result<()> {
        self.shadow_point_voxel_render(mesh, pc)
    }

    fn reload_shaders(&mut self) -> Result<bool> {
        self.reload_shaders()
    }

    fn resize_viewport(
        &mut self,
        width: u32,
        height: u32,
        aa_amount: AntiAliasingAmount,
    ) -> Result<()> {
        self.resize_viewport(vk::Extent2D { width, height }, aa_amount)
    }

    fn last_fence_wait_ns(&self) -> u64 {
        self.last_fence_wait_ns
    }

    fn get_viewport_texture_id(&self) -> Option<TextureId> {
        Some(self.viewport_texture_id)
    }

    fn get_viewport_extent(&mut self) -> Extent2D {
        self.viewport_extent
    }

    fn update_command_buffer(&mut self) {}

    fn recreate_swapchain(&mut self) {
        if let Err(e) = self.swapchain.resize() {
            eprintln!("Failed to recreate swapchain: {}", e);
        }
    }

    fn resize(&mut self) -> Result<()> {
        self.swapchain.resize()
    }

    fn get_buffer_graveyard(&mut self) -> &mut Vec<(vk::Buffer, vk::DeviceMemory)> {
        &mut self.buffer_graveyard
    }

    fn get_command_pool(&self) -> Result<CommandPool> {
        Ok(self.context.command_pool)
    }

    fn get_aspect(&self) -> f32 {
        self.viewport_extent.width as f32 / self.viewport_extent.height as f32
    }

    fn get_descriptor_pool(&self) -> vk::DescriptorPool {
        self.voxel_descriptor_pool
    }

    fn get_voxel_descriptor_set_layout(&self) -> vk::DescriptorSetLayout {
        self.voxel_descriptor_set_layout
    }
}
