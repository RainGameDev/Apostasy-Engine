use anyhow::Result;
use apostasy_core::{
    egui,
    objects::{
        resources::input_manager::{InputManager, KeyBind, ModifierKeys},
        world::World,
        worldspace_streaming::{MAX_RENDER_DISTANCE, MIN_RENDER_DISTANCE, WorldspaceStreaming},
    },
    rendering::shared::{
        UpdateRenderer,
        anti_alisaing::{AntiAliasing, AntiAliasingAmount},
        shadow_settings::ShadowDistance,
    },
    ui::{
        FontRegistry,
        ui_context::{EguiContext, ViewportSize},
    },
    update,
    winit::keyboard::PhysicalKey,
};
use apostasy_macros::Resource;

use super::{
    EditorStyle,
    keybind_widget::{KeybindCapture, is_modifier_key, keybind_editor, pretty_bind},
    style::Theme,
};
use crate::{objects::editor_camera::EditorCameraSettings, ui::viewport_panel::EditorGraphics};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct EditorPreferences {
    pub theme: Theme,
    pub font_size: u8,
    pub active_font: String,
    #[serde(default = "EditorPreferences::default_camera_speed")]
    pub camera_speed: f32,
    #[serde(default = "EditorPreferences::default_render_distance")]
    pub render_distance: i32,
    #[serde(default = "EditorPreferences::default_shadow_distance")]
    pub shadow_distance: f32,
    #[serde(default = "EditorPreferences::default_cascade_count")]
    pub cascade_count: usize,
    #[serde(default = "EditorPreferences::default_bias_constant")]
    pub bias_constant: f32,
    #[serde(default = "EditorPreferences::default_bias_slope")]
    pub bias_slope: f32,
    #[serde(default = "EditorPreferences::default_shadow_map_size")]
    pub shadow_map_size: u32,
    #[serde(default)]
    pub last_scene: String,
}

impl Default for EditorPreferences {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            font_size: 13,
            active_font: String::new(),
            camera_speed: 5.0,
            render_distance: Self::default_render_distance(),
            shadow_distance: Self::default_shadow_distance(),
            cascade_count: Self::default_cascade_count(),
            bias_constant: Self::default_bias_constant(),
            bias_slope: Self::default_bias_slope(),
            shadow_map_size: Self::default_shadow_map_size(),
            last_scene: String::new(),
        }
    }
}

impl EditorPreferences {
    pub const PATH: &'static str = "res/.editor/editor_preferences.yaml";

    fn default_camera_speed() -> f32 {
        5.0
    }

    fn default_render_distance() -> i32 {
        8
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::PATH)
            .ok()
            .and_then(|s| serde_yaml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = std::path::Path::new(Self::PATH);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(yaml) = serde_yaml::to_string(self) {
            let _ = std::fs::write(path, yaml);
        }
    }

    pub fn save_camera_speed(speed: f32) {
        let mut prefs = Self::load();
        prefs.camera_speed = speed;
        prefs.save();
    }

    pub fn save_last_scene(name: &str) {
        let mut prefs = Self::load();
        prefs.last_scene = name.to_string();
        prefs.save();
    }

    fn default_shadow_distance() -> f32 {
        128.0
    }

    fn default_cascade_count() -> usize {
        4
    }

    fn default_bias_constant() -> f32 {
        2.0
    }

    fn default_bias_slope() -> f32 {
        2.0
    }

    fn default_shadow_map_size() -> u32 {
        2048
    }

    pub fn save_render_distance(distance: i32) {
        let mut prefs = Self::load();
        prefs.render_distance = distance;
        prefs.save();
    }

    pub fn save_shadow_distance(distance: f32) {
        let mut prefs = Self::load();
        prefs.shadow_distance = distance;
        prefs.save();
    }

    pub fn save_cascade_count(count: usize) {
        let mut prefs = Self::load();
        prefs.cascade_count = count;
        prefs.save();
    }

    pub fn save_bias(constant: f32, slope: f32) {
        let mut prefs = Self::load();
        prefs.bias_constant = constant;
        prefs.bias_slope = slope;
        prefs.save();
    }

    pub fn save_shadow_map_size(size: u32) {
        let mut prefs = Self::load();
        prefs.shadow_map_size = size;
        prefs.save();
    }
}

struct PageMeta {
    label: &'static str,
    terms: &'static [&'static str],
}

const PAGES: &[PageMeta] = &[
    PageMeta {
        label: "Editor",
        terms: &["Theme", "Font", "Font Size"],
    },
    PageMeta {
        label: "Viewport",
        terms: &["Camera Speed"],
    },
    PageMeta {
        label: "Graphics",
        terms: &[
            "SSAA",
            "MSAA",
            "Render Distance",
            "Shadow",
            "Cascades",
            "Bias",
            "Resolution",
            "Rendering",
            "Anti-Aliasing",
        ],
    },
    PageMeta {
        label: "Keybinds",
        terms: &[
            "Move",
            "Rotate",
            "Scale",
            "Snap",
            "Undo",
            "Redo",
            "Copy",
            "Paste",
            "Duplicate",
            "Control",
            "Modifier",
        ],
    },
];

fn page_matches(page: &PageMeta, query: &str) -> bool {
    query.is_empty()
        || page.label.to_lowercase().contains(query)
        || page.terms.iter().any(|t| t.to_lowercase().contains(query))
}
fn draw_setting(
    ui: &mut egui::Ui,
    dim_col: egui::Color32,
    name: &str,
    query: &str,
    draw: impl FnOnce(&mut egui::Ui),
) {
    if !query.is_empty() && !name.to_lowercase().contains(query) {
        return;
    }
    const LABEL_W: f32 = 160.0;
    ui.horizontal(|ui| {
        ui.add_sized(
            egui::Vec2::new(LABEL_W, ui.spacing().interact_size.y),
            egui::Label::new(egui::RichText::new(name).color(dim_col)),
        );
        draw(ui);
    });
    ui.add_space(4.0);
}

fn draw_section_header(
    ui: &mut egui::Ui,
    dim_col: egui::Color32,
    div_col: egui::Color32,
    label: &str,
    query: &str,
) {
    if !query.is_empty() && !label.to_lowercase().contains(query) {
        return;
    }
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let resp = ui.label(
            egui::RichText::new(label.to_uppercase())
                .color(dim_col)
                .small()
                .strong(),
        );
        ui.add_space(4.0);
        let line_w = (ui.available_width() - 8.0).max(0.0);
        if line_w > 0.0 {
            let (rect, _) =
                ui.allocate_at_least(egui::Vec2::new(line_w, 1.0), egui::Sense::hover());
            ui.painter().hline(
                rect.x_range(),
                resp.rect.center().y,
                egui::Stroke::new(1.0, div_col),
            );
        }
    });
    ui.add_space(6.0);
}
// Editor tab

struct EditorPage {
    theme: Theme,
    font_size: u8,
    font: String,
    available_fonts: Vec<String>,
}

impl EditorPage {
    fn from_world(world: &World) -> Self {
        let style = world
            .get_resource::<EditorStyle>()
            .ok()
            .cloned()
            .unwrap_or_default();
        let fonts = world
            .get_resource::<FontRegistry>()
            .ok()
            .cloned()
            .unwrap_or_default();
        Self {
            theme: style.theme,
            font_size: style.font_size,
            font: fonts.active,
            available_fonts: fonts.fonts,
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui, style: &EditorStyle, query: &str) {
        let text_col = style.text_col;
        let dim_col = style.dim_col;

        draw_setting(ui, dim_col, "Theme", query, |ui| {
            egui::ComboBox::from_id_salt("theme_combo")
                .selected_text(egui::RichText::new(self.theme.label()).color(text_col))
                .show_ui(ui, |ui| {
                    for theme in [
                        Theme::Dark,
                        Theme::Light,
                        Theme::AyuDark,
                        Theme::AyuMirage,
                        Theme::AyuLight,
                        Theme::GithubDark,
                        Theme::GithubDarkDimmed,
                        Theme::GithubLight,
                        Theme::NightOwl,
                        Theme::LightOwl,
                        Theme::OneDark,
                        Theme::SlackDark,
                        Theme::Rogue,
                        Theme::TokyoNight,
                        Theme::TokyoNightStorm,
                        Theme::Nord,
                        Theme::Dracula,
                        Theme::SolarizedDark,
                        Theme::SolarizedLight,
                        Theme::Monokai,
                        Theme::RosePine,
                        Theme::RosePineDawn,
                        Theme::CatppuccinMocha,
                        Theme::CatppuccinMacchiato,
                        Theme::CatppuccinFrappe,
                        Theme::CatppuccinLatte,
                        Theme::GruvboxDarkHard,
                        Theme::GruvboxDarkMedium,
                        Theme::GruvboxDarkSoft,
                        Theme::GruvboxLightHard,
                        Theme::GruvboxLightMedium,
                        Theme::GruvboxLightSoft,
                    ] {
                        ui.selectable_value(&mut self.theme, theme, theme.label());
                    }
                });
        });

        draw_setting(ui, dim_col, "Font Size", query, |ui| {
            ui.add(egui::Slider::new(&mut self.font_size, 8u8..=16u8).integer());
        });

        if !self.available_fonts.is_empty() {
            let fonts = self.available_fonts.clone();
            draw_setting(ui, dim_col, "Font", query, |ui| {
                egui::ComboBox::from_id_salt("font_combo")
                    .selected_text(egui::RichText::new(self.font.as_str()).color(text_col))
                    .show_ui(ui, |ui| {
                        for font in &fonts {
                            ui.selectable_value(&mut self.font, font.clone(), font.as_str());
                        }
                    });
            });
        }
    }

    fn apply(&self, world: &mut World) -> Result<()> {
        let style = world
            .get_resource::<EditorStyle>()
            .ok()
            .cloned()
            .unwrap_or_default();
        let fonts = world
            .get_resource::<FontRegistry>()
            .ok()
            .cloned()
            .unwrap_or_default();

        let style_changed = self.theme != style.theme || self.font_size != style.font_size;
        let font_changed = self.font != fonts.active;

        if style_changed {
            let mut new_style = EditorStyle::from_theme(self.theme);
            new_style.font_size = self.font_size;
            world.insert_resource(new_style);
        }
        if font_changed {
            if let Ok(reg) = world.get_resource_mut::<FontRegistry>() {
                reg.set_active(self.font.clone());
            }
        }
        if style_changed || font_changed {
            let mut prefs = EditorPreferences::load();
            prefs.theme = self.theme;
            prefs.font_size = self.font_size;
            prefs.active_font = self.font.clone();
            prefs.save();
        }
        Ok(())
    }
}

// Viewport tab

struct ViewportPage {
    camera_speed: f32,
}

impl ViewportPage {
    fn from_world(world: &World) -> Self {
        Self {
            camera_speed: world
                .get_resource::<EditorCameraSettings>()
                .map(|s| s.move_speed)
                .unwrap_or(5.0),
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui, style: &EditorStyle, query: &str) {
        draw_setting(ui, style.dim_col, "Camera Speed", query, |ui| {
            ui.add(egui::Slider::new(&mut self.camera_speed, 1.0_f32..=256.0).suffix(" m/s"));
        });
    }

    fn apply(&self, world: &mut World) -> Result<()> {
        let current = world
            .get_resource::<EditorCameraSettings>()
            .map(|s| s.move_speed)
            .unwrap_or(5.0);
        if (self.camera_speed - current).abs() > f32::EPSILON {
            if let Ok(s) = world.get_resource_mut::<EditorCameraSettings>() {
                s.move_speed = self.camera_speed;
            }
            EditorPreferences::save_camera_speed(self.camera_speed);
        }
        Ok(())
    }
}

// Graphics tab

struct GraphicsPage {
    supersample: f32,
    anti_aliasing: AntiAliasingAmount,
    available_aa: Vec<AntiAliasingAmount>,
    render_distance: i32,
    shadow_distance: f32,
    cascade_count: usize,
    bias_constant: f32,
    bias_slope: f32,
    shadow_map_size: u32,
}

impl GraphicsPage {
    fn from_world(world: &World) -> Self {
        let (available_aa, anti_aliasing) = world
            .get_resource::<AntiAliasing>()
            .map(|aa| (aa.available_options.clone(), aa.amount))
            .unwrap_or_default();
        Self {
            supersample: world
                .get_resource::<ViewportSize>()
                .map(|v| v.supersample)
                .unwrap_or(1.0),
            anti_aliasing,
            available_aa,
            render_distance: world
                .get_resource::<WorldspaceStreaming>()
                .map(|s| s.render_distance)
                .unwrap_or_else(|_| EditorPreferences::load().render_distance),
            shadow_distance: world
                .get_resource::<ShadowDistance>()
                .map(|s| s.distance)
                .unwrap_or(EditorPreferences::default().shadow_distance),
            cascade_count: world
                .get_resource::<ShadowDistance>()
                .map(|s| s.cascade_count)
                .unwrap_or(EditorPreferences::default().cascade_count),
            bias_constant: world
                .get_resource::<ShadowDistance>()
                .map(|s| s.bias_constant)
                .unwrap_or(EditorPreferences::default().bias_constant),
            bias_slope: world
                .get_resource::<ShadowDistance>()
                .map(|s| s.bias_slope)
                .unwrap_or(EditorPreferences::default().bias_slope),
            shadow_map_size: world
                .get_resource::<ShadowDistance>()
                .map(|s| s.shadow_map_size)
                .unwrap_or(EditorPreferences::default().shadow_map_size),
        }
    }

    fn draw(&mut self, ui: &mut egui::Ui, style: &EditorStyle, query: &str) {
        let (text_col, dim_col, div_col) = (style.text_col, style.dim_col, style.div_col);

        draw_section_header(ui, dim_col, div_col, "Rendering", query);

        draw_setting(ui, dim_col, "Render Distance", query, |ui| {
            ui.add(
                egui::Slider::new(
                    &mut self.render_distance,
                    MIN_RENDER_DISTANCE..=MAX_RENDER_DISTANCE,
                )
                .integer()
                .suffix(" cells"),
            );
        });

        draw_section_header(ui, dim_col, div_col, "Shadows", query);

        draw_setting(ui, dim_col, "Shadow Distance", query, |ui| {
            ui.add(
                egui::Slider::new(&mut self.shadow_distance, 8.0_f32..=1024.0)
                    .suffix(" units")
                    .logarithmic(true),
            );
        });

        draw_setting(ui, dim_col, "Shadow Cascades", query, |ui| {
            let label = match self.cascade_count {
                1 => "1 — no CSM",
                2 => "2",
                3 => "3",
                _ => "4",
            };
            egui::ComboBox::from_id_salt("cascade_count_combo")
                .selected_text(egui::RichText::new(label).color(text_col))
                .show_ui(ui, |ui| {
                    for (count, lbl) in [(1, "1 — no CSM"), (2, "2"), (3, "3"), (4, "4")] {
                        ui.selectable_value(&mut self.cascade_count, count, lbl);
                    }
                });
        });

        draw_setting(ui, dim_col, "Shadow Map Resolution", query, |ui| {
            let mb = |s: u32| s as u64 * s as u64 * 4 / 1_000_000;
            let label = format!(
                "{} — {} MB/cascade",
                self.shadow_map_size,
                mb(self.shadow_map_size)
            );
            egui::ComboBox::from_id_salt("shadow_map_size_combo")
                .selected_text(egui::RichText::new(label).color(text_col))
                .width(220.0)
                .show_ui(ui, |ui| {
                    for size in [512u32, 1024, 2048, 4096, 8192, 16384] {
                        let lbl = format!("{size} — {} MB/cascade", mb(size));
                        ui.selectable_value(&mut self.shadow_map_size, size, lbl);
                    }
                });
        });

        draw_setting(ui, dim_col, "Depth Bias Constant", query, |ui| {
            ui.add(egui::Slider::new(&mut self.bias_constant, 0.0_f32..=10.0).step_by(0.1));
        });

        draw_setting(ui, dim_col, "Depth Bias Slope", query, |ui| {
            ui.add(egui::Slider::new(&mut self.bias_slope, 0.0_f32..=10.0).step_by(0.1));
        });

        draw_section_header(ui, dim_col, div_col, "Anti-Aliasing", query);

        draw_setting(ui, dim_col, "Supersampling (SSAA)", query, |ui| {
            ui.add(egui::Slider::new(&mut self.supersample, 1.0_f32..=4.0));
        });

        let available = self.available_aa.clone();
        let aa_label = match self.anti_aliasing {
            AntiAliasingAmount::X0 => "None",
            AntiAliasingAmount::X2 => "X2",
            AntiAliasingAmount::X4 => "X4",
            AntiAliasingAmount::X8 => "X8",
        };
        draw_setting(ui, dim_col, "Anti-Aliasing (MSAA)", query, |ui| {
            egui::ComboBox::from_id_salt("prefs_msaa_combo")
                .selected_text(egui::RichText::new(aa_label).color(text_col))
                .show_ui(ui, |ui| {
                    for (amount, label) in [
                        (AntiAliasingAmount::X0, "None"),
                        (AntiAliasingAmount::X2, "X2"),
                        (AntiAliasingAmount::X4, "X4"),
                        (AntiAliasingAmount::X8, "X8"),
                    ] {
                        if available.contains(&amount) {
                            ui.selectable_value(&mut self.anti_aliasing, amount, label);
                        }
                    }
                });
        });
    }

    fn apply(&self, world: &mut World) -> Result<()> {
        let (aa_cur, ss_cur) = world
            .get_resource::<AntiAliasing>()
            .map(|aa| {
                (
                    aa.amount,
                    world
                        .get_resource::<ViewportSize>()
                        .map(|v| v.supersample)
                        .unwrap_or(1.0),
                )
            })
            .unwrap_or_default();

        let rd_cur = world
            .get_resource::<WorldspaceStreaming>()
            .map(|s| s.render_distance)
            .unwrap_or(self.render_distance);
        let rd_changed = self.render_distance != rd_cur;
        if rd_changed {
            if let Ok(s) = world.get_resource_mut::<WorldspaceStreaming>() {
                s.set_render_distance(self.render_distance);
            }
        }

        let (sd_cur, cc_cur, bc_cur, bs_cur, sms_cur) = world
            .get_resource::<ShadowDistance>()
            .map(|s| {
                (
                    s.distance,
                    s.cascade_count,
                    s.bias_constant,
                    s.bias_slope,
                    s.shadow_map_size,
                )
            })
            .unwrap_or((
                self.shadow_distance,
                self.cascade_count,
                self.bias_constant,
                self.bias_slope,
                self.shadow_map_size,
            ));
        let shadow_changed = (self.shadow_distance - sd_cur).abs() > f32::EPSILON
            || self.cascade_count != cc_cur
            || (self.bias_constant - bc_cur).abs() > f32::EPSILON
            || (self.bias_slope - bs_cur).abs() > f32::EPSILON
            || self.shadow_map_size != sms_cur;
        if shadow_changed {
            if let Ok(s) = world.get_resource_mut::<ShadowDistance>() {
                s.distance = self.shadow_distance;
                s.cascade_count = self.cascade_count;
                s.bias_constant = self.bias_constant;
                s.bias_slope = self.bias_slope;
                s.shadow_map_size = self.shadow_map_size;
            }
        }

        let aa_changed = self.anti_aliasing != aa_cur;
        let ss_changed = (self.supersample - ss_cur).abs() > f32::EPSILON;
        if aa_changed {
            if let Ok(aa) = world.get_resource_mut::<AntiAliasing>() {
                aa.amount = self.anti_aliasing;
            }
            world.insert_resource(UpdateRenderer);
        }
        if ss_changed {
            if let Ok(vs) = world.get_resource_mut::<ViewportSize>() {
                vs.supersample = self.supersample;
            }
        }

        if rd_changed || shadow_changed {
            let mut prefs = EditorPreferences::load();
            prefs.render_distance = self.render_distance;
            prefs.shadow_distance = self.shadow_distance;
            prefs.cascade_count = self.cascade_count;
            prefs.bias_constant = self.bias_constant;
            prefs.bias_slope = self.bias_slope;
            prefs.shadow_map_size = self.shadow_map_size;
            prefs.save();
        }
        if aa_changed || ss_changed {
            EditorGraphics {
                supersample: self.supersample,
                anti_aliasing: self.anti_aliasing,
            }
            .save();
        }
        Ok(())
    }
}

// Keybinds tab

/// Ordered list of (bind_name, display_label) shown in the Keybinds tab.
const KEYBIND_DISPLAY: &[(&str, &str)] = &[
    ("GizmoTranslate", "Gizmo: Move"),
    ("GizmoRotate", "Gizmo: Rotate"),
    ("GizmoScale", "Gizmo: Scale"),
    ("SnapModifier", "Snap Modifier"),
    ("Undo", "Undo"),
    ("Redo", "Redo"),
    ("Copy", "Copy"),
    ("Paste", "Paste"),
    ("Duplicate", "Duplicate"),
];

/// Returns the display label of the first other keybind that shares the same
/// key + modifier combo (non-modifier keys only — modifier keys may be shared).
fn find_conflict(
    binds: &[(&'static str, &'static str, Option<KeyBind>, Option<KeyBind>)],
    key: &PhysicalKey,
    modifiers: &ModifierKeys,
    skip_name: &str,
) -> Option<&'static str> {
    if is_modifier_key(key) {
        return None;
    }
    binds.iter().find_map(|(name, label, bind, _default)| {
        if *name == skip_name {
            return None;
        }
        bind.as_ref()
            .filter(|b| b.key == *key && b.modifiers == *modifiers)
            .map(|_| *label)
    })
}

struct KeybindsPage {
    /// (bind_name, display_label, current bind, default bind)
    binds: Vec<(&'static str, &'static str, Option<KeyBind>, Option<KeyBind>)>,
}

impl KeybindsPage {
    fn from_world(world: &World) -> Self {
        let inputs = world.get_resource::<InputManager>().ok();
        let binds = KEYBIND_DISPLAY
            .iter()
            .map(|&(name, label)| {
                let bind = inputs.and_then(|i| i.keybinds.get(name).cloned());
                let default = inputs.and_then(|i| i.default_keybinds.get(name).cloned());
                (name, label, bind, default)
            })
            .collect();
        Self { binds }
    }

    fn draw(
        &self,
        ui: &mut egui::Ui,
        style: &EditorStyle,
        query: &str,
        capture: &KeybindCapture,
        error: &mut Option<String>,
        pending: &mut Vec<(&'static str, KeyBind)>,
    ) {
        let row_h = style.row_height();

        // Conflict / error banner
        if let Some(ref msg) = error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), msg);
            });
            ui.add_space(4.0);
        }

        for (name, label, bind, default) in &self.binds {
            if !query.is_empty() && !label.to_lowercase().contains(query) {
                continue;
            }

            ui.horizontal(|ui| {
                ui.add_sized(
                    egui::Vec2::new(160.0, row_h),
                    egui::Label::new(egui::RichText::new(*label).color(style.dim_col)),
                );

                let id = egui::Id::new(("keybind_chip", *name));
                if let Some(new_bind) = keybind_editor(ui, id, bind.as_ref(), default.as_ref(), capture, style) {
                    if let Some(other) = find_conflict(&self.binds, &new_bind.key, &new_bind.modifiers, name) {
                        *error = Some(format!(
                            "'{}' is already bound to '{}'. Choose a different key or add a modifier.",
                            pretty_bind(&new_bind),
                            other,
                        ));
                    } else {
                        *error = None;
                        pending.push((name, new_bind));
                    }
                }
            });

            ui.add_space(2.0);
        }
    }
}

// Preferences window state

#[derive(Resource, Clone)]
pub struct PreferencesState {
    pub open: bool,
    pub selected_tab: usize,
    pub tab_search: String,
    /// Conflict message shown when the chosen key is already taken by another action.
    pub rebind_error: Option<String>,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            open: false,
            selected_tab: 0,
            tab_search: String::new(),
            rebind_error: None,
        }
    }
}

#[update(mode = "editor")]
pub fn preferences(world: &mut World) -> Result<()> {
    if !world.has_resource::<PreferencesState>() {
        world.insert_resource(PreferencesState::default());
    }
    if !world.get_resource::<PreferencesState>()?.open {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let style = world
        .get_resource::<EditorStyle>()
        .ok()
        .cloned()
        .unwrap_or_default();
    let mut state = world.get_resource::<PreferencesState>()?.clone();
    let mut window_open = state.open;

    let mut editor = EditorPage::from_world(world);
    let mut viewport = ViewportPage::from_world(world);
    let mut graphics = GraphicsPage::from_world(world);
    let keybinds = KeybindsPage::from_world(world);

    let capture = world
        .get_resource::<InputManager>()
        .map(|i| KeybindCapture::from_inputs(i))
        .unwrap_or_else(|_| KeybindCapture::none());

    let mut pending: Vec<(&'static str, KeyBind)> = Vec::new();

    egui::Window::new("Preferences")
        .id(egui::Id::new("preferences_window"))
        .open(&mut window_open)
        .default_size([680.0, 480.0])
        .min_size([560.0, 360.0])
        .resizable(true)
        .movable(true)
        .frame(style.window_frame(&ctx))
        .show(&ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            let avail = ui.available_rect_before_wrap();
            let (total_w, total_h) = (avail.width(), avail.height());
            let tab_w = 140.0_f32;

            let left_rect = egui::Rect::from_min_size(avail.min, egui::Vec2::new(tab_w, total_h));
            let right_rect = egui::Rect::from_min_size(
                avail.min + egui::Vec2::new(tab_w + 1.0, 0.0),
                egui::Vec2::new((total_w - tab_w - 1.0).max(0.0), total_h),
            );
            ui.advance_cursor_after_rect(egui::Rect::from_min_size(
                avail.min,
                egui::Vec2::new(total_w, total_h),
            ));

            // Left tab list
            let mut left = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(left_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            left.spacing_mut().item_spacing = egui::Vec2::ZERO;
            left.painter().rect_filled(left_rect, 0.0, style.dark_bg);
            left.add_space(6.0);
            left.horizontal(|ui| {
                ui.add_space(6.0);
                ui.add_sized(
                    egui::Vec2::new(tab_w - 12.0, style.row_height()),
                    egui::TextEdit::singleline(&mut state.tab_search).hint_text("Search..."),
                );
            });
            left.add_space(6.0);

            let query = state.tab_search.to_lowercase();

            if !page_matches(&PAGES[state.selected_tab], &query) {
                if let Some(i) = PAGES.iter().position(|p| page_matches(p, &query)) {
                    state.selected_tab = i;
                }
            }

            for (i, page) in PAGES.iter().enumerate() {
                if !page_matches(page, &query) {
                    continue;
                }
                let selected = state.selected_tab == i;
                let (row_rect, resp) = left.allocate_exact_size(
                    egui::Vec2::new(tab_w, style.header_height()),
                    egui::Sense::click(),
                );
                let bg = if selected {
                    style.sel_bg
                } else if resp.hovered() {
                    style.hover_bg
                } else {
                    style.dark_bg
                };
                left.painter().rect_filled(row_rect, 0.0, bg);
                if selected {
                    let accent = egui::Rect::from_min_size(
                        row_rect.min,
                        egui::Vec2::new(3.0, row_rect.height()),
                    );
                    left.painter().rect_filled(accent, 0.0, style.text_col);
                }
                left.painter().text(
                    egui::Pos2::new(row_rect.left() + 14.0, row_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    page.label,
                    style.font_ui(),
                    if selected {
                        style.text_col
                    } else {
                        style.dim_col
                    },
                );
                if resp.clicked() {
                    state.selected_tab = i;
                }
            }

            // Divider
            ui.painter().line_segment(
                [
                    egui::Pos2::new(left_rect.right(), left_rect.top()),
                    egui::Pos2::new(left_rect.right(), left_rect.bottom()),
                ],
                egui::Stroke::new(1.0, style.div_col),
            );

            // Right content
            let mut right = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(right_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            right.spacing_mut().item_spacing = egui::Vec2::new(8.0, 8.0);
            right.painter().rect_filled(right_rect, 0.0, style.dark_bg);

            // When the whole tab name matches the query, show all its settings.
            let show_all = query.is_empty()
                || PAGES
                    .get(state.selected_tab)
                    .map(|p| p.label.to_lowercase().contains(&query))
                    .unwrap_or(true);
            let effective_query = if show_all { "" } else { query.as_str() };

            egui::ScrollArea::vertical()
                .id_salt("prefs_content_scroll")
                .show(&mut right, |ui| {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            match state.selected_tab {
                                0 => editor.draw(ui, &style, effective_query),
                                1 => viewport.draw(ui, &style, effective_query),
                                2 => graphics.draw(ui, &style, effective_query),
                                3 => keybinds.draw(
                                    ui,
                                    &style,
                                    effective_query,
                                    &capture,
                                    &mut state.rebind_error,
                                    &mut pending,
                                ),
                                _ => {}
                            }
                            ui.add_space(14.0);
                        });
                    });
                });
        });

    {
        let s = world.get_resource_mut::<PreferencesState>()?;
        s.open = window_open;
        s.tab_search = state.tab_search;
        s.selected_tab = state.selected_tab;
        s.rebind_error = state.rebind_error;
    }

    for (name, bind) in pending {
        if let Ok(inputs) = world.get_resource_mut::<InputManager>() {
            inputs.rebind_key(name.to_string(), bind);
        }
    }

    editor.apply(world)?;
    viewport.apply(world)?;
    graphics.apply(world)?;

    Ok(())
}
