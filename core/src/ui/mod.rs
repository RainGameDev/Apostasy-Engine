use anyhow::Result;
use ash::vk::DescriptorSet;
use egui::{Context, FontDefinitions, FontFamily};
use egui_ash_renderer::{DynamicRendering, Options, Renderer};
use egui_winit::State;
use std::sync::Arc;
use winit::window::Window;

use crate::rendering::vulkan::{
    rendering_context::VulkanRenderingContext, swapchain::VulkanSwapchain,
};

pub mod ui_context;

pub struct UIRenderer {
    pub state: State,
    pub renderer: Renderer,
    pub context: Context,
    pub window: Arc<Window>,
}

impl UIRenderer {
    pub fn new(
        context: VulkanRenderingContext,
        swapchain: &VulkanSwapchain,
        window: Arc<Window>,
    ) -> Result<Self> {
        let mut renderer = Renderer::with_default_allocator(
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

        renderer.add_user_texture(DescriptorSet::default());
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
            state,
            renderer,
            context,
            window,
        })
    }
}
