use super::EditorStyle;
use super::shared::WindowLayout;
use anyhow::Result;
use apostasy_core::assets::asset_manager::AssetManager;
use apostasy_core::assets::loaders::worldspace_loader::WorldspaceLoader;
use apostasy_core::objects::worldspace_serializer::load_worldspace;
use apostasy_core::{
    egui::{self, Color32, CursorIcon, FontId, Pos2, Rect, ScrollArea, Sense, Stroke, Ui, Vec2, Window},
    objects::world::World,
    ui::ui_context::EguiContext,
    update,
};
use apostasy_macros::Resource;
use std::sync::Arc;

use crate::ui::asset_editor::AssetEditorState;
use crate::ui::preferences_panel::EditorPreferences;

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

    pub is_first_frame: bool,

    pub renaming_entry: Option<String>,
    pub rename_buf: String,
    pub rename_request_focus: bool,
}

impl Default for ObjectWindowState {
    fn default() -> Self {
        Self {
            open: true,
            show_used_in_cell: false,
            col_widths: [190.0, 130.0, 150.0],
            filter_tree: vec![FilterNode::branch("Data", &[], vec![])],
            selected_filter: None,
            entries: Vec::new(),
            filter_string: "".to_string(),
            sort_col: SortColumn::EditorId,
            sort_dir: SortDir::Asc,
            selected_entry: None,
            is_first_frame: true,

            renaming_entry: None,
            rename_buf: String::new(),
            rename_request_focus: false,
        }
    }
}

impl ObjectWindowState {
    pub fn populate(
        &mut self,
        registry_data: Vec<(String, Vec<(String, String)>)>,
        models: Vec<String>,
        shaders: Vec<String>,
        textures: Vec<String>,
    ) {
        let mut tree = Vec::new();
        let mut entries = Vec::new();

        // "Data" branch yaml-loaded definitions
        let data_path = vec!["Data".to_string()];
        let mut data_children = Vec::new();
        for (class_name, class_entries) in &registry_data {
            if class_entries.is_empty() {
                continue;
            }
            let class_path = vec!["Data".to_string(), class_name.clone()];
            data_children.push(FilterNode::leaf(class_name, &data_path));
            for (namespace, name) in class_entries {
                entries.push(ObjectEntry {
                    editor_id: format!("{}:{}:{}", namespace, class_name, name),
                    name: name.clone(),
                    count: 0,
                    category_path: class_path.clone(),
                });
            }
        }
        if !data_children.is_empty() {
            tree.push(FilterNode::branch("Data", &[], data_children));
        }

        // "Graphics" branch models and shaders
        let gfx_path = vec!["Graphics".to_string()];
        let mut gfx_children = Vec::new();

        if !models.is_empty() {
            let models_path = vec!["Graphics".to_string(), "Models".to_string()];
            gfx_children.push(FilterNode::leaf("Models", &gfx_path));
            for name in &models {
                entries.push(ObjectEntry {
                    editor_id: format!("model:{}", name),
                    name: name.clone(),
                    count: 0,
                    category_path: models_path.clone(),
                });
            }
        }

        if !shaders.is_empty() {
            let shaders_path = vec!["Graphics".to_string(), "Shaders".to_string()];
            gfx_children.push(FilterNode::leaf("Shaders", &gfx_path));
            for name in &shaders {
                entries.push(ObjectEntry {
                    editor_id: format!("shader:{}", name),
                    name: name.clone(),
                    count: 0,
                    category_path: shaders_path.clone(),
                });
            }
        }

        if !textures.is_empty() {
            let textures_path = vec!["Graphics".to_string(), "Textures".to_string()];
            gfx_children.push(FilterNode::leaf("Textures", &gfx_path));
            for name in &textures {
                entries.push(ObjectEntry {
                    editor_id: format!("texture:{}", name),
                    name: name.clone(),
                    count: 0,
                    category_path: textures_path.clone(),
                });
            }
        }

        if !gfx_children.is_empty() {
            tree.push(FilterNode::branch("Graphics", &[], gfx_children));
        }

        self.filter_tree = tree;
        self.entries = entries;
    }
}

#[update(mode = "editor")]
pub fn object_window(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();
    if world.get_resource::<ObjectWindowState>().is_err() {
        world.insert_resource(ObjectWindowState::default());
    }
    if !world.has_resource::<WindowLayout>() {
        return Ok(());
    }

    let state = match world.get_resource::<WindowLayout>() {
        Ok(l) => l.object_window.clone(),
        Err(_) => return Ok(()),
    };

    let pos = state.to_pos();
    let size = state.to_size();

    let window = Window::new("Object Window")
        .default_pos(pos)
        .default_size(size)
        .resizable(true)
        .movable(true);

    let needs_populate = world
        .get_resource::<ObjectWindowState>()
        .map(|s| s.is_first_frame)
        .unwrap_or(false);
    let populate_data = if needs_populate {
        world
            .get_resource::<AssetManager>()
            .ok()
            .map(|am| (am.all_loader_entries(), am.model_names(), am.shader_names(), am.texture_names()))
    } else {
        None
    };

    let mut window_open = world
        .get_resource::<ObjectWindowState>()
        .map(|s| s.open)
        .unwrap_or(true);
    if !window_open {
        return Ok(());
    }

    let object_window_resource = world.get_resource_mut::<ObjectWindowState>()?;
    if object_window_resource.is_first_frame {
        if let Some((registry_data, models, shaders, textures)) = populate_data {
            object_window_resource.populate(registry_data, models, shaders, textures);
        }
        object_window_resource.is_first_frame = false;
    }

    let mut pending_scene_load: Option<String> = None;
    let mut pending_scene_delete: Option<String> = None;
    let mut pending_scene_rename: Option<(String, String)> = None;
    let mut pending_open_in_editor: Option<String> = None;
    let mut pending_new_asset = false;
    let mut pending_refresh = false;

    let window = window
        .open(&mut window_open)
        .resizable(true)
        .movable(true)
        .frame(style.window_frame(&ctx))
        .show(&ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            let filter_w = object_window_resource.col_widths[0];
            let edid_w = object_window_resource.col_widths[1];
            let name_w = object_window_resource.col_widths[2];
            let total_w = ui.available_width();
            let refresh_btn_w = 26.0;
            let count_w = (total_w - filter_w - edid_w - name_w - refresh_btn_w).max(50.0);
            let table_w = edid_w + name_w + count_w;
            let header_h = style.header_height();
            let row_h = style.row_height();

            // header bar
            let (header_rect, _) =
                ui.allocate_exact_size(Vec2::new(total_w, header_h), Sense::hover());
            ui.painter().rect_filled(header_rect, 0.0, style.header_bg);

            let font_hdr = style.font_ui();

            ui.painter().text(
                Pos2::new(header_rect.left() + 6.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "Filter",
                font_hdr.clone(),
                style.text_col,
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
                    style.text_col,
                );
            }

            // Refresh button at the right edge of the header
            let refresh_rect = Rect::from_min_size(
                Pos2::new(header_rect.right() - refresh_btn_w, header_rect.top()),
                Vec2::new(refresh_btn_w, header_h),
            );
            let refresh_resp =
                ui.interact(refresh_rect, ui.id().with("refresh_btn"), Sense::click());
            if refresh_resp.hovered() {
                ui.painter()
                    .rect_filled(refresh_rect, 0.0, Color32::from_rgb(40, 40, 40));
            }
            ui.painter().text(
                refresh_rect.center(),
                egui::Align2::CENTER_CENTER,
                "↺",
                font_hdr.clone(),
                style.text_col,
            );
            if refresh_resp.on_hover_text("Refresh asset lists").clicked() {
                pending_refresh = true;
            }

            ui.painter().line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                Stroke::new(1.0, style.div_col),
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
                        Stroke::new(1.0, style.div_col),
                    );
                    ui.add_space(3.0);

                    draw_tree(
                        ui,
                        &object_window_resource.filter_tree.clone(),
                        0,
                        &object_window_resource.selected_filter,
                        style.text_col,
                        style.dim_col,
                        style.sel_bg,
                        filter_w,
                        style.row_height(),
                        style.font_ui(),
                        style.font_small(),
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
                Stroke::new(1.0, style.div_col),
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

                        let is_texture = entry.editor_id.starts_with("texture:");
                        let is_model = entry.editor_id.starts_with("model:");
                        let (row_rect, row_resp) =
                            ui.allocate_exact_size(Vec2::new(table_w, row_h), Sense::click_and_drag());

                        let is_scene =
                            entry.category_path.len() >= 2 && entry.category_path[1] == "worldspace";
                        let is_renaming = object_window_resource.renaming_entry.as_deref()
                            == Some(entry.editor_id.as_str());

                        if row_resp.clicked() {
                            object_window_resource.selected_entry = Some(entry.editor_id.clone());
                        }

                        // Texture and model rows are drag sources for DnD fields
                        if is_texture || is_model {
                            row_resp.dnd_set_drag_payload(entry.editor_id.clone());
                            if row_resp.hovered() {
                                ui.ctx().set_cursor_icon(CursorIcon::Grab);
                            }
                        }

                        row_resp.context_menu(|ui| {
                            if ui.button("Edit Asset").clicked() {
                                pending_open_in_editor = Some(entry.editor_id.clone());
                                ui.close();
                            }
                            if ui.button("New Asset").clicked() {
                                pending_new_asset = true;
                                ui.close();
                            }

                            if is_scene {
                                ui.separator();
                                if ui.button("Load").clicked() {
                                    pending_scene_load = Some(entry.name.clone());
                                    ui.close();
                                }
                                if ui.button("Rename").clicked() {
                                    object_window_resource.renaming_entry =
                                        Some(entry.editor_id.clone());
                                    object_window_resource.rename_buf = entry.name.clone();
                                    object_window_resource.rename_request_focus = true;
                                    ui.close();
                                }
                                ui.separator();
                                if ui.button("Delete").clicked() {
                                    pending_scene_delete = Some(entry.name.clone());
                                    ui.close();
                                }
                            }
                        });

                        let bg = if is_selected {
                            style.sel_bg
                        } else if row_resp.hovered() {
                            style.hover_bg
                        } else if idx % 2 == 0 {
                            style.dark_bg
                        } else {
                            style.row_alt
                        };

                        ui.painter().rect_filled(row_rect, 0.0, bg);

                        let rl = row_rect.left();
                        let cy = row_rect.center().y;
                        let fnt = style.font_ui();

                        paint_clipped(
                            ui,
                            Pos2::new(rl + 6.0, cy),
                            edid_w - 12.0,
                            &entry.editor_id,
                            fnt.clone(),
                            style.dim_col,
                        );
                        if is_renaming {
                            let name_rect = Rect::from_min_size(
                                Pos2::new(rl + edid_w + 2.0, row_rect.top() + 1.0),
                                Vec2::new(name_w - 4.0, row_h - 2.0),
                            );
                            let te =
                                egui::TextEdit::singleline(&mut object_window_resource.rename_buf)
                                    .font(fnt.clone());
                            let te_resp = ui.put(name_rect, te);
                            if object_window_resource.rename_request_focus {
                                te_resp.request_focus();
                                object_window_resource.rename_request_focus = false;
                            }
                            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if (te_resp.lost_focus() && !escape) || enter {
                                let new_name = object_window_resource.rename_buf.trim().to_string();
                                if !new_name.is_empty() && new_name != entry.name {
                                    pending_scene_rename = Some((entry.name.clone(), new_name));
                                }
                                object_window_resource.renaming_entry = None;
                            } else if escape {
                                object_window_resource.renaming_entry = None;
                            }
                        } else {
                            paint_clipped(
                                ui,
                                Pos2::new(rl + edid_w + 6.0, cy),
                                name_w - 12.0,
                                &entry.name,
                                fnt.clone(),
                                style.dim_col,
                            );
                        }
                        paint_clipped(
                            ui,
                            Pos2::new(rl + edid_w + name_w + 6.0, cy),
                            count_w - 12.0,
                            &entry.count.to_string(),
                            fnt.clone(),
                            style.dim_col,
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
                                Stroke::new(1.0, style.div_col),
                            );
                        }
                    }

                    // filler rows always at least 8 below the content
                    let rows_drawn = filtered.len();
                    let remaining_h = ui.available_height();
                    let filler_rows = (remaining_h / row_h).ceil() as usize;
                    let filler_rows = filler_rows.max(8);

                    for i in 0..filler_rows {
                        let idx = rows_drawn + i;
                        let bg = if idx.is_multiple_of(2) {
                            style.dark_bg
                        } else {
                            style.row_alt
                        };
                        let (row_rect, row_resp) =
                            ui.allocate_exact_size(Vec2::new(table_w, row_h), Sense::click());
                        ui.painter().rect_filled(row_rect, 0.0, bg);

                        row_resp.context_menu(|ui| {
                            if ui.button("New Asset").clicked() {
                                pending_new_asset = true;
                                ui.close();
                            }
                        });

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
                                Stroke::new(1.0, style.div_col),
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

    if let Some(response) = window {
        let rect = response.response.rect;

        let layout = world.get_resource_mut::<WindowLayout>()?;
        layout.object_window.update_from_rect(rect);
        layout.dirty = true;
    }

    world.get_resource_mut::<ObjectWindowState>()?.open = window_open;

    if let Some(name) = pending_scene_load {
        ow_scene_load(world, &name);
    }
    if let Some(name) = pending_scene_delete {
        ow_scene_delete(world, &name);
        world
            .get_resource_mut::<ObjectWindowState>()?
            .is_first_frame = true;
    }
    if let Some((old, new)) = pending_scene_rename {
        ow_scene_rename(world, &old, &new);
        world
            .get_resource_mut::<ObjectWindowState>()?
            .is_first_frame = true;
    }

    if let Some(ref id) = pending_open_in_editor {
        if let Ok(ow) = world.get_resource_mut::<ObjectWindowState>() {
            ow.selected_entry = Some(id.clone());
        }
        if let Ok(ae) = world.get_resource_mut::<AssetEditorState>() {
            ae.open = true;
        }
        if let Ok(layout) = world.get_resource_mut::<WindowLayout>() {
            layout.asset_editor_open = true;
            layout.dirty = true;
        }
    }

    if pending_new_asset {
        if let Ok(ae) = world.get_resource_mut::<AssetEditorState>() {
            ae.new_open = true;
            ae.open = true;
            ae.new_name.clear();
        }
        if let Ok(layout) = world.get_resource_mut::<WindowLayout>() {
            layout.asset_editor_open = true;
            layout.dirty = true;
        }
    }

    if pending_refresh {
        if let Ok(ow) = world.get_resource_mut::<ObjectWindowState>() {
            ow.is_first_frame = true;
        }
    }

    Ok(())
}

fn ow_scene_load(world: &mut World, name: &str) {
    let scene_value: Option<serde_yaml::Value> = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .and_then(|l| l.registry.read().ok()?.worldspaces.get(name).cloned());

    if let Some(value) = scene_value {
        EditorPreferences::save_last_scene(name);
        let _ = load_worldspace(world, &value, &["EditorCamera"]);
        if let Ok(s) = world.get_resource_mut::<crate::ui::cell_panel::CellSearchState>() {
            s.selected_obj = None;
        }
    }
}

fn ow_scene_delete(world: &mut World, name: &str) {
    let registry_arc = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .map(|l| Arc::clone(&l.registry));

    if let Some(arc) = registry_arc {
        if let Ok(mut reg) = arc.write() {
            reg.worldspaces.remove(name);
        }
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("res/worldspaces")
        .join(format!("{}.yaml", name));
    let _ = std::fs::remove_file(&path);

    let prefs = EditorPreferences::load();
    if prefs.last_scene == name {
        EditorPreferences::save_last_scene("");
    }
}

fn ow_scene_rename(world: &mut World, old_name: &str, new_name: &str) {
    let registry_arc = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .map(|l| Arc::clone(&l.registry));

    if let Some(arc) = registry_arc {
        if let Ok(mut reg) = arc.write() {
            if let Some(mut value) = reg.worldspaces.remove(old_name) {
                if let serde_yaml::Value::Mapping(ref mut map) = value {
                    map.insert(
                        serde_yaml::Value::String("name".into()),
                        serde_yaml::Value::String(new_name.into()),
                    );
                }
                let scenes_dir =
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res/worldspaces");
                let new_path = scenes_dir.join(format!("{}.yaml", new_name));
                if let Ok(yaml) = serde_yaml::to_string(&value) {
                    let _ = std::fs::write(&new_path, yaml);
                }
                let old_path = scenes_dir.join(format!("{}.yaml", old_name));
                let _ = std::fs::remove_file(&old_path);
                reg.worldspaces.insert(new_name.to_string(), value);
            }
        }
    }

    let prefs = EditorPreferences::load();
    if prefs.last_scene == old_name {
        EditorPreferences::save_last_scene(new_name);
    }
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
    row_h: f32,
    font: egui::FontId,
    font_arrow: egui::FontId,
    toggle_path: &mut Option<Vec<String>>,
    select_path: &mut Option<Option<Vec<String>>>,
) {
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
                font_arrow.clone(),
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
            font.clone(),
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
                row_h,
                font.clone(),
                font_arrow.clone(),
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
