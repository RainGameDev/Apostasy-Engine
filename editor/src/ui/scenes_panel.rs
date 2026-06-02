use anyhow::Result;
use apostasy_core::egui::{Color32, Margin, Pos2, Rect, ScrollArea, Sense, Stroke, Vec2, Window};
use apostasy_core::objects::scene::ObjectId;
use apostasy_core::objects::world::World;
use apostasy_core::objects::{Object, fmt_key};
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{egui, update};
use apostasy_macros::Resource;

use crate::ui::assets_panel::paint_clipped;
use crate::ui::{
    DARK_BG, DIM_COL, DIV_COL, HEADER_BG, HOVER_BG, PANEL_BG, ROW_ALT, SEL_BG, TEXT_COL,
};

#[derive(Clone)]
pub struct CellEntry {
    pub editor_id: String,
    pub name: String,
    pub location: String,
}

#[derive(Clone)]
pub struct ObjectRefEntry {
    pub obj_name: String,
    pub id: String,
    pub object_id: ObjectId,
}

#[derive(Clone, Resource)]
pub struct CellSearchState {
    pub open: bool,
    pub cell_filter: String,
    pub obj_filter: String,
    pub cell_entries: Vec<CellEntry>,
    pub obj_entries: Vec<ObjectRefEntry>,
    pub selected_cell: Option<String>,
    pub selected_obj: Option<ObjectId>,
    pub clicked_cell: Option<String>,
    pub clicked_obj: Option<ObjectId>,
    pub copied_obj: Option<Object>,
}

impl Default for CellSearchState {
    fn default() -> Self {
        Self {
            open: true,
            cell_filter: String::new(),
            obj_filter: String::new(),
            cell_entries: vec![
                CellEntry {
                    editor_id: "BobsShack01".into(),
                    name: "Shack".into(),
                    location: "ApostasyWorld".into(),
                },
                CellEntry {
                    editor_id: "BobsShack02".into(),
                    name: "Shack".into(),
                    location: "ApostasyWorld".into(),
                },
                CellEntry {
                    editor_id: "BobsShack03".into(),
                    name: "Shack".into(),
                    location: "ApostasyWorld".into(),
                },
                CellEntry {
                    editor_id: "BobsShack04".into(),
                    name: "Shack".into(),
                    location: "ApostasyWorld".into(),
                },
            ],
            obj_entries: vec![],
            selected_cell: None,
            selected_obj: None,
            clicked_cell: None,
            clicked_obj: None,
            copied_obj: None,
        }
    }
}

#[allow(deprecated)]
#[update(mode = "editor")]
pub fn cell_search(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

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

    // read-only data for rendering
    let open = world.get_resource::<CellSearchState>()?.open;
    let cell_entries = world
        .get_resource::<CellSearchState>()?
        .cell_entries
        .clone();
    let obj_entries = world.get_resource::<CellSearchState>()?.obj_entries.clone();

    if !open {
        return Ok(());
    }

    // all pending mutations collected during UI, applied after
    let mut pending_selected_obj: Option<ObjectId> =
        world.get_resource::<CellSearchState>()?.selected_obj;
    let mut pending_clicked_obj: Option<ObjectId> =
        world.get_resource::<CellSearchState>()?.clicked_obj;
    let mut pending_selected_cell: Option<String> = world
        .get_resource::<CellSearchState>()?
        .selected_cell
        .clone();
    let mut pending_cell_filter: String =
        world.get_resource::<CellSearchState>()?.cell_filter.clone();
    let mut pending_obj_filter: String =
        world.get_resource::<CellSearchState>()?.obj_filter.clone();
    let mut pending_delete: Option<ObjectId> = None;
    let mut pending_add = false;
    let mut object_to_copy: Option<Object> = None;

    let row_h = 20.0;
    let header_h = 26.0;
    let font_hdr = egui::FontId::proportional(13.0);
    let font_row = egui::FontId::proportional(12.0);

    Window::new("Cell Search")
        .default_pos([100.0, 100.0])
        .default_size([760.0, 340.0])
        .resizable(true)
        .movable(true)
        .frame(
            egui::Frame::window(&ctx.global_style())
                .fill(DARK_BG)
                .inner_margin(Margin::same(0)),
        )
        .show(&ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

            let total_w = ui.available_width();
            let panel_w = (total_w - 8.0) / 2.0;
            let panel_h = ui.available_height();

            ui.horizontal(|ui| {
                // two panels side by side
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);

                    // LEFT: cell list
                    let left_rect =
                        Rect::from_min_size(ui.cursor().min, Vec2::new(panel_w, panel_h));
                    let mut left = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(left_rect)
                            .layout(egui::Layout::top_down(egui::Align::LEFT)),
                    );
                    left.spacing_mut().item_spacing = Vec2::ZERO;

                    let frame = egui::Frame::new()
                        .fill(PANEL_BG)
                        .stroke(Stroke::new(1.0, DIV_COL))
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
                                egui::CornerRadius {
                                    nw: 4,
                                    ne: 4,
                                    sw: 0,
                                    se: 0,
                                },
                                HEADER_BG,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Cell Search",
                                font_hdr.clone(),
                                TEXT_COL,
                            );
                            ui.painter().line_segment(
                                [title_rect.left_bottom(), title_rect.right_bottom()],
                                Stroke::new(1.0, DIV_COL),
                            );

                            // search box
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.add_sized(
                                    Vec2::new(avail_w - 8.0, 20.0),
                                    egui::TextEdit::singleline(&mut pending_cell_filter)
                                        .hint_text("Placeholder..."),
                                )
                                .on_hover_text(concat!(
                                    "id:         - filter by ID\n",
                                    "name:       - filter by name\n",
                                    "location:   - filter by location\n",
                                    "(no prefix) - filter by name",
                                ));
                            });
                            ui.add_space(4.0);

                            ui.painter().line_segment(
                                [
                                    Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
                                    Pos2::new(ui.cursor().min.x + avail_w, ui.cursor().min.y),
                                ],
                                Stroke::new(1.0, DIV_COL),
                            );

                            // column widths
                            let eid_w = avail_w * 0.4;
                            let name_w = avail_w * 0.2;
                            let loc_w = avail_w - eid_w - name_w;

                            // column headers
                            let (hdr_rect, _) = ui
                                .allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
                            ui.painter().rect_filled(hdr_rect, 0.0, HEADER_BG);
                            for (label, offset) in [
                                ("EditorID", 0.0_f32),
                                ("Name", eid_w),
                                ("Location", eid_w + name_w),
                            ] {
                                ui.painter().text(
                                    Pos2::new(hdr_rect.left() + offset + 6.0, hdr_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    label,
                                    font_hdr.clone(),
                                    TEXT_COL,
                                );
                            }
                            ui.painter().line_segment(
                                [hdr_rect.left_bottom(), hdr_rect.right_bottom()],
                                Stroke::new(1.0, DIV_COL),
                            );
                            for offset in [eid_w, eid_w + name_w] {
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(hdr_rect.left() + offset, hdr_rect.top()),
                                        Pos2::new(hdr_rect.left() + offset, hdr_rect.bottom()),
                                    ],
                                    Stroke::new(1.0, DIV_COL),
                                );
                            }

                            // rows

                            // parse filter string
                            let filter_splits =
                                pending_cell_filter.split(':').collect::<Vec<&str>>();
                            let (filter_type, filter_value) = if filter_splits.len() > 1 {
                                (filter_splits[0].to_string(), filter_splits[1].to_string())
                            } else {
                                (String::new(), filter_splits[0].to_string())
                            };

                            let filtered: Vec<&CellEntry> = cell_entries
                                .iter()
                                .filter(|e| {
                                    if filter_value.trim().is_empty() {
                                        return true;
                                    }
                                    let val = filter_value.trim().to_lowercase();
                                    match filter_type.trim().to_lowercase().as_str() {
                                        "eid" | "id" => e.editor_id.to_lowercase().contains(&val),
                                        "name" => e.name.to_lowercase().contains(&val),
                                        "location" => e.location.to_lowercase().contains(&val),
                                        _ => e.name.to_lowercase().contains(&val),
                                    }
                                })
                                .collect();

                            let table_h = ui.available_height();
                            ScrollArea::vertical()
                                .id_salt("cell_scroll")
                                .auto_shrink([false; 2])
                                .max_height(table_h)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    for (idx, entry) in filtered.iter().enumerate() {
                                        let is_sel = pending_selected_cell.as_deref()
                                            == Some(entry.editor_id.as_str());
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        if row_resp.clicked() {
                                            pending_selected_cell = Some(entry.editor_id.clone());
                                        }
                                        let bg = if is_sel {
                                            SEL_BG
                                        } else if row_resp.hovered() {
                                            HOVER_BG
                                        } else if idx % 2 == 0 {
                                            DARK_BG
                                        } else {
                                            ROW_ALT
                                        };
                                        ui.painter().rect_filled(row_rect, 0.0, bg);

                                        let rl = row_rect.left();
                                        let cy = row_rect.center().y;
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + 6.0, cy),
                                            eid_w - 10.0,
                                            &entry.editor_id,
                                            font_row.clone(),
                                            DIM_COL,
                                        );
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + eid_w + 6.0, cy),
                                            name_w - 10.0,
                                            &entry.name,
                                            font_row.clone(),
                                            DIM_COL,
                                        );
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + eid_w + name_w + 6.0, cy),
                                            loc_w - 10.0,
                                            &entry.location,
                                            font_row.clone(),
                                            DIM_COL,
                                        );

                                        ui.painter().line_segment(
                                            [row_rect.left_bottom(), row_rect.right_bottom()],
                                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                        );
                                        for offset in [eid_w, eid_w + name_w] {
                                            ui.painter().line_segment(
                                                [
                                                    Pos2::new(rl + offset, row_rect.top()),
                                                    Pos2::new(rl + offset, row_rect.bottom()),
                                                ],
                                                Stroke::new(1.0, DIV_COL),
                                            );
                                        }
                                    }

                                    // filler rows
                                    let rows_drawn = filtered.len();
                                    let remaining_rows =
                                        (ui.available_height() / row_h).ceil() as usize;
                                    for i in 0..remaining_rows {
                                        let idx = rows_drawn + i;
                                        let bg = if idx.is_multiple_of(2) {
                                            DARK_BG
                                        } else {
                                            ROW_ALT
                                        };
                                        let (row_rect, _) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_filled(row_rect, 0.0, bg);
                                        let rl = row_rect.left();
                                        ui.painter().line_segment(
                                            [row_rect.left_bottom(), row_rect.right_bottom()],
                                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                        );
                                        for offset in [eid_w, eid_w + name_w] {
                                            ui.painter().line_segment(
                                                [
                                                    Pos2::new(rl + offset, row_rect.top()),
                                                    Pos2::new(rl + offset, row_rect.bottom()),
                                                ],
                                                Stroke::new(1.0, DIV_COL),
                                            );
                                        }
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
                        .fill(PANEL_BG)
                        .stroke(Stroke::new(1.0, DIV_COL))
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
                                egui::CornerRadius {
                                    nw: 4,
                                    ne: 4,
                                    sw: 4,
                                    se: 4,
                                },
                                HEADER_BG,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Object Search",
                                font_hdr.clone(),
                                TEXT_COL,
                            );
                            ui.painter().line_segment(
                                [title_rect.left_bottom(), title_rect.right_bottom()],
                                Stroke::new(1.0, DIV_COL),
                            );

                            // search box
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.add_sized(
                                    Vec2::new(avail_w - 8.0, 20.0),
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
                                Stroke::new(1.0, DIV_COL),
                            );

                            // column widths
                            let name_w = avail_w * 0.55;
                            let id_w = avail_w - name_w;

                            // column headers
                            let (hdr_rect, _) = ui
                                .allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
                            ui.painter().rect_filled(hdr_rect, 0.0, HEADER_BG);
                            for (label, offset) in [("Obj Name", 0.0_f32), ("Id", name_w)] {
                                ui.painter().text(
                                    Pos2::new(hdr_rect.left() + offset + 6.0, hdr_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    label,
                                    font_hdr.clone(),
                                    TEXT_COL,
                                );
                            }
                            ui.painter().line_segment(
                                [hdr_rect.left_bottom(), hdr_rect.right_bottom()],
                                Stroke::new(1.0, DIV_COL),
                            );
                            ui.painter().line_segment(
                                [
                                    Pos2::new(hdr_rect.left() + name_w, hdr_rect.top()),
                                    Pos2::new(hdr_rect.left() + name_w, hdr_rect.bottom()),
                                ],
                                Stroke::new(1.0, DIV_COL),
                            );

                            // rows

                            // parse filter string
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

                            let table_h = ui.available_height();
                            ScrollArea::vertical()
                                .id_salt("obj_scroll")
                                .auto_shrink([false; 2])
                                .max_height(table_h)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    for (idx, entry) in filtered.iter().enumerate() {
                                        let is_sel = pending_selected_obj == Some(entry.object_id);
                                        let is_clicked = pending_clicked_obj == Some(entry.object_id);
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        if row_resp.double_clicked() {
                                            pending_selected_obj = Some(entry.object_id);
                                        }if row_resp.clicked() {
                                            pending_clicked_obj = Some(entry.object_id);
                                        }

                                        // adds a popup menu
                                        row_resp.context_menu(|ui| {
                                            if ui.button("Teleport to Object").clicked() {
                                                // do stuff
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
                                            SEL_BG
                                        } else if row_resp.hovered(){ 
                                            HOVER_BG
                                        } else if is_clicked {
                                            HOVER_BG
                                        } 
                                        else if idx % 2 == 0 {
                                            DARK_BG
                                        } else {
                                            ROW_ALT
                                        };
                                        ui.painter().rect_filled(row_rect, 0.0, bg);

                                        let rl = row_rect.left();
                                        let cy = row_rect.center().y;
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + 6.0, cy),
                                            name_w - 10.0,
                                            &entry.obj_name,
                                            font_row.clone(),
                                            DIM_COL,
                                        );
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + name_w + 6.0, cy),
                                            id_w - 10.0,
                                            &entry.id,
                                            font_row.clone(),
                                            DIM_COL,
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
                                            Stroke::new(1.0, DIV_COL),
                                        );
                                    }

                                    // filler rows
                                    let rows_drawn = filtered.len();
                                    let remaining_rows =
                                        (ui.available_height() / row_h).ceil() as usize;
                                    for i in 0..remaining_rows {
                                        let idx = rows_drawn + i;
                                        let bg = if idx.is_multiple_of(2) {
                                            DARK_BG
                                        } else {
                                            ROW_ALT
                                        };
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        // adds a popup menu
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
                                            Stroke::new(1.0, DIV_COL),
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
        s.selected_cell = pending_selected_cell;
        s.cell_filter = pending_cell_filter;
        s.obj_filter = pending_obj_filter;
    }

    // act on pending world mutations now that the UI borrow is released
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

    Ok(())
}
