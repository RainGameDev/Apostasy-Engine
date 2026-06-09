use apostasy_core::egui::{self, Color32};
use apostasy_macros::Resource;

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Theme {
    Dark,
    Light,
    #[serde(alias = "Gruvbox")]
    GruvboxDarkHard,
    GruvboxDarkMedium,
    GruvboxDarkSoft,
    GruvboxLightHard,
    GruvboxLightMedium,
    GruvboxLightSoft,
    CatppuccinMocha,
    CatppuccinMacchiato,
    CatppuccinFrappe,
    CatppuccinLatte,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::GruvboxDarkHard
    }
}

impl Theme {
    pub fn label(&self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
            Theme::GruvboxDarkHard => "Gruvbox Dark Hard",
            Theme::GruvboxDarkMedium => "Gruvbox Dark",
            Theme::GruvboxDarkSoft => "Gruvbox Dark Soft",
            Theme::GruvboxLightHard => "Gruvbox Light Hard",
            Theme::GruvboxLightMedium => "Gruvbox Light",
            Theme::GruvboxLightSoft => "Gruvbox Light Soft",
            Theme::CatppuccinMocha => "Catppuccin Mocha",
            Theme::CatppuccinMacchiato => "Catppuccin Macchiato",
            Theme::CatppuccinFrappe => "Catppuccin Frappé",
            Theme::CatppuccinLatte => "Catppuccin Latte",
        }
    }

    pub fn is_light(&self) -> bool {
        matches!(
            self,
            Theme::Light
                | Theme::GruvboxLightHard
                | Theme::GruvboxLightMedium
                | Theme::GruvboxLightSoft
                | Theme::CatppuccinLatte
        )
    }
}

#[derive(Resource, Clone)]
pub struct EditorStyle {
    pub theme: Theme,
    pub font_size: u8,
    pub dark_bg: Color32,
    pub panel_bg: Color32,
    pub header_bg: Color32,
    pub row_alt: Color32,
    pub div_col: Color32,
    pub text_col: Color32,
    pub dim_col: Color32,
    pub sel_bg: Color32,
    pub hover_bg: Color32,
}

impl Default for EditorStyle {
    fn default() -> Self {
        Self::gruvbox_dark_hard()
    }
}

impl EditorStyle {
    pub fn from_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
            Theme::GruvboxDarkHard => Self::gruvbox_dark_hard(),
            Theme::GruvboxDarkMedium => Self::gruvbox_dark_medium(),
            Theme::GruvboxDarkSoft => Self::gruvbox_dark_soft(),
            Theme::GruvboxLightHard => Self::gruvbox_light_hard(),
            Theme::GruvboxLightMedium => Self::gruvbox_light_medium(),
            Theme::GruvboxLightSoft => Self::gruvbox_light_soft(),
            Theme::CatppuccinMocha => Self::catppuccin_mocha(),
            Theme::CatppuccinMacchiato => Self::catppuccin_macchiato(),
            Theme::CatppuccinFrappe => Self::catppuccin_frappe(),
            Theme::CatppuccinLatte => Self::catppuccin_latte(),
        }
    }

    pub fn dark() -> Self {
        Self {
            theme: Theme::Dark,
            font_size: 13,
            dark_bg: Color32::from_rgb(18, 18, 18),
            panel_bg: Color32::from_rgb(24, 24, 24),
            header_bg: Color32::from_rgb(30, 30, 30),
            row_alt: Color32::from_rgb(28, 28, 28),
            div_col: Color32::from_rgb(60, 60, 60),
            text_col: Color32::WHITE,
            dim_col: Color32::from_rgb(170, 170, 170),
            sel_bg: Color32::from_rgb(40, 80, 140),
            hover_bg: Color32::from_rgb(38, 38, 50),
        }
    }

    pub fn light() -> Self {
        let vis = egui::Visuals::light();
        Self {
            theme: Theme::Light,
            font_size: 13,
            dark_bg: vis.window_fill,
            panel_bg: vis.panel_fill,
            header_bg: vis.widgets.inactive.weak_bg_fill,
            row_alt: Color32::from_gray(240),
            div_col: vis.window_stroke.color,
            text_col: vis.widgets.inactive.fg_stroke.color,
            dim_col: vis.widgets.noninteractive.fg_stroke.color,
            sel_bg: vis.selection.bg_fill,
            hover_bg: vis.widgets.hovered.weak_bg_fill,
        }
    }

    pub fn scale(&self) -> f32 {
        self.font_size as f32 / 13.0
    }

    pub fn row_height(&self) -> f32 {
        (self.font_size as f32 * 1.54).ceil()
    }

    pub fn header_height(&self) -> f32 {
        (self.font_size as f32 * 2.0).ceil()
    }

    pub fn font_ui(&self) -> egui::FontId {
        egui::FontId::proportional(self.font_size as f32)
    }

    pub fn font_small(&self) -> egui::FontId {
        egui::FontId::proportional((self.font_size as f32 * 0.7).max(7.0))
    }

    pub fn apply_to_context(&self, ctx: &egui::Context) {
        ctx.global_style_mut(|style| {
            if self.theme == Theme::Light {
                style.visuals = egui::Visuals::light();
            } else {
                style.visuals = if self.theme.is_light() {
                    egui::Visuals::light()
                } else {
                    egui::Visuals::dark()
                };

                style.visuals.window_fill = self.dark_bg;
                style.visuals.panel_fill = self.panel_bg;
                style.visuals.window_stroke = egui::Stroke::new(1.0, self.div_col);
                style.visuals.extreme_bg_color = self.dark_bg;
                style.visuals.faint_bg_color = self.row_alt;
                style.visuals.selection.bg_fill = self.sel_bg;

                let dim = egui::Stroke::new(1.0, self.div_col);
                let txt = egui::Stroke::new(1.0, self.text_col);
                let w = &mut style.visuals.widgets;

                w.noninteractive.bg_fill = self.panel_bg;
                w.noninteractive.weak_bg_fill = self.dark_bg;
                w.noninteractive.bg_stroke = dim;
                w.noninteractive.fg_stroke = egui::Stroke::new(1.0, self.dim_col);

                w.inactive.bg_fill = self.header_bg;
                w.inactive.weak_bg_fill = self.panel_bg;
                w.inactive.bg_stroke = dim;
                w.inactive.fg_stroke = txt;

                w.hovered.bg_fill = self.hover_bg;
                w.hovered.weak_bg_fill = self.hover_bg;
                w.hovered.bg_stroke = egui::Stroke::new(1.0, self.dim_col);
                w.hovered.fg_stroke = egui::Stroke::new(1.5, self.text_col);

                w.active.bg_fill = self.sel_bg;
                w.active.weak_bg_fill = self.sel_bg;
                w.active.bg_stroke = egui::Stroke::new(1.0, self.sel_bg);
                w.active.fg_stroke = egui::Stroke::new(2.0, self.text_col);

                w.open.bg_fill = self.dark_bg;
                w.open.weak_bg_fill = self.dark_bg;
                w.open.bg_stroke = dim;
                w.open.fg_stroke = txt;
            }
            let s = self.font_size as f32;
            style.text_styles = [
                (egui::TextStyle::Small, egui::FontId::proportional(s * 0.75)),
                (egui::TextStyle::Body, egui::FontId::proportional(s)),
                (egui::TextStyle::Button, egui::FontId::proportional(s)),
                (
                    egui::TextStyle::Heading,
                    egui::FontId::proportional(s * 1.5),
                ),
                (egui::TextStyle::Monospace, egui::FontId::monospace(s)),
            ]
            .into();
            let k = s / 13.0;
            style.spacing.item_spacing = egui::Vec2::new(8.0 * k, 3.0 * k);
            style.spacing.button_padding = egui::Vec2::new(4.0 * k, 1.0 * k);
            style.spacing.interact_size = egui::Vec2::new(40.0 * k, 18.0 * k);
            style.spacing.icon_width = 14.0 * k;
            style.spacing.icon_width_inner = 8.0 * k;
            style.spacing.icon_spacing = 4.0 * k;
            style.spacing.indent = 18.0 * k;
        });
    }

    pub fn window_frame(&self, ctx: &egui::Context) -> egui::Frame {
        egui::Frame::window(&ctx.global_style())
            .fill(self.dark_bg)
            .stroke(egui::Stroke::new(1.0, self.div_col))
            .outer_margin(egui::Margin::same(0))
            .inner_margin(egui::Margin::same(0))
    }

    pub fn gruvbox_dark_hard() -> Self {
        Self {
            theme: Theme::GruvboxDarkHard,
            font_size: 13,
            dark_bg: Color32::from_rgb(29, 32, 33), // bg0_h #1d2021
            panel_bg: Color32::from_rgb(50, 48, 47), // bg0_s #32302f
            header_bg: Color32::from_rgb(60, 56, 54), // bg1   #3c3836
            row_alt: Color32::from_rgb(69, 64, 60), // bg1.5
            div_col: Color32::from_rgb(80, 73, 69), // bg2   #504945
            text_col: Color32::from_rgb(235, 219, 178), // fg1   #ebdbb2
            dim_col: Color32::from_rgb(168, 153, 132), // fg4   #a89984
            sel_bg: Color32::from_rgb(69, 133, 136), // aqua  #458588
            hover_bg: Color32::from_rgb(80, 73, 69), // bg2
        }
    }

    pub fn gruvbox_dark_medium() -> Self {
        Self {
            theme: Theme::GruvboxDarkMedium,
            font_size: 13,
            dark_bg: Color32::from_rgb(40, 40, 40), // bg0   #282828
            panel_bg: Color32::from_rgb(50, 48, 47), // bg0_s #32302f
            header_bg: Color32::from_rgb(60, 56, 54), // bg1   #3c3836
            row_alt: Color32::from_rgb(69, 64, 60), // bg1.5
            div_col: Color32::from_rgb(80, 73, 69), // bg2   #504945
            text_col: Color32::from_rgb(235, 219, 178), // fg1   #ebdbb2
            dim_col: Color32::from_rgb(168, 153, 132), // fg4   #a89984
            sel_bg: Color32::from_rgb(69, 133, 136), // aqua  #458588
            hover_bg: Color32::from_rgb(80, 73, 69), // bg2
        }
    }

    pub fn gruvbox_dark_soft() -> Self {
        Self {
            theme: Theme::GruvboxDarkSoft,
            font_size: 13,
            dark_bg: Color32::from_rgb(50, 48, 47), // bg0_s #32302f
            panel_bg: Color32::from_rgb(60, 56, 54), // bg1   #3c3836
            header_bg: Color32::from_rgb(80, 73, 69), // bg2   #504945
            row_alt: Color32::from_rgb(91, 82, 75), // bg2.5
            div_col: Color32::from_rgb(102, 92, 84), // bg3   #665c54
            text_col: Color32::from_rgb(235, 219, 178), // fg1   #ebdbb2
            dim_col: Color32::from_rgb(168, 153, 132), // fg4   #a89984
            sel_bg: Color32::from_rgb(69, 133, 136), // aqua  #458588
            hover_bg: Color32::from_rgb(102, 92, 84), // bg3
        }
    }

    pub fn gruvbox_light_hard() -> Self {
        Self {
            theme: Theme::GruvboxLightHard,
            font_size: 13,
            dark_bg: Color32::from_rgb(249, 245, 215), // bg0_h #f9f5d7
            panel_bg: Color32::from_rgb(251, 241, 199), // bg0   #fbf1c7
            header_bg: Color32::from_rgb(235, 219, 178), // bg1   #ebdbb2
            row_alt: Color32::from_rgb(242, 229, 188), // bg0_s #f2e5bc
            div_col: Color32::from_rgb(213, 196, 161), // bg2   #d5c4a1
            text_col: Color32::from_rgb(60, 56, 54),   // fg1   #3c3836
            dim_col: Color32::from_rgb(102, 92, 84),   // fg3   #665c54
            sel_bg: Color32::from_rgb(69, 133, 136),   // aqua  #458588
            hover_bg: Color32::from_rgb(213, 196, 161), // bg2
        }
    }

    pub fn gruvbox_light_medium() -> Self {
        Self {
            theme: Theme::GruvboxLightMedium,
            font_size: 13,
            dark_bg: Color32::from_rgb(251, 241, 199), // bg0   #fbf1c7
            panel_bg: Color32::from_rgb(242, 229, 188), // bg0_s #f2e5bc
            header_bg: Color32::from_rgb(235, 219, 178), // bg1   #ebdbb2
            row_alt: Color32::from_rgb(249, 245, 215), // bg0_h — lighter alt
            div_col: Color32::from_rgb(213, 196, 161), // bg2   #d5c4a1
            text_col: Color32::from_rgb(60, 56, 54),   // fg1   #3c3836
            dim_col: Color32::from_rgb(102, 92, 84),   // fg3   #665c54
            sel_bg: Color32::from_rgb(69, 133, 136),   // aqua  #458588
            hover_bg: Color32::from_rgb(213, 196, 161), // bg2
        }
    }

    pub fn gruvbox_light_soft() -> Self {
        Self {
            theme: Theme::GruvboxLightSoft,
            font_size: 13,
            dark_bg: Color32::from_rgb(242, 229, 188), // bg0_s #f2e5bc
            panel_bg: Color32::from_rgb(235, 219, 178), // bg1   #ebdbb2
            header_bg: Color32::from_rgb(213, 196, 161), // bg2   #d5c4a1
            row_alt: Color32::from_rgb(251, 241, 199), // bg0   — lighter alt
            div_col: Color32::from_rgb(189, 174, 147), // bg3   #bdae93
            text_col: Color32::from_rgb(60, 56, 54),   // fg1   #3c3836
            dim_col: Color32::from_rgb(102, 92, 84),   // fg3   #665c54
            sel_bg: Color32::from_rgb(69, 133, 136),   // aqua  #458588
            hover_bg: Color32::from_rgb(189, 174, 147), // bg3
        }
    }

    pub fn catppuccin_mocha() -> Self {
        Self {
            theme: Theme::CatppuccinMocha,
            font_size: 13,
            dark_bg: Color32::from_rgb(24, 24, 37), // Mantle  #181825
            panel_bg: Color32::from_rgb(30, 30, 46), // Base    #1e1e2e
            header_bg: Color32::from_rgb(49, 50, 68), // Surface0 #313244
            row_alt: Color32::from_rgb(49, 50, 68), // Surface0
            div_col: Color32::from_rgb(88, 91, 112), // Surface2 #585b70
            text_col: Color32::from_rgb(205, 214, 244), // Text    #cdd6f4
            dim_col: Color32::from_rgb(166, 173, 200), // Subtext0 #a6adc8
            sel_bg: Color32::from_rgb(203, 166, 247), // Mauve   #cba6f7
            hover_bg: Color32::from_rgb(69, 71, 90), // Surface1 #45475a
        }
    }

    pub fn catppuccin_macchiato() -> Self {
        Self {
            theme: Theme::CatppuccinMacchiato,
            font_size: 13,
            dark_bg: Color32::from_rgb(30, 32, 48), // Mantle  #1e2030
            panel_bg: Color32::from_rgb(36, 39, 58), // Base    #24273a
            header_bg: Color32::from_rgb(54, 58, 79), // Surface0 #363a4f
            row_alt: Color32::from_rgb(54, 58, 79), // Surface0
            div_col: Color32::from_rgb(91, 96, 120), // Surface2 #5b6078
            text_col: Color32::from_rgb(202, 211, 245), // Text    #cad3f5
            dim_col: Color32::from_rgb(165, 173, 203), // Subtext0 #a5adcb
            sel_bg: Color32::from_rgb(198, 160, 246), // Mauve   #c6a0f6
            hover_bg: Color32::from_rgb(73, 77, 100), // Surface1 #494d64
        }
    }

    pub fn catppuccin_frappe() -> Self {
        Self {
            theme: Theme::CatppuccinFrappe,
            font_size: 13,
            dark_bg: Color32::from_rgb(41, 44, 60), // Mantle  #292c3c
            panel_bg: Color32::from_rgb(48, 52, 70), // Base    #303446
            header_bg: Color32::from_rgb(65, 69, 89), // Surface0 #414559
            row_alt: Color32::from_rgb(65, 69, 89), // Surface0
            div_col: Color32::from_rgb(98, 104, 128), // Surface2 #626880
            text_col: Color32::from_rgb(198, 208, 245), // Text    #c6d0f5
            dim_col: Color32::from_rgb(165, 173, 206), // Subtext0 #a5adce
            sel_bg: Color32::from_rgb(202, 158, 230), // Mauve   #ca9ee6
            hover_bg: Color32::from_rgb(81, 87, 109), // Surface1 #51576d
        }
    }
    pub fn catppuccin_latte() -> Self {
        Self {
            theme: Theme::CatppuccinLatte,
            font_size: 13,
            dark_bg: Color32::from_rgb(230, 233, 239), // Mantle  #e6e9ef
            panel_bg: Color32::from_rgb(239, 241, 245), // Base    #eff1f5
            header_bg: Color32::from_rgb(204, 208, 218), // Surface0 #ccd0da
            row_alt: Color32::from_rgb(220, 224, 232), // Crust   #dce0e8
            div_col: Color32::from_rgb(188, 192, 204), // Surface1 #bcc0cc
            text_col: Color32::from_rgb(76, 79, 105),  // Text    #4c4f69
            dim_col: Color32::from_rgb(108, 111, 133), // Subtext0 #6c6f85
            sel_bg: Color32::from_rgb(114, 135, 253),  // Lavender #7287fd
            hover_bg: Color32::from_rgb(188, 192, 204), // Surface1
        }
    }
}
