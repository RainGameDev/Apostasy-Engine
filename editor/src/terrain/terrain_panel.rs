use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use apostasy_core::{
    egui::{self, DragAndDrop, Margin, RichText, Slider, TextEdit, Vec2, Window, Color32},
    objects::world::World,
    terrain::{TerrainAtlasNeedsRebuild, TerrainSettings, load_terrain_texture},
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
    let mut new_resolution = resolution;
    let mut resolution_changed = false;

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

    let mut editing = world
        .get_resource::<EditingTexturePath>()
        .cloned()
        .unwrap_or(EditingTexturePath(None));
    let mut commit_path: Option<(usize, String)> = None;

    Window::new("Terrain")
        .open(&mut true)
        .resizable(true)
        .collapsible(true)
        .default_width(280.0)
        .frame(style.window_frame(&ctx).inner_margin(Margin {
            left: 8,
            right: 8,
            bottom: 0,
            top: 0,
        }))
        .show(&ctx, |ui| {
            ui.label(RichText::new("Tool").strong());
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tool, TerrainTool::Raise, "Raise");
                ui.selectable_value(&mut state.tool, TerrainTool::Lower, "Lower");
                ui.selectable_value(&mut state.tool, TerrainTool::Smooth, "Smooth");
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tool, TerrainTool::Flatten, "Flatten");
                ui.selectable_value(&mut state.tool, TerrainTool::Paint, "Paint");
            });

            ui.separator();

            ui.label(RichText::new("Brush").strong());
            ui.horizontal(|ui| {
                ui.label("Radius");
                ui.add(Slider::new(&mut state.brush_radius, 1.0..=64.0).suffix("u"));
            });
            ui.horizontal(|ui| {
                ui.label("Strength");
                ui.add(Slider::new(&mut state.brush_strength, 0.01..=1.0));
            });

            if state.tool == TerrainTool::Flatten {
                if let Some(h) = state.flatten_height {
                    ui.label(format!("Target height: {:.2}", h));
                } else {
                    ui.label("Click to sample height");
                }
            }

            if state.tool == TerrainTool::Paint {
                let texture_paths = world
                    .get_resource::<TerrainSettings>()
                    .map(|s| s.texture_layers.clone())
                    .unwrap_or_default();

                ui.separator();
                ui.label(RichText::new("Textures").strong());

                let thumb_size = Vec2::new(48.0, 48.0);

                // Ensure thumbnails are cached
                let mut cache = world.get_resource_mut::<ThumbnailCache>().ok();
                if let Some(ref mut cache) = cache {
                    for path in &texture_paths {
                        if !cache.handles.contains_key(path) {
                            if let Some(handle) = load_thumbnail(&ctx, path) {
                                cache.handles.insert(path.clone(), handle);
                            }
                        }
                    }
                    cache.handles.retain(|k, _| texture_paths.contains(k));
                }

                // Check if anything is being dragged from the assets panel
                let has_drag = DragAndDrop::has_payload_of_type::<String>(ui.ctx());

                // Texture grid
                let spacing = 6.0;
                let item_w = thumb_size.x + spacing;
                let available = ui.available_width();
                let n_cols = ((available / item_w).floor().max(1.0)) as usize;

                let mut i = 0;
                while i < texture_paths.len() {
                    ui.horizontal(|ui| {
                        let end = (i + n_cols).min(texture_paths.len());
                        for idx in i..end {
                            let path = &texture_paths[idx];
                            let selected = state.paint_layer == idx;

                            // Wrap the slot in a Frame so its response rect matches the visuals
                            let frame = egui::Frame::new()
                                .fill(Color32::from_gray(50))
                                .inner_margin(Margin::same(2));
                            let frame_resp = frame
                                .show(ui, |ui| {
                                    ui.set_min_size(thumb_size);

                                    // Selection highlight border
                                    if selected {
                                        let r = ui.max_rect();
                                        ui.painter().rect_stroke(
                                            r.shrink(1.0),
                                            2.0,
                                            egui::Stroke::new(2.0, Color32::from_rgb(60, 120, 220)),
                                            egui::StrokeKind::Inside,
                                        );
                                    }

                                    // Thumbnail
                                    let (thumb_rect, thumb_resp) = ui.allocate_exact_size(
                                        thumb_size,
                                        egui::Sense::click(),
                                    );
                                    let cache = world.get_resource::<ThumbnailCache>().ok();
                                    if let Some(ref cache) = cache {
                                        if let Some(handle) = cache.handles.get(path) {
                                            ui.put(
                                                thumb_rect,
                                                egui::Image::new((handle.id(), thumb_size)),
                                            );
                                        } else {
                                            let name = Path::new(path)
                                                .file_stem()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or(path);
                                            ui.put(
                                                thumb_rect,
                                                egui::Label::new(
                                                    RichText::new(name).size(10.0).weak(),
                                                ),
                                            );
                                        }
                                    }
                                    if thumb_resp.clicked() {
                                        state.paint_layer = idx;
                                    }

                                    // Path text or inline editor, and remove button
                                    ui.vertical(|ui| {
                                        if editing.0 == Some(idx) {
                                            let mut buf = path.clone();
                                            let resp = ui.add(
                                                TextEdit::singleline(&mut buf)
                                                    .desired_width(thumb_size.x)
                                                    .font(egui::TextStyle::Small),
                                            );
                                            if resp.lost_focus()
                                                || ctx.input(|i| i.key_pressed(egui::Key::Enter))
                                            {
                                                if buf != *path && !buf.is_empty() {
                                                    commit_path = Some((idx, buf));
                                                }
                                                editing.0 = None;
                                            }
                                        } else {
                                            let short = Path::new(path)
                                                .file_name()
                                                .and_then(|s| s.to_str())
                                                .unwrap_or(path);
                                            let label = RichText::new(short).size(9.0).weak();
                                            if ui
                                                .add(
                                                    egui::Label::new(label)
                                                        .sense(egui::Sense::click()),
                                                )
                                                .clicked()
                                            {
                                                editing.0 = Some(idx);
                                            }
                                        }

                                        if texture_paths.len() > 1 {
                                            if ui
                                                .add(
                                                    egui::Button::new("✕")
                                                        .small()
                                                        .fill(Color32::from_gray(80)),
                                                )
                                                .clicked()
                                            {
                                                if let Ok(settings) =
                                                    world.get_resource_mut::<TerrainSettings>()
                                                {
                                                    settings.texture_layers.remove(idx);
                                                    textures_changed = true;
                                                }
                                            }
                                        }
                                    });
                                })
                                .response;

                            // DnD hover: overlay green highlight
                            if has_drag && ui.rect_contains_pointer(frame_resp.rect) {
                                ui.painter().rect_filled(
                                    frame_resp.rect,
                                    2.0,
                                    Color32::from_rgba_premultiplied(60, 180, 60, 120),
                                );
                                if let Some(payload) =
                                    frame_resp.dnd_release_payload::<String>()
                                {
                                    let id_str = (*payload).clone();
                                    let new_path =
                                        id_str.strip_prefix("texture:").unwrap_or(&id_str).to_string();
                                    if !new_path.is_empty() {
                                        if let Ok(settings) =
                                            world.get_resource_mut::<TerrainSettings>()
                                        {
                                            if idx < settings.texture_layers.len() {
                                                settings.texture_layers[idx] = new_path;
                                                textures_changed = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                    i += n_cols;
                }

                // Add texture button
                ui.horizontal(|ui| {
                    if ui.button("+ Add Texture").clicked() {
                        if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
                            settings
                                .texture_layers
                                .push("textures/grass.png".to_string());
                            textures_changed = true;
                        }
                    }
                });

                // Show active texture
                if let Some(path) = texture_paths.get(state.paint_layer) {
                    ui.label(RichText::new(format!("Active: {}", path)).size(10.0).weak());
                }
            }

            ui.separator();
            ui.label(RichText::new("Settings").strong());
            ui.horizontal(|ui| {
                ui.label("Resolution");
                if ui
                    .add(egui::DragValue::new(&mut new_resolution).range(4_u32..=512))
                    .changed()
                {
                    resolution_changed = true;
                }
            });
        });

    // Apply OS drag-and-drop additions
    if !drop_add.is_empty() {
        if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
            for p in drop_add {
                if !settings.texture_layers.contains(&p) {
                    settings.texture_layers.push(p);
                }
            }
            textures_changed = true;
        }
    }

    // Apply committed path edits
    if let Some((idx, new_path)) = commit_path {
        if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
            if idx < settings.texture_layers.len() {
                settings.texture_layers[idx] = new_path;
                textures_changed = true;
            }
        }
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

    if resolution_changed {
        if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
            settings.resolution = new_resolution.max(4);
        }
    }

    Ok(())
}

fn load_thumbnail(ctx: &egui::Context, path: &str) -> Option<egui::TextureHandle> {
    let img = load_terrain_texture(path);
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture(path, color_image, egui::TextureOptions::LINEAR))
}
