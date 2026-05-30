use anyhow::Result;
use apostasy_core::{
    egui::{Color32, ScrollArea, Window},
    objects::world::World,
    ui::ui_context::EguiContext,
    update,
};

#[update]
pub fn assets(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    Window::new("Assets")
        .default_pos([100.0, 100.0])
        .movable(true)
        .title_bar(false)
        .show(&ctx, |ui| {
            ui.vertical(|ui| {
                ui.colored_label(Color32::WHITE, "Assets");
                ui.separator();

                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        // Asset items
                        ui.label("• Shader 1");
                        ui.label("• Model 1");
                        ui.label("• Texture 1");
                        ui.label("• Font 1");

                        // Add more assets as they're loaded
                    });
            });
        });

    Ok(())
}
