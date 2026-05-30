use anyhow::Result;
use apostasy_core::egui::{
    Color32, CursorIcon, FontId, Margin, Pos2, Rect, ScrollArea, Sense, Stroke, Ui, Vec2, Window,
};
use apostasy_core::{egui, objects::world::World, ui::ui_context::EguiContext, update};
use apostasy_macros::Resource;

#[derive(Clone, PartialEq)]
pub enum SortColumn {
    EditorId,
    Name,
    Count,
}

#[derive(Clone, PartialEq)]
pub enum SortDir {
    Asc,
    Desc,
}

/// Container for a piece of data
#[derive(Clone)]
pub struct ObjectEntry {
    pub editor_id: String,
    pub name: String,
    pub count: u32,
    pub category_path: Vec<String>,
}

/// Container for a filter
#[derive(Clone)]
pub struct FilterNode {
    pub label: String,
    pub path: Vec<String>,
    pub expanded: bool,
    pub children: Vec<FilterNode>,
}

impl FilterNode {
    fn leaf(label: &str, parent: &[String]) -> Self {
        let mut path = parent.to_vec();
        path.push(label.to_string());
        Self {
            label: label.to_string(),
            path,
            expanded: false,
            children: vec![],
        }
    }
    fn branch(label: &str, parent: &[String], children: Vec<FilterNode>) -> Self {
        let mut path = parent.to_vec();
        path.push(label.to_string());
        Self {
            label: label.to_string(),
            path,
            expanded: true,
            children,
        }
    }
}

#[derive(Clone, Resource)]
pub struct ObjectWindowState {
    pub open: bool,
    pub show_used_in_cell: bool,
    pub col_widths: [f32; 3],
    pub filter_tree: Vec<FilterNode>,
    pub selected_filter: Option<Vec<String>>,
    pub entries: Vec<ObjectEntry>,
    pub filter_string: String,
    pub sort_col: SortColumn,
    pub sort_dir: SortDir,
    pub selected_entry: Option<String>,
}

impl Default for ObjectWindowState {
    fn default() -> Self {
        let world_p = ["Data", "World"].map(|s| s.to_string());
        let enemies_p = ["Data", "Enemies"].map(|s| s.to_string());
        let data_p = ["Data"].map(|s| s.to_string());

        let world_node = FilterNode::branch(
            "World",
            &data_p,
            vec![
                FilterNode::leaf("Climate", &world_p),
                FilterNode::leaf("Lighting", &world_p),
                FilterNode::leaf("Locations", &world_p),
                FilterNode::leaf("Cells", &world_p),
            ],
        );
        let enemies_node = FilterNode::branch(
            "Enemies",
            &data_p,
            vec![
                FilterNode::leaf("Enemy Bases", &enemies_p),
                FilterNode::leaf("Enemy Upgrades", &enemies_p),
            ],
        );
        let data_node = FilterNode::branch("Data", &[], vec![world_node, enemies_node]);

        let mut entries = Vec::new();
        let paths: &[&[&str]] = &[
            &["Data", "World", "Climate"],
            &["Data", "World", "Lighting"],
            &["Data", "World", "Locations"],
            &["Data", "World", "Cells"],
            &["Data", "Enemies", "Enemy Bases"],
            &["Data", "Enemies", "Enemy Upgrades"],
        ];
        for (i, path) in paths.iter().enumerate() {
            for j in 0..8u32 {
                let n = (i as u32) * 8 + j;
                entries.push(ObjectEntry {
                    editor_id: format!("ED_{:04}", n),
                    name: format!("{}_{}", path.last().unwrap_or(&""), j),
                    count: n * 3 + 1,
                    category_path: path.iter().map(|s| s.to_string()).collect(),
                });
            }
        }

        Self {
            open: true,
            show_used_in_cell: false,
            col_widths: [190.0, 130.0, 150.0],
            filter_tree: vec![data_node],
            selected_filter: None,
            entries,
            filter_string: "".to_string(),
            sort_col: SortColumn::EditorId,
            sort_dir: SortDir::Asc,
            selected_entry: None,
        }
    }
}

#[update]
pub fn object_window(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if world.get_resource::<ObjectWindowState>().is_err() {
        world.insert_resource(ObjectWindowState::default());
    }
    let object_window_resource = world.get_resource_mut::<ObjectWindowState>()?;
    if !object_window_resource.open {
        return Ok(());
    }

    let dark_bg = Color32::from_rgb(18, 18, 18);
    let header_bg = Color32::from_rgb(28, 28, 28);
    let row_alt = Color32::from_rgb(24, 24, 24);
    let div_col = Color32::from_rgb(60, 60, 60);
    let text_col = Color32::WHITE;
    let dim_col = Color32::from_rgb(170, 170, 170);
    let sel_bg = Color32::from_rgb(40, 80, 140);
    let hover_bg = Color32::from_rgb(38, 38, 50);

    Window::new("Object Window")
        .default_pos([60.0, 60.0])
        .default_size([640.0, 520.0])
        .resizable(true)
        .movable(true)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(dark_bg)
                .inner_margin(Margin::same(0)),
        )
        .show(&ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            let filter_w = object_window_resource.col_widths[0];
            let edid_w = object_window_resource.col_widths[1];
            let name_w = object_window_resource.col_widths[2];
            let total_w = ui.available_width();
            let count_w = (total_w - filter_w - edid_w - name_w).max(50.0);
            let table_w = edid_w + name_w + count_w;
            let header_h = 26.0;
            let row_h = 20.0;

            // header bar
            let (header_rect, _) =
                ui.allocate_exact_size(Vec2::new(total_w, header_h), Sense::hover());
            ui.painter().rect_filled(header_rect, 0.0, header_bg);

            let font_hdr = egui::FontId::proportional(13.0);

            ui.painter().text(
                Pos2::new(header_rect.left() + 6.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "Filter",
                font_hdr.clone(),
                text_col,
            );
            ui.add_sized(
                Vec2::new(filter_w, 18.0),
                egui::TextEdit::singleline(&mut object_window_resource.filter_string)
                    .hint_text("Placeholder..."),
            )
            .on_hover_text(concat!(
                "eid: / id:  - filter by ID\n",
                "name:       - filter by name\n",
                "(no prefix) - filter by name",
            ));

            let data_left = header_rect.left() + filter_w + 2.0;
            let col_specs: [(&str, f32, SortColumn); 3] = [
                ("Editor Id", 0.0, SortColumn::EditorId),
                ("Name", edid_w, SortColumn::Name),
                ("Count", edid_w + name_w, SortColumn::Count),
            ];
            for (label, offset, col) in col_specs {
                let col_w = match col {
                    SortColumn::EditorId => edid_w,
                    SortColumn::Name => name_w,
                    SortColumn::Count => count_w,
                };
                let rect = Rect::from_min_size(
                    Pos2::new(data_left + offset, header_rect.top()),
                    Vec2::new(col_w, header_h),
                );
                let resp = ui.interact(rect, ui.id().with(label), Sense::click());
                if resp.hovered() {
                    ui.painter()
                        .rect_filled(rect, 0.0, Color32::from_rgb(40, 40, 40));
                }
                if resp.clicked() {
                    if object_window_resource.sort_col == col {
                        object_window_resource.sort_dir =
                            if object_window_resource.sort_dir == SortDir::Asc {
                                SortDir::Desc
                            } else {
                                SortDir::Asc
                            };
                    } else {
                        object_window_resource.sort_col = col.clone();
                        object_window_resource.sort_dir = SortDir::Asc;
                    }
                }
                let arrow = if object_window_resource.sort_col == col {
                    if object_window_resource.sort_dir == SortDir::Asc {
                        " ▲"
                    } else {
                        " ▼"
                    }
                } else {
                    ""
                };
                paint_clipped(
                    ui,
                    Pos2::new(data_left + offset + 6.0, header_rect.center().y),
                    col_w - 12.0,
                    &format!("{}{}", label, arrow),
                    font_hdr.clone(),
                    text_col,
                );
            }

            ui.painter().line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                Stroke::new(1.0, div_col),
            );

            // body
            let body_top = ui.cursor().min;
            let body_h = ui.available_height();

            let left_rect = Rect::from_min_size(body_top, Vec2::new(filter_w, body_h));
            let right_rect = Rect::from_min_size(
                body_top + Vec2::new(filter_w + 1.0, 0.0),
                Vec2::new(table_w, body_h),
            );

            let body_rect = Rect::from_min_size(body_top, Vec2::new(total_w, body_h));
            ui.advance_cursor_after_rect(body_rect);

            // filter panel
            let mut toggle_path: Option<Vec<String>> = None;
            let mut select_path: Option<Option<Vec<String>>> = None;

            let mut left_child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(left_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            left_child.spacing_mut().item_spacing = Vec2::ZERO;

            ScrollArea::vertical()
                .id_salt("filter_scroll")
                .auto_shrink([false; 2])
                .show(&mut left_child, |ui| {
                    ui.set_min_width(filter_w);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    ui.add_space(4.0);

                    let (cb_rect, _) =
                        ui.allocate_exact_size(Vec2::new(filter_w, 22.0), Sense::hover());
                    let mut cb_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(cb_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );

                    cb_ui.add_space(6.0);

                    cb_ui.checkbox(
                        &mut object_window_resource.show_used_in_cell,
                        "Show used in cell",
                    );

                    let sep_y = ui.cursor().min.y;
                    ui.painter().line_segment(
                        [
                            Pos2::new(left_rect.left(), sep_y),
                            Pos2::new(left_rect.right(), sep_y),
                        ],
                        Stroke::new(1.0, div_col),
                    );
                    ui.add_space(3.0);

                    draw_tree(
                        ui,
                        &object_window_resource.filter_tree.clone(),
                        0,
                        &object_window_resource.selected_filter,
                        text_col,
                        dim_col,
                        sel_bg,
                        filter_w,
                        &mut toggle_path,
                        &mut select_path,
                    );
                });

            if let Some(ref p) = toggle_path {
                toggle_node(&mut object_window_resource.filter_tree, p);
            }
            if let Some(sel) = select_path {
                object_window_resource.selected_filter = sel;
            }

            ui.painter().line_segment(
                [
                    Pos2::new(left_rect.right(), left_rect.top()),
                    Pos2::new(left_rect.right(), left_rect.bottom()),
                ],
                Stroke::new(1.0, div_col),
            );

            // parse filter string
            let filter_splits = object_window_resource
                .filter_string
                .split(':')
                .collect::<Vec<&str>>();
            let (filter_type, filter_value) = if filter_splits.len() > 1 {
                (filter_splits[0].to_string(), filter_splits[1].to_string())
            } else {
                (String::new(), filter_splits[0].to_string())
            };

            // filter + sort entries
            let mut filtered: Vec<&ObjectEntry> = object_window_resource
                .entries
                .iter()
                .filter(|e| match &object_window_resource.selected_filter {
                    None => true,
                    Some(sel) => {
                        e.category_path.len() >= sel.len()
                            && &e.category_path[..sel.len()] == sel.as_slice()
                    }
                })
                .filter(|e| {
                    if filter_value.trim().is_empty() {
                        return true;
                    }
                    let val = filter_value.trim().to_lowercase();
                    match filter_type.trim().to_lowercase().as_str() {
                        "eid" | "id" => e.editor_id.to_lowercase().contains(&val),
                        "name" => e.name.to_lowercase().contains(&val),
                        _ => e.name.to_lowercase().contains(&val),
                    }
                })
                .collect();

            filtered.sort_by(|a, b| {
                let ord = match object_window_resource.sort_col {
                    SortColumn::EditorId => a.editor_id.cmp(&b.editor_id),
                    SortColumn::Name => a.name.cmp(&b.name),
                    SortColumn::Count => a.count.cmp(&b.count),
                };
                if object_window_resource.sort_dir == SortDir::Desc {
                    ord.reverse()
                } else {
                    ord
                }
            });

            // data table single loop inside the ScrollArea
            let mut right_child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(right_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );

            right_child.spacing_mut().item_spacing = Vec2::ZERO;

            ScrollArea::vertical()
                .id_salt("data_scroll")
                .auto_shrink([false; 2])
                .show(&mut right_child, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    for (idx, entry) in filtered.iter().enumerate() {
                        let is_selected = object_window_resource.selected_entry.as_deref()
                            == Some(entry.editor_id.as_str());

                        let (row_rect, row_resp) =
                            ui.allocate_exact_size(Vec2::new(table_w, row_h), Sense::click());

                        if row_resp.clicked() {
                            object_window_resource.selected_entry = Some(entry.editor_id.clone());
                        }

                        let bg = if is_selected {
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
                        let fnt = egui::FontId::proportional(12.0);

                        paint_clipped(
                            ui,
                            Pos2::new(rl + 6.0, cy),
                            edid_w - 12.0,
                            &entry.editor_id,
                            fnt.clone(),
                            dim_col,
                        );
                        paint_clipped(
                            ui,
                            Pos2::new(rl + edid_w + 6.0, cy),
                            name_w - 12.0,
                            &entry.name,
                            fnt.clone(),
                            dim_col,
                        );
                        paint_clipped(
                            ui,
                            Pos2::new(rl + edid_w + name_w + 6.0, cy),
                            count_w - 12.0,
                            &entry.count.to_string(),
                            fnt.clone(),
                            dim_col,
                        );

                        ui.painter().line_segment(
                            [row_rect.left_bottom(), row_rect.right_bottom()],
                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                        );
                        for offset in [edid_w, edid_w + name_w] {
                            ui.painter().line_segment(
                                [
                                    Pos2::new(rl + offset, row_rect.top()),
                                    Pos2::new(rl + offset, row_rect.bottom()),
                                ],
                                Stroke::new(1.0, div_col),
                            );
                        }
                    }

                    // filler rows to continue the alternating pattern
                    let rows_drawn = filtered.len();
                    let remaining_h = ui.available_height();
                    let remaining_rows = (remaining_h / row_h).ceil() as usize;

                    for i in 0..remaining_rows {
                        let idx = rows_drawn + i;
                        let bg = if idx.is_multiple_of(2) {
                            dark_bg
                        } else {
                            row_alt
                        };
                        let (row_rect, _) =
                            ui.allocate_exact_size(Vec2::new(table_w, row_h), Sense::hover());
                        ui.painter().rect_filled(row_rect, 0.0, bg);

                        let rl = row_rect.left();
                        ui.painter().line_segment(
                            [row_rect.left_bottom(), row_rect.right_bottom()],
                            Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                        );
                        for offset in [edid_w, edid_w + name_w] {
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

            // column drag handles
            let win_top = header_rect.top();
            let win_bot = win_top + header_h + body_h;
            let left_edge = header_rect.left();

            for (i, dx) in [
                left_edge + filter_w,
                left_edge + filter_w + edid_w,
                left_edge + filter_w + edid_w + name_w,
            ]
            .iter()
            .enumerate()
            {
                let handle =
                    Rect::from_min_max(Pos2::new(dx - 4.0, win_top), Pos2::new(dx + 4.0, win_bot));
                let resp = ui.allocate_rect(handle, Sense::drag());
                if resp.hovered() || resp.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                if resp.dragged() {
                    let d = resp.drag_delta().x;
                    match i {
                        0 => {
                            object_window_resource.col_widths[0] =
                                (object_window_resource.col_widths[0] + d).max(80.0)
                        }
                        1 => {
                            object_window_resource.col_widths[1] =
                                (object_window_resource.col_widths[1] + d).max(50.0)
                        }
                        2 => {
                            object_window_resource.col_widths[2] =
                                (object_window_resource.col_widths[2] + d).max(50.0)
                        }
                        _ => {}
                    }
                }
            }
        });

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn draw_tree(
    ui: &mut Ui,
    nodes: &[FilterNode],
    depth: u32,
    selected: &Option<Vec<String>>,
    text_col: Color32,
    dim_col: Color32,
    sel_bg: Color32,
    panel_w: f32,
    toggle_path: &mut Option<Vec<String>>,
    select_path: &mut Option<Option<Vec<String>>>,
) {
    let row_h = 20.0;
    let indent_px = depth as f32 * 14.0 + 6.0;

    for node in nodes {
        let is_sel = selected.as_ref().map_or(false, |s| s == &node.path);
        let has_kids = !node.children.is_empty();

        let (row_rect, row_resp) =
            ui.allocate_exact_size(Vec2::new(panel_w, row_h), Sense::click());

        if is_sel {
            ui.painter().rect_filled(row_rect, 2.0, sel_bg);
        } else if row_resp.hovered() {
            ui.painter()
                .rect_filled(row_rect, 2.0, Color32::from_rgb(42, 42, 42));
        }

        let cy = row_rect.center().y;
        let col = if is_sel { text_col } else { dim_col };

        if has_kids {
            let arrow_rect = Rect::from_center_size(
                Pos2::new(row_rect.left() + indent_px + 5.0, cy),
                Vec2::new(16.0, row_h),
            );
            let arrow_resp = ui.interact(arrow_rect, ui.id().with(&node.path), Sense::click());
            if arrow_resp.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            if arrow_resp.clicked() {
                *toggle_path = Some(node.path.clone());
            }

            ui.painter().text(
                Pos2::new(row_rect.left() + indent_px, cy),
                egui::Align2::LEFT_CENTER,
                if node.expanded { "▼" } else { "▶" },
                egui::FontId::proportional(9.0),
                col,
            );
        }

        let label_x = row_rect.left() + indent_px + 14.0;
        let max_w = panel_w - indent_px - 14.0 - 6.0;
        paint_clipped(
            ui,
            Pos2::new(label_x, cy),
            max_w,
            &node.label,
            egui::FontId::proportional(12.0),
            col,
        );

        if row_resp.clicked() {
            *select_path = Some(Some(node.path.clone()));
        }
        if node.expanded && has_kids {
            draw_tree(
                ui,
                &node.children,
                depth + 1,
                selected,
                text_col,
                dim_col,
                sel_bg,
                panel_w,
                toggle_path,
                select_path,
            );
        }
    }
}

pub fn toggle_node(nodes: &mut Vec<FilterNode>, target: &[String]) {
    for node in nodes.iter_mut() {
        if node.path == target {
            node.expanded = !node.expanded;
            return;
        }
        toggle_node(&mut node.children, target);
    }
}

pub fn paint_clipped(ui: &Ui, origin: Pos2, max_w: f32, text: &str, font: FontId, color: Color32) {
    let painter = ui.painter();
    let clip = Rect::from_min_size(origin - Vec2::new(0.0, 20.0), Vec2::new(max_w, 40.0));
    let painter = painter.with_clip_rect(clip);

    let galley = painter.layout_no_wrap(text.to_string(), font.clone(), color);
    if galley.size().x <= max_w {
        painter.galley(
            origin - Vec2::new(0.0, galley.size().y * 0.5),
            galley,
            color,
        );
    } else {
        let ellipsis = painter.layout_no_wrap("…".to_string(), font.clone(), color);
        let budget = (max_w - ellipsis.size().x).max(0.0);
        let chars: Vec<char> = text.chars().collect();
        let mut lo = 0usize;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let s: String = chars[..mid].iter().collect();
            let w = painter.layout_no_wrap(s, font.clone(), color).size().x;
            if w <= budget {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let truncated = chars[..lo].iter().collect::<String>() + "…";
        let g = painter.layout_no_wrap(truncated, font.clone(), color);
        painter.galley(origin - Vec2::new(0.0, g.size().y * 0.5), g, color);
    }
}
