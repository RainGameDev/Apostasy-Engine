use apostasy_core::{
    anyhow::Result, egui, objects::world::World, ui::ui_context::EguiContext, update,
};

use crate::states::HasInitGeneration;

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    if !world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    egui::Area::new(egui::Id::new("crosshair"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.label(
                egui::RichText::new("+")
                    .size(24.0)
                    .color(egui::Color32::WHITE),
            );
        });

    Ok(())
}
