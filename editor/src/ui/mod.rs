use anyhow::Result;
use apostasy_core::egui::Color32;

pub mod assets_panel;
pub mod cell_panel;
pub mod inspector_panel;
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

use apostasy_core::egui::{self, Pos2, Vec2};
use apostasy_core::objects::world::World;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::update;
use apostasy_macros::Resource;
use serde::{Deserialize, Serialize};

#[derive(Clone, Resource)]
pub struct ScreenSize {
    pub w: f32,
    pub h: f32,
}

impl Default for ScreenSize {
    fn default() -> Self {
        Self {
            w: 1920.0,
            h: 1080.0,
        }
    }
}

/// Normalized  position and size for a window
#[derive(Clone, Serialize, Deserialize)]
pub struct NormalizedWindow {
    pub pos: [f32; 2],  // fraction of screen width/height
    pub size: [f32; 2], // fraction of screen width/height
}

impl NormalizedWindow {
    pub fn to_pos(&self, sw: f32, sh: f32) -> Pos2 {
        Pos2::new(self.pos[0] * sw, self.pos[1] * sh)
    }
    pub fn to_size(&self, sw: f32, sh: f32) -> Vec2 {
        Vec2::new(self.size[0] * sw, self.size[1] * sh)
    }

    /// Call this after egui gives you the window's actual rect
    pub fn update_from_rect(&mut self, rect: egui::Rect, sw: f32, sh: f32) {
        self.pos = [rect.min.x / sw, rect.min.y / sh];
        self.size = [rect.size().x / sw, rect.size().y / sh];
    }
}

#[derive(Clone, Resource, Serialize, Deserialize)]
pub struct WindowLayout {
    pub cell_search: NormalizedWindow,
    pub object_window: NormalizedWindow,
    pub viewport: NormalizedWindow,
}

impl Default for WindowLayout {
    fn default() -> Self {
        Self {
            cell_search: NormalizedWindow {
                pos: [0.05, 0.05],
                size: [0.40, 0.32],
            },
            object_window: NormalizedWindow {
                pos: [0.03, 0.06],
                size: [0.33, 0.48],
            },
            viewport: NormalizedWindow {
                pos: [0.50, 0.05],
                size: [0.50, 0.50],
            },
        }
    }
}

#[update(mode = "editor")]
pub fn update_screen_size(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let screen = ctx.viewport_rect();

    if !world.has_resource::<ScreenSize>() {
        world.insert_resource(ScreenSize::default());
    }
    let s = world.get_resource_mut::<ScreenSize>()?;
    s.w = screen.width();
    s.h = screen.height();
    Ok(())
}

pub fn save_layout(layout: &WindowLayout) {
    if let Ok(json) = serde_yaml::to_string(layout) {
        let _ = std::fs::write("editor_layout.json", json);
    }
}

pub fn load_layout() -> WindowLayout {
    std::fs::read_to_string("editor_layout.json")
        .ok()
        .and_then(|s| serde_yaml::from_str(&s).ok())
        .unwrap_or_default()
}
