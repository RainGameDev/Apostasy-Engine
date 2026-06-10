use anyhow::Result;
use apostasy_core::assets::asset_manager::AssetManager;
use apostasy_core::assets::loaders::scene_loader::SceneLoader;
use apostasy_core::egui::{Color32, Pos2, Rect, ScrollArea, Sense, Stroke, Vec2, Window};
use apostasy_core::objects::scene::ObjectId;
use apostasy_core::objects::scene_serializer::load_scene;
use apostasy_core::objects::world::World;
use apostasy_core::objects::{Object, fmt_key};
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{egui, update};
use apostasy_macros::Resource;
use std::sync::Arc;

use crate::ui::assets_panel::paint_clipped;
use crate::ui::inspector_panel::InspectorPanelState;
use crate::ui::preferences_panel::EditorPreferences;
use super::shared::{WindowLayout, save_layout};
use super::EditorStyle;

#[derive(Clone)]
pub struct ObjectRefEntry {
    pub obj_name: String,
    pub id: String,
    pub object_id: ObjectId,
}

#[derive(Clone, Resource)]
pub struct CellSearchState {
    pub open: bool,
    pub obj_filter: String,
    pub obj_entries: Vec<ObjectRefEntry>,
    pub selected_obj: Option<ObjectId>,
    pub clicked_obj: Option<ObjectId>,
    pub copied_obj: Option<Object>,
    pub renaming_obj: Option<ObjectId>,
    pub rename_buf: String,
    pub rename_request_focus: bool,
    // scene rename state
    pub renaming_scene: Option<String>,
    pub scene_rename_buf: String,
    pub scene_rename_focus: bool,
}

impl Default for CellSearchState {
    fn default() -> Self {
        Self {
            open: true,
            obj_filter: String::new(),
            obj_entries: vec![],
            selected_obj: None,
            clicked_obj: None,
            copied_obj: None,
            renaming_obj: None,
            rename_buf: String::new(),
            rename_request_focus: false,
            renaming_scene: None,
            scene_rename_buf: String::new(),
            scene_rename_focus: false,
        }
    }
}

#[allow(deprecated)]
#[update(mode = "editor")]
pub fn cell_search(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let style = world.get_resource::<EditorStyle>().cloned().unwrap_or_default();

    if world.get_resource::<CellSearchState>().is_err() {
        world.insert_resource(CellSearchState::default());
    }

    let obj_entries: Vec<ObjectRefEntry> = world
        .get_all_objects()
        .iter()
        .map(|(id, obj)| ObjectRefEntry {
            obj_name: obj.name.clone(),
            id: fmt_key(*id),
            object_id: *id,
        })
        .collect();
    world.get_resource_mut::<CellSearchState>()?.obj_entries = obj_entries;

    let current_scene = EditorPreferences::load().last_scene;
    let scene_names: Vec<String> = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<SceneLoader>())
        .map(|l| {
            l.registry
                .read()
                .ok()
                .map(|r| {
                    let mut names: Vec<String> = r.scenes.keys().cloned().collect();
                    names.sort();
                    names
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let mut open = world.get_resource::<CellSearchState>()?.open;
    let obj_entries = world.get_resource::<CellSearchState>()?.obj_entries.clone();

    if !open {
        return Ok(());
    }

    let mut pending_selected_obj: Option<ObjectId> =
        world.get_resource::<CellSearchState>()?.selected_obj;
    let mut pending_clicked_obj: Option<ObjectId> =
        world.get_resource::<CellSearchState>()?.clicked_obj;
    let mut pending_obj_filter: String =
        world.get_resource::<CellSearchState>()?.obj_filter.clone();
    let mut pending_delete: Option<ObjectId> = None;
    let mut pending_add = false;
    let mut object_to_copy: Option<Object> = None;
    let mut renaming_id: Option<ObjectId> = world.get_resource::<CellSearchState>()?.renaming_obj;
    let mut rename_buf: String = world.get_resource::<CellSearchState>()?.rename_buf.clone();
    let mut rename_request_focus: bool =
        world.get_resource::<CellSearchState>()?.rename_request_focus;
    let mut pending_rename: Option<(ObjectId, String)> = None;

    let mut renaming_scene: Option<String> =
        world.get_resource::<CellSearchState>()?.renaming_scene.clone();
    let mut scene_rename_buf: String =
        world.get_resource::<CellSearchState>()?.scene_rename_buf.clone();
    let mut scene_rename_focus: bool =
        world.get_resource::<CellSearchState>()?.scene_rename_focus;
    let mut pending_scene_load: Option<String> = None;
    let mut pending_scene_delete: Option<String> = None;
    let mut pending_scene_rename: Option<(String, String)> = None;

    let row_h = style.row_height();
    let header_h = style.header_height();
    let font_hdr = style.font_ui();
    let font_row = style.font_ui();

    let layout = world.get_resource::<WindowLayout>().ok();
    let state = if let Some(layout) = layout {
        layout.cell_search.clone()
    } else {
        return Ok(());
    };

    let pos = state.to_pos();
    let size = state.to_size();

    let window = Window::new("Cell Panel")
        .open(&mut open)
        .default_pos(pos)
        .default_size(size)
        .resizable(true)
        .movable(true);

    let window = window
        .resizable(true)
        .movable(true)
        .frame(style.window_frame(&ctx))
        .show(&ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

            let total_w = ui.available_width();
            let panel_w = (total_w - 8.0) / 2.0;
            let panel_h = ui.available_height();

            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

                    // LEFT: scenes list
                    let left_rect =
                        Rect::from_min_size(ui.cursor().min, Vec2::new(panel_w, panel_h));
                    let mut left = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(left_rect)
                            .layout(egui::Layout::top_down(egui::Align::LEFT)),
                    );
                    left.spacing_mut().item_spacing = Vec2::ZERO;

                    let frame = egui::Frame::new()
                        .fill(style.panel_bg)
                        .stroke(Stroke::new(1.0, style.div_col))
                        .corner_radius(4.0)
                        .inner_margin(4.0)
                        .show(&mut left, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            let avail_w = ui.available_width();

                            // title
                            let (title_rect, _) = ui
                                .allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
                            ui.painter().rect_filled(
                                title_rect,
                                egui::CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 },
                                style.header_bg,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Scenes",
                                font_hdr.clone(),
                                style.text_col,
                            );
                            ui.painter().line_segment(
                                [title_rect.left_bottom(), title_rect.right_bottom()],
                                Stroke::new(1.0, style.div_col),
                            );

                            let table_h = ui.available_height();
                            ScrollArea::vertical()
                                .id_salt("scenes_scroll")
                                .auto_shrink([false; 2])
                                .max_height(table_h)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                                    if scene_names.is_empty() {
                                        let (row_rect, _) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(row_rect, 0.0, style.dark_bg);
                                        ui.painter().text(
                                            Pos2::new(row_rect.left() + 6.0, row_rect.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            "No saved scenes",
                                            font_row.clone(),
                                            style.dim_col,
                                        );
                                    }

                                    for (idx, name) in scene_names.iter().enumerate() {
                                        let is_current = *name == current_scene;
                                        let is_renaming =
                                            renaming_scene.as_deref() == Some(name.as_str());

                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );

                                        let bg = if is_current && !is_renaming {
                                            style.sel_bg
                                        } else if row_resp.hovered() || is_renaming {
                                            style.hover_bg
                                        } else if idx % 2 == 0 {
                                            style.dark_bg
                                        } else {
                                            style.row_alt
                                        };
                                        ui.painter().rect_filled(row_rect, 0.0, bg);

                                        if is_renaming {
                                            let edit_rect = Rect::from_min_size(
                                                Pos2::new(
                                                    row_rect.left() + 2.0,
                                                    row_rect.top() + 1.0,
                                                ),
                                                Vec2::new(avail_w - 4.0, row_h - 2.0),
                                            );
                                            let te =
                                                egui::TextEdit::singleline(&mut scene_rename_buf)
                                                    .font(font_row.clone());
                                            let te_resp = ui.put(edit_rect, te);
                                            if scene_rename_focus {
                                                te_resp.request_focus();
                                                scene_rename_focus = false;
                                            }
                                            let escape =
                                                ui.input(|i| i.key_pressed(egui::Key::Escape));
                                            let enter =
                                                ui.input(|i| i.key_pressed(egui::Key::Enter));
                                            if (te_resp.lost_focus() && !escape) || enter {
                                                let new_name = scene_rename_buf.trim().to_string();
                                                if !new_name.is_empty() && new_name != *name {
                                                    pending_scene_rename =
                                                        Some((name.clone(), new_name));
                                                }
                                                renaming_scene = None;
                                            } else if escape {
                                                renaming_scene = None;
                                            }
                                        } else {
                                            let text_col = if is_current {
                                                style.text_col
                                            } else {
                                                style.dim_col
                                            };
                                            paint_clipped(
                                                ui,
                                                Pos2::new(row_rect.left() + 6.0, row_rect.center().y),
                                                avail_w - 10.0,
                                                name,
                                                font_row.clone(),
                                                text_col,
                                            );
                                        }

                                        row_resp.context_menu(|ui| {
                                            ui.set_min_width(120.0);
                                            if ui.button("Load").clicked() {
                                                pending_scene_load = Some(name.clone());
                                                ui.close();
                                            }
                                            if ui.button("Rename").clicked() {
                                                renaming_scene = Some(name.clone());
                                                scene_rename_buf = name.clone();
                                                scene_rename_focus = true;
                                                ui.close();
                                            }
                                            ui.separator();
                                            if ui.button("Delete").clicked() {
                                                pending_scene_delete = Some(name.clone());
                                                ui.close();
                                            }
                                        });

                                        ui.painter().line_segment(
                                            [row_rect.left_bottom(), row_rect.right_bottom()],
                                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                        );
                                    }

                                    // filler rows
                                    let rows_drawn = scene_names.len();
                                    let remaining =
                                        (ui.available_height() / row_h).ceil() as usize;
                                    for i in 0..remaining {
                                        let idx = rows_drawn + i;
                                        let bg = if idx.is_multiple_of(2) {
                                            style.dark_bg
                                        } else {
                                            style.row_alt
                                        };
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        row_resp.context_menu(|ui| {
                                            ui.set_min_width(120.0);
                                        });
                                        ui.painter().rect_filled(row_rect, 0.0, bg);
                                        ui.painter().line_segment(
                                            [row_rect.left_bottom(), row_rect.right_bottom()],
                                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                        );
                                    }
                                });
                        });

                    let gap = total_w - frame.response.rect.size().x;
                    ui.add_space(gap);

                    // RIGHT: object ref list
                    let right_rect =
                        Rect::from_min_size(ui.cursor().min, Vec2::new(panel_w, panel_h));
                    let mut right = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(right_rect)
                            .layout(egui::Layout::top_down(egui::Align::LEFT)),
                    );
                    right.spacing_mut().item_spacing = Vec2::ZERO;

                    egui::Frame::new()
                        .fill(style.panel_bg)
                        .stroke(Stroke::new(1.0, style.div_col))
                        .corner_radius(4.0)
                        .inner_margin(4.0)
                        .show(&mut right, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            let avail_w = ui.available_width();

                            // title
                            let (title_rect, _) = ui
                                .allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
                            ui.painter().rect_filled(
                                title_rect,
                                egui::CornerRadius { nw: 4, ne: 4, sw: 4, se: 4 },
                                style.header_bg,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Object Search",
                                font_hdr.clone(),
                                style.text_col,
                            );
                            ui.painter().line_segment(
                                [title_rect.left_bottom(), title_rect.right_bottom()],
                                Stroke::new(1.0, style.div_col),
                            );

                            // search box
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.add_sized(
                                    Vec2::new(avail_w - 8.0, row_h),
                                    egui::TextEdit::singleline(&mut pending_obj_filter)
                                        .hint_text("Placeholder..."),
                                )
                                .on_hover_text(concat!(
                                    "id:         - filter by ID\n",
                                    "name:       - filter by name\n",
                                    "(no prefix) - filter by name",
                                ));
                            });
                            ui.add_space(4.0);

                            ui.painter().line_segment(
                                [
                                    Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
                                    Pos2::new(ui.cursor().min.x + avail_w, ui.cursor().min.y),
                                ],
                                Stroke::new(1.0, style.div_col),
                            );

                            // column widths
                            let name_w = avail_w * 0.55;
                            let id_w = avail_w - name_w;

                            // column headers
                            let (hdr_rect, _) = ui
                                .allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
                            ui.painter().rect_filled(hdr_rect, 0.0, style.header_bg);
                            for (label, offset) in [("Obj Name", 0.0_f32), ("Id", name_w)] {
                                ui.painter().text(
                                    Pos2::new(
                                        hdr_rect.left() + offset + 6.0,
                                        hdr_rect.center().y,
                                    ),
                                    egui::Align2::LEFT_CENTER,
                                    label,
                                    font_hdr.clone(),
                                    style.text_col,
                                );
                            }
                            ui.painter().line_segment(
                                [hdr_rect.left_bottom(), hdr_rect.right_bottom()],
                                Stroke::new(1.0, style.div_col),
                            );
                            ui.painter().line_segment(
                                [
                                    Pos2::new(hdr_rect.left() + name_w, hdr_rect.top()),
                                    Pos2::new(hdr_rect.left() + name_w, hdr_rect.bottom()),
                                ],
                                Stroke::new(1.0, style.div_col),
                            );

                            // parse filter
                            let filter_splits =
                                pending_obj_filter.split(':').collect::<Vec<&str>>();
                            let (filter_type, filter_value) = if filter_splits.len() > 1 {
                                (filter_splits[0].to_string(), filter_splits[1].to_string())
                            } else {
                                (String::new(), filter_splits[0].to_string())
                            };

                            let filtered: Vec<&ObjectRefEntry> = obj_entries
                                .iter()
                                .filter(|e| {
                                    if filter_value.trim().is_empty() {
                                        return true;
                                    }
                                    let val = filter_value.trim().to_lowercase();
                                    match filter_type.trim().to_lowercase().as_str() {
                                        "id" => e.id.to_lowercase().contains(&val),
                                        "name" => e.obj_name.to_lowercase().contains(&val),
                                        _ => e.obj_name.to_lowercase().contains(&val),
                                    }
                                })
                                .collect();

                            if ui.input(|i| i.key_pressed(egui::Key::F2)) {
                                if let Some(id) = pending_selected_obj {
                                    if let Some(entry) =
                                        obj_entries.iter().find(|e| e.object_id == id)
                                    {
                                        renaming_id = Some(id);
                                        rename_buf = entry.obj_name.clone();
                                        rename_request_focus = true;
                                    }
                                }
                            }

                            let table_h = ui.available_height();
                            ScrollArea::vertical()
                                .id_salt("obj_scroll")
                                .auto_shrink([false; 2])
                                .max_height(table_h)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    for (idx, entry) in filtered.iter().enumerate() {
                                        let is_sel = pending_selected_obj == Some(entry.object_id);
                                        let is_clicked =
                                            pending_clicked_obj == Some(entry.object_id);
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        if row_resp.double_clicked() {
                                            renaming_id = Some(entry.object_id);
                                            rename_buf = entry.obj_name.clone();
                                            rename_request_focus = true;
                                        }
                                        if row_resp.clicked() {
                                            pending_selected_obj = Some(entry.object_id);
                                            pending_clicked_obj = Some(entry.object_id);
                                        }

                                        row_resp.context_menu(|ui| {
                                            if ui.button("Rename").clicked() {
                                                renaming_id = Some(entry.object_id);
                                                rename_buf = entry.obj_name.clone();
                                                rename_request_focus = true;
                                                ui.close();
                                            }
                                            if ui.button("Teleport to Object").clicked() {
                                                ui.close();
                                            }
                                            if ui.button("Delete Object").clicked() {
                                                pending_delete = Some(entry.object_id);
                                                ui.close();
                                            }
                                            if ui.button("Add new Object").clicked() {
                                                pending_add = true;
                                                ui.close();
                                            }
                                            if ui.button("Inspect").clicked() {
                                                pending_selected_obj = Some(entry.object_id);
                                                if let Ok(inspector_state) =
                                                    world.get_resource_mut::<InspectorPanelState>()
                                                {
                                                    inspector_state.visible = true;
                                                }
                                                ui.close();
                                            }

                                            ui.separator();
                                            if ui.button("Copy ID").clicked() {
                                                ui.copy_text(entry.id.clone());
                                                ui.close();
                                            }
                                            if ui.button("Copy Object").clicked() {
                                                object_to_copy = Some(
                                                    world
                                                        .get_object(entry.object_id)
                                                        .unwrap()
                                                        .clone(),
                                                );
                                                ui.close();
                                            }
                                            if ui.button("Cut Object").clicked() {
                                                object_to_copy = Some(
                                                    world
                                                        .get_object(entry.object_id)
                                                        .unwrap()
                                                        .clone(),
                                                );
                                                pending_delete = Some(entry.object_id);
                                                ui.close();
                                            }
                                            if ui.button("Paste Object").clicked() {
                                                let s = world
                                                    .get_resource::<CellSearchState>()
                                                    .unwrap();
                                                if let Some(obj) = s.copied_obj.clone() {
                                                    world.add_object(obj);
                                                }
                                                ui.close();
                                            }
                                        });

                                        let bg = if is_sel {
                                            style.sel_bg
                                        } else if row_resp.hovered() {
                                            style.hover_bg
                                        } else if is_clicked {
                                            style.hover_bg
                                        } else if idx % 2 == 0 {
                                            style.dark_bg
                                        } else {
                                            style.row_alt
                                        };
                                        ui.painter().rect_filled(row_rect, 0.0, bg);

                                        let rl = row_rect.left();
                                        let cy = row_rect.center().y;
                                        if renaming_id == Some(entry.object_id) {
                                            let name_rect = Rect::from_min_size(
                                                Pos2::new(rl + 2.0, row_rect.top() + 1.0),
                                                Vec2::new(name_w - 4.0, row_h - 2.0),
                                            );
                                            let te = egui::TextEdit::singleline(&mut rename_buf)
                                                .font(font_row.clone());
                                            let te_resp = ui.put(name_rect, te);
                                            if rename_request_focus {
                                                te_resp.request_focus();
                                                rename_request_focus = false;
                                            }
                                            let escape =
                                                ui.input(|i| i.key_pressed(egui::Key::Escape));
                                            let enter =
                                                ui.input(|i| i.key_pressed(egui::Key::Enter));
                                            if (te_resp.lost_focus() && !escape) || enter {
                                                pending_rename =
                                                    Some((entry.object_id, rename_buf.clone()));
                                                renaming_id = None;
                                            } else if escape {
                                                renaming_id = None;
                                            }
                                        } else {
                                            paint_clipped(
                                                ui,
                                                Pos2::new(rl + 6.0, cy),
                                                name_w - 10.0,
                                                &entry.obj_name,
                                                font_row.clone(),
                                                style.dim_col,
                                            );
                                        }
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + name_w + 6.0, cy),
                                            id_w - 10.0,
                                            &entry.id,
                                            font_row.clone(),
                                            style.dim_col,
                                        );

                                        ui.painter().line_segment(
                                            [row_rect.left_bottom(), row_rect.right_bottom()],
                                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                        );
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(rl + name_w, row_rect.top()),
                                                Pos2::new(rl + name_w, row_rect.bottom()),
                                            ],
                                            Stroke::new(1.0, style.div_col),
                                        );
                                    }

                                    // filler rows
                                    let rows_drawn = filtered.len();
                                    let remaining_rows =
                                        (ui.available_height() / row_h).ceil() as usize;
                                    for i in 0..remaining_rows {
                                        let idx = rows_drawn + i;
                                        let bg = if idx.is_multiple_of(2) {
                                            style.dark_bg
                                        } else {
                                            style.row_alt
                                        };
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        row_resp.context_menu(|ui| {
                                            if ui.button("Add new Object").clicked() {
                                                pending_add = true;
                                                ui.close();
                                            }
                                            if ui.button("Paste Object").clicked() {
                                                let s = world
                                                    .get_resource::<CellSearchState>()
                                                    .unwrap();
                                                if let Some(obj) = s.copied_obj.clone() {
                                                    world.add_object(obj);
                                                }
                                                ui.close();
                                            }
                                        });

                                        ui.painter().rect_filled(row_rect, 0.0, bg);
                                        let rl = row_rect.left();
                                        ui.painter().line_segment(
                                            [row_rect.left_bottom(), row_rect.right_bottom()],
                                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                        );
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(rl + name_w, row_rect.top()),
                                                Pos2::new(rl + name_w, row_rect.bottom()),
                                            ],
                                            Stroke::new(1.0, style.div_col),
                                        );
                                    }
                                });
                        });
                });
            });
            ui.allocate_space(ui.available_size());
        });

    {
        let s = world.get_resource_mut::<CellSearchState>()?;
        s.selected_obj = pending_selected_obj;
        s.clicked_obj = pending_clicked_obj;
        s.obj_filter = pending_obj_filter;
        s.renaming_obj = renaming_id;
        s.rename_buf = rename_buf;
        s.rename_request_focus = rename_request_focus;
        s.open = open;
        s.renaming_scene = renaming_scene;
        s.scene_rename_buf = scene_rename_buf;
        s.scene_rename_focus = scene_rename_focus;
    }

    if let Some(response) = window {
        let rect = response.response.rect;
        let layout = world.get_resource_mut::<WindowLayout>()?;
        layout.cell_search.update_from_rect(rect);
        save_layout(layout);
    }

    if let Some((id, new_name)) = pending_rename {
        if let Some(obj) = world.get_object_mut(id) {
            obj.name = new_name;
        }
    }

    if let Some(id) = pending_delete {
        world.remove_object(id);
    }

    if object_to_copy.is_some() {
        let s = world.get_resource_mut::<CellSearchState>()?;
        s.copied_obj = object_to_copy;
    }

    if pending_add {
        world.add_new_object();
    }

    if let Some(name) = pending_scene_load {
        scene_load(world, &name);
    }
    if let Some(name) = pending_scene_delete {
        scene_delete(world, &name);
    }
    if let Some((old, new)) = pending_scene_rename {
        scene_rename(world, &old, &new);
    }

    Ok(())
}

fn scene_load(world: &mut World, name: &str) {
    let scene_value: Option<serde_yaml::Value> = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<SceneLoader>())
        .and_then(|l| l.registry.read().ok()?.scenes.get(name).cloned());

    if let Some(value) = scene_value {
        EditorPreferences::save_last_scene(name);
        let _ = load_scene(world, &value, &["EditorCamera"]);
        if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
            s.selected_obj = None;
        }
    }
}

fn scene_delete(world: &mut World, name: &str) {
    let registry_arc = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<SceneLoader>())
        .map(|l| Arc::clone(&l.registry));

    if let Some(arc) = registry_arc {
        if let Ok(mut reg) = arc.write() {
            reg.scenes.remove(name);
        }
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("res/scenes")
        .join(format!("{}.yaml", name));
    let _ = std::fs::remove_file(&path);

    let prefs = EditorPreferences::load();
    if prefs.last_scene == name {
        EditorPreferences::save_last_scene("");
    }
}

fn scene_rename(world: &mut World, old_name: &str, new_name: &str) {
    let registry_arc = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<SceneLoader>())
        .map(|l| Arc::clone(&l.registry));

    if let Some(arc) = registry_arc {
        if let Ok(mut reg) = arc.write() {
            if let Some(mut value) = reg.scenes.remove(old_name) {
                if let serde_yaml::Value::Mapping(ref mut map) = value {
                    map.insert(
                        serde_yaml::Value::String("name".into()),
                        serde_yaml::Value::String(new_name.into()),
                    );
                }
                let scenes_dir =
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res/scenes");
                let new_path = scenes_dir.join(format!("{}.yaml", new_name));
                if let Ok(yaml) = serde_yaml::to_string(&value) {
                    let _ = std::fs::write(&new_path, yaml);
                }
                let old_path = scenes_dir.join(format!("{}.yaml", old_name));
                let _ = std::fs::remove_file(&old_path);
                reg.scenes.insert(new_name.to_string(), value);
            }
        }
    }

    let prefs = EditorPreferences::load();
    if prefs.last_scene == old_name {
        EditorPreferences::save_last_scene(new_name);
    }
}
