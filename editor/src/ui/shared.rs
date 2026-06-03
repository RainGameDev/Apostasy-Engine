use apostasy_core::egui::{self, Pos2, Vec2};
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

/// Normalized position and size for a window
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizedWindow {
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

impl NormalizedWindow {
    pub fn to_pos(&self, sw: f32, sh: f32) -> Pos2 {
        let size = self.to_size(sw, sh);
        let cx = self.pos[0] * sw;
        let cy = self.pos[1] * sh;
        Pos2::new(cx - size.x / 2.0, cy - size.y / 2.0)
    }
    pub fn to_size(&self, sw: f32, sh: f32) -> Vec2 {
        Vec2::new(self.size[0] * sw, self.size[1] * sh)
    }

    pub fn update_from_rect(&mut self, rect: egui::Rect, sw: f32, sh: f32) {
        let center = rect.center();
        self.pos = [center.x / sw, center.y / sh];
        self.size = [rect.size().x / sw, rect.size().y / sh];
    }
}

#[derive(Clone, Resource, Serialize, Debug, Deserialize)]
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

pub fn save_layout(layout: &WindowLayout) {
    if let Ok(yaml) = serde_yaml::to_string(layout) {
        std::fs::write("editor_layout.yaml", yaml).unwrap();
    }
}

pub fn load_layout() -> WindowLayout {
    std::fs::read_to_string("editor_layout.yaml")
        .ok()
        .and_then(|s| serde_yaml::from_str(&s).ok())
        .unwrap_or_default()
}
