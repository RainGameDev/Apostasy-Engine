use anyhow::Result;
use egui::{Context, TextureId};
use epaint::ImageDelta;
use winit::event::WindowEvent;

use crate::rendering::vulkan::VulkanRenderer;

impl VulkanRenderer {
    pub(crate) fn begin_ui(&mut self) {
        let mut state = self.ui_renderer.state.lock().unwrap();
        let raw_input = state.take_egui_input(&self.ui_renderer.window);
        self.ui_renderer.context.begin_pass(raw_input);
    }

    pub(crate) fn end_ui(&mut self) -> Result<()> {
        let full_output = self.ui_renderer.context.end_pass();
        let mut state = self.ui_renderer.state.lock().unwrap();
        let mut renderer = self.ui_renderer.renderer.lock().unwrap();

        state.handle_platform_output(&self.ui_renderer.window, full_output.platform_output);

        if !self.ui_pending_texture_frees.is_empty() {
            let to_free = std::mem::take(&mut self.ui_pending_texture_frees);
            renderer.free_textures(&to_free)?;
        }

        let pixels_per_point = full_output.pixels_per_point;

        self.ui_cached_primitives = self
            .ui_renderer
            .context
            .tessellate(full_output.shapes, pixels_per_point);

        if !full_output.textures_delta.set.is_empty() {
            let texture_updates: Vec<(TextureId, ImageDelta)> = full_output
                .textures_delta
                .set
                .iter()
                .map(|(id, delta)| (*id, delta.clone()))
                .collect();
            renderer.set_textures(
                self.context.queues[&self.context.queue_families.graphics],
                self.context.command_pool,
                &texture_updates,
            )?;
        }

        renderer.cmd_draw(
            self.frames[self.current_frame].command_buffer,
            self.swapchain.extent,
            pixels_per_point,
            &self.ui_cached_primitives,
        )?;

        // Defer these frees until next frame (see `ui_pending_texture_frees`).
        self.ui_pending_texture_frees = full_output.textures_delta.free;

        Ok(())
    }

    pub(crate) fn handle_ui_event(&mut self, event: &WindowEvent) -> bool {
        let mut state = self.ui_renderer.state.lock().unwrap();
        state
            .on_window_event(&self.ui_renderer.window, event)
            .consumed
    }

    pub(crate) fn get_egui_context(&self) -> Context {
        self.ui_renderer.context.clone()
    }
}
