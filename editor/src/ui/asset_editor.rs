use anyhow::Result;
use apostasy_core::{
    assets::asset_manager::AssetManager,
    egui::{self, Color32, Margin, ScrollArea, Stroke, Window},
    objects::world::World,
    ui::ui_context::EguiContext,
    update,
};
use apostasy_macros::Resource;
use std::path::{Path, PathBuf};

use super::{
    EditorStyle,
    assets_panel::ObjectWindowState,
    shared::{WindowLayout, save_layout},
};

// Known asset class names for the "New" dialog.
const CLASS_OPTIONS: &[&str] = &["Voxel", "Biome", "Item", "Structure"];

fn default_template_for_class(class: &str) -> serde_yaml::Value {
    let mut map = serde_yaml::Mapping::new();
    match class {
        "voxel" => {
            let mut tex = serde_yaml::Mapping::new();
            tex.insert("all".into(), "".into());
            map.insert("textures".into(), serde_yaml::Value::Mapping(tex));
            map.insert(
                "components".into(),
                serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            );
        }
        "item" | "structure" => {
            map.insert(
                "components".into(),
                serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            );
        }
        "biome" => {
            map.insert("amplitude".into(), serde_yaml::Value::Number(15.0.into()));
            map.insert(
                "frequency".into(),
                serde_yaml::Value::Number(serde_yaml::Number::from(0.01)),
            );
            map.insert("octaves".into(), serde_yaml::Value::Number(5.into()));
            map.insert(
                "humidity".into(),
                serde_yaml::Value::Number(serde_yaml::Number::from(0.5)),
            );
            map.insert(
                "temperature".into(),
                serde_yaml::Value::Number(serde_yaml::Number::from(0.5)),
            );
            let mut ts = serde_yaml::Mapping::new();
            ts.insert("preset".into(), "flat".into());
            ts.insert(
                "ridge_strength".into(),
                serde_yaml::Value::Number(serde_yaml::Number::from(0.04)),
            );
            ts.insert(
                "peak_strength".into(),
                serde_yaml::Value::Number(serde_yaml::Number::from(0.03)),
            );
            ts.insert(
                "continental_scale".into(),
                serde_yaml::Value::Number(serde_yaml::Number::from(12.0)),
            );
            ts.insert(
                "height_curve".into(),
                serde_yaml::Value::Number(serde_yaml::Number::from(1.0)),
            );
            map.insert("terrain_shaping".into(), serde_yaml::Value::Mapping(ts));
            let mut vox = serde_yaml::Mapping::new();
            vox.insert(
                "surface".into(),
                serde_yaml::Value::Sequence(vec!["".into()]),
            );
            vox.insert(
                "subsurface".into(),
                serde_yaml::Value::Sequence(vec!["".into()]),
            );
            vox.insert(
                "underground".into(),
                serde_yaml::Value::Sequence(vec!["".into()]),
            );
            map.insert("voxel".into(), serde_yaml::Value::Mapping(vox));
            map.insert("structures".into(), serde_yaml::Value::Sequence(vec![]));
            map.insert(
                "default_weather".into(),
                serde_yaml::Value::Sequence(vec![]),
            );
            map.insert("seasons".into(), serde_yaml::Value::Sequence(vec![]));
            let mut ag = serde_yaml::Mapping::new();
            ag.insert(
                "water_color".into(),
                serde_yaml::Value::Sequence(vec![
                    serde_yaml::Value::Number(200.into()),
                    serde_yaml::Value::Number(220.into()),
                    serde_yaml::Value::Number(240.into()),
                ]),
            );
            ag.insert(
                "foliage_color".into(),
                serde_yaml::Value::Sequence(vec![
                    serde_yaml::Value::Number(100.into()),
                    serde_yaml::Value::Number(160.into()),
                    serde_yaml::Value::Number(100.into()),
                ]),
            );
            map.insert("ambient_graphics".into(), serde_yaml::Value::Mapping(ag));
        }
        _ => {}
    }
    serde_yaml::Value::Mapping(map)
}

/// Build a template from an existing asset's YAML, stripping the identity fields.
fn template_from_example(example: &serde_yaml::Value) -> serde_yaml::Value {
    const SKIP: &[&str] = &["name", "namespace", "class"];
    match example {
        serde_yaml::Value::Mapping(m) => {
            let mut result = serde_yaml::Mapping::new();
            for (k, v) in m.iter() {
                let key_str = match k {
                    serde_yaml::Value::String(s) => s.as_str(),
                    _ => continue,
                };
                if SKIP.contains(&key_str) {
                    continue;
                }
                result.insert(k.clone(), v.clone());
            }
            serde_yaml::Value::Mapping(result)
        }
        _ => serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
    }
}

#[derive(Resource, Clone)]
pub struct AssetEditorState {
    pub open: bool,
    /// editor_id of the asset currently shown ("namespace:class:name").
    pub current_id: Option<String>,
    /// Working copy of the YAML being edited.
    pub yaml: Option<serde_yaml::Value>,
    /// Where to write it back.
    pub file_path: Option<PathBuf>,
    pub is_dirty: bool,
    pub error: Option<String>,
    // New-asset dialog
    pub new_open: bool,
    pub new_name: String,
    pub new_namespace: String,
    pub new_class_idx: usize,
    // Add-component picker (main editor)
    pub add_comp_open: bool,
    pub add_comp_search: String,
    // New-asset template editing
    pub new_template: serde_yaml::Value,
    pub new_template_class_idx: usize,
    pub new_add_comp_open: bool,
    pub new_add_comp_search: String,
    // Component clipboard (shared between main editor and new-asset dialog)
    pub comp_clipboard: Option<(String, serde_yaml::Value)>,
}

impl Default for AssetEditorState {
    fn default() -> Self {
        Self {
            open: false,
            current_id: None,
            yaml: None,
            file_path: None,
            is_dirty: false,
            error: None,
            new_open: false,
            new_name: String::new(),
            new_namespace: "game".to_string(),
            new_class_idx: 0,
            add_comp_open: false,
            add_comp_search: String::new(),
            new_template: serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            new_template_class_idx: usize::MAX,
            new_add_comp_open: false,
            new_add_comp_search: String::new(),
            comp_clipboard: None,
        }
    }
}

#[update(mode = "editor")]
pub fn asset_editor(world: &mut World) -> Result<()> {
    if !world.has_resource::<AssetEditorState>() {
        world.insert_resource(AssetEditorState::default());
    }

    let selected_id = world
        .get_resource::<ObjectWindowState>()
        .ok()
        .and_then(|s| s.selected_entry.clone());

    let current_id = world
        .get_resource::<AssetEditorState>()
        .map(|s| s.current_id.clone())
        .unwrap_or(None);

    if current_id != selected_id {
        let loaded = selected_id.as_deref().and_then(find_asset_file);
        let state = world.get_resource_mut::<AssetEditorState>()?;
        state.current_id = selected_id.clone();
        state.is_dirty = false;
        state.error = None;
        match loaded {
            Some((path, yaml)) => {
                state.yaml = Some(yaml);
                state.file_path = Some(path);
            }
            None => {
                state.yaml = None;
                state.file_path = None;
                if selected_id.is_some() {
                    state.error = Some("Asset file not found on disk.".to_string());
                }
            }
        }
    }

    let (open, new_open_flag) = world
        .get_resource::<AssetEditorState>()
        .map(|s| (s.open, s.new_open))
        .unwrap_or((false, false));
    if !open && !new_open_flag {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();
    let layout = world
        .get_resource::<WindowLayout>()
        .cloned()
        .unwrap_or_default();

    // Check for duplicate name/namespace/class before the mutable state borrow.
    let (new_name_chk, new_ns_chk, new_class_chk) = world
        .get_resource::<AssetEditorState>()
        .map(|s| {
            let class = CLASS_OPTIONS
                .get(s.new_class_idx)
                .copied()
                .unwrap_or("")
                .to_lowercase();
            (
                s.new_name.trim().to_lowercase(),
                s.new_namespace.trim().to_lowercase(),
                class,
            )
        })
        .unwrap_or_default();

    let name_taken = !new_name_chk.is_empty()
        && !new_ns_chk.is_empty()
        && world
            .get_resource::<AssetManager>()
            .ok()
            .map(|am| {
                am.all_loader_entries().iter().any(|(class, entries)| {
                    class.to_lowercase() == new_class_chk
                        && entries.iter().any(|(ns, name)| {
                            ns.to_lowercase() == new_ns_chk && name.to_lowercase() == new_name_chk
                        })
                })
            })
            .unwrap_or(false);

    // Pre-compute an example asset for the currently-selected new-asset class.
    // This must happen before the mutable state borrow so we can also read AssetManager.
    // The result is used during template reinit inside the modal to populate all fields.
    let current_new_class_idx = world
        .get_resource::<AssetEditorState>()
        .map(|s| s.new_class_idx)
        .unwrap_or(0);
    let precomputed_example: Option<serde_yaml::Value> = {
        let class = CLASS_OPTIONS
            .get(current_new_class_idx)
            .copied()
            .unwrap_or("")
            .to_lowercase();
        world
            .get_resource::<AssetManager>()
            .ok()
            .and_then(|am| {
                am.all_loader_entries()
                    .iter()
                    .find(|(c, entries)| c.to_lowercase() == class && !entries.is_empty())
                    .and_then(|(c, entries)| {
                        entries
                            .first()
                            .map(|(ns, name)| format!("{}:{}:{}", ns, c, name))
                    })
            })
            .and_then(|id| find_asset_file(&id))
            .map(|(_, yaml)| yaml)
    };

    let state = world.get_resource_mut::<AssetEditorState>()?;
    let mut window_open = state.open;
    let mut do_save = false;
    let mut do_revert = false;
    let mut open_new = false;
    let mut confirm_new: Option<(String, String, String, serde_yaml::Value)> = None; // (name, ns, class, template)
    let mut close_new = false;
    // Local copies so the window closure doesn't fight the yaml/dirty borrows.
    let mut add_comp_open = state.add_comp_open;
    let mut add_comp_search = state.add_comp_search.clone();
    let mut pending_add_comp: Option<String> = None;
    let mut pending_remove_comp: Option<String> = None;
    let mut pending_copy_comp: Option<(String, serde_yaml::Value)> = None;
    let mut pending_paste_comp = false;

    Window::new("Asset Editor")
        .id(egui::Id::new("asset_editor_window"))
        .open(&mut window_open)
        .default_pos(layout.asset_editor.to_pos())
        .default_size(layout.asset_editor.to_size())
        .resizable(true)
        .movable(true)
        .frame(style.window_frame(&ctx).inner_margin(Margin {
            left: 8,
            right: 8,
            bottom: 0,
            top: 0,
        }))
        .show(&ctx, |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(state.is_dirty, egui::Button::new("Save"))
                    .clicked()
                {
                    do_save = true;
                }
                if ui
                    .add_enabled(state.is_dirty, egui::Button::new("Revert"))
                    .clicked()
                {
                    do_revert = true;
                }
                ui.separator();
                if ui.button("New").clicked() {
                    open_new = true;
                }
            });
            ui.separator();

            // Error / no-selection state
            if let Some(ref err) = state.error {
                ui.colored_label(Color32::from_rgb(220, 80, 80), err.as_str());
                return;
            }
            if state.yaml.is_none() {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Select an asset in the Object Window")
                        .italics()
                        .weak(),
                );
                return;
            }

            // File path hint
            if let Some(ref path) = state.file_path {
                let short = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(short).weak().small());
                    if state.is_dirty {
                        ui.label(
                            egui::RichText::new("●")
                                .color(Color32::from_rgb(255, 200, 60))
                                .strong(),
                        );
                    }
                });
                ui.separator();
            }

            // Detect asset class once (shared borrow, dropped before mutable borrow below)
            let asset_class = match &state.yaml {
                Some(serde_yaml::Value::Mapping(m)) => m
                    .get(&serde_yaml::Value::String("class".to_string()))
                    .and_then(|v| v.as_str())
                    .map(str::to_lowercase)
                    .unwrap_or_default(),
                _ => String::new(),
            };
            let supports_components = !asset_class.is_empty() && asset_class != "scene";

            // Property editor
            ScrollArea::vertical()
                .id_salt("asset_editor_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    if let Some(ref mut yaml) = state.yaml {
                        let dirty = &mut state.is_dirty;
                        if let serde_yaml::Value::Mapping(map) = yaml {
                            let keys: Vec<serde_yaml::Value> = map.keys().cloned().collect();
                            for k in &keys {
                                let key_str = yaml_key_str(k);
                                // Handled in the dedicated section below
                                if supports_components && key_str == "components" {
                                    continue;
                                }
                                let child_id = format!("root/{}", key_str);
                                if let Some(v) = map.get_mut(k) {
                                    render_node(ui, &key_str, v, &child_id, dirty, &style);
                                }
                            }

                            if supports_components {
                                // Collect existing names via shared borrow before mutable borrow
                                let existing_comp_names: Vec<String> = map
                                    .get(&serde_yaml::Value::String("components".to_string()))
                                    .and_then(|v| v.as_mapping())
                                    .map(|m| {
                                        m.keys()
                                            .filter_map(|k| k.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                ui.separator();

                                // Section header + paste/add toggles
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Components").strong());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let icon =
                                                if add_comp_open { "▲" } else { "+" };
                                            if ui
                                                .add(egui::Button::new(icon).small())
                                                .on_hover_text("Add component")
                                                .clicked()
                                            {
                                                add_comp_open = !add_comp_open;
                                                add_comp_search.clear();
                                            }
                                            if let Some((ref clip_name, _)) =
                                                state.comp_clipboard
                                            {
                                                let already_has = existing_comp_names
                                                    .iter()
                                                    .any(|n| n.eq_ignore_ascii_case(clip_name));
                                                let tip = if already_has {
                                                    "Already present"
                                                } else {
                                                    "Paste component"
                                                };
                                                if ui
                                                    .add_enabled(
                                                        !already_has,
                                                        egui::Button::new(format!(
                                                            "Paste {}",
                                                            clip_name
                                                        ))
                                                        .small(),
                                                    )
                                                    .on_hover_text(tip)
                                                    .clicked()
                                                {
                                                    pending_paste_comp = true;
                                                }
                                            }
                                        },
                                    );
                                });

                                // Render each component; right-click to copy/remove
                                let comp_key =
                                    serde_yaml::Value::String("components".to_string());
                                if let Some(serde_yaml::Value::Mapping(comp_map)) =
                                    map.get_mut(&comp_key)
                                {
                                    let comp_keys: Vec<serde_yaml::Value> =
                                        comp_map.keys().cloned().collect();
                                    for ck in &comp_keys {
                                        let comp_name = yaml_key_str(ck);
                                        let comp_val_copy = comp_map
                                            .get(ck)
                                            .cloned()
                                            .unwrap_or(serde_yaml::Value::Null);
                                        let frame = egui::Frame::new()
                                            .fill(style.dark_bg)
                                            .stroke(Stroke::new(1.0, style.div_col))
                                            .corner_radius(3.0)
                                            .inner_margin(egui::Margin::symmetric(8, 3));
                                        let fr = frame.show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                if ui
                                                    .add(
                                                        egui::Button::new("−")
                                                            .small()
                                                            .fill(Color32::TRANSPARENT),
                                                    )
                                                    .on_hover_text(format!(
                                                        "Remove {}",
                                                        comp_name
                                                    ))
                                                    .clicked()
                                                {
                                                    pending_remove_comp =
                                                        Some(comp_name.clone());
                                                }
                                                ui.separator();
                                                ui.vertical(|ui| {
                                                    if let Some(v) = comp_map.get_mut(ck) {
                                                        render_node(
                                                            ui,
                                                            &comp_name,
                                                            v,
                                                            &format!("comp/{}", comp_name),
                                                            dirty,
                                                            &style,
                                                        );
                                                    }
                                                });
                                            });
                                        });
                                        fr.response.context_menu(|ui| {
                                            if ui
                                                .button(format!("Copy {}", comp_name))
                                                .clicked()
                                            {
                                                pending_copy_comp = Some((
                                                    comp_name.clone(),
                                                    comp_val_copy.clone(),
                                                ));
                                                ui.close();
                                            }
                                            ui.separator();
                                            if ui
                                                .button(format!("Remove {}", comp_name))
                                                .clicked()
                                            {
                                                pending_remove_comp = Some(comp_name.clone());
                                                ui.close();
                                            }
                                        });
                                    }
                                }

                                // Component picker panel
                                if add_comp_open {
                                    let frame = egui::Frame::new()
                                        .fill(style.dark_bg)
                                        .stroke(Stroke::new(1.0, style.div_col))
                                        .corner_radius(3.0)
                                        .inner_margin(egui::Margin::symmetric(8, 4));
                                    frame.show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut add_comp_search)
                                                .hint_text("Search components…")
                                                .desired_width(f32::INFINITY)
                                                .font(style.font_ui()),
                                        );
                                        let q = add_comp_search.to_lowercase();
                                        ScrollArea::vertical()
                                            .id_salt("comp_picker_scroll")
                                            .max_height(140.0)
                                            .show(ui, |ui| {
                                                use apostasy_core::objects::component::ComponentRegistration;
                                                let mut any = false;
                                                for reg in inventory::iter::<ComponentRegistration>() {
                                                    let name = reg.type_name;
                                                    if (!q.is_empty()
                                                        && !name.to_lowercase().contains(&q))
                                                        || existing_comp_names
                                                            .iter()
                                                            .any(|n| n.eq_ignore_ascii_case(name))
                                                    {
                                                        continue;
                                                    }
                                                    any = true;
                                                    if ui.selectable_label(false, name).clicked()
                                                    {
                                                        pending_add_comp =
                                                            Some(name.to_string());
                                                        add_comp_open = false;
                                                        add_comp_search.clear();
                                                    }
                                                }
                                                if !any {
                                                    ui.label(
                                                        egui::RichText::new("No matches")
                                                            .weak()
                                                            .italics(),
                                                    );
                                                }
                                            });
                                    });
                                }
                            }
                        } else {
                            render_node(ui, "value", yaml, "root", &mut state.is_dirty, &style);
                        }
                    }
                    ui.add_space(8.0);
                });
        });

    // Sync local picker state back
    state.add_comp_open = add_comp_open;
    state.add_comp_search = add_comp_search;

    // Apply pending component mutations (state is still live here, before the New-asset modal)
    if let Some((name, val)) = pending_copy_comp {
        state.comp_clipboard = Some((name, val));
    }
    if pending_paste_comp {
        if let Some((ref name, ref val)) = state.comp_clipboard.clone() {
            let comp_key = serde_yaml::Value::String("components".to_string());
            if let Some(serde_yaml::Value::Mapping(root)) = state.yaml.as_mut() {
                let already = root
                    .get(&comp_key)
                    .and_then(|v| v.as_mapping())
                    .map(|m| m.contains_key(&serde_yaml::Value::String(name.clone())))
                    .unwrap_or(false);
                if !already {
                    if !root.contains_key(&comp_key) {
                        root.insert(
                            comp_key.clone(),
                            serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                        );
                    }
                    if let Some(serde_yaml::Value::Mapping(comp_map)) = root.get_mut(&comp_key) {
                        comp_map.insert(serde_yaml::Value::String(name.clone()), val.clone());
                    }
                    state.is_dirty = true;
                }
            }
        }
    }
    if let Some(ref name) = pending_remove_comp {
        let comp_key = serde_yaml::Value::String("components".to_string());
        if let Some(serde_yaml::Value::Mapping(root)) = state.yaml.as_mut() {
            if let Some(serde_yaml::Value::Mapping(comp_map)) = root.get_mut(&comp_key) {
                comp_map.remove(&serde_yaml::Value::String(name.clone()));
                state.is_dirty = true;
            }
        }
    }
    if let Some(ref name) = pending_add_comp {
        let comp_key = serde_yaml::Value::String("components".to_string());
        if let Some(serde_yaml::Value::Mapping(root)) = state.yaml.as_mut() {
            if !root.contains_key(&comp_key) {
                root.insert(
                    comp_key.clone(),
                    serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                );
            }
            if let Some(serde_yaml::Value::Mapping(comp_map)) = root.get_mut(&comp_key) {
                comp_map.insert(
                    serde_yaml::Value::String(name.clone()),
                    serde_yaml::Value::Null,
                );
            }
            state.is_dirty = true;
        }
    }

    //  New asset
    if state.new_open || open_new {
        if open_new {
            state.new_open = true;
            state.new_name.clear();
            state.new_template_class_idx = usize::MAX;
        }

        // Reinit template when class changes or on first open
        if state.new_class_idx != state.new_template_class_idx {
            let class = CLASS_OPTIONS
                .get(state.new_class_idx)
                .copied()
                .unwrap_or("")
                .to_lowercase();
            if current_new_class_idx == state.new_class_idx {
                // Example was pre-computed for the correct class.
                state.new_template = match precomputed_example {
                    Some(ref ex) => template_from_example(ex),
                    None => default_template_for_class(&class),
                };
                state.new_template_class_idx = state.new_class_idx;
            } else {
                state.new_template = default_template_for_class(&class);
            }
        }

        let mut new_add_comp_open = state.new_add_comp_open;
        let mut new_add_comp_search = state.new_add_comp_search.clone();
        let mut pending_new_add_comp: Option<String> = None;
        let mut pending_new_remove_comp: Option<String> = None;
        let mut pending_new_copy_comp: Option<(String, serde_yaml::Value)> = None;
        let mut pending_new_paste_comp = false;

        let screen = ctx.viewport_rect();
        let pos = egui::pos2(
            (screen.width() - 380.0) * 0.5,
            (screen.height() - 500.0) * 0.5,
        );
        let mut new_modal_open = true;

        egui::Window::new("New Asset")
            .open(&mut new_modal_open)
            .default_pos(pos)
            .default_size(egui::vec2(380.0, 500.0))
            .min_size(egui::vec2(300.0, 260.0))
            .collapsible(false)
            .resizable(true)
            .show(&ctx, |ui| {
                let can_create = !name_taken
                    && !state.new_name.trim().is_empty()
                    && !state.new_namespace.trim().is_empty();

                // Identity fields grid
                egui::Grid::new("new_asset_grid")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(&mut state.new_name)
                            .desired_width(f32::INFINITY));
                        ui.end_row();

                        if name_taken {
                            ui.label("");
                            ui.label(
                                egui::RichText::new("Already exists")
                                    .color(Color32::from_rgb(220, 80, 80))
                                    .small(),
                            );
                            ui.end_row();
                        }

                        ui.label("Namespace");
                        ui.add(egui::TextEdit::singleline(&mut state.new_namespace)
                            .desired_width(f32::INFINITY));
                        ui.end_row();

                        ui.label("Class");
                        egui::ComboBox::from_id_salt("new_class_combo")
                            .selected_text(
                                CLASS_OPTIONS.get(state.new_class_idx).copied().unwrap_or(""),
                            )
                            .show_ui(ui, |ui| {
                                for (i, &c) in CLASS_OPTIONS.iter().enumerate() {
                                    ui.selectable_value(&mut state.new_class_idx, i, c);
                                }
                            });
                        ui.end_row();
                    });

                ui.separator();

                let template_class = CLASS_OPTIONS
                    .get(state.new_class_idx)
                    .copied()
                    .unwrap_or("")
                    .to_lowercase();
                let supports_comps = !template_class.is_empty() && template_class != "scene";

                // Template fields scroll area
                let avail_h = ui.available_height() - 36.0;
                ScrollArea::vertical()
                    .id_salt("new_asset_template_scroll")
                    .auto_shrink([false; 2])
                    .max_height(avail_h.max(80.0))
                    .show(ui, |ui| {
                        let mut tmpl_modified = false;
                        if let serde_yaml::Value::Mapping(map) = &mut state.new_template {
                            let keys: Vec<serde_yaml::Value> = map.keys().cloned().collect();
                            for k in &keys {
                                let key_str = yaml_key_str(k);
                                if supports_comps && key_str == "components" {
                                    continue;
                                }
                                let child_id = format!("new_tmpl/{}", key_str);
                                if let Some(v) = map.get_mut(k) {
                                    render_node(ui, &key_str, v, &child_id, &mut tmpl_modified, &style);
                                }
                            }

                            if supports_comps {
                                let existing_new_comp_names: Vec<String> = map
                                    .get(&serde_yaml::Value::String("components".to_string()))
                                    .and_then(|v| v.as_mapping())
                                    .map(|m| {
                                        m.keys()
                                            .filter_map(|k| k.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                ui.separator();
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Components").strong());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let icon = if new_add_comp_open { "▲" } else { "+" };
                                            if ui
                                                .add(egui::Button::new(icon).small())
                                                .on_hover_text("Add component")
                                                .clicked()
                                            {
                                                new_add_comp_open = !new_add_comp_open;
                                                new_add_comp_search.clear();
                                            }
                                            if let Some((ref clip_name, _)) =
                                                state.comp_clipboard
                                            {
                                                let already_has = existing_new_comp_names
                                                    .iter()
                                                    .any(|n| n.eq_ignore_ascii_case(clip_name));
                                                let tip = if already_has {
                                                    "Already present"
                                                } else {
                                                    "Paste component"
                                                };
                                                if ui
                                                    .add_enabled(
                                                        !already_has,
                                                        egui::Button::new(format!(
                                                            "Paste {}",
                                                            clip_name
                                                        ))
                                                        .small(),
                                                    )
                                                    .on_hover_text(tip)
                                                    .clicked()
                                                {
                                                    pending_new_paste_comp = true;
                                                }
                                            }
                                        },
                                    );
                                });

                                let comp_key =
                                    serde_yaml::Value::String("components".to_string());
                                if let Some(serde_yaml::Value::Mapping(comp_map)) =
                                    map.get_mut(&comp_key)
                                {
                                    let comp_keys: Vec<serde_yaml::Value> =
                                        comp_map.keys().cloned().collect();
                                    for ck in &comp_keys {
                                        let comp_name = yaml_key_str(ck);
                                        let comp_val_copy = comp_map
                                            .get(ck)
                                            .cloned()
                                            .unwrap_or(serde_yaml::Value::Null);
                                        let frame = egui::Frame::new()
                                            .fill(style.dark_bg)
                                            .stroke(Stroke::new(1.0, style.div_col))
                                            .corner_radius(3.0)
                                            .inner_margin(egui::Margin::symmetric(8, 3));
                                        let fr = frame.show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                if ui
                                                    .add(
                                                        egui::Button::new("−")
                                                            .small()
                                                            .fill(Color32::TRANSPARENT),
                                                    )
                                                    .on_hover_text(format!(
                                                        "Remove {}",
                                                        comp_name
                                                    ))
                                                    .clicked()
                                                {
                                                    pending_new_remove_comp =
                                                        Some(comp_name.clone());
                                                }
                                                ui.separator();
                                                ui.vertical(|ui| {
                                                    if let Some(v) = comp_map.get_mut(ck) {
                                                        render_node(
                                                            ui,
                                                            &comp_name,
                                                            v,
                                                            &format!(
                                                                "new_comp/{}",
                                                                comp_name
                                                            ),
                                                            &mut tmpl_modified,
                                                            &style,
                                                        );
                                                    }
                                                });
                                            });
                                        });
                                        fr.response.context_menu(|ui| {
                                            if ui
                                                .button(format!("Copy {}", comp_name))
                                                .clicked()
                                            {
                                                pending_new_copy_comp = Some((
                                                    comp_name.clone(),
                                                    comp_val_copy.clone(),
                                                ));
                                                ui.close();
                                            }
                                            ui.separator();
                                            if ui
                                                .button(format!("Remove {}", comp_name))
                                                .clicked()
                                            {
                                                pending_new_remove_comp =
                                                    Some(comp_name.clone());
                                                ui.close();
                                            }
                                        });
                                    }
                                }

                                if new_add_comp_open {
                                    let frame = egui::Frame::new()
                                        .fill(style.dark_bg)
                                        .stroke(Stroke::new(1.0, style.div_col))
                                        .corner_radius(3.0)
                                        .inner_margin(egui::Margin::symmetric(8, 4));
                                    frame.show(ui, |ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut new_add_comp_search)
                                                .hint_text("Search components…")
                                                .desired_width(f32::INFINITY)
                                                .font(style.font_ui()),
                                        );
                                        let q = new_add_comp_search.to_lowercase();
                                        ScrollArea::vertical()
                                            .id_salt("new_comp_picker_scroll")
                                            .max_height(140.0)
                                            .show(ui, |ui| {
                                                use apostasy_core::objects::component::ComponentRegistration;
                                                let mut any = false;
                                                for reg in
                                                    inventory::iter::<ComponentRegistration>()
                                                {
                                                    let name = reg.type_name;
                                                    if (!q.is_empty()
                                                        && !name.to_lowercase().contains(&q))
                                                        || existing_new_comp_names
                                                            .iter()
                                                            .any(|n| n.eq_ignore_ascii_case(name))
                                                    {
                                                        continue;
                                                    }
                                                    any = true;
                                                    if ui.selectable_label(false, name).clicked()
                                                    {
                                                        pending_new_add_comp =
                                                            Some(name.to_string());
                                                        new_add_comp_open = false;
                                                        new_add_comp_search.clear();
                                                    }
                                                }
                                                if !any {
                                                    ui.label(
                                                        egui::RichText::new("No matches")
                                                            .weak()
                                                            .italics(),
                                                    );
                                                }
                                            });
                                    });
                                }
                            }
                        }
                        ui.add_space(4.0);
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(can_create, egui::Button::new("Create"))
                        .clicked()
                    {
                        let class = CLASS_OPTIONS
                            .get(state.new_class_idx)
                            .copied()
                            .unwrap_or("Voxel")
                            .to_string();
                        confirm_new = Some((
                            state.new_name.trim().to_string(),
                            state.new_namespace.trim().to_string(),
                            class,
                            state.new_template.clone(),
                        ));
                    }
                    if ui.button("Cancel").clicked() {
                        close_new = true;
                    }
                });
            });

        state.new_add_comp_open = new_add_comp_open;
        state.new_add_comp_search = new_add_comp_search;

        if let Some((name, val)) = pending_new_copy_comp {
            state.comp_clipboard = Some((name, val));
        }
        if pending_new_paste_comp {
            if let Some((ref name, ref val)) = state.comp_clipboard.clone() {
                let comp_key = serde_yaml::Value::String("components".to_string());
                if let serde_yaml::Value::Mapping(root) = &mut state.new_template {
                    let already = root
                        .get(&comp_key)
                        .and_then(|v| v.as_mapping())
                        .map(|m| m.contains_key(&serde_yaml::Value::String(name.clone())))
                        .unwrap_or(false);
                    if !already {
                        if !root.contains_key(&comp_key) {
                            root.insert(
                                comp_key.clone(),
                                serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                            );
                        }
                        if let Some(serde_yaml::Value::Mapping(comp_map)) = root.get_mut(&comp_key)
                        {
                            comp_map.insert(serde_yaml::Value::String(name.clone()), val.clone());
                        }
                    }
                }
            }
        }
        if let Some(ref name) = pending_new_remove_comp {
            let comp_key = serde_yaml::Value::String("components".to_string());
            if let serde_yaml::Value::Mapping(root) = &mut state.new_template {
                if let Some(serde_yaml::Value::Mapping(comp_map)) = root.get_mut(&comp_key) {
                    comp_map.remove(&serde_yaml::Value::String(name.clone()));
                }
            }
        }
        if let Some(ref name) = pending_new_add_comp {
            let comp_key = serde_yaml::Value::String("components".to_string());
            if let serde_yaml::Value::Mapping(root) = &mut state.new_template {
                if !root.contains_key(&comp_key) {
                    root.insert(
                        comp_key.clone(),
                        serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
                    );
                }
                if let Some(serde_yaml::Value::Mapping(comp_map)) = root.get_mut(&comp_key) {
                    comp_map.insert(
                        serde_yaml::Value::String(name.clone()),
                        serde_yaml::Value::Null,
                    );
                }
            }
        }

        if !new_modal_open {
            close_new = true;
        }
    }

    if !window_open {
        if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
            s.open = false;
        }
        if let Ok(layout) = world.get_resource_mut::<WindowLayout>() {
            layout.asset_editor_open = false;
        }
        if let Ok(layout) = world.get_resource::<WindowLayout>() {
            save_layout(layout);
        }
        return Ok(());
    }

    if close_new {
        if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
            s.new_open = false;
        }
    }

    if let Some((name, ns, class, template)) = confirm_new {
        create_asset(world, &name, &ns, &class, &template);
        if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
            s.new_open = false;
        }
    }

    if do_save {
        save_asset(world);
    }

    if do_revert {
        revert_asset(world);
    }

    Ok(())
}

fn render_node(
    ui: &mut egui::Ui,
    key: &str,
    value: &mut serde_yaml::Value,
    id: &str,
    modified: &mut bool,
    style: &EditorStyle,
) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            egui::CollapsingHeader::new(egui::RichText::new(key).strong())
                .id_salt(id)
                .default_open(true)
                .show(ui, |ui| {
                    let frame = egui::Frame::new()
                        .fill(style.dark_bg)
                        .stroke(Stroke::new(1.0, style.div_col))
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(8, 4));
                    frame.show(ui, |ui| {
                        let keys: Vec<serde_yaml::Value> = map.keys().cloned().collect();
                        for k in keys {
                            let ks = yaml_key_str(&k);
                            let child_id = format!("{}/{}", id, ks);
                            if let Some(v) = map.get_mut(&k) {
                                render_node(ui, &ks, v, &child_id, modified, style);
                            }
                        }
                    });
                });
        }

        serde_yaml::Value::Sequence(seq) => {
            if seq.len() <= 4 && seq.iter().all(|v| v.is_number()) {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(key).strong());
                    for item in seq.iter_mut() {
                        if let serde_yaml::Value::Number(n) = item {
                            if let Some(f) = n.as_f64() {
                                let mut buf = format_number(f);
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut buf)
                                        .desired_width(60.0)
                                        .font(style.font_ui()),
                                );
                                if resp.changed() {
                                    if let Some(parsed) =
                                        buf.parse::<f64>().ok().map(serde_yaml::Number::from)
                                    {
                                        *n = parsed;
                                        *modified = true;
                                    }
                                }
                            }
                        }
                    }
                });
            } else {
                egui::CollapsingHeader::new(egui::RichText::new(key).strong())
                    .id_salt(id)
                    .default_open(false)
                    .show(ui, |ui| {
                        let frame = egui::Frame::new()
                            .fill(style.dark_bg)
                            .stroke(Stroke::new(1.0, style.div_col))
                            .corner_radius(3.0)
                            .inner_margin(egui::Margin::symmetric(8, 4));
                        frame.show(ui, |ui| {
                            for (i, item) in seq.iter_mut().enumerate() {
                                let child_id = format!("{}/{}", id, i);
                                render_node(ui, &i.to_string(), item, &child_id, modified, style);
                            }
                        });
                    });
            }
        }

        serde_yaml::Value::String(s) => {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(key).strong());
                if ui
                    .add(
                        egui::TextEdit::singleline(s)
                            .desired_width(f32::INFINITY)
                            .font(style.font_ui()),
                    )
                    .changed()
                {
                    *modified = true;
                }
            });
        }

        serde_yaml::Value::Number(n) => {
            let f_val = n.as_f64();
            let n_str = n.to_string();
            let mut buf = f_val.map(format_number).unwrap_or(n_str);
            let mut new_num: Option<serde_yaml::Value> = None;

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(key).strong());
                if f_val.is_some() {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .desired_width(120.0)
                            .font(style.font_ui()),
                    );
                    if resp.changed() {
                        if let Ok(i) = buf.trim().parse::<i64>() {
                            new_num = Some(serde_yaml::Value::Number(i.into()));
                        } else if let Ok(f) = buf.trim().parse::<f64>() {
                            new_num = Some(serde_yaml::Value::Number(serde_yaml::Number::from(f)));
                        }
                    }
                } else {
                    ui.label(&buf);
                }
            });

            if let Some(nv) = new_num {
                *value = nv;
                *modified = true;
            }
        }

        serde_yaml::Value::Bool(b) => {
            ui.horizontal(|ui| {
                if ui.checkbox(b, egui::RichText::new(key).strong()).changed() {
                    *modified = true;
                }
            });
        }

        serde_yaml::Value::Null => {
            let mut buf = String::new();
            let mut new_val: Option<serde_yaml::Value> = None;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(key).strong());
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut buf)
                        .desired_width(f32::INFINITY)
                        .hint_text("null")
                        .font(style.font_ui()),
                );
                if resp.changed() && !buf.is_empty() {
                    let t = buf.trim();
                    if t == "true" {
                        new_val = Some(serde_yaml::Value::Bool(true));
                    } else if t == "false" {
                        new_val = Some(serde_yaml::Value::Bool(false));
                    } else if let Ok(i) = t.parse::<i64>() {
                        new_val = Some(serde_yaml::Value::Number(i.into()));
                    } else if let Ok(f) = t.parse::<f64>() {
                        new_val = Some(serde_yaml::Value::Number(serde_yaml::Number::from(f)));
                    } else {
                        new_val = Some(serde_yaml::Value::String(buf.clone()));
                    }
                }
            });
            if let Some(nv) = new_val {
                *value = nv;
                *modified = true;
            }
        }

        other => {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(key).strong());
                ui.label(egui::RichText::new(format!("{:?}", other)).weak());
            });
        }
    }
}

fn yaml_key_str(k: &serde_yaml::Value) -> String {
    match k {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        _ => format!("{:?}", k),
    }
}

fn format_number(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        // Trim trailing zeros but keep at least one decimal place
        let s = format!("{:.6}", f);
        let s = s.trim_end_matches('0');
        if s.ends_with('.') {
            format!("{}0", s)
        } else {
            s.to_string()
        }
    }
}

//  File discovery

fn find_asset_file(editor_id: &str) -> Option<(PathBuf, serde_yaml::Value)> {
    let parts: Vec<&str> = editor_id.splitn(3, ':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (namespace, _class, name) = (parts[0], parts[1], parts[2]);

    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dirs = [
        base.join("../game/res"),
        base.join("../core/res"),
        base.join("res/scenes"),
    ];

    for dir in &dirs {
        if let Some(found) = scan_dir(dir, namespace, name) {
            return Some(found);
        }
    }
    None
}

fn scan_dir(dir: &Path, namespace: &str, name: &str) -> Option<(PathBuf, serde_yaml::Value)> {
    if !dir.exists() {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = scan_dir(&path, namespace, name) {
                return Some(found);
            }
        } else if path.extension().map_or(false, |e| e == "yaml") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    let y_name = yaml.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let y_ns = yaml.get("namespace").and_then(|v| v.as_str()).unwrap_or("");
                    if y_name.eq_ignore_ascii_case(name) && y_ns.eq_ignore_ascii_case(namespace) {
                        return Some((path, yaml));
                    }
                }
            }
        }
    }
    None
}

//  Save / revert / create

fn save_asset(world: &mut World) {
    let (yaml, path) = world
        .get_resource::<AssetEditorState>()
        .map(|s| (s.yaml.clone(), s.file_path.clone()))
        .unwrap_or((None, None));

    if let (Some(yaml), Some(path)) = (yaml, path) {
        match serde_yaml::to_string(&yaml) {
            Ok(text) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, &text) {
                    Ok(()) => {
                        if let Ok(am) = world.get_resource_mut::<AssetManager>() {
                            let _ = am.load_file(&path);
                        }
                        if let Ok(ow) = world.get_resource_mut::<ObjectWindowState>() {
                            ow.is_first_frame = true;
                        }
                        if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
                            s.is_dirty = false;
                            s.error = None;
                        }
                    }
                    Err(e) => {
                        if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
                            s.error = Some(format!("Save failed: {}", e));
                        }
                    }
                }
            }
            Err(e) => {
                if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
                    s.error = Some(format!("Serialize error: {}", e));
                }
            }
        }
    }
}

fn revert_asset(world: &mut World) {
    let current_id = world
        .get_resource::<AssetEditorState>()
        .ok()
        .and_then(|s| s.current_id.clone());

    if let Some(ref id) = current_id {
        let loaded = find_asset_file(id);
        if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
            match loaded {
                Some((path, yaml)) => {
                    s.yaml = Some(yaml);
                    s.file_path = Some(path);
                    s.is_dirty = false;
                    s.error = None;
                }
                None => {
                    s.error = Some("Could not find file to revert.".to_string());
                }
            }
        }
    }
}

fn create_asset(
    world: &mut World,
    name: &str,
    namespace: &str,
    class: &str,
    template: &serde_yaml::Value,
) {
    // Start from the user-configured template, then stamp in identity fields.
    let mut map = match template {
        serde_yaml::Value::Mapping(m) => m.clone(),
        _ => serde_yaml::Mapping::new(),
    };
    map.insert("name".into(), name.into());
    map.insert("namespace".into(), namespace.into());
    map.insert("class".into(), class.into());
    let yaml = serde_yaml::Value::Mapping(map);

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../game/res")
        .join(class.to_lowercase());
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}.yaml", name.to_lowercase()));

    match serde_yaml::to_string(&yaml) {
        Ok(text) => match std::fs::write(&path, &text) {
            Ok(()) => {
                if let Ok(am) = world.get_resource_mut::<AssetManager>() {
                    let _ = am.load_file(&path);
                }
                if let Ok(ow) = world.get_resource_mut::<ObjectWindowState>() {
                    ow.is_first_frame = true;
                }
                // Load the new asset into the editor immediately.
                if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
                    s.yaml = Some(yaml);
                    s.file_path = Some(path);
                    s.current_id = Some(format!("{}:{}:{}", namespace, class, name));
                    s.is_dirty = false;
                    s.error = None;
                }
            }
            Err(e) => {
                if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
                    s.error = Some(format!("Create failed: {}", e));
                }
            }
        },
        Err(e) => {
            if let Ok(s) = world.get_resource_mut::<AssetEditorState>() {
                s.error = Some(format!("Serialize error: {}", e));
            }
        }
    }
}
