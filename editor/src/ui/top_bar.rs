use anyhow::Result;
use apostasy_core::egui::{Color32, TopBottomPanel, Window};
use apostasy_core::objects::world::World;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{egui, update};

#[update]
pub fn top_bar(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    TopBottomPanel::top("Hierarchy").show(&ctx, |ui| {
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Files").clicked() {}
            if ui.button("Edit").clicked() {}
            if ui.button("View").clicked() {}
        });
        ui.separator();
    });

    Ok(())
}
