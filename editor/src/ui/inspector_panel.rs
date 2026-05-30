use anyhow::Result;
use apostasy_core::{
    egui::{Color32, ScrollArea, Window},
    objects::world::World,
    ui::ui_context::EguiContext,
    update,
};

#[update]
pub fn inspector(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    Window::new("Inspector")
        .default_pos([100.0, 100.0])
        .movable(true)
        .title_bar(false)
        .show(&ctx, |ui| {
            ui.colored_label(Color32::from_rgb(100, 200, 100), "Inspector");
            ui.separator();

            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.label("No object selected");

                    ui.separator();
                    ui.group(|ui| {
                        ui.label("Transform");
                        ui.label("  Position: (0, 0, 0)");
                        ui.label("  Rotation: (0, 0, 0)");
                        ui.label("  Scale: (1, 1, 1)");
                    });
                });
        });

    Ok(())
}
