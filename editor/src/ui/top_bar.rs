use anyhow::Result;
use apostasy_core::assets::asset_manager::AssetManager;
use apostasy_core::assets::loaders::worldspace_loader::WorldspaceLoader;
use apostasy_core::egui::{self};
use apostasy_core::objects::resources::input_manager::InputManager;
use apostasy_core::objects::world::World;
use apostasy_core::objects::worldspace_serializer::{load_worldspace, save_worldspace};
use apostasy_core::terrain::persistence::{load_terrain_cells, save_terrain_cells};
use apostasy_core::ui::FontRegistry;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{log, update};
use apostasy_macros::Resource;

use crate::ui::EditorStyle;
use crate::ui::asset_editor::AssetEditorState;
use crate::ui::assets_panel::ObjectWindowState;
use crate::ui::cell_panel::CellSearchState;
use crate::ui::inspector_panel::InspectorPanelState;
use crate::ui::preferences_panel::{EditorPreferences, PreferencesState};
use apostasy_core::ui::ProfilerPanelState;
use crate::ui::shared::WindowLayout;
use crate::ui::viewport_panel::ViewportInfo;

#[derive(Resource, Clone, Default)]
pub struct SaveAsDialog {
    pub open: bool,
    pub name_buf: String,
}

fn terrain_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res/terrain")
}

fn do_save(world: &mut World, name: &str) {
    let scenes_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res/worldspaces");
    let path = scenes_dir.join(format!("{}.yaml", name));
    if let Err(e) = save_worldspace(world, name, "game", &path) {
        apostasy_core::log_warn!("Failed to save scene '{}': {}", name, e);
    } else {
        if let Ok(am) = world.get_resource_mut::<AssetManager>() {
            let _ = am.load_file(&path);
        }
        EditorPreferences::save_last_scene(name);
    }
    if let Err(e) = save_terrain_cells(world, &terrain_dir()) {
        apostasy_core::log_warn!("Failed to save terrain: {}", e);
    }
}

#[update(mode = "editor")]
pub fn top_bar(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let save_pressed = world
        .get_resource::<InputManager>()?
        .is_keybind_active("Save");
    let save_as_pressed = world
        .get_resource::<InputManager>()?
        .is_keybind_active("SaveAs");

    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();
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
    let asset_editor_open = world
        .get_resource::<AssetEditorState>()
        .map(|s| s.open)
        .unwrap_or(false);
    let profiler_open = world
        .get_resource::<ProfilerPanelState>()
        .map(|s| s.open)
        .unwrap_or(false);

    let current_scene = EditorPreferences::load().last_scene;

    let mut toggle_viewport = false;
    let mut toggle_object_window = false;
    let mut toggle_cell = false;
    let mut toggle_inspector = false;
    let mut toggle_asset_editor = false;
    let mut toggle_preferences = false;
    let mut toggle_profiler = false;
    let mut do_save_current = false;
    let mut open_save_as = false;
    let mut load_scene_name: Option<String> = None;

    if save_as_pressed {
        open_save_as = true;
    }

    if save_pressed {
        log!("Saving Current");
        do_save_current = true;
    }

    let scene_names: Vec<String> = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .map(|l| {
            l.registry
                .read()
                .ok()
                .map(|r| r.worldspaces.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let save_as_open = world
        .get_resource::<SaveAsDialog>()
        .map(|d| d.open)
        .unwrap_or(false);

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
                                ui.menu_button("Files", |ui| {
                                    ui.set_min_width(160.0);

                                    let save_label = if current_scene.is_empty() {
                                        "Save".to_string()
                                    } else {
                                        format!("Save  \"{}\"", current_scene)
                                    };
                                    if ui
                                        .add_enabled(!save_as_open, egui::Button::new(save_label))
                                        .clicked()
                                    {
                                        do_save_current = true;
                                        ui.close();
                                    }
                                    if ui
                                        .add_enabled(!save_as_open, egui::Button::new("Save As..."))
                                        .clicked()
                                    {
                                        open_save_as = true;
                                        ui.close();
                                    }

                                    ui.separator();

                                    ui.menu_button("Load Scene", |ui| {
                                        if scene_names.is_empty() {
                                            ui.label("No saved scenes");
                                        }
                                        for name in &scene_names {
                                            if ui.button(name).clicked() {
                                                load_scene_name = Some(name.clone());
                                                ui.close();
                                            }
                                        }
                                    });
                                });
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
                                    if ui
                                        .selectable_label(asset_editor_open, "Asset Editor")
                                        .clicked()
                                    {
                                        toggle_asset_editor = true;
                                        ui.close();
                                    }
                                    if ui
                                        .selectable_label(profiler_open, "Profiler")
                                        .clicked()
                                    {
                                        toggle_profiler = true;
                                        ui.close();
                                    }
                                });
                            });
                            ui.separator();
                        });
                    });
                });
        });

    // Save As dialog
    let mut confirm_save_as: Option<String> = None;
    let mut close_save_as = false;

    if save_as_open {
        let mut name_buf = world
            .get_resource::<SaveAsDialog>()
            .map(|d| d.name_buf.clone())
            .unwrap_or_default();

        let screen_rect = ctx.viewport_rect();
        let win_w = 320.0_f32;
        let win_h = 110.0_f32;
        let pos = egui::pos2(
            (screen_rect.width() - win_w) * 0.5,
            (screen_rect.height() - win_h) * 0.5,
        );

        egui::Window::new("Save Scene As")
            .fixed_pos(pos)
            .fixed_size(egui::vec2(win_w, win_h))
            .collapsible(false)
            .resizable(false)
            .show(&ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Scene name:");
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut name_buf)
                            .desired_width(180.0)
                            .hint_text("untitled"),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !name_buf.trim().is_empty() {
                            confirm_save_as = Some(name_buf.trim().to_string());
                        }
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let can_save = !name_buf.trim().is_empty();
                    if ui
                        .add_enabled(can_save, egui::Button::new("Save"))
                        .clicked()
                    {
                        confirm_save_as = Some(name_buf.trim().to_string());
                    }
                    if ui.button("Cancel").clicked() {
                        close_save_as = true;
                    }
                });

                if let Ok(d) = world.get_resource_mut::<SaveAsDialog>() {
                    d.name_buf = name_buf;
                }
            });
    }

    // Apply all deferred state mutations
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
    if toggle_asset_editor {
        if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
            s.open = !s.open;
        }
    }
    if toggle_viewport
        || toggle_object_window
        || toggle_cell
        || toggle_inspector
        || toggle_asset_editor
    {
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
        let asset_editor_open = world
            .get_resource::<AssetEditorState>()
            .map(|s| s.open)
            .unwrap_or(false);
        if let Ok(layout) = world.get_resource_mut::<WindowLayout>() {
            layout.viewport_open = viewport_open;
            layout.object_window_open = object_window_open;
            layout.cell_open = cell_open;
            layout.inspector_visible = inspector_visible;
            layout.asset_editor_open = asset_editor_open;
            layout.dirty = true;
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
    if toggle_profiler {
        if let Ok(s) = world.get_resource_mut::<ProfilerPanelState>() {
            s.open = !s.open;
        } else {
            world.insert_resource(ProfilerPanelState { open: true });
        }
    }

    if do_save_current {
        let name = if current_scene.is_empty() {
            "default".to_string()
        } else {
            current_scene.clone()
        };
        do_save(world, &name);
    }

    if open_save_as {
        let name_buf = if current_scene.is_empty() {
            String::new()
        } else {
            current_scene.clone()
        };
        world.insert_resource(SaveAsDialog {
            open: true,
            name_buf,
        });
    }

    if let Some(name) = confirm_save_as {
        do_save(world, &name);
        world.insert_resource(SaveAsDialog::default());
    }

    if close_save_as {
        world.insert_resource(SaveAsDialog::default());
    }

    if let Some(ref scene_name) = load_scene_name {
        let scene_value: Option<serde_yaml::Value> = world
            .get_resource::<AssetManager>()
            .ok()
            .and_then(|am| am.get_loader::<WorldspaceLoader>())
            .and_then(|l| {
                l.registry
                    .read()
                    .ok()?
                    .worldspaces
                    .get(scene_name.as_str())
                    .cloned()
            });

        if let Some(value) = scene_value {
            EditorPreferences::save_last_scene(scene_name);
            let _ = load_worldspace(world, &value, &["EditorCamera"]);
            let _ = load_terrain_cells(world, &terrain_dir());
            if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
                s.selected_obj = None;
            }
        }
    }

    Ok(())
}
