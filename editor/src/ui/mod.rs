use apostasy_core::egui::Color32;

pub mod assets_panel;
pub mod inspector_panel;
pub mod scenes_panel;
pub mod top_bar;
pub mod ui_manager;
pub mod viewport_panel;

const DARK_BG: Color32 = Color32::from_rgb(18, 18, 18);
const PANEL_BG: Color32 = Color32::from_rgb(24, 24, 24);
const HEADER_BG: Color32 = Color32::from_rgb(30, 30, 30);
const ROW_ALT: Color32 = Color32::from_rgb(28, 28, 28);
const DIV_COL: Color32 = Color32::from_rgb(60, 60, 60);
const TEXT_COL: Color32 = Color32::WHITE;
const DIM_COL: Color32 = Color32::from_rgb(170, 170, 170);
const SEL_BG: Color32 = Color32::from_rgb(40, 80, 140);
const HOVER_BG: Color32 = Color32::from_rgb(38, 38, 50);
