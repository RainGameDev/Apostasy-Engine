use anyhow::Result;
use apostasy_core::egui::{Color32, Window};
use apostasy_core::objects::world::World;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{egui, update};

#[update]
pub fn scenes(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    Window::new("Scenes")
        .default_pos([100.0, 100.0])
        .movable(true)
        .title_bar(false)
        .show(&ctx, |ui| {
            ui.vertical(|ui| {
                ui.colored_label(Color32::from_rgb(100, 100, 150), "Scenes");
                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        // Scene items
                        if ui.selectable_label(true, "Main Scene").clicked() {
                            // Load scene
                        }
                        if ui.selectable_label(false, "Editor Scene").clicked() {
                            // Load scene
                        }

                        ui.separator();
                        if ui.button("+ New Scene").clicked() {
                            // Create new scene
                        }
                    });
            });
        });

    Ok(())
}
