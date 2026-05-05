use apostasy_core::{
    anyhow::Result, egui, log, noise::Worley, objects::world::World, start, states::ShouldExit,
    ui::ui_context::EguiContext, update,
};

use crate::states::{GetNewSeed, HasInitGeneration, IsPaused};

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

    egui::Window::new("Main Menu")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.vertical(|ui| {
                if ui.button("New Game").clicked() {
                    world.remove_resource::<IsPaused>();
                    world.insert_resource(HasInitGeneration);
                    world.insert_resource(GetNewSeed);
                }
                ui.button("Settings");
                if ui.button("Exit").clicked() {
                    log!("Exiting via main menu");
                    world.insert_resource(ShouldExit);
                }
            });
        });

    Ok(())
}

#[start]
pub fn main_menu_start(world: &mut World) -> Result<()> {
    world.insert_resource(IsPaused);

    Ok(())
}
