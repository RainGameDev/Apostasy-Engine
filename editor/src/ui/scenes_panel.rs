use anyhow::Result;
use apostasy_core::egui::{Color32, Margin, Pos2, Rect, ScrollArea, Sense, Stroke, Vec2, Window};
use apostasy_core::objects::world::World;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{egui, update};
use apostasy_macros::Resource;

use crate::ui::assets_panel::paint_clipped;

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
}

#[derive(Clone, Resource)]
pub struct CellSearchState {
    pub open: bool,
    pub cell_filter: String,
    pub obj_filter: String,
    pub cell_entries: Vec<CellEntry>,
    pub obj_entries: Vec<ObjectRefEntry>,
    pub selected_cell: Option<String>,
    pub selected_obj: Option<String>,
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
            obj_entries: vec![
                ObjectRefEntry {
                    obj_name: "Obj 1".into(),
                    id: "12345678".into(),
                },
                ObjectRefEntry {
                    obj_name: "Obj 1".into(),
                    id: "12345678".into(),
                },
                ObjectRefEntry {
                    obj_name: "Obj 1".into(),
                    id: "12345678".into(),
                },
                ObjectRefEntry {
                    obj_name: "Obj 1".into(),
                    id: "12345678".into(),
                },
                ObjectRefEntry {
                    obj_name: "Obj 1".into(),
                    id: "12345678".into(),
                },
            ],
            selected_cell: None,
            selected_obj: None,
        }
    }
}

#[update]
pub fn cell_search(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if world.get_resource::<CellSearchState>().is_err() {
        world.insert_resource(CellSearchState::default());
    }
    let cell_search_state = world.get_resource_mut::<CellSearchState>()?;
    if !cell_search_state.open {
        return Ok(());
    }

    let dark_bg = Color32::from_rgb(18, 18, 18);
    let panel_bg = Color32::from_rgb(24, 24, 24);
    let header_bg = Color32::from_rgb(30, 30, 30);
    let row_alt = Color32::from_rgb(28, 28, 28);
    let div_col = Color32::from_rgb(60, 60, 60);
    let text_col = Color32::WHITE;
    let dim_col = Color32::from_rgb(170, 170, 170);
    let sel_bg = Color32::from_rgb(40, 80, 140);
    let hover_bg = Color32::from_rgb(38, 38, 50);

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
            egui::Frame::window(&ctx.style())
                .fill(dark_bg)
                .inner_margin(Margin::same(8)),
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
                        .fill(panel_bg)
                        .stroke(Stroke::new(1.0, div_col))
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
                                header_bg,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Cell Search",
                                font_hdr.clone(),
                                text_col,
                            );
                            ui.painter().line_segment(
                                [title_rect.left_bottom(), title_rect.right_bottom()],
                                Stroke::new(1.0, div_col),
                            );

                            // search box
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.add_sized(
                                    Vec2::new(avail_w - 8.0, 20.0),
                                    egui::TextEdit::singleline(&mut cell_search_state.cell_filter)
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
                                Stroke::new(1.0, div_col),
                            );

                            // column widths
                            let eid_w = avail_w * 0.4;
                            let name_w = avail_w * 0.2;
                            let loc_w = avail_w - eid_w - name_w;

                            // column headers
                            let (hdr_rect, _) = ui
                                .allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
                            ui.painter().rect_filled(hdr_rect, 0.0, header_bg);
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
                                    text_col,
                                );
                            }
                            ui.painter().line_segment(
                                [hdr_rect.left_bottom(), hdr_rect.right_bottom()],
                                Stroke::new(1.0, div_col),
                            );
                            for offset in [eid_w, eid_w + name_w] {
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(hdr_rect.left() + offset, hdr_rect.top()),
                                        Pos2::new(hdr_rect.left() + offset, hdr_rect.bottom()),
                                    ],
                                    Stroke::new(1.0, div_col),
                                );
                            }

                            // rows

                            // parse filter string
                            let filter_splits = cell_search_state
                                .cell_filter
                                .split(':')
                                .collect::<Vec<&str>>();
                            let (filter_type, filter_value) = if filter_splits.len() > 1 {
                                (filter_splits[0].to_string(), filter_splits[1].to_string())
                            } else {
                                (String::new(), filter_splits[0].to_string())
                            };

                            let filtered: Vec<&CellEntry> = cell_search_state
                                .cell_entries
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
                                        let is_sel = cell_search_state.selected_cell.as_deref()
                                            == Some(entry.editor_id.as_str());
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        if row_resp.clicked() {
                                            cell_search_state.selected_cell =
                                                Some(entry.editor_id.clone());
                                        }
                                        let bg = if is_sel {
                                            sel_bg
                                        } else if row_resp.hovered() {
                                            hover_bg
                                        } else if idx % 2 == 0 {
                                            dark_bg
                                        } else {
                                            row_alt
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
                                            dim_col,
                                        );
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + eid_w + 6.0, cy),
                                            name_w - 10.0,
                                            &entry.name,
                                            font_row.clone(),
                                            dim_col,
                                        );
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + eid_w + name_w + 6.0, cy),
                                            loc_w - 10.0,
                                            &entry.location,
                                            font_row.clone(),
                                            dim_col,
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
                                                Stroke::new(1.0, div_col),
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
                                            dark_bg
                                        } else {
                                            row_alt
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
                                                Stroke::new(1.0, div_col),
                                            );
                                        }
                                    }
                                });
                        });

                    let gap = total_w - frame.response.rect.size().x;
                    ui.add_space(gap);

                    // TODO: make this take from the world
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
                        .fill(panel_bg)
                        .stroke(Stroke::new(1.0, div_col))
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
                                header_bg,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Object Search",
                                font_hdr.clone(),
                                text_col,
                            );
                            ui.painter().line_segment(
                                [title_rect.left_bottom(), title_rect.right_bottom()],
                                Stroke::new(1.0, div_col),
                            );

                            // search box
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.add_space(4.0);
                                ui.add_sized(
                                    Vec2::new(avail_w - 8.0, 20.0),
                                    egui::TextEdit::singleline(&mut cell_search_state.obj_filter)
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
                                Stroke::new(1.0, div_col),
                            );

                            // column widths
                            let name_w = avail_w * 0.55;
                            let id_w = avail_w - name_w;

                            // column headers
                            let (hdr_rect, _) = ui
                                .allocate_exact_size(Vec2::new(avail_w, header_h), Sense::hover());
                            ui.painter().rect_filled(hdr_rect, 0.0, header_bg);
                            for (label, offset) in [("Obj Name", 0.0_f32), ("Id", name_w)] {
                                ui.painter().text(
                                    Pos2::new(hdr_rect.left() + offset + 6.0, hdr_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    label,
                                    font_hdr.clone(),
                                    text_col,
                                );
                            }
                            ui.painter().line_segment(
                                [hdr_rect.left_bottom(), hdr_rect.right_bottom()],
                                Stroke::new(1.0, div_col),
                            );
                            ui.painter().line_segment(
                                [
                                    Pos2::new(hdr_rect.left() + name_w, hdr_rect.top()),
                                    Pos2::new(hdr_rect.left() + name_w, hdr_rect.bottom()),
                                ],
                                Stroke::new(1.0, div_col),
                            );

                            // rows

                            // parse filter string
                            let filter_splits = cell_search_state
                                .obj_filter
                                .split(':')
                                .collect::<Vec<&str>>();
                            let (filter_type, filter_value) = if filter_splits.len() > 1 {
                                (filter_splits[0].to_string(), filter_splits[1].to_string())
                            } else {
                                (String::new(), filter_splits[0].to_string())
                            };

                            let filtered: Vec<&ObjectRefEntry> = cell_search_state
                                .obj_entries
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
                                        let is_sel = cell_search_state.selected_obj.as_deref()
                                            == Some(entry.id.as_str());
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click(),
                                        );
                                        if row_resp.clicked() {
                                            cell_search_state.selected_obj = Some(entry.id.clone());
                                        }
                                        let bg = if is_sel {
                                            sel_bg
                                        } else if row_resp.hovered() {
                                            hover_bg
                                        } else if idx % 2 == 0 {
                                            dark_bg
                                        } else {
                                            row_alt
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
                                            dim_col,
                                        );
                                        paint_clipped(
                                            ui,
                                            Pos2::new(rl + name_w + 6.0, cy),
                                            id_w - 10.0,
                                            &entry.id,
                                            font_row.clone(),
                                            dim_col,
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
                                            Stroke::new(1.0, div_col),
                                        );
                                    }

                                    // filler rows
                                    let rows_drawn = filtered.len();
                                    let remaining_rows =
                                        (ui.available_height() / row_h).ceil() as usize;
                                    for i in 0..remaining_rows {
                                        let idx = rows_drawn + i;
                                        let bg = if idx.is_multiple_of(2) {
                                            dark_bg
                                        } else {
                                            row_alt
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
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(rl + name_w, row_rect.top()),
                                                Pos2::new(rl + name_w, row_rect.bottom()),
                                            ],
                                            Stroke::new(1.0, div_col),
                                        );
                                    }
                                });
                        });
                });
            });
            ui.allocate_space(ui.available_size());
        });

    Ok(())
}
