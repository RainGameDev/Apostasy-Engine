use apostasy_core::{anyhow::Result, egui, update, ui::ui_context::EguiContext, objects::world::World};
use crate::world::loading_state::LoadingState;

#[update]
pub fn render_loading_screen(world: &mut World) -> Result<()> {
    let loading_state = world.get_resource::<LoadingState>()?;

    // Only show loading screen if loading is not complete
    if loading_state.is_complete {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let progress = loading_state.progress();
    let chunks_loaded = loading_state.chunks_loaded;
    let total_chunks = loading_state.total_chunks_expected;

    egui::Area::new(egui::Id::new("loading_screen"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.vertical(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Loading World...")
                        .size(32.0)
                        .color(egui::Color32::WHITE),
                );
                ui.add_space(30.0);

                // Progress bar
                ui.add(
                    egui::ProgressBar::new(progress as f32)
                        .show_percentage()
                        .desired_width(300.0),
                );

                ui.add_space(15.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Loading chunks: {}/{}",
                        chunks_loaded, total_chunks
                    ))
                    .size(16.0)
                    .color(egui::Color32::LIGHT_GRAY),
                );

                ui.add_space(15.0);
                ui.label(
                    egui::RichText::new(format!("{:.1}%", progress * 100.0))
                        .size(20.0)
                        .color(egui::Color32::YELLOW),
                );

                ui.add_space(20.0);
            });
        });

    Ok(())
}
