use anyhow::Result;
use egui::{Color32, Context, FontDefinitions, FontFamily};
use egui_ash_renderer::{DynamicRendering, Options, Renderer};
use egui_winit::State;
use std::sync::Arc;
use winit::window::Window;

use crate::rendering::vulkan::{
    rendering_context::VulkanRenderingContext, swapchain::VulkanSwapchain,
};

pub mod ui_context;
use std::sync::Mutex;

#[derive(Clone)]
pub struct UIRenderer {
    pub state: Arc<Mutex<State>>,
    pub renderer: Arc<Mutex<Renderer>>,
    pub context: Context,
    pub window: Arc<Window>,
}

impl UIRenderer {
    pub fn new(
        context: VulkanRenderingContext,
        swapchain: &VulkanSwapchain,
        window: Arc<Window>,
    ) -> Result<Self> {
        let renderer = Renderer::with_default_allocator(
            &context.instance,
            context.physical_device.handle,
            context.device.clone(),
            DynamicRendering {
                color_attachment_format: swapchain.format,
                depth_attachment_format: Some(swapchain.depth_format),
            },
            Options {
                srgb_framebuffer: true,
                ..Default::default()
            },
        )?;
        let mut fonts = FontDefinitions::default();

        fonts.font_data.insert(
            "monocraft".to_owned(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../res/fonts/monocraft.ttc"
            ))),
        );

        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "monocraft".to_owned());

        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "monocraft".to_owned());

        fonts.families.insert(
            FontFamily::Name("monocraft".into()),
            vec!["monocraft".to_owned()],
        );

        let context = Context::default();
        context.set_fonts(fonts);

        // TODO: make style
        // context.set_style(style);

        let state = State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        );

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            renderer: Arc::new(Mutex::new(renderer)),
            context,
            window,
        })
    }
}

pub const DARK_BG: Color32 = Color32::from_rgb(18, 18, 18);
pub const PANEL_BG: Color32 = Color32::from_rgb(24, 24, 24);
pub const HEADER_BG: Color32 = Color32::from_rgb(30, 30, 30);
pub const ROW_ALT: Color32 = Color32::from_rgb(28, 28, 28);
pub const DIV_COL: Color32 = Color32::from_rgb(60, 60, 60);
pub const TEXT_COL: Color32 = Color32::WHITE;
pub const DIM_COL: Color32 = Color32::from_rgb(170, 170, 170);
pub const SEL_BG: Color32 = Color32::from_rgb(40, 80, 140);
pub const HOVER_BG: Color32 = Color32::from_rgb(38, 38, 50);
pub const DRAG_SIZE: egui::Vec2 = egui::vec2(60.0, 20.0);
pub const LABEL_WIDTH: f32 = 100.0;
