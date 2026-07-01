use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use apostasy_core::assets::asset_manager::AssetManager;
use apostasy_core::assets::loaders::worldspace_loader::WorldspaceLoader;
use apostasy_core::cgmath::Vector3;
use apostasy_core::ecs::cell::{CELL_SIZE, EntityBlob, EntityId, world_to_cell};
use apostasy_core::ecs::components::transform::Transform;
use apostasy_core::ecs::resources::input_manager::InputManager;
use apostasy_core::ecs::world::World;
use apostasy_core::ecs::worldspace_serializer::load_worldspace;
use apostasy_core::egui::{Color32, Margin, Pos2, Rect, ScrollArea, Sense, Stroke, Vec2, Window};
use apostasy_core::rendering::components::camera::EditorCamera;
use apostasy_core::terrain::chunk::TerrainChunk;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::worldspaces::cell_streaming::CellMigrations;
use apostasy_core::{egui, update};
use apostasy_macros::Resource;

use std::collections::{HashMap, HashSet};

use super::EditorStyle;
use super::shared::WindowLayout;
use crate::systems::history::{EditorCommand, History, RemoveEntityCmd, SetParentCmd};
use crate::ui::assets_panel::paint_clipped;
use crate::ui::inspector_panel::InspectorPanelState;
use crate::ui::preferences_panel::EditorPreferences;

#[derive(Clone)]
pub struct EntityRefEntry {
    pub entity_name: String,
    pub id: String,
    pub entity_id: EntityId,
}

#[derive(Clone, Resource)]
pub struct CellSearchState {
    pub open: bool,
    pub entity_filter: String,
    pub entity_entries: Vec<EntityRefEntry>,
    pub selected_entity: Option<EntityId>,
    pub clicked_entity: Option<EntityId>,
    pub copied_entity: Option<EntityBlob>,
    pub renaming_entity: Option<EntityId>,
    pub rename_buf: String,
    pub rename_request_focus: bool,
    // currently selected cell; when set, filters the entity list to that cell
    pub selected_cell: Option<Vector3<i32>>,
    // cell rename state
    pub renaming_cell: Option<Vector3<i32>>,
    pub cell_rename_buf: String,
    pub cell_rename_focus: bool,
    // whether to show worldspaces list instead of cells
    pub show_worldspaces: bool,
    // selected worldspace name (in worldspaces tab)
    pub selected_worldspace: Option<String>,
    // create worldspace dialog state
    pub create_ws_open: bool,
    pub create_ws_name: String,
    pub create_ws_interior: bool,
    // entity ids currently collapsed in the hierarchy tree
    pub collapsed_entities: HashSet<EntityId>,
    // entity currently being drag-dropped to reparent
    pub dragging_entity: Option<EntityId>,
}

impl Default for CellSearchState {
    fn default() -> Self {
        Self {
            open: true,
            entity_filter: String::new(),
            entity_entries: vec![],
            selected_entity: None,
            clicked_entity: None,
            copied_entity: None,
            renaming_entity: None,
            rename_buf: String::new(),
            rename_request_focus: false,
            selected_cell: None,
            renaming_cell: None,
            cell_rename_buf: String::new(),
            cell_rename_focus: false,
            show_worldspaces: false,
            selected_worldspace: None,
            create_ws_open: false,
            create_ws_name: String::new(),
            create_ws_interior: false,
            collapsed_entities: HashSet::new(),
            dragging_entity: None,
        }
    }
}

#[update(mode = "editor", priority = 10000)]
pub fn remap_selection_after_migration(world: &mut World) -> Result<()> {
    let remap = match world.get_resource::<CellMigrations>() {
        Ok(m) if !m.remap.is_empty() => m.remap.clone(),
        _ => return Ok(()),
    };

    if let Ok(state) = world.get_resource_mut::<CellSearchState>() {
        let fix = |slot: &mut Option<EntityId>| {
            if let Some(id) = *slot
                && let Some(&new_id) = remap.get(&id)
            {
                *slot = Some(new_id);
            }
        };
        fix(&mut state.selected_entity);
        fix(&mut state.clicked_entity);
        fix(&mut state.renaming_entity);
    }

    Ok(())
}

#[allow(deprecated)]
// Runs (along with `inspector`) before `clear_drag_state`, so both drag-and-drop
// consumers see `dragging_entity` before it's reset for the next drag.
#[update(mode = "editor", priority = 1)]
pub fn cell_search(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let to_delete = !ctx.egui_wants_keyboard_input()
        && world
            .get_resource::<InputManager>()?
            .is_keybind_active("Delete");
    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();

    if world.get_resource::<CellSearchState>().is_err() {
        world.insert_resource(CellSearchState::default());
    }

    // Track camera's current cell
    if let Ok(cam_id) = world.get_entity_with_tag::<EditorCamera>()
        && let Some(transform) = world.get_component::<Transform>(cam_id)
    {
        let cam_cell = world_to_cell(transform.global_position);
        world.get_resource_mut::<CellSearchState>()?.selected_cell = Some(cam_cell);
    }

    let entity_entries: Vec<EntityRefEntry> = world
        .get_all_ids()
        .iter()
        .filter(|&&id| {
            !world.has_component::<TerrainChunk>(id) && !world.has_tag::<EditorCamera>(id)
        })
        .map(|&id| EntityRefEntry {
            entity_name: world.get_name(id).unwrap_or("Entity").to_string(),
            id: format!("{:?}", id),
            entity_id: id,
        })
        .collect();
    world.get_resource_mut::<CellSearchState>()?.entity_entries = entity_entries;

    // Loaded cells of the active worldspace: (coord, name, entity count), sorted by X then Z.
    let mut cell_entries: Vec<(Vector3<i32>, String, usize)> = world
        .worldspace()
        .loaded_cells()
        .map(|c| (c.coord, c.name.clone(), c.len()))
        .collect();
    cell_entries.sort_by_key(|(coord, _, _)| (coord.x, coord.z));

    let mut open = world.get_resource::<CellSearchState>()?.open;
    let entity_entries = world
        .get_resource::<CellSearchState>()?
        .entity_entries
        .clone();

    if !open {
        return Ok(());
    }

    let mut pending_selected_entity: Option<EntityId> =
        world.get_resource::<CellSearchState>()?.selected_entity;
    let mut pending_clicked_entity: Option<EntityId> =
        world.get_resource::<CellSearchState>()?.clicked_entity;
    let mut pending_entity_filter: String = world
        .get_resource::<CellSearchState>()?
        .entity_filter
        .clone();
    let mut pending_delete: Option<EntityId> = None;
    let mut pending_add = false;
    let mut entity_to_copy: Option<EntityBlob> = None;
    let mut renaming_id: Option<EntityId> =
        world.get_resource::<CellSearchState>()?.renaming_entity;
    let mut rename_buf: String = world.get_resource::<CellSearchState>()?.rename_buf.clone();
    let mut rename_request_focus: bool = world
        .get_resource::<CellSearchState>()?
        .rename_request_focus;
    let mut pending_rename: Option<(EntityId, String)> = None;
    let mut pending_reparent: Option<(EntityId, Option<EntityId>)> = None;
    let mut collapsed_entities: HashSet<EntityId> = world
        .get_resource::<CellSearchState>()?
        .collapsed_entities
        .clone();
    let mut dragging_entity: Option<EntityId> =
        world.get_resource::<CellSearchState>()?.dragging_entity;

    let mut selected_cell: Option<Vector3<i32>> =
        world.get_resource::<CellSearchState>()?.selected_cell;
    let mut renaming_cell: Option<Vector3<i32>> =
        world.get_resource::<CellSearchState>()?.renaming_cell;
    let mut cell_rename_buf: String = world
        .get_resource::<CellSearchState>()?
        .cell_rename_buf
        .clone();
    let mut cell_rename_focus: bool = world.get_resource::<CellSearchState>()?.cell_rename_focus;
    let mut pending_goto_cell: Option<Vector3<i32>> = None;
    let mut pending_cell_rename: Option<(Vector3<i32>, String)> = None;
    let mut show_worldspaces = world.get_resource::<CellSearchState>()?.show_worldspaces;
    let mut selected_worldspace: Option<String> =
        world.get_resource::<CellSearchState>()?.selected_worldspace.clone();

    let row_h = style.row_height();
    let header_h = style.header_height();
    let font_hdr = style.font_ui();
    let font_row = style.font_ui();

    let state = match world.get_resource::<WindowLayout>() {
        Ok(l) => l.cell_search.clone(),
        Err(_) => return Ok(()),
    };

    let pos = state.to_pos();
    let size = state.to_size();

    let window = Window::new("Cell Panel")
        .open(&mut open)
        .collapsible(false)
        .default_pos(pos)
        .default_size(size)
        .resizable(true)
        .movable(true);

    let window = window
        .resizable(true)
        .movable(true)
        .frame(style.window_frame(&ctx).inner_margin(Margin {
            bottom: 8,
            left: 8,
            right: 8,
            top: 0,
        }))
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
                            let title = if show_worldspaces {
                                "Worldspaces"
                            } else {
                                "Cells"
                            };
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
                                style.header_bg,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                title,
                                font_hdr.clone(),
                                style.text_col,
                            );

                            // toggle bar
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::ZERO;
                                let half_w = (ui.available_width() / 2.0).floor().max(1.0);
                                if ui
                                    .add_sized(Vec2::new(half_w, 25.0), egui::Button::new("Cells"))
                                    .clicked()
                                {
                                    show_worldspaces = false;
                                }
                                if ui
                                    .add_sized(
                                        Vec2::new(half_w, 25.0),
                                        egui::Button::new("Worldspaces"),
                                    )
                                    .clicked()
                                {
                                    show_worldspaces = true;
                                }
                            });
                            ui.painter().line_segment(
                                [
                                    Pos2::new(ui.cursor().min.x, ui.cursor().min.y),
                                    Pos2::new(ui.cursor().min.x + avail_w, ui.cursor().min.y),
                                ],
                                Stroke::new(1.0, style.div_col),
                            );

                            if show_worldspaces {
                                let ws_name_w = avail_w * 0.60;
                                let ws_int_w = avail_w - ws_name_w;

                                // column headers
                                let (hdr_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(avail_w, header_h),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(hdr_rect, 0.0, style.header_bg);
                                for (label, offset) in [("Name", 0.0_f32), ("Interior", ws_name_w)]
                                {
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
                                        Pos2::new(hdr_rect.left() + ws_name_w, hdr_rect.top()),
                                        Pos2::new(hdr_rect.left() + ws_name_w, hdr_rect.bottom()),
                                    ],
                                    Stroke::new(1.0, style.div_col),
                                );

                                // get worldspace list
                                let ws_names: Vec<String> = world
                                    .get_resource::<AssetManager>()
                                    .ok()
                                    .and_then(|am| am.get_loader::<WorldspaceLoader>())
                                    .map(|l| {
                                        let r = l.registry.read();
                                        let mut names: Vec<String> =
                                            r.worldspaces.keys().cloned().collect();
                                        names.sort();
                                        names
                                    })
                                    .unwrap_or_default();

                                let table_h = ui.available_height();
                                ScrollArea::vertical()
                                    .id_salt("ws_scroll")
                                    .auto_shrink([false; 2])
                                    .max_height(table_h)
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                                        for (idx, ws_name) in ws_names.iter().enumerate() {
                                            let is_current = ws_name == &world.worldspace().name;
                                            let is_ws_selected =
                                                selected_worldspace.as_deref() == Some(ws_name);
                                            let (row_rect, row_resp) = ui.allocate_exact_size(
                                                Vec2::new(avail_w, row_h),
                                                Sense::click(),
                                            );
                                            if row_resp.clicked() {
                                                selected_worldspace = Some(ws_name.clone());
                                            }
                                            let bg = if is_ws_selected {
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
                                            paint_clipped(
                                                ui,
                                                Pos2::new(rl + 6.0, cy),
                                                ws_name_w - 12.0,
                                                ws_name,
                                                font_row.clone(),
                                                if is_ws_selected || is_current {
                                                    style.text_col
                                                } else {
                                                    style.dim_col
                                                },
                                            );

                                            // check interior flag from registry
                                            let is_interior: bool = world
                                                .get_resource::<AssetManager>()
                                                .ok()
                                                .and_then(|am| am.get_loader::<WorldspaceLoader>())
                                                .and_then(|l| {
                                                    l.registry
                                                        .read()
                                                        .worldspaces
                                                        .get(ws_name)
                                                        .cloned()
                                                })
                                                .and_then(|v| {
                                                    v.get("is_interior").and_then(|v| v.as_bool())
                                                })
                                                .unwrap_or(false);
                                            paint_clipped(
                                                ui,
                                                Pos2::new(rl + ws_name_w + 6.0, cy),
                                                ws_int_w - 12.0,
                                                if is_interior { "Yes" } else { "No" },
                                                font_row.clone(),
                                                style.dim_col,
                                            );

                                            row_resp.context_menu(|ui| {
                                                ui.set_min_width(120.0);
                                                if ui.button("Load").clicked() {
                                                    ow_scene_load_internal(world, ws_name);
                                                    ui.close();
                                                }
                                                if ui.button("Delete").clicked() {
                                                    ow_scene_delete_internal(world, ws_name);
                                                    ui.close();
                                                }
                                                ui.separator();
                                                if ui.button("New Worldspace...").clicked() {
                                                    if let Ok(s) =
                                                        world.get_resource_mut::<CellSearchState>()
                                                    {
                                                        s.create_ws_open = true;
                                                        s.create_ws_name.clear();
                                                        s.create_ws_interior = false;
                                                    }
                                                    ui.close();
                                                }
                                            });

                                            ui.painter().line_segment(
                                                [row_rect.left_bottom(), row_rect.right_bottom()],
                                                Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                            );
                                            ui.painter().line_segment(
                                                [
                                                    Pos2::new(rl + ws_name_w, row_rect.top()),
                                                    Pos2::new(rl + ws_name_w, row_rect.bottom()),
                                                ],
                                                Stroke::new(1.0, style.div_col),
                                            );
                                        }

                                        // filler rows
                                        let rows_drawn = ws_names.len();
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
                                            ui.painter().rect_filled(row_rect, 0.0, bg);
                                            row_resp.context_menu(|ui| {
                                                ui.set_min_width(120.0);
                                                if ui.button("New Worldspace...").clicked() {
                                                    if let Ok(s) =
                                                        world.get_resource_mut::<CellSearchState>()
                                                    {
                                                        s.create_ws_open = true;
                                                        s.create_ws_name.clear();
                                                        s.create_ws_interior = false;
                                                    }
                                                    ui.close();
                                                }
                                            });
                                            ui.painter().line_segment(
                                                [row_rect.left_bottom(), row_rect.right_bottom()],
                                                Stroke::new(0.5, Color32::from_rgb(38, 38, 38)),
                                            );
                                            ui.painter().line_segment(
                                                [
                                                    Pos2::new(
                                                        row_rect.left() + ws_name_w,
                                                        row_rect.top(),
                                                    ),
                                                    Pos2::new(
                                                        row_rect.left() + ws_name_w,
                                                        row_rect.bottom(),
                                                    ),
                                                ],
                                                Stroke::new(1.0, style.div_col),
                                            );
                                        }
                                    });
                            } else {
                                // column widths
                                let name_w = avail_w * 0.40;
                                let pos_w = avail_w * 0.30;
                                let count_w = avail_w - name_w - pos_w;

                                // column headers
                                let (hdr_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(avail_w, header_h),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(hdr_rect, 0.0, style.header_bg);
                                for (label, offset) in [
                                    ("Name", 0.0_f32),
                                    ("Position", name_w),
                                    ("Ref count", name_w + pos_w),
                                ] {
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
                                for offset in [name_w, name_w + pos_w] {
                                    ui.painter().line_segment(
                                        [
                                            Pos2::new(hdr_rect.left() + offset, hdr_rect.top()),
                                            Pos2::new(hdr_rect.left() + offset, hdr_rect.bottom()),
                                        ],
                                        Stroke::new(1.0, style.div_col),
                                    );
                                }

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
                                                Pos2::new(
                                                    row_rect.left() + 6.0,
                                                    row_rect.center().y,
                                                ),
                                                egui::Align2::LEFT_CENTER,
                                                "No loaded cells",
                                                font_row.clone(),
                                                style.dim_col,
                                            );
                                            for offset in [name_w, name_w + pos_w] {
                                                ui.painter().line_segment(
                                                    [
                                                        Pos2::new(
                                                            row_rect.left() + offset,
                                                            row_rect.top(),
                                                        ),
                                                        Pos2::new(
                                                            row_rect.left() + offset,
                                                            row_rect.bottom(),
                                                        ),
                                                    ],
                                                    Stroke::new(1.0, style.div_col),
                                                );
                                            }
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
                                                selected_cell = Some(*coord);
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

                                            let rl = row_rect.left();
                                            let cy = row_rect.center().y;
                                            if is_renaming {
                                                let edit_rect = Rect::from_min_size(
                                                    Pos2::new(rl + 2.0, row_rect.top() + 1.0),
                                                    Vec2::new(name_w - 4.0, row_h - 2.0),
                                                );
                                                let te = egui::TextEdit::singleline(
                                                    &mut cell_rename_buf,
                                                )
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
                                                let display_name = if name.is_empty() {
                                                    "Wilderness".to_string()
                                                } else {
                                                    name.clone()
                                                };
                                                paint_clipped(
                                                    ui,
                                                    Pos2::new(rl + 6.0, cy),
                                                    name_w - 12.0,
                                                    &display_name,
                                                    font_row.clone(),
                                                    text_col,
                                                );
                                                paint_clipped(
                                                    ui,
                                                    Pos2::new(rl + name_w + 6.0, cy),
                                                    pos_w - 12.0,
                                                    &format!("({}, {})", coord.x, coord.z),
                                                    font_row.clone(),
                                                    style.dim_col,
                                                );
                                                paint_clipped(
                                                    ui,
                                                    Pos2::new(rl + name_w + pos_w + 6.0, cy),
                                                    count_w - 12.0,
                                                    &count.to_string(),
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
                                            for offset in [name_w, name_w + pos_w] {
                                                ui.painter().line_segment(
                                                    [
                                                        Pos2::new(rl + offset, row_rect.top()),
                                                        Pos2::new(rl + offset, row_rect.bottom()),
                                                    ],
                                                    Stroke::new(1.0, style.div_col),
                                                );
                                            }
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
                                            for offset in [name_w, name_w + pos_w] {
                                                ui.painter().line_segment(
                                                    [
                                                        Pos2::new(
                                                            row_rect.left() + offset,
                                                            row_rect.top(),
                                                        ),
                                                        Pos2::new(
                                                            row_rect.left() + offset,
                                                            row_rect.bottom(),
                                                        ),
                                                    ],
                                                    Stroke::new(1.0, style.div_col),
                                                );
                                            }
                                        }
                                    });
                            }
                        });

                    let gap = total_w - frame.response.rect.size().x;
                    ui.add_space(gap);

                    // RIGHT: entity ref list
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
                                egui::CornerRadius {
                                    nw: 4,
                                    ne: 4,
                                    sw: 4,
                                    se: 4,
                                },
                                style.header_bg,
                            );
                            ui.painter().text(
                                title_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Entity Search",
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
                                    egui::TextEdit::singleline(&mut pending_entity_filter)
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
                            for (label, offset) in [("entity Name", 0.0_f32), ("Id", name_w)] {
                                ui.painter().text(
                                    Pos2::new(hdr_rect.left() + offset + 6.0, hdr_rect.center().y),
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
                                pending_entity_filter.split(':').collect::<Vec<&str>>();
                            let (filter_type, filter_value) = if filter_splits.len() > 1 {
                                (filter_splits[0].to_string(), filter_splits[1].to_string())
                            } else {
                                (String::new(), filter_splits[0].to_string())
                            };

                            let filtered: Vec<&EntityRefEntry> = entity_entries
                                .iter()
                                .filter(|e| {
                                    if !show_worldspaces {
                                        let Some(cell) = selected_cell else {
                                            return false;
                                        };
                                        if e.entity_id.cell != cell {
                                            return false;
                                        }
                                    }
                                    if filter_value.trim().is_empty() {
                                        return true;
                                    }
                                    let val = filter_value.trim().to_lowercase();
                                    match filter_type.trim().to_lowercase().as_str() {
                                        "id" => e.id.to_lowercase().contains(&val),
                                        "name" => e.entity_name.to_lowercase().contains(&val),
                                        _ => e.entity_name.to_lowercase().contains(&val),
                                    }
                                })
                                .collect();

                            if ui.input(|i| i.key_pressed(egui::Key::F2))
                                && let Some(id) = pending_selected_entity
                                && let Some(entry) =
                                    entity_entries.iter().find(|e| e.entity_id == id)
                            {
                                renaming_id = Some(id);
                                rename_buf = entry.entity_name.clone();
                                rename_request_focus = true;
                            }

                            // Build the display order: a parent/child tree when there's no
                            // active search filter (so hierarchy is visible), otherwise a
                            // flat filtered list.
                            struct DisplayRow<'a> {
                                entry: &'a EntityRefEntry,
                                depth: usize,
                                has_children: bool,
                            }

                            let tree_active =
                                !show_worldspaces && filter_value.trim().is_empty();

                            let rows: Vec<DisplayRow> = if tree_active {
                                let cell =
                                    selected_cell.unwrap_or_else(|| Vector3::new(0, 0, 0));
                                let id_set: HashSet<EntityId> = entity_entries
                                    .iter()
                                    .filter(|e| e.entity_id.cell == cell)
                                    .map(|e| e.entity_id)
                                    .collect();
                                let entry_by_id: HashMap<EntityId, &EntityRefEntry> =
                                    entity_entries
                                        .iter()
                                        .filter(|e| id_set.contains(&e.entity_id))
                                        .map(|e| (e.entity_id, e))
                                        .collect();

                                let mut children_of: HashMap<EntityId, Vec<EntityId>> =
                                    HashMap::new();
                                let mut roots: Vec<EntityId> = Vec::new();
                                for &id in &id_set {
                                    match world.get_parent_id(id).filter(|p| id_set.contains(p)) {
                                        Some(parent) => {
                                            children_of.entry(parent).or_default().push(id)
                                        }
                                        None => roots.push(id),
                                    }
                                }
                                let name_of = |id: &EntityId| -> String {
                                    entry_by_id
                                        .get(id)
                                        .map(|e| e.entity_name.to_lowercase())
                                        .unwrap_or_default()
                                };
                                roots.sort_by_key(&name_of);
                                for kids in children_of.values_mut() {
                                    kids.sort_by_key(&name_of);
                                }

                                let mut out = Vec::new();
                                let mut stack: Vec<(EntityId, usize)> =
                                    roots.into_iter().rev().map(|id| (id, 0)).collect();
                                while let Some((id, depth)) = stack.pop() {
                                    let has_children = children_of
                                        .get(&id)
                                        .map(|c| !c.is_empty())
                                        .unwrap_or(false);
                                    if let Some(&entry) = entry_by_id.get(&id) {
                                        out.push(DisplayRow { entry, depth, has_children });
                                    }
                                    if has_children && !collapsed_entities.contains(&id) {
                                        for &child in children_of[&id].iter().rev() {
                                            stack.push((child, depth + 1));
                                        }
                                    }
                                }
                                out
                            } else {
                                filtered
                                    .iter()
                                    .map(|&entry| DisplayRow {
                                        entry,
                                        depth: 0,
                                        has_children: false,
                                    })
                                    .collect()
                            };

                            let table_h = ui.available_height();
                            ScrollArea::vertical()
                                .id_salt("entity_scroll")
                                .auto_shrink([false; 2])
                                .max_height(table_h)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    for (idx, row) in rows.iter().enumerate() {
                                        let entry = row.entry;
                                        let indent = row.depth as f32 * 14.0;
                                        let arrow_w = 12.0;
                                        let is_sel =
                                            pending_selected_entity == Some(entry.entity_id);
                                        let is_clicked =
                                            pending_clicked_entity == Some(entry.entity_id);
                                        let (row_rect, row_resp) = ui.allocate_exact_size(
                                            Vec2::new(avail_w, row_h),
                                            Sense::click_and_drag(),
                                        );
                                        if row_resp.double_clicked() {
                                            renaming_id = Some(entry.entity_id);
                                            rename_buf = entry.entity_name.clone();
                                            rename_request_focus = true;
                                        }
                                        if row_resp.clicked() {
                                            pending_selected_entity = Some(entry.entity_id);
                                            pending_clicked_entity = Some(entry.entity_id);
                                        }
                                        if row_resp.drag_started() {
                                            dragging_entity = Some(entry.entity_id);
                                        }
                                        let is_drop_target = tree_active
                                            && dragging_entity.is_some()
                                            && dragging_entity != Some(entry.entity_id)
                                            && row_resp.contains_pointer();
                                        if is_drop_target
                                            && ui.input(|i| i.pointer.any_released())
                                            && let Some(dragging_id) = dragging_entity
                                        {
                                            pending_reparent =
                                                Some((dragging_id, Some(entry.entity_id)));
                                        }

                                        row_resp.context_menu(|ui| {
                                            if ui.button("Rename").clicked() {
                                                renaming_id = Some(entry.entity_id);
                                                rename_buf = entry.entity_name.clone();
                                                rename_request_focus = true;
                                                ui.close();
                                            }
                                            if ui.button("Teleport to Entity").clicked() {
                                                ui.close();
                                            }
                                            if world.get_parent_id(entry.entity_id).is_some()
                                                && ui.button("Unparent").clicked()
                                            {
                                                pending_reparent = Some((entry.entity_id, None));
                                                ui.close();
                                            }
                                            if ui.button("Delete Entity").clicked() {
                                                pending_delete = Some(entry.entity_id);
                                                ui.close();
                                            }
                                            if ui.button("Add new Entity").clicked() {
                                                pending_add = true;
                                                ui.close();
                                            }
                                            if ui.button("Inspect").clicked() {
                                                pending_selected_entity = Some(entry.entity_id);
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
                                            if ui.button("Copy Entity").clicked() {
                                                entity_to_copy =
                                                    world.capture_entity(entry.entity_id);
                                                ui.close();
                                            }
                                            if ui.button("Cut Entity").clicked() {
                                                entity_to_copy =
                                                    world.capture_entity(entry.entity_id);
                                                pending_delete = Some(entry.entity_id);
                                                ui.close();
                                            }
                                            if ui.button("Paste Entity").clicked() {
                                                let s = world
                                                    .get_resource::<CellSearchState>()
                                                    .unwrap();
                                                if let Some(blob) = s.copied_entity.clone() {
                                                    world.spawn_from_blob(&blob);
                                                }
                                                ui.close();
                                            }
                                        });

                                        let bg = if is_drop_target {
                                            style.hover_bg
                                        } else if is_sel {
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
                                        if dragging_entity == Some(entry.entity_id) {
                                            ui.painter().rect_stroke(
                                                row_rect,
                                                0.0,
                                                Stroke::new(1.0, style.text_col),
                                                egui::StrokeKind::Inside,
                                            );
                                        }

                                        let rl = row_rect.left();
                                        let cy = row_rect.center().y;

                                        if row.has_children {
                                            let arrow_rect = Rect::from_min_size(
                                                Pos2::new(rl + indent, row_rect.top()),
                                                Vec2::new(arrow_w, row_h),
                                            );
                                            let arrow_resp = ui.interact(
                                                arrow_rect,
                                                ui.id().with(("tree_arrow", entry.entity_id)),
                                                Sense::click(),
                                            );
                                            let symbol = if collapsed_entities
                                                .contains(&entry.entity_id)
                                            {
                                                "\u{25B6}"
                                            } else {
                                                "\u{25BC}"
                                            };
                                            ui.painter().text(
                                                arrow_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                symbol,
                                                font_row.clone(),
                                                style.dim_col,
                                            );
                                            if arrow_resp.clicked() {
                                                if collapsed_entities.contains(&entry.entity_id) {
                                                    collapsed_entities.remove(&entry.entity_id);
                                                } else {
                                                    collapsed_entities.insert(entry.entity_id);
                                                }
                                            }
                                        }

                                        let name_x = rl + 6.0 + indent + arrow_w;
                                        let name_avail_w = (name_w - 10.0 - indent - arrow_w).max(10.0);
                                        if renaming_id == Some(entry.entity_id) {
                                            let name_rect = Rect::from_min_size(
                                                Pos2::new(name_x - 4.0, row_rect.top() + 1.0),
                                                Vec2::new(name_avail_w, row_h - 2.0),
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
                                                    Some((entry.entity_id, rename_buf.clone()));
                                                renaming_id = None;
                                            } else if escape {
                                                renaming_id = None;
                                            }
                                        } else {
                                            paint_clipped(
                                                ui,
                                                Pos2::new(name_x, cy),
                                                name_avail_w,
                                                &entry.entity_name,
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

                                    // Drag release is consumed by whichever drop target it
                                    // landed on; `clear_drag_state` clears the shared
                                    // dragging_entity resource once every consumer has run.

                                    // filler rows
                                    let rows_drawn = rows.len();
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
                                            if ui.button("Add new Entity").clicked() {
                                                pending_add = true;
                                                ui.close();
                                            }
                                            if ui.button("Paste Entity").clicked() {
                                                let s = world
                                                    .get_resource::<CellSearchState>()
                                                    .unwrap();
                                                if let Some(blob) = s.copied_entity.clone() {
                                                    world.spawn_from_blob(&blob);
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
        s.selected_entity = pending_selected_entity;
        s.clicked_entity = pending_clicked_entity;
        s.entity_filter = pending_entity_filter;
        s.renaming_entity = renaming_id;
        s.rename_buf = rename_buf;
        s.rename_request_focus = rename_request_focus;
        s.open = open;
        s.selected_cell = selected_cell;
        s.renaming_cell = renaming_cell;
        s.cell_rename_buf = cell_rename_buf;
        s.cell_rename_focus = cell_rename_focus;
        s.show_worldspaces = show_worldspaces;
        s.selected_worldspace = selected_worldspace;
        s.collapsed_entities = collapsed_entities;
        s.dragging_entity = dragging_entity;
    }

    if let Some((id, new_parent)) = pending_reparent {
        let old_parent = world.get_parent_id(id);
        let mut cmd = Box::new(SetParentCmd {
            id,
            old_parent,
            new_parent,
        });
        if cmd.execute(world).is_ok() {
            world.get_resource_mut::<History>()?.push(cmd);
        }
    }

    if let Some(response) = window {
        let rect = response.response.rect;
        let layout = world.get_resource_mut::<WindowLayout>()?;
        layout.cell_search.update_from_rect(rect);
        layout.dirty = true;
    }

    if let Some((id, new_name)) = pending_rename {
        let old_name = world.get_name(id).unwrap_or_default().to_string();
        let mut cmd = Box::new(crate::systems::history::RenameEntityCmd {
            id,
            old_name,
            new_name,
        });
        cmd.execute(world)?;
        world
            .get_resource_mut::<crate::systems::history::History>()?
            .push(cmd);
    }

    if to_delete
        && let Some(id) = world.get_resource::<CellSearchState>()?.selected_entity
        && let Some(mut cmd) = RemoveEntityCmd::new(id, world).map(Box::new)
    {
        cmd.execute(world)?;
        world.get_resource_mut::<History>()?.push(cmd);
    }

    if let Some(id) = pending_delete
        && let Some(mut cmd) =
            crate::systems::history::RemoveEntityCmd::new(id, world).map(Box::new)
    {
        cmd.execute(world)?;
        world
            .get_resource_mut::<crate::systems::history::History>()?
            .push(cmd);
    }

    if entity_to_copy.is_some() {
        let s = world.get_resource_mut::<CellSearchState>()?;
        s.copied_entity = entity_to_copy;
    }

    if pending_add {
        // Position the new entity ~10 units in front of the editor camera.
        let front_pos = world
            .get_entity_with_tag::<EditorCamera>()
            .ok()
            .and_then(|cam_id| world.get_component::<Transform>(cam_id))
            .map(|cam| cam.global_position + cam.global_rotation * Vector3::new(0.0, 0.0, -10.0));

        // Spawn a fresh blank entity blob
        let blank_id = match selected_cell {
            Some(coord) => world.spawn_in_cell(coord).id(),
            None => world.spawn().id(),
        };
        world.set_name(blank_id, "Entity");
        world.add_component(
            blank_id,
            Transform {
                local_position: front_pos.unwrap_or_else(|| Vector3::new(0.0, 0.0, 0.0)),
                ..Default::default()
            },
        );
        // Capture it as a blob for the undo command
        if let Some(blob) = world.capture_entity(blank_id) {
            // We already spawned it; set the history cmd to track it
            let cmd = Box::new(crate::systems::history::AddEntityCmd {
                blob,
                cell: selected_cell,
                added_id: Some(blank_id),
            });
            world
                .get_resource_mut::<crate::systems::history::History>()?
                .push(cmd);
        }
    }

    if let Some((coord, new_name)) = pending_cell_rename {
        world.worldspace_mut().set_cell_name(coord, new_name);
    }

    if let Some(coord) = pending_goto_cell {
        goto_cell(world, coord);
    }

    if world.get_resource::<CellSearchState>()?.create_ws_open {
        let mut ws_name = world
            .get_resource::<CellSearchState>()?
            .create_ws_name
            .clone();
        let mut is_interior = world.get_resource::<CellSearchState>()?.create_ws_interior;
        let ctx_clone = ctx.clone();
        let mut open = true;
        Window::new("Create Worldspace")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx_clone, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut ws_name);
                ui.checkbox(&mut is_interior, "Interior");
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() && !ws_name.trim().is_empty() {
                        let name = ws_name.trim().to_string();
                        let scenes_dir =
                            Path::new(env!("CARGO_MANIFEST_DIR")).join("res/worldspaces");
                        let path = scenes_dir.join(format!("{}.yaml", name));
                        let mut doc = serde_yaml::Mapping::new();
                        doc.insert("name".into(), name.clone().into());
                        doc.insert("namespace".into(), "game".into());
                        doc.insert("class".into(), "worldspace".into());
                        doc.insert("is_interior".into(), is_interior.into());
                        doc.insert(
                            "cells".into(),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                        );
                        let yaml = serde_yaml::to_string(&serde_yaml::Value::Mapping(doc)).unwrap();
                        let _ = std::fs::write(&path, yaml);
                        if let Ok(am) = world.get_resource_mut::<AssetManager>() {
                            let _ = am.load_file(&path);
                        }
                        ow_scene_load_internal(world, &name);
                        if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
                            s.create_ws_open = false;
                        }
                        ui.close();
                    }
                    if ui.button("Cancel").clicked() {
                        if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
                            s.create_ws_open = false;
                        }
                        ui.close();
                    }
                });
            });
        if !open {
            world.get_resource_mut::<CellSearchState>()?.create_ws_open = false;
        } else {
            world.get_resource_mut::<CellSearchState>()?.create_ws_name = ws_name;
            world
                .get_resource_mut::<CellSearchState>()?
                .create_ws_interior = is_interior;
        }
    }

    Ok(())
}

fn ow_scene_load_internal(world: &mut World, name: &str) {
    let scene_value: Option<serde_yaml::Value> = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .and_then(|l| l.registry.read().worldspaces.get(name).cloned());

    if let Some(value) = scene_value {
        EditorPreferences::save_last_scene(name);
        let _ = load_worldspace(world, &value, &["EditorCamera"]);
        if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
            s.selected_entity = None;
        }
    }
}

fn ow_scene_delete_internal(world: &mut World, name: &str) {
    let registry_arc = world
        .get_resource::<AssetManager>()
        .ok()
        .and_then(|am| am.get_loader::<WorldspaceLoader>())
        .map(|l| Arc::clone(&l.registry));

    if let Some(arc) = registry_arc {
        {
            let mut reg = arc.write();
            reg.worldspaces.remove(name);
        }
    }

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("res/worldspaces")
        .join(format!("{}.yaml", name));
    let _ = std::fs::remove_file(&path);

    let prefs = EditorPreferences::load();
    if prefs.last_scene == name {
        EditorPreferences::save_last_scene("");
    }
}

/// Moves the editor camera to the center of cell on the X & Z planes
fn goto_cell(world: &mut World, coord: Vector3<i32>) {
    if let Ok(cam_id) = world.get_entity_with_tag::<EditorCamera>()
        && let Some(transform) = world.get_component_mut::<Transform>(cam_id)
    {
        let size = CELL_SIZE as f32;
        transform.local_position.x = coord.x as f32 * size + size / 2.0;
        transform.local_position.z = coord.z as f32 * size + size / 2.0;
    }
}

/// Resets the shared entity drag-and-drop state once the pointer is released.
/// Runs after `cell_search` and `inspector` (both `priority = 1`) so every
/// drop-target panel gets a chance to see `dragging_entity` before it's cleared.
#[update(mode = "editor")]
pub fn clear_drag_state(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    if ctx.input(|i| i.pointer.any_released())
        && let Ok(state) = world.get_resource_mut::<CellSearchState>()
        && state.dragging_entity.is_some()
    {
        state.dragging_entity = None;
    }
    Ok(())
}
