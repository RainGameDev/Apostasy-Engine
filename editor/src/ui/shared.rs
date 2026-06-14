use anyhow::Result;
use apostasy_core::{egui::{self, Pos2, Vec2}, objects::world::World};
use apostasy_macros::{Resource, late_update};
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

fn default_scenes_panel() -> NormalizedWindow {
    NormalizedWindow { pos: [96.0, 420.0], size: [300.0, 200.0] }
}

fn default_asset_editor() -> NormalizedWindow {
    NormalizedWindow { pos: [700.0, 54.0], size: [380.0, 560.0] }
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
    #[serde(default = "default_scenes_panel")]
    pub scenes_panel: NormalizedWindow,
    #[serde(default)]
    pub scenes_open: bool,
    #[serde(default = "default_asset_editor")]
    pub asset_editor: NormalizedWindow,
    #[serde(default)]
    pub asset_editor_open: bool,
    #[serde(skip)]
    pub dirty: bool,
}

impl Default for WindowLayout {
    fn default() -> Self {
        Self {
            cell_search: NormalizedWindow { pos: [96.0, 54.0], size: [768.0, 346.0] },
            object_window: NormalizedWindow { pos: [58.0, 65.0], size: [634.0, 518.0] },
            viewport: NormalizedWindow { pos: [960.0, 54.0], size: [960.0, 540.0] },
            viewport_open: true,
            object_window_open: true,
            cell_open: true,
            inspector_visible: false,
            scenes_panel: default_scenes_panel(),
            scenes_open: false,
            asset_editor: default_asset_editor(),
            asset_editor_open: false,
            dirty: false,
        }
    }
}

const LAYOUT_PATH: &str = "res/.editor/editor_layout.yaml";

fn save_layout(layout: &WindowLayout) {
    let path = std::path::Path::new(LAYOUT_PATH);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml::to_string(layout) {
        let _ = std::fs::write(path, yaml);
    }
}

pub fn load_layout() -> WindowLayout {
    std::fs::read_to_string(LAYOUT_PATH)
        .ok()
        .and_then(|s| serde_yaml::from_str(&s).ok())
        .unwrap_or_default()
}

#[late_update(mode = "editor")]
pub fn flush_layout(world: &mut World) -> Result<()> {
    let should_save = if let Ok(layout) = world.get_resource_mut::<WindowLayout>() {
        if layout.dirty {
            layout.dirty = false;
            true
        } else {
            false
        }
    } else {
        false
    };

    if should_save {
        if let Ok(layout) = world.get_resource::<WindowLayout>() {
            save_layout(layout);
        }
    }
    Ok(())
}
