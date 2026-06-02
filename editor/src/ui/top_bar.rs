use anyhow::Result;
use apostasy_core::egui::{self};
use apostasy_core::objects::world::World;
use apostasy_core::ui::DRAG_SIZE;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::update;

#[update]
pub fn top_bar(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let screen_width = ctx.viewport_rect().width();

    egui::Area::new(egui::Id::new("top_bar"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(screen_width, 40.0));
                    let button_height = DRAG_SIZE.y;
                    let offset = (40.0 - button_height) / 6.0;
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.separator();
                            ui.add_space(offset);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(egui::Button::new("Files").min_size(DRAG_SIZE))
                                    .clicked()
                                {}
                                ui.add_space(8.0);
                                if ui
                                    .add(egui::Button::new("Edit").min_size(DRAG_SIZE))
                                    .clicked()
                                {}
                                ui.add_space(8.0);
                                if ui
                                    .add(egui::Button::new("View").min_size(DRAG_SIZE))
                                    .clicked()
                                {}
                            });
                            ui.separator();
                        });
                    });
                });
        });
    Ok(())
}
