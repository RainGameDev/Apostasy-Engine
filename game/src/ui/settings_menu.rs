use apostasy_core::{
    anyhow::Result,
    egui::{self, Pos2, Slider},
    objects::world::World,
    ui::ui_context::EguiContext,
    update,
};
use apostasy_macros::Resource;

use crate::{states::IsPaused, world::chunk_loader::ChunkLoader};

#[derive(Resource, Clone, Copy, Default)]
pub struct IsSettingsOpen;

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if !world.has_resource::<IsPaused>() {
        return Ok(());
    }
    if !world.has_resource::<IsSettingsOpen>() {
        return Ok(());
    }

    let chunk_loader = world.get_resource_mut::<ChunkLoader>().unwrap();
    let mut chunk_load_radius = chunk_loader.load_radius;
    let mut v_chunk_load_radius = chunk_loader.v_load_radius;

    let screen_rect = ctx.viewport_rect();
    let center = screen_rect.center();
    let pos = Pos2::new(center.x + 150.0, center.y);
    egui::Window::new("settings_menu")
        .resizable(true)
        .movable(true)
        .fixed_size([420.0, 380.0])
        .default_pos(center)
        .show(&ctx, |ui| {
            ui.label("Settings");

            ui.add(Slider::new(&mut chunk_load_radius, 1..=32).text("Render Distance"));
            ui.add(Slider::new(&mut v_chunk_load_radius, 1..=32).text("Vertical Render Distance"));
        });

    chunk_loader.load_radius = chunk_load_radius;
    chunk_loader.v_load_radius = v_chunk_load_radius;

    Ok(())
}
