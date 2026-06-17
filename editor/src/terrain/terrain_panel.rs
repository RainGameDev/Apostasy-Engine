use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use apostasy_core::{
    egui::{
        self, Color32, DragValue, Margin, Pos2, RichText, ScrollArea, Slider, Vec2, Window, vec2,
    },
    objects::world::World,
    terrain::{TerrainAtlasNeedsRebuild, TerrainSettings, load_terrain_texture},
    ui::DRAG_SIZE,
    update,
};
use apostasy_macros::Resource;

use crate::{
    terrain::{TerrainTool, TerrainToolState},
    ui::EditorStyle,
};

/// Cache of egui texture handles for terrain layer thumbnails.
#[derive(Clone, Resource, Default)]
struct ThumbnailCache {
    handles: HashMap<String, egui::TextureHandle>,
}

/// Which texture path is being edited inline.
#[derive(Clone, Resource, Default)]
struct EditingTexturePath(Option<usize>);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum TextureSortColumn {
    Id,
    FileName,
    #[default]
    Index,
}

#[derive(Clone, Default, Copy, PartialEq, Eq)]
enum SortDir {
    #[default]
    Asc,
    Desc,
}

/// Sort state for the texture list.
#[derive(Clone, Copy, Resource, Default)]
struct TextureSortState {
    column: TextureSortColumn,
    dir: SortDir,
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "tga", "bmp", "hdr"];

#[update(mode = "editor")]
pub fn terrain_panel(world: &mut World) -> Result<()> {
    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();

    if !world.has_resource::<TerrainToolState>() {
        world.insert_resource(TerrainToolState::default());
    }
    if !world.has_resource::<EditingTexturePath>() {
        world.insert_resource(EditingTexturePath(None));
    }
    if !world.has_resource::<TextureSortState>() {
        world.insert_resource(TextureSortState::default());
    }

    let active = world
        .get_resource::<TerrainToolState>()
        .map(|s| s.active)
        .unwrap_or(false);
    if !active {
        return Ok(());
    }

    let ctx = world
        .get_resource::<apostasy_core::ui::ui_context::EguiContext>()?
        .0
        .clone();

    let resolution = world
        .get_resource::<TerrainSettings>()
        .map(|s| s.resolution)
        .unwrap_or(128);

    let mut state = world.get_resource::<TerrainToolState>()?.clone();
    let new_resolution = resolution;
    let resolution_changed = false;

    let mut textures_changed = false;

    if !world.has_resource::<ThumbnailCache>() {
        world.insert_resource(ThumbnailCache::default());
    }

    // OS drag-and-drop: process dropped files from the file manager
    let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
    let mut drop_add = Vec::new();
    if !dropped_files.is_empty() {
        for f in &dropped_files {
            if let Some(p) = &f.path {
                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                if IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    drop_add.push(p.to_string_lossy().to_string());
                }
            }
        }
    }

    let editing = world
        .get_resource::<EditingTexturePath>()
        .cloned()
        .unwrap_or(EditingTexturePath(None));
    let commit_path: Option<(usize, String)> = None;

    Window::new("Terrain")
        .open(&mut true)
        .resizable(true)
        .collapsible(true)
        .order(egui::Order::Foreground)
        .default_width(280.0)
        .frame(style.window_frame(&ctx).inner_margin(Margin {
            left: 8,
            right: 8,
            bottom: 0,
            top: 0,
        }))
        .show(&ctx, |ui| {
            let id = ui.id().with("toolbar_width");
            let last_width: f32 = ui.memory(|mem| mem.data.get_temp(id)).unwrap_or(0.0);

            ui.add_space(5.0);

            // States
            ui.horizontal(|ui| {
                let available = ui.available_width();
                if last_width < available {
                    ui.add_space((available - last_width) / 2.0);
                }

                let inner = ui.scope(|ui| {
                    ui.selectable_value(&mut state.tool, TerrainTool::Modify, "Modify");
                    ui.selectable_value(&mut state.tool, TerrainTool::Smooth, "Smooth");
                    ui.selectable_value(&mut state.tool, TerrainTool::Flatten, "Flatten");
                });

                ui.memory_mut(|mem| mem.data.insert_temp(id, inner.response.rect.width()));
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    ui.add(Slider::new(&mut state.brush_radius, 1.0..=64.0));
                });
                ui.horizontal(|ui| {
                    ui.label("Strength %:");
                    ui.add(Slider::new(&mut state.brush_strength, 0.01..=1.0));
                });
            });
            ui.separator();

            // Textures
            ui.label("Textures");

            let sort_state = world
                .get_resource::<TextureSortState>()
                .cloned()
                .unwrap_or(TextureSortState::default());

            let texture_paths: Vec<(usize, String)> = world
                .get_resource::<TerrainSettings>()
                .map(|s| s.texture_layers.clone())
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .collect();

            // Apply sorting
            let mut sorted_paths = texture_paths;
            sorted_paths.sort_by(|a, b| {
                let ord = match sort_state.column {
                    TextureSortColumn::Id => {
                        let name_a = Path::new(&a.1)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&a.1);
                        let name_b = Path::new(&b.1)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&b.1);
                        name_a.to_lowercase().cmp(&name_b.to_lowercase())
                    }
                    TextureSortColumn::FileName => {
                        let file_a = Path::new(&a.1)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&a.1);
                        let file_b = Path::new(&b.1)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&b.1);
                        file_a.to_lowercase().cmp(&file_b.to_lowercase())
                    }
                    TextureSortColumn::Index => a.0.cmp(&b.0),
                };
                if sort_state.dir == SortDir::Asc {
                    ord
                } else {
                    ord.reverse()
                }
            });

            let header_h = 24.0;
            let row_h = 22.0;
            let total_w = ui.available_width();
            let col_id_w = total_w * 0.3;
            let col_file_w = total_w * 0.5;
            let col_idx_w = total_w * 0.2;

            // Header - clickable columns
            let (header_rect, _) =
                ui.allocate_exact_size(Vec2::new(total_w, header_h), egui::Sense::hover());
            ui.painter()
                .rect_filled(header_rect, 0.0, Color32::from_gray(40));
            let font_hdr = egui::FontId::proportional(12.0);
            let header_left = header_rect.left() + 6.0;
            let cy = header_rect.center().y;

            // ID column
            let id_rect = egui::Rect::from_min_size(
                Pos2::new(header_rect.left(), header_rect.top()),
                Vec2::new(col_id_w, header_h),
            );
            let id_resp = ui.interact(id_rect, ui.id().with("sort_id"), egui::Sense::click());
            let mut new_sort = sort_state;
            if id_resp.clicked() {
                if new_sort.column == TextureSortColumn::Id {
                    new_sort.dir = if new_sort.dir == SortDir::Asc {
                        SortDir::Desc
                    } else {
                        SortDir::Asc
                    };
                } else {
                    new_sort.column = TextureSortColumn::Id;
                    new_sort.dir = SortDir::Asc;
                }
                world.insert_resource(new_sort);
            }
            let id_arrow = if sort_state.column == TextureSortColumn::Id {
                if sort_state.dir == SortDir::Asc {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            };
            ui.painter().text(
                Pos2::new(header_left, cy),
                egui::Align2::LEFT_CENTER,
                format!("ID{}", id_arrow),
                font_hdr.clone(),
                Color32::WHITE,
            );

            // File Name column
            let file_rect = egui::Rect::from_min_size(
                Pos2::new(header_rect.left() + col_id_w, header_rect.top()),
                Vec2::new(col_file_w, header_h),
            );
            let file_resp = ui.interact(file_rect, ui.id().with("sort_file"), egui::Sense::click());
            if file_resp.clicked() {
                let mut new_sort = sort_state;
                if new_sort.column == TextureSortColumn::FileName {
                    new_sort.dir = if new_sort.dir == SortDir::Asc {
                        SortDir::Desc
                    } else {
                        SortDir::Asc
                    };
                } else {
                    new_sort.column = TextureSortColumn::FileName;
                    new_sort.dir = SortDir::Asc;
                }
                world.insert_resource(new_sort);
            }
            let file_arrow = if sort_state.column == TextureSortColumn::FileName {
                if sort_state.dir == SortDir::Asc {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            };
            ui.painter().text(
                Pos2::new(header_left + col_id_w, cy),
                egui::Align2::LEFT_CENTER,
                format!("File Name{}", file_arrow),
                font_hdr.clone(),
                Color32::WHITE,
            );

            // Index column
            let idx_rect = egui::Rect::from_min_size(
                Pos2::new(
                    header_rect.left() + col_id_w + col_file_w,
                    header_rect.top(),
                ),
                Vec2::new(col_idx_w, header_h),
            );
            let idx_resp = ui.interact(idx_rect, ui.id().with("sort_idx"), egui::Sense::click());
            if idx_resp.clicked() {
                let mut new_sort = sort_state;
                if new_sort.column == TextureSortColumn::Index {
                    new_sort.dir = if new_sort.dir == SortDir::Asc {
                        SortDir::Desc
                    } else {
                        SortDir::Asc
                    };
                } else {
                    new_sort.column = TextureSortColumn::Index;
                    new_sort.dir = SortDir::Asc;
                }
                world.insert_resource(new_sort);
            }
            let idx_arrow = if sort_state.column == TextureSortColumn::Index {
                if sort_state.dir == SortDir::Asc {
                    " ▲"
                } else {
                    " ▼"
                }
            } else {
                ""
            };
            ui.painter().text(
                Pos2::new(header_left + col_id_w + col_file_w, cy),
                egui::Align2::LEFT_CENTER,
                format!("Index{}", idx_arrow),
                font_hdr.clone(),
                Color32::WHITE,
            );

            // Divider
            ui.painter().line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                egui::Stroke::new(1.0, Color32::from_gray(60)),
            );

            ScrollArea::vertical()
                .id_salt("textures_scroll")
                .auto_shrink([true; 2])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for (display_idx, (orig_idx, path)) in sorted_paths.iter().enumerate() {
                        let (row_rect, row_resp) =
                            ui.allocate_exact_size(Vec2::new(total_w, row_h), egui::Sense::click());
                        let is_selected = state.paint_layer == *orig_idx;
                        let bg = if is_selected {
                            Color32::from_rgb(60, 100, 140)
                        } else if display_idx % 2 == 0 {
                            Color32::from_gray(30)
                        } else {
                            Color32::from_gray(25)
                        };
                        ui.painter().rect_filled(row_rect, 0.0, bg);

                        if row_resp.clicked() {
                            state.paint_layer = *orig_idx;
                        }

                        let name = Path::new(path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(path);
                        let file_name = Path::new(path)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(path);

                        let fnt = egui::FontId::proportional(11.0);
                        let rl = row_rect.left() + 6.0;
                        let cy = row_rect.center().y;
                        let text_color = if is_selected {
                            Color32::WHITE
                        } else {
                            Color32::from_gray(200)
                        };
                        ui.painter().text(
                            Pos2::new(rl, cy),
                            egui::Align2::LEFT_CENTER,
                            name,
                            fnt.clone(),
                            text_color,
                        );
                        ui.painter().text(
                            Pos2::new(rl + col_id_w, cy),
                            egui::Align2::LEFT_CENTER,
                            file_name,
                            fnt.clone(),
                            if is_selected {
                                Color32::WHITE
                            } else {
                                Color32::from_gray(180)
                            },
                        );
                        ui.painter().text(
                            Pos2::new(rl + col_id_w + col_file_w, cy),
                            egui::Align2::LEFT_CENTER,
                            orig_idx.to_string(),
                            fnt.clone(),
                            if is_selected {
                                Color32::WHITE
                            } else {
                                Color32::from_gray(150)
                            },
                        );

                        ui.painter().line_segment(
                            [row_rect.left_bottom(), row_rect.right_bottom()],
                            egui::Stroke::new(0.5, Color32::from_gray(40)),
                        );
                    }
                });

            ui.separator();
            if state.tool == TerrainTool::Flatten {
                if let Some(h) = state.flatten_height {
                    ui.label(format!("Target height: {:.2}", h));
                } else {
                    ui.label("Click to sample height");
                }
            }

            ui.separator();

            let width = 105.0;
            let button_size = egui::vec2(width * 1.5, 30.0);

            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    let id = ui.id().with("texture_buttons");
                    let last_width: f32 = ui.memory(|mem| mem.data.get_temp(id)).unwrap_or(0.0);
                    let available = ui.available_width();
                    if last_width < available {
                        ui.add_space((available - last_width) / 2.0);
                    }

                    let inner = ui.scope(|ui| {
                        ui.scope(|ui| {
                            ui.spacing_mut().interact_size = button_size;
                            if ui.button("Add Texture").clicked() {}
                            ui.separator();
                            if ui.button("Show Preview").clicked() {}
                        });
                    });
                    ui.memory_mut(|mem| mem.data.insert_temp(id, inner.response.rect.width()));
                });

                // ui.horizontal(|ui| {});
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Vertex Color").strong());
            });
            ui.separator();

            let button_size = egui::vec2(width, 20.0);
            let vetex_color_box_size = egui::vec2(width, 40.0);
            ui.indent("vertexcolorindent", |ui| {
                ui.horizontal(|ui| {
                    ui.horizontal(|ui| {
                        // Color A
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("R").strong());
                                ui.add_sized(
                                    DRAG_SIZE,
                                    DragValue::new(&mut state.vertex_color_a[0])
                                        .range(0.0..=1.0)
                                        .speed(1.0 / 255.0)
                                        .custom_formatter(|n, _| {
                                            format!("{}", (n * 255.0).round() as i64)
                                        })
                                        .custom_parser(|s| {
                                            s.parse::<f64>()
                                                .ok()
                                                .map(|v| (v / 255.0).clamp(0.0, 1.0))
                                        }),
                                );
                            });

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("G").strong());
                                ui.add_sized(
                                    DRAG_SIZE,
                                    DragValue::new(&mut state.vertex_color_a[1])
                                        .range(0.0..=1.0)
                                        .speed(1.0 / 255.0)
                                        .custom_formatter(|n, _| {
                                            format!("{}", (n * 255.0).round() as i64)
                                        })
                                        .custom_parser(|s| {
                                            s.parse::<f64>()
                                                .ok()
                                                .map(|v| (v / 255.0).clamp(0.0, 1.0))
                                        }),
                                );
                            });

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("B").strong());
                                ui.add_sized(
                                    DRAG_SIZE,
                                    DragValue::new(&mut state.vertex_color_a[2])
                                        .range(0.0..=1.0)
                                        .speed(1.0 / 255.0)
                                        .custom_formatter(|n, _| {
                                            format!("{}", (n * 255.0).round() as i64)
                                        })
                                        .custom_parser(|s| {
                                            s.parse::<f64>()
                                                .ok()
                                                .map(|v| (v / 255.0).clamp(0.0, 1.0))
                                        }),
                                );
                            });
                        });

                        let mut rgb = [
                            state.vertex_color_a[0],
                            state.vertex_color_a[1],
                            state.vertex_color_a[2],
                        ];
                        ui.vertical(|ui| {
                            ui.scope(|ui| {
                                ui.spacing_mut().interact_size = vetex_color_box_size;
                                if ui.color_edit_button_rgb(&mut rgb).changed() {
                                    state.vertex_color_a = rgb;
                                }
                            });
                            ui.scope(|ui| {
                                ui.spacing_mut().interact_size = button_size;
                                if ui.button("Select Color").clicked() {}
                            });
                        });

                        state.vertex_color_a[0] = state.vertex_color_a[0].clamp(0.0, 1.0);
                        state.vertex_color_a[1] = state.vertex_color_a[1].clamp(0.0, 1.0);
                        state.vertex_color_a[2] = state.vertex_color_a[2].clamp(0.0, 1.0);
                    });

                    ui.separator();

                    // Color B
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("R").strong());
                            ui.add_sized(
                                DRAG_SIZE,
                                DragValue::new(&mut state.vertex_color_b[0])
                                    .range(0.0..=1.0)
                                    .speed(1.0 / 255.0)
                                    .custom_formatter(|n, _| {
                                        format!("{}", (n * 255.0).round() as i64)
                                    })
                                    .custom_parser(|s| {
                                        s.parse::<f64>().ok().map(|v| (v / 255.0).clamp(0.0, 1.0))
                                    }),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("G").strong());
                            ui.add_sized(
                                DRAG_SIZE,
                                DragValue::new(&mut state.vertex_color_b[1])
                                    .range(0.0..=1.0)
                                    .speed(1.0 / 255.0)
                                    .custom_formatter(|n, _| {
                                        format!("{}", (n * 255.0).round() as i64)
                                    })
                                    .custom_parser(|s| {
                                        s.parse::<f64>().ok().map(|v| (v / 255.0).clamp(0.0, 1.0))
                                    }),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("B").strong());
                            ui.add_sized(
                                DRAG_SIZE,
                                DragValue::new(&mut state.vertex_color_b[2])
                                    .range(0.0..=1.0)
                                    .speed(1.0 / 255.0)
                                    .custom_formatter(|n, _| {
                                        format!("{}", (n * 255.0).round() as i64)
                                    })
                                    .custom_parser(|s| {
                                        s.parse::<f64>().ok().map(|v| (v / 255.0).clamp(0.0, 1.0))
                                    }),
                            );
                        });
                    });

                    let mut rgb = [
                        state.vertex_color_b[0],
                        state.vertex_color_b[1],
                        state.vertex_color_b[2],
                    ];
                    ui.vertical(|ui| {
                        ui.scope(|ui| {
                            ui.spacing_mut().interact_size = vetex_color_box_size;
                            if ui.color_edit_button_rgb(&mut rgb).changed() {
                                state.vertex_color_b = rgb;
                            }
                        });
                        ui.scope(|ui| {
                            ui.spacing_mut().interact_size = button_size;
                            if ui.button("Select Color").clicked() {}
                        });
                    });

                    state.vertex_color_b[0] = state.vertex_color_b[0].clamp(0.0, 1.0);
                    state.vertex_color_b[1] = state.vertex_color_b[1].clamp(0.0, 1.0);
                    state.vertex_color_b[2] = state.vertex_color_b[2].clamp(0.0, 1.0);

                    ui.separator();
                });
                ui.horizontal(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Strength %:").strong());
                        ui.add_sized(
                            DRAG_SIZE,
                            DragValue::new(&mut state.vertex_strength)
                                .range(0.0..=100.0)
                                .speed(1.0),
                        );
                    });
                    ui.checkbox(&mut state.is_vertex_painting, "Edit Colours");
                });
            });
            ui.add_space(8.0);
        });

    // Apply OS drag-and-drop additions
    if !drop_add.is_empty()
        && let Ok(settings) = world.get_resource_mut::<TerrainSettings>()
    {
        for p in drop_add {
            if !settings.texture_layers.contains(&p) {
                settings.texture_layers.push(p);
            }
        }
        textures_changed = true;
    }

    // Apply committed path edits
    if let Some((idx, new_path)) = commit_path
        && let Ok(settings) = world.get_resource_mut::<TerrainSettings>()
        && idx < settings.texture_layers.len()
    {
        settings.texture_layers[idx] = new_path;
        textures_changed = true;
    }

    if let Ok(e) = world.get_resource_mut::<EditingTexturePath>() {
        e.0 = editing.0;
    }

    if let Ok(s) = world.get_resource_mut::<TerrainToolState>() {
        *s = state;
    }

    if textures_changed {
        world.insert_resource(TerrainAtlasNeedsRebuild);
    }

    if resolution_changed && let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
        settings.resolution = new_resolution.max(4);
    }

    Ok(())
}

fn load_thumbnail(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let img = load_terrain_texture(path);
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture(path, color_image, egui::TextureOptions::LINEAR))
}
