use anyhow::Result;
use apostasy_core::cgmath::Vector3;
use apostasy_core::egui::{Color32, Pos2, Rect, ScrollArea, Sense, Stroke, Vec2, Window};
use apostasy_core::objects::cell::{CELL_SIZE, ObjectId};
use apostasy_core::objects::cell_streaming::CellMigrations;
use apostasy_core::objects::components::transform::Transform;
use apostasy_core::rendering::components::camera::EditorCamera;
use apostasy_core::objects::world::World;
use apostasy_core::objects::{Object, fmt_key};
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{egui, update};
use apostasy_macros::Resource;

use crate::ui::assets_panel::paint_clipped;
use crate::ui::inspector_panel::InspectorPanelState;
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
    // currently selected cell; when set, filters the object list to that cell
    pub selected_cell: Option<Vector3<i32>>,
    // cell rename state
    pub renaming_cell: Option<Vector3<i32>>,
    pub cell_rename_buf: String,
    pub cell_rename_focus: bool,
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
            selected_cell: None,
            renaming_cell: None,
            cell_rename_buf: String::new(),
            cell_rename_focus: false,
        }
    }
}

/// When an object crosses a cell boundary its `ObjectId` changes. Runs before the
/// rest of the editor UI (high priority) and rewrites the cached selection ids so
/// the inspector and object list keep tracking the migrated object.
#[update(mode = "editor", priority = 10000)]
pub fn remap_selection_after_migration(world: &mut World) -> Result<()> {
    let remap = match world.get_resource::<CellMigrations>() {
        Ok(m) if !m.remap.is_empty() => m.remap.clone(),
        _ => return Ok(()),
    };

    if let Ok(state) = world.get_resource_mut::<CellSearchState>() {
        let fix = |slot: &mut Option<ObjectId>| {
            if let Some(id) = *slot
                && let Some(&new_id) = remap.get(&id)
            {
                *slot = Some(new_id);
            }
        };
        fix(&mut state.selected_obj);
        fix(&mut state.clicked_obj);
        fix(&mut state.renaming_obj);
    }

    Ok(())
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

    // Loaded cells of the active worldspace: (coord, name, object count), sorted by X then Z.
    let mut cell_entries: Vec<(Vector3<i32>, String, usize)> = world
        .worldspace()
        .loaded_cells()
        .map(|c| (c.coord, c.name.clone(), c.len()))
        .collect();
    cell_entries.sort_by_key(|(coord, _, _)| (coord.x, coord.z));

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

    let mut selected_cell: Option<Vector3<i32>> =
        world.get_resource::<CellSearchState>()?.selected_cell;
    let mut renaming_cell: Option<Vector3<i32>> =
        world.get_resource::<CellSearchState>()?.renaming_cell;
    let mut cell_rename_buf: String =
        world.get_resource::<CellSearchState>()?.cell_rename_buf.clone();
    let mut cell_rename_focus: bool =
        world.get_resource::<CellSearchState>()?.cell_rename_focus;
    let mut pending_goto_cell: Option<Vector3<i32>> = None;
    let mut pending_cell_rename: Option<(Vector3<i32>, String)> = None;

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
                                "Cells",
                                font_hdr.clone(),
                                style.text_col,
                            );
                            ui.painter().line_segment(
                                [title_rect.left_bottom(), title_rect.right_bottom()],
                                Stroke::new(1.0, style.div_col),
                            );

                            let table_h = ui.available_height();
                            ScrollArea::vertical()
                                .id_salt("cells_scroll")
                                .auto_shrink([false; 2])
                                .max_height(table_h)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                                    if cell_entries.is_empty() {
                                        let (row_rect, _) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(row_rect, 0.0, style.dark_bg);
                                        ui.painter().text(
                                            Pos2::new(row_rect.left() + 6.0, row_rect.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            "No loaded cells",
                                            font_row.clone(),
                                            style.dim_col,
                                        );
                                    }

                                    for (idx, (coord, name, count)) in
                                        cell_entries.iter().enumerate()
                                    {
                                        let is_selected = selected_cell == Some(*coord);
                                        let is_renaming = renaming_cell == Some(*coord);

                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );

                                        if row_resp.clicked() && !is_renaming {
                                            // toggle: clicking the selected cell clears the filter
                                            selected_cell =
                                                if is_selected { None } else { Some(*coord) };
                                        }
                                        if row_resp.double_clicked() {
                                            renaming_cell = Some(*coord);
                                            cell_rename_buf = name.clone();
                                            cell_rename_focus = true;
                                        }

                                        let bg = if is_selected && !is_renaming {
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
                                                Pos2::new(row_rect.left() + 2.0, row_rect.top() + 1.0),
                                                Vec2::new(avail_w - 4.0, row_h - 2.0),
                                            );
                                            let te =
                                                egui::TextEdit::singleline(&mut cell_rename_buf)
                                                    .font(font_row.clone());
                                            let te_resp = ui.put(edit_rect, te);
                                            if cell_rename_focus {
                                                te_resp.request_focus();
                                                cell_rename_focus = false;
                                            }
                                            let escape =
                                                ui.input(|i| i.key_pressed(egui::Key::Escape));
                                            let enter =
                                                ui.input(|i| i.key_pressed(egui::Key::Enter));
                                            if (te_resp.lost_focus() && !escape) || enter {
                                                pending_cell_rename = Some((
                                                    *coord,
                                                    cell_rename_buf.trim().to_string(),
                                                ));
                                                renaming_cell = None;
                                            } else if escape {
                                                renaming_cell = None;
                                            }
                                        } else {
                                            let text_col = if is_selected {
                                                style.text_col
                                            } else {
                                                style.dim_col
                                            };
                                            // "name (X, Z)" when named, else "(X, Z)"
                                            let label = if name.is_empty() {
                                                format!("({}, {})", coord.x, coord.z)
                                            } else {
                                                format!("{} ({}, {})", name, coord.x, coord.z)
                                            };
                                            paint_clipped(
                                                ui,
                                                Pos2::new(
                                                    row_rect.left() + 6.0,
                                                    row_rect.center().y,
                                                ),
                                                avail_w - 48.0,
                                                &label,
                                                font_row.clone(),
                                                text_col,
                                            );
                                            // object count, right-aligned
                                            ui.painter().text(
                                                Pos2::new(
                                                    row_rect.right() - 6.0,
                                                    row_rect.center().y,
                                                ),
                                                egui::Align2::RIGHT_CENTER,
                                                count.to_string(),
                                                font_row.clone(),
                                                style.dim_col,
                                            );
                                        }

                                        row_resp.context_menu(|ui| {
                                            ui.set_min_width(120.0);
                                            if ui.button("Go to cell").clicked() {
                                                pending_goto_cell = Some(*coord);
                                                ui.close();
                                            }
                                            if ui.button("Rename").clicked() {
                                                renaming_cell = Some(*coord);
                                                cell_rename_buf = name.clone();
                                                cell_rename_focus = true;
                                                ui.close();
                                            }
                                        });

                                        ui.painter().line_segment(
                                            [row_rect.left_bottom(), row_rect.right_bottom()],
                                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                        );
                                    }

                                    // filler rows
                                    let rows_drawn = cell_entries.len();
                                    let remaining =
                                        (ui.available_height() / row_h).ceil() as usize;
                                    for i in 0..remaining {
                                        let idx = rows_drawn + i;
                                        let bg = if idx.is_multiple_of(2) {
                                            style.dark_bg
                                        } else {
                                            style.row_alt
                                        };
                                        let (row_rect, _) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::hover(),
                                        );
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
                                    // when a cell is selected, only show that cell's objects
                                    if let Some(cell) = selected_cell {
                                        if e.object_id.cell != cell {
                                            return false;
                                        }
                                    }
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
        s.selected_cell = selected_cell;
        s.renaming_cell = renaming_cell;
        s.cell_rename_buf = cell_rename_buf;
        s.cell_rename_focus = cell_rename_focus;
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
        match selected_cell {
            Some(cell) => {
                world.add_object_to_cell(cell, Object::default());
            }
            None => {
                world.add_new_object();
            }
        }
    }

    if let Some((coord, new_name)) = pending_cell_rename {
        world.worldspace_mut().set_cell_name(coord, new_name);
    }

    if let Some(coord) = pending_goto_cell {
        goto_cell(world, coord);
    }

    Ok(())
}

/// Moves the editor camera to the center of `coord` on the XZ plane, keeping its
/// current height. The center is at coord * CELL_SIZE + CELL_SIZE/2.
fn goto_cell(world: &mut World, coord: Vector3<i32>) {
    if let Ok(camera) = world.get_object_with_tag_mut::<EditorCamera>()
        && let Ok(transform) = camera.get_component_mut::<Transform>()
    {
        let size = CELL_SIZE as f32;
        transform.local_position.x = coord.x as f32 * size + size / 2.0;
        transform.local_position.z = coord.z as f32 * size + size / 2.0;
    }
}
