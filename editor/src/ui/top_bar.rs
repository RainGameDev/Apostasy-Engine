use anyhow::Result;
use apostasy_core::egui::{self};
use apostasy_core::objects::world::World;
use apostasy_core::ui::DRAG_SIZE;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::update;

use crate::ui::assets_panel::ObjectWindowState;
use crate::ui::cell_panel::CellSearchState;
use crate::ui::inspector_panel::InspectorPanelState;
use crate::ui::viewport_panel::ViewportInfo;

#[update(mode = "editor")]
pub fn top_bar(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
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

    Ok(())
}
