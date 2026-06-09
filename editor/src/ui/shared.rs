use apostasy_core::egui::{self, Pos2, Vec2};
use apostasy_macros::Resource;
use serde::{Deserialize, Serialize};

/// Fixed position and size for a window (in pixels)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizedWindow {
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

impl NormalizedWindow {
    pub fn to_pos(&self) -> Pos2 {
        Pos2::new(self.pos[0], self.pos[1])
    }
    pub fn to_size(&self) -> Vec2 {
        Vec2::new(self.size[0], self.size[1])
    }

    pub fn update_from_rect(&mut self, rect: egui::Rect) {
        self.pos = [rect.left(), rect.top()];
        self.size = [rect.size().x, rect.size().y];
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Resource, Serialize, Debug, Deserialize)]
pub struct WindowLayout {
    pub cell_search: NormalizedWindow,
    pub object_window: NormalizedWindow,
    pub viewport: NormalizedWindow,
    #[serde(default = "default_true")]
    pub viewport_open: bool,
    #[serde(default = "default_true")]
    pub object_window_open: bool,
    #[serde(default = "default_true")]
    pub cell_open: bool,
    #[serde(default)]
    pub inspector_visible: bool,
}

impl Default for WindowLayout {
    fn default() -> Self {
        Self {
            cell_search: NormalizedWindow {
                pos: [96.0, 54.0],
                size: [768.0, 346.0],
            },
            object_window: NormalizedWindow {
                pos: [58.0, 65.0],
                size: [634.0, 518.0],
            },
            viewport: NormalizedWindow {
                pos: [960.0, 54.0],
                size: [960.0, 540.0],
            },
            viewport_open: true,
            object_window_open: true,
            cell_open: true,
            inspector_visible: false,
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
