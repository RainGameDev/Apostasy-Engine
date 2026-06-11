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
    TokyoNight,
    TokyoNightStorm,
    Nord,
    Dracula,
    OneDark,
    SolarizedDark,
    SolarizedLight,
    Monokai,
    RosePine,
    RosePineDawn,
    AyuDark,
    AyuMirage,
    AyuLight,
    GithubDark,
    GithubDarkDimmed,
    GithubLight,
    SlackDark,
    NightOwl,
    LightOwl,
    Rogue,
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
            Theme::TokyoNight => "Tokyo Night",
            Theme::TokyoNightStorm => "Tokyo Night Storm",
            Theme::Nord => "Nord",
            Theme::Dracula => "Dracula",
            Theme::OneDark => "One Dark",
            Theme::SolarizedDark => "Solarized Dark",
            Theme::SolarizedLight => "Solarized Light",
            Theme::Monokai => "Monokai",
            Theme::RosePine => "Rosé Pine",
            Theme::RosePineDawn => "Rosé Pine Dawn",
            Theme::AyuDark => "Ayu Dark",
            Theme::AyuMirage => "Ayu Mirage",
            Theme::AyuLight => "Ayu Light",
            Theme::GithubDark => "GitHub Dark",
            Theme::GithubDarkDimmed => "GitHub Dark Dimmed",
            Theme::GithubLight => "GitHub Light",
            Theme::SlackDark => "Slack Dark",
            Theme::NightOwl => "Night Owl",
            Theme::LightOwl => "Light Owl",
            Theme::Rogue => "Rogue",
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
                | Theme::SolarizedLight
                | Theme::RosePineDawn
                | Theme::AyuLight
                | Theme::GithubLight
                | Theme::LightOwl
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
            Theme::TokyoNight => Self::tokyo_night(),
            Theme::TokyoNightStorm => Self::tokyo_night_storm(),
            Theme::Nord => Self::nord(),
            Theme::Dracula => Self::dracula(),
            Theme::OneDark => Self::one_dark(),
            Theme::SolarizedDark => Self::solarized_dark(),
            Theme::SolarizedLight => Self::solarized_light(),
            Theme::Monokai => Self::monokai(),
            Theme::RosePine => Self::rose_pine(),
            Theme::RosePineDawn => Self::rose_pine_dawn(),
            Theme::AyuDark => Self::ayu_dark(),
            Theme::AyuMirage => Self::ayu_mirage(),
            Theme::AyuLight => Self::ayu_light(),
            Theme::GithubDark => Self::github_dark(),
            Theme::GithubDarkDimmed => Self::github_dark_dimmed(),
            Theme::GithubLight => Self::github_light(),
            Theme::SlackDark => Self::slack_dark(),
            Theme::NightOwl => Self::night_owl(),
            Theme::LightOwl => Self::light_owl(),
            Theme::Rogue => Self::rogue(),
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
            style.interaction.selectable_labels = false;
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

    pub fn tokyo_night() -> Self {
        Self {
            theme: Theme::TokyoNight,
            font_size: 13,
            dark_bg: Color32::from_rgb(16, 16, 28),    // bg       #10101c
            panel_bg: Color32::from_rgb(26, 27, 38),   // bg       #1a1b26
            header_bg: Color32::from_rgb(31, 35, 53),  // bg_dark  #1f2335
            row_alt: Color32::from_rgb(36, 40, 59),    // bg_highlight #24283b
            div_col: Color32::from_rgb(65, 72, 104),   // terminal_black #41486a (approx)
            text_col: Color32::from_rgb(192, 202, 245), // fg      #c0caf5
            dim_col: Color32::from_rgb(169, 177, 214), // comment #a9b1d6
            sel_bg: Color32::from_rgb(54, 74, 130),    // blue     #364A82
            hover_bg: Color32::from_rgb(41, 46, 66),   // bg_highlight+
        }
    }

    pub fn tokyo_night_storm() -> Self {
        Self {
            theme: Theme::TokyoNightStorm,
            font_size: 13,
            dark_bg: Color32::from_rgb(24, 25, 38),    // bg       #181926 (approx)
            panel_bg: Color32::from_rgb(31, 35, 53),   // bg       #1f2335
            header_bg: Color32::from_rgb(36, 40, 59),  // bg_dark  #24283b
            row_alt: Color32::from_rgb(41, 46, 66),    // bg_highlight
            div_col: Color32::from_rgb(65, 72, 104),
            text_col: Color32::from_rgb(192, 202, 245), // fg      #c0caf5
            dim_col: Color32::from_rgb(169, 177, 214), // comment #a9b1d6
            sel_bg: Color32::from_rgb(61, 89, 161),    // blue     #3d59a1
            hover_bg: Color32::from_rgb(54, 59, 85),
        }
    }

    pub fn nord() -> Self {
        Self {
            theme: Theme::Nord,
            font_size: 13,
            dark_bg: Color32::from_rgb(46, 52, 64),    // nord0    #2E3440
            panel_bg: Color32::from_rgb(59, 66, 82),   // nord1    #3B4252
            header_bg: Color32::from_rgb(67, 76, 94),  // nord2    #434C5E
            row_alt: Color32::from_rgb(76, 86, 106),   // nord3    #4C5666 (approx)
            div_col: Color32::from_rgb(76, 86, 106),   // nord3    #4C5666
            text_col: Color32::from_rgb(236, 239, 244), // nord6   #ECEFF4
            dim_col: Color32::from_rgb(216, 222, 233), // nord4    #D8DEE9
            sel_bg: Color32::from_rgb(94, 129, 172),   // nord10   #5E81AC
            hover_bg: Color32::from_rgb(67, 76, 94),   // nord2
        }
    }

    pub fn dracula() -> Self {
        Self {
            theme: Theme::Dracula,
            font_size: 13,
            dark_bg: Color32::from_rgb(33, 34, 44),    // background #21222c
            panel_bg: Color32::from_rgb(40, 42, 54),   // background #282a36
            header_bg: Color32::from_rgb(68, 71, 90),  // current line #44475a
            row_alt: Color32::from_rgb(55, 57, 72),    // selection (dim)
            div_col: Color32::from_rgb(68, 71, 90),    // comment  #44475a
            text_col: Color32::from_rgb(248, 248, 242), // foreground #f8f8f2
            dim_col: Color32::from_rgb(98, 114, 164),  // comment  #6272a4
            sel_bg: Color32::from_rgb(189, 147, 249),  // purple   #bd93f9
            hover_bg: Color32::from_rgb(68, 71, 90),   // current line
        }
    }

    pub fn one_dark() -> Self {
        Self {
            theme: Theme::OneDark,
            font_size: 13,
            dark_bg: Color32::from_rgb(30, 33, 39),    // bg       #1e2127
            panel_bg: Color32::from_rgb(33, 37, 43),   // bg       #21252b
            header_bg: Color32::from_rgb(44, 49, 58),  // bg2      #2c313a
            row_alt: Color32::from_rgb(39, 44, 52),    // bg1      #272c34 (approx)
            div_col: Color32::from_rgb(57, 63, 73),    // border   #393f49
            text_col: Color32::from_rgb(171, 178, 191), // fg      #abb2bf
            dim_col: Color32::from_rgb(92, 99, 112),   // comment  #5c6370
            sel_bg: Color32::from_rgb(82, 139, 255),   // blue     #528bff
            hover_bg: Color32::from_rgb(44, 49, 58),   // bg2
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            theme: Theme::SolarizedDark,
            font_size: 13,
            dark_bg: Color32::from_rgb(0, 43, 54),     // base03   #002b36
            panel_bg: Color32::from_rgb(7, 54, 66),    // base02   #073642
            header_bg: Color32::from_rgb(88, 110, 117), // base01  #586e75
            row_alt: Color32::from_rgb(0, 43, 54),     // base03
            div_col: Color32::from_rgb(88, 110, 117),  // base01   #586e75
            text_col: Color32::from_rgb(131, 148, 150), // base0   #839496
            dim_col: Color32::from_rgb(101, 123, 131), // base00   #657b83
            sel_bg: Color32::from_rgb(38, 139, 210),   // blue     #268bd2
            hover_bg: Color32::from_rgb(7, 54, 66),    // base02
        }
    }

    pub fn solarized_light() -> Self {
        Self {
            theme: Theme::SolarizedLight,
            font_size: 13,
            dark_bg: Color32::from_rgb(253, 246, 227), // base3    #fdf6e3
            panel_bg: Color32::from_rgb(238, 232, 213), // base2   #eee8d5
            header_bg: Color32::from_rgb(147, 161, 161), // base1  #93a1a1
            row_alt: Color32::from_rgb(253, 246, 227), // base3
            div_col: Color32::from_rgb(147, 161, 161), // base1    #93a1a1
            text_col: Color32::from_rgb(101, 123, 131), // base00  #657b83
            dim_col: Color32::from_rgb(131, 148, 150), // base0    #839496
            sel_bg: Color32::from_rgb(38, 139, 210),   // blue     #268bd2
            hover_bg: Color32::from_rgb(238, 232, 213), // base2
        }
    }

    pub fn monokai() -> Self {
        Self {
            theme: Theme::Monokai,
            font_size: 13,
            dark_bg: Color32::from_rgb(30, 31, 28),    // bg dark  #1e1f1c
            panel_bg: Color32::from_rgb(39, 40, 34),   // bg       #272822
            header_bg: Color32::from_rgb(62, 61, 50),  // line hl  #3e3d32
            row_alt: Color32::from_rgb(50, 51, 43),    // mid
            div_col: Color32::from_rgb(75, 75, 65),    // border
            text_col: Color32::from_rgb(248, 248, 242), // fg      #f8f8f2
            dim_col: Color32::from_rgb(117, 113, 94),  // comment  #75715e
            sel_bg: Color32::from_rgb(102, 217, 232),  // cyan     #66d9e8
            hover_bg: Color32::from_rgb(62, 61, 50),   // line hl
        }
    }

    pub fn rose_pine() -> Self {
        Self {
            theme: Theme::RosePine,
            font_size: 13,
            dark_bg: Color32::from_rgb(21, 20, 30),    // base     #15141e (approx)
            panel_bg: Color32::from_rgb(25, 23, 36),   // base     #191724
            header_bg: Color32::from_rgb(31, 29, 46),  // surface  #1f1d2e
            row_alt: Color32::from_rgb(38, 35, 58),    // overlay  #26233a
            div_col: Color32::from_rgb(64, 61, 82),    // muted    #403d52 (approx)
            text_col: Color32::from_rgb(224, 222, 244), // text    #e0def4
            dim_col: Color32::from_rgb(144, 140, 170), // subtle   #908caa
            sel_bg: Color32::from_rgb(196, 167, 231),  // iris     #c4a7e7
            hover_bg: Color32::from_rgb(38, 35, 58),   // overlay
        }
    }

    pub fn rose_pine_dawn() -> Self {
        Self {
            theme: Theme::RosePineDawn,
            font_size: 13,
            dark_bg: Color32::from_rgb(250, 244, 237), // base     #faf4ed
            panel_bg: Color32::from_rgb(255, 250, 243), // base    #fffaf3
            header_bg: Color32::from_rgb(242, 233, 222), // surface #f2e9de
            row_alt: Color32::from_rgb(234, 224, 213), // overlay  #eae0d4 (approx)
            div_col: Color32::from_rgb(215, 200, 184), // muted    #d7c8b8 (approx)
            text_col: Color32::from_rgb(87, 82, 121),  // text     #575279
            dim_col: Color32::from_rgb(121, 117, 147), // subtle   #797593
            sel_bg: Color32::from_rgb(144, 122, 169),  // iris     #907aa9
            hover_bg: Color32::from_rgb(234, 224, 213),
        }
    }

    pub fn ayu_dark() -> Self {
        Self {
            theme: Theme::AyuDark,
            font_size: 13,
            dark_bg: Color32::from_rgb(10, 14, 20),    // bg       #0a0e14
            panel_bg: Color32::from_rgb(13, 16, 23),   // bg+      #0d1017
            header_bg: Color32::from_rgb(15, 19, 26),  // bg++     #0f131a
            row_alt: Color32::from_rgb(19, 23, 32),
            div_col: Color32::from_rgb(47, 52, 64),    // guide    #2f3440
            text_col: Color32::from_rgb(179, 177, 173), // fg      #b3b1ad
            dim_col: Color32::from_rgb(61, 66, 77),    // comment  #3d424d
            sel_bg: Color32::from_rgb(57, 186, 230),   // blue     #39bae6
            hover_bg: Color32::from_rgb(19, 23, 32),
        }
    }

    pub fn ayu_mirage() -> Self {
        Self {
            theme: Theme::AyuMirage,
            font_size: 13,
            dark_bg: Color32::from_rgb(31, 36, 48),    // bg       #1f2430
            panel_bg: Color32::from_rgb(36, 41, 54),   // panel    #242936
            header_bg: Color32::from_rgb(45, 51, 67),  // header   #2d3343
            row_alt: Color32::from_rgb(40, 46, 60),
            div_col: Color32::from_rgb(90, 98, 116),   // guide    #5a6274
            text_col: Color32::from_rgb(204, 202, 194), // fg      #cccac2
            dim_col: Color32::from_rgb(112, 122, 140), // comment  #707a8c
            sel_bg: Color32::from_rgb(255, 204, 102),  // yellow   #ffcc66
            hover_bg: Color32::from_rgb(45, 51, 67),
        }
    }

    pub fn ayu_light() -> Self {
        Self {
            theme: Theme::AyuLight,
            font_size: 13,
            dark_bg: Color32::from_rgb(250, 250, 250), // bg       #fafafa
            panel_bg: Color32::from_rgb(243, 244, 245), // panel   #f3f4f5
            header_bg: Color32::from_rgb(234, 238, 242), // header #eaeef2
            row_alt: Color32::from_rgb(250, 250, 250),
            div_col: Color32::from_rgb(198, 204, 214), // border   #c6ccd6
            text_col: Color32::from_rgb(87, 95, 102),  // fg       #575f66
            dim_col: Color32::from_rgb(171, 176, 182), // dim      #abb0b6
            sel_bg: Color32::from_rgb(255, 170, 51),   // orange   #ffaa33
            hover_bg: Color32::from_rgb(234, 238, 242),
        }
    }

    pub fn github_dark() -> Self {
        Self {
            theme: Theme::GithubDark,
            font_size: 13,
            dark_bg: Color32::from_rgb(13, 17, 23),    // canvas.default  #0d1117
            panel_bg: Color32::from_rgb(22, 27, 34),   // canvas.subtle   #161b22
            header_bg: Color32::from_rgb(33, 38, 45),  // neutral.muted   #21262d
            row_alt: Color32::from_rgb(22, 27, 34),
            div_col: Color32::from_rgb(48, 54, 61),    // border.default  #30363d
            text_col: Color32::from_rgb(201, 209, 217), // text.primary   #c9d1d9
            dim_col: Color32::from_rgb(139, 148, 158), // text.secondary  #8b949e
            sel_bg: Color32::from_rgb(31, 111, 235),   // accent.fg       #1f6feb
            hover_bg: Color32::from_rgb(33, 38, 45),
        }
    }

    pub fn github_dark_dimmed() -> Self {
        Self {
            theme: Theme::GithubDarkDimmed,
            font_size: 13,
            dark_bg: Color32::from_rgb(28, 33, 40),    // #1c2128
            panel_bg: Color32::from_rgb(34, 39, 46),   // #22272e
            header_bg: Color32::from_rgb(45, 51, 59),  // #2d333b
            row_alt: Color32::from_rgb(34, 39, 46),
            div_col: Color32::from_rgb(68, 76, 86),    // #444c56
            text_col: Color32::from_rgb(173, 186, 199), // #adbac7
            dim_col: Color32::from_rgb(99, 110, 123),  // #636e7b
            sel_bg: Color32::from_rgb(49, 109, 202),   // #316dca
            hover_bg: Color32::from_rgb(45, 51, 59),
        }
    }

    pub fn github_light() -> Self {
        Self {
            theme: Theme::GithubLight,
            font_size: 13,
            dark_bg: Color32::from_rgb(255, 255, 255), // #ffffff
            panel_bg: Color32::from_rgb(246, 248, 250), // #f6f8fa
            header_bg: Color32::from_rgb(234, 238, 242), // #eaeef2
            row_alt: Color32::from_rgb(246, 248, 250),
            div_col: Color32::from_rgb(208, 215, 222), // #d0d7de
            text_col: Color32::from_rgb(36, 41, 47),   // #24292f
            dim_col: Color32::from_rgb(110, 119, 129), // #6e7781
            sel_bg: Color32::from_rgb(9, 105, 218),    // #0969da
            hover_bg: Color32::from_rgb(234, 238, 242),
        }
    }

    pub fn slack_dark() -> Self {
        Self {
            theme: Theme::SlackDark,
            font_size: 13,
            dark_bg: Color32::from_rgb(26, 29, 33),    // #1a1d21
            panel_bg: Color32::from_rgb(34, 37, 41),   // #222529
            header_bg: Color32::from_rgb(43, 45, 49),  // #2b2d31
            row_alt: Color32::from_rgb(38, 41, 46),
            div_col: Color32::from_rgb(60, 63, 68),    // divider
            text_col: Color32::from_rgb(209, 210, 211), // #d1d2d3
            dim_col: Color32::from_rgb(141, 142, 144), // #8d8e90
            sel_bg: Color32::from_rgb(17, 100, 163),   // #1164a3
            hover_bg: Color32::from_rgb(43, 45, 49),
        }
    }

    pub fn night_owl() -> Self {
        Self {
            theme: Theme::NightOwl,
            font_size: 13,
            dark_bg: Color32::from_rgb(1, 22, 39),     // bg       #011627
            panel_bg: Color32::from_rgb(11, 41, 66),   // surface  #0b2942
            header_bg: Color32::from_rgb(13, 58, 92),  // header   #0d3a5c
            row_alt: Color32::from_rgb(7, 32, 52),
            div_col: Color32::from_rgb(29, 59, 83),    // selection bg
            text_col: Color32::from_rgb(214, 222, 235), // fg      #d6deeb
            dim_col: Color32::from_rgb(99, 119, 119),  // comment  #637777
            sel_bg: Color32::from_rgb(130, 170, 255),  // blue     #82aaff
            hover_bg: Color32::from_rgb(13, 58, 92),
        }
    }

    pub fn light_owl() -> Self {
        Self {
            theme: Theme::LightOwl,
            font_size: 13,
            dark_bg: Color32::from_rgb(251, 251, 251), // #fbfbfb
            panel_bg: Color32::from_rgb(240, 240, 240), // #f0f0f0
            header_bg: Color32::from_rgb(231, 231, 231), // #e7e7e7
            row_alt: Color32::from_rgb(245, 245, 245),
            div_col: Color32::from_rgb(210, 210, 210),
            text_col: Color32::from_rgb(64, 63, 83),   // #403f53
            dim_col: Color32::from_rgb(144, 167, 178), // comment  #90a7b2
            sel_bg: Color32::from_rgb(12, 150, 155),   // teal     #0c969b
            hover_bg: Color32::from_rgb(231, 231, 231),
        }
    }

    pub fn rogue() -> Self {
        Self {
            theme: Theme::Rogue,
            font_size: 13,
            dark_bg: Color32::from_rgb(14, 8, 8),      // deep blood-dark  #0e0808
            panel_bg: Color32::from_rgb(22, 12, 12),   // #160c0c
            header_bg: Color32::from_rgb(33, 16, 16),  // #211010
            row_alt: Color32::from_rgb(28, 14, 14),
            div_col: Color32::from_rgb(64, 32, 32),    // #402020
            text_col: Color32::from_rgb(232, 216, 200), // warm parchment   #e8d8c8
            dim_col: Color32::from_rgb(140, 100, 90),  // muted rust        #8c645a
            sel_bg: Color32::from_rgb(180, 40, 40),    // crimson           #b42828
            hover_bg: Color32::from_rgb(44, 20, 20),
        }
    }
}
