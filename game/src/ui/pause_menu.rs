use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    egui,
    objects::{components::transform::Transform, tags::Player, world::World},
    start,
    ui::ui_context::EguiContext,
    update,
    voxels::chunk::{self, Chunk},
};

use crate::{
    entities::loading_gate::LoadingGate,
    states::{HasInitGeneration, IsPaused},
    world::loading_state::LoadingState,
};
#[update]
pub fn hud(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if !world.get_resource::<IsPaused>().is_ok() {
        return Ok(());
    }

    if !world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

    egui::Window::new("pause_menu")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .frame(egui::Frame {
            fill: egui::Color32::TRANSPARENT,
            ..Default::default()
        })
        .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
        .fixed_size([420.0, 380.0])
        .show(&ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Game title
                ui.add_space(24.0);

                let title_text = "Apostasy";
                let title_font = egui::FontId::new(52.0, egui::FontFamily::Monospace);
                let galley = ui.painter().layout_no_wrap(
                    title_text.to_owned(),
                    title_font.clone(),
                    egui::Color32::WHITE,
                );
                let title_size = galley.size();
                let (title_rect, _) = ui.allocate_exact_size(title_size, egui::Sense::hover());

                // Shadow
                ui.painter().text(
                    title_rect.center() + egui::vec2(4.0, 4.0),
                    egui::Align2::CENTER_CENTER,
                    title_text,
                    title_font.clone(),
                    egui::Color32::from_rgb(80, 40, 0),
                );
                // text
                ui.painter().text(
                    title_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    title_text,
                    title_font,
                    egui::Color32::from_rgb(255, 210, 50),
                );

                ui.add_space(12.0);

                ui.add_space(20.0);

                // Buttons
                if ui.button("Resume").clicked() {
                    world.remove_resource::<IsPaused>();
                }

                ui.add_space(6.0);
                ui.button("Settings");

                ui.add_space(6.0);
                if ui.button("Quit Game").clicked() {
                    world.remove_resource::<HasInitGeneration>();
                    let chunk_ids: Vec<_> = world
                        .get_objects_with_component_with_ids::<Chunk>()
                        .iter()
                        .map(|(id, _)| id.clone())
                        .collect();

                    for id in chunk_ids {
                        world.remove_object(id);
                    }

                    let player = world.get_object_with_tag_mut::<Player>().unwrap();
                    player.add_tag(LoadingGate);
                }

                // version
                ui.add_space(20.0);
                let ver_font = egui::FontId::new(11.0, egui::FontFamily::Monospace);
                ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "v0.1.0 / Apostasy");
            });
        });

    Ok(())
}

#[start]
pub fn main_menu_start(world: &mut World) -> Result<()> {
    world.insert_resource(IsPaused);
    Ok(())
}
