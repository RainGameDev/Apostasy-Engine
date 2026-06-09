use anyhow::Result;
use apostasy_core::egui::{self};
use apostasy_core::objects::world::World;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::ui::FontRegistry;
use apostasy_core::update;

use crate::ui::assets_panel::ObjectWindowState;
use crate::ui::cell_panel::CellSearchState;
use crate::ui::inspector_panel::InspectorPanelState;
use crate::ui::preferences_panel::PreferencesState;
use crate::ui::viewport_panel::ViewportInfo;
use crate::ui::EditorStyle;

#[update(mode = "editor")]
pub fn top_bar(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    let style = world.get_resource::<EditorStyle>().cloned().unwrap_or_default();
    style.apply_to_context(&ctx);
    if let Ok(reg) = world.get_resource_mut::<FontRegistry>() {
        reg.apply_if_needed(&ctx);
    }

    let screen_width = ctx.viewport_rect().width();

    let viewport_open = world
        .get_resource::<ViewportInfo>()
        .map(|s| s.open)
        .unwrap_or(true);
    let object_window_open = world
        .get_resource::<ObjectWindowState>()
        .map(|s| s.open)
        .unwrap_or(true);
    let cell_open = world
        .get_resource::<CellSearchState>()
        .map(|s| s.open)
        .unwrap_or(true);
    let inspector_visible = world
        .get_resource::<InspectorPanelState>()
        .map(|s| s.visible)
        .unwrap_or(false);

    let mut toggle_viewport = false;
    let mut toggle_object_window = false;
    let mut toggle_cell = false;
    let mut toggle_inspector = false;
    let mut toggle_preferences = false;

    egui::Area::new(egui::Id::new("top_bar"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(ui.visuals().panel_fill)
                .show(ui, |ui| {
                    let bar_h = style.row_height() * 2.0;
                    ui.set_min_size(egui::vec2(screen_width, bar_h));
                    let offset = bar_h * 0.1;
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.separator();
                            ui.add_space(offset);
                            ui.horizontal(|ui| {
                                if ui.button("Files").clicked() {}
                                ui.add_space(8.0);
                                ui.menu_button("Edit", |ui| {
                                    ui.set_min_width(120.0);
                                    if ui.button("Preferences").clicked() {
                                        toggle_preferences = true;
                                        ui.close();
                                    }
                                });
                                ui.add_space(8.0);
                                ui.menu_button("View", |ui| {
                                    if ui.selectable_label(viewport_open, "Viewport").clicked() {
                                        toggle_viewport = true;
                                        ui.close();
                                    }
                                    if ui
                                        .selectable_label(object_window_open, "Object Window")
                                        .clicked()
                                    {
                                        toggle_object_window = true;
                                        ui.close();
                                    }
                                    if ui.selectable_label(cell_open, "Cell View").clicked() {
                                        toggle_cell = true;
                                        ui.close();
                                    }
                                    if ui
                                        .selectable_label(inspector_visible, "Inspector")
                                        .clicked()
                                    {
                                        toggle_inspector = true;
                                        ui.close();
                                    }
                                });
                            });
                            ui.separator();
                        });
                    });
                });
        });

    if toggle_viewport {
        if let Ok(s) = world.get_resource_mut::<ViewportInfo>() {
            s.open = !s.open;
        }
    }
    if toggle_object_window {
        if let Ok(s) = world.get_resource_mut::<ObjectWindowState>() {
            s.open = !s.open;
        }
    }
    if toggle_cell {
        if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
            s.open = !s.open;
        }
    }
    if toggle_inspector {
        if let Ok(s) = world.get_resource_mut::<InspectorPanelState>() {
            s.visible = !s.visible;
        }
    }
    if toggle_preferences {
        if let Ok(s) = world.get_resource_mut::<PreferencesState>() {
            s.open = !s.open;
        } else {
            world.insert_resource(PreferencesState {
                open: true,
                ..Default::default()
            });
        }
    }

    Ok(())
}
