use apostasy_core::{
    anyhow::Result,
    cgmath::Vector3,
    egui, log,
    objects::world::World,
    rand::{RngExt, rng},
    start,
    states::ShouldExit,
    ui::ui_context::EguiContext,
    update,
};
use apostasy_macros::Resource;

use crate::{
    states::{GetNewSeed, HasInitGeneration, IsPaused},
    world::chunk_loader::ChunkLoader,
};

const SPLASH_TEXTS: [&str; 7] = [
    "Now with 100% more Rust",
    "It's been oxidized",
    "Why would you do that to the gods :<",
    "Azomuth was pretty cool guys",
    "Also try Voxel Eras",
    "Also try Minecraft",
    "Also try Morrowind",
];

#[derive(Resource, Clone)]
pub struct SelectedSplash(String);

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

    egui::Area::new("backdrop".into())
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Background)
        .show(&ctx, |ui| {
            let screen = ui.ctx().screen_rect();

            ui.painter().rect_filled(
                screen,
                0.0,
                egui::Color32::from_rgb(10, 14, 28), // deep night sky
            );

            let dirt_h = 80.0;
            let dirt_rect = egui::Rect::from_min_size(
                egui::pos2(screen.left(), screen.bottom() - dirt_h),
                egui::vec2(screen.width(), dirt_h),
            );
            ui.painter()
                .rect_filled(dirt_rect, 0.0, egui::Color32::from_rgb(82, 54, 30));

            let grass_rect = egui::Rect::from_min_size(
                egui::pos2(screen.left(), screen.bottom() - dirt_h),
                egui::vec2(screen.width(), 8.0),
            );
            ui.painter()
                .rect_filled(grass_rect, 0.0, egui::Color32::from_rgb(86, 130, 50));
        });

    egui::Window::new("main_menu")
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

                // Subtitle splash text
                if !world.get_resource::<SelectedSplash>().is_ok() {
                    let mut rand = rng();
                    let rand_splash = SPLASH_TEXTS[rand.random_range(0..SPLASH_TEXTS.len())];
                    world.insert_resource(SelectedSplash(rand_splash.to_string()));
                } else {
                    let splash_text = world
                        .get_resource::<SelectedSplash>()
                        .unwrap()
                        .0
                        .to_string();
                    let splash_font = egui::FontId::new(13.0, egui::FontFamily::Monospace);
                    let splash_galley = ui.painter().layout_no_wrap(
                        splash_text.to_owned(),
                        splash_font.clone(),
                        egui::Color32::WHITE,
                    );
                    let splash_size = splash_galley.size();
                    let (splash_rect, _) =
                        ui.allocate_exact_size(splash_size, egui::Sense::hover());
                    ui.painter().text(
                        splash_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        splash_text,
                        splash_font,
                        egui::Color32::from_rgb(255, 255, 60),
                    );
                }

                ui.add_space(20.0);

                // Buttons
                if ui.button("Singleplayer").clicked() {
                    world.remove_resource::<IsPaused>();
                    world.insert_resource(HasInitGeneration);
                    world.insert_resource(GetNewSeed);

                    world
                        .get_resource_mut::<ChunkLoader>()
                        .unwrap()
                        .last_chunk_position = Vector3::new(i32::MAX, i32::MAX, i32::MAX);
                }

                ui.add_space(6.0);
                ui.button("Settings");

                ui.add_space(6.0);
                if ui.button("Quit Game").clicked() {
                    log!("Exiting via main menu");
                    world.insert_resource(ShouldExit);
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
    // world.insert_resource(IsPaused);
    Ok(())
}
