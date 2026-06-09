use anyhow::Result;
use apostasy_core::ui::FontRegistry;
use apostasy_core::{egui, objects::world::World, ui::ui_context::EguiContext, update};
use apostasy_macros::Resource;

use super::{EditorStyle, style::Theme};

// ---------------------------------------------------------------------------
// Persistent preferences — saved to / loaded from disk
// ---------------------------------------------------------------------------

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct EditorPreferences {
    pub theme: Theme,
    pub font_size: u8,
    pub active_font: String,
}

impl Default for EditorPreferences {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            font_size: 13,
            active_font: String::new(),
        }
    }
}

impl EditorPreferences {
    pub const PATH: &'static str = "res/.editor/editor_preferences.yaml";

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
}

struct Tab {
    name: &'static str,
    settings: &'static [&'static str],
}

const TABS: &[Tab] = &[Tab {
    name: "Editor",
    settings: &["Theme", "Font", "Font Size"],
}];

fn tab_matches(tab: &Tab, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    tab.name.to_lowercase().contains(query)
        || tab
            .settings
            .iter()
            .any(|s| s.to_lowercase().contains(query))
}

#[derive(Resource, Clone)]
pub struct PreferencesState {
    pub open: bool,
    pub selected_tab: usize,
    pub tab_search: String,
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            open: false,
            selected_tab: 0,
            tab_search: String::new(),
        }
    }
}

struct EditorTabPending {
    theme: Theme,
    font: String,
    font_size: u8,
}

impl EditorTabPending {
    fn from_resources(style: &EditorStyle, fonts: &FontRegistry) -> Self {
        Self {
            theme: style.theme,
            font: fonts.active.clone(),
            font_size: style.font_size,
        }
    }

    fn apply_changes(
        &self,
        style: &EditorStyle,
        fonts: &FontRegistry,
        world: &mut World,
    ) -> Result<()> {
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
            EditorPreferences {
                theme: self.theme,
                font_size: self.font_size,
                active_font: self.font.clone(),
            }
            .save();
        }
        Ok(())
    }
}

struct SettingCtx<'a> {
    ui: &'a mut egui::Ui,
    style: &'a EditorStyle,
    query: &'a str,
    show_all: bool,
}

impl<'a> SettingCtx<'a> {
    fn visible(&self, name: &str) -> bool {
        self.show_all || name.to_lowercase().contains(self.query)
    }

    fn label(&mut self, name: &str) {
        self.ui
            .label(egui::RichText::new(name).color(self.style.dim_col));
        self.ui.add_space(4.0);
    }

    fn gap(&mut self) {
        self.ui.add_space(8.0);
    }
}

fn setting_theme(ctx: &mut SettingCtx, value: &mut Theme) {
    if !ctx.visible("Theme") {
        return;
    }
    ctx.label("Theme");
    egui::ComboBox::from_id_salt("theme_combo")
        .selected_text(egui::RichText::new(value.label()).color(ctx.style.text_col))
        .show_ui(ctx.ui, |ui| {
            for theme in [
                Theme::Dark,
                Theme::Light,
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
                ui.selectable_value(value, theme, theme.label());
            }
        });
    ctx.gap();
}

fn setting_font_size(ctx: &mut SettingCtx, value: &mut u8) {
    if !ctx.visible("Font Size") {
        return;
    }
    ctx.label("Font Size");

    ctx.ui.add(egui::Slider::new(value, 8u8..=16u8).integer());
    ctx.gap();
}

fn setting_font(ctx: &mut SettingCtx, fonts: &[String], value: &mut String) {
    if !ctx.visible("Font") || fonts.is_empty() {
        return;
    }
    ctx.label("Font");
    egui::ComboBox::from_id_salt("font_combo")
        .selected_text(egui::RichText::new(value.as_str()).color(ctx.style.text_col))
        .show_ui(ctx.ui, |ui| {
            for font in fonts {
                ui.selectable_value(value, font.clone(), font.as_str());
            }
        });
    ctx.gap();
}

fn draw_editor_tab(
    ui: &mut egui::Ui,
    style: &EditorStyle,
    fonts: &[String],
    pending: &mut EditorTabPending,
    query: &str,
    show_all: bool,
) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        ui.vertical(|ui| {
            let mut ctx = SettingCtx {
                ui,
                style,
                query,
                show_all,
            };
            setting_theme(&mut ctx, &mut pending.theme);
            setting_font_size(&mut ctx, &mut pending.font_size);
            setting_font(&mut ctx, fonts, &mut pending.font);
        });
    });
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
        .cloned()
        .unwrap_or_default();
    let font_reg = world
        .get_resource::<FontRegistry>()
        .cloned()
        .unwrap_or_default();
    let mut state = world.get_resource::<PreferencesState>()?.clone();
    let mut window_open = state.open;
    let mut pending = EditorTabPending::from_resources(&style, &font_reg);

    egui::Window::new("Preferences")
        .id(egui::Id::new("preferences_window"))
        .open(&mut window_open)
        .default_size([600.0, 400.0])
        .resizable(true)
        .movable(true)
        .frame(style.window_frame(&ctx))
        .show(&ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

            let avail = ui.available_rect_before_wrap();
            let total_w = avail.width();
            let total_h = avail.height();
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

            if !tab_matches(&TABS[state.selected_tab], &query) {
                if let Some(first) = TABS.iter().position(|t| tab_matches(t, &query)) {
                    state.selected_tab = first;
                }
            }

            for (i, tab) in TABS.iter().enumerate() {
                if !tab_matches(tab, &query) {
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
                left.painter().text(
                    egui::Pos2::new(row_rect.left() + 10.0, row_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    tab.name,
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

            if let Some(tab) = TABS.get(state.selected_tab) {
                let show_all = query.is_empty() || tab.name.to_lowercase().contains(&query);
                match state.selected_tab {
                    0 => draw_editor_tab(
                        &mut right,
                        &style,
                        &font_reg.fonts,
                        &mut pending,
                        &query,
                        show_all,
                    ),
                    _ => {}
                }
            }
        });

    {
        let s = world.get_resource_mut::<PreferencesState>()?;
        s.open = window_open;
        s.tab_search = state.tab_search;
        s.selected_tab = state.selected_tab;
    }

    pending.apply_changes(&style, &font_reg, world)?;

    Ok(())
}
