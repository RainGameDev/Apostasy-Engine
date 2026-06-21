use anyhow::Result;
use apostasy_core::{
    cgmath::Vector3,
    ecs::{resources::input_manager::InputManager, world::World},
    ui::ui_context::EguiContext,
    update,
};
use apostasy_macros::Resource;

pub mod terrain_input;
pub mod terrain_panel;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TerrainTool {
    Modify,
    Smooth,
    Flatten,
}

#[derive(Resource, Clone)]
pub struct TerrainToolState {
    pub active: bool,
    pub tool: TerrainTool,
    pub brush_radius: f32,
    pub brush_strength: f32,
    /// Target height used by the Flatten tool. Set on first drag click.
    pub flatten_height: Option<f32>,
    /// Which texture layer index (into TerrainSettings.texture_layers) to paint.
    pub paint_layer: usize,
    /// Whether the user is currently dragging (mouse held).
    pub dragging: bool,

    /// Is vertex painting enabled.
    pub is_vertex_painting: bool,
    /// How strong is the color (basically its alpha)
    pub vertex_strength: f32,
    /// Vertex color A.
    pub vertex_color_a: [f32; 3],
    /// Vertex color B.
    pub vertex_color_b: [f32; 3],
}

impl Default for TerrainToolState {
    fn default() -> Self {
        Self {
            active: false,
            tool: TerrainTool::Modify,
            brush_radius: 10.0,
            brush_strength: 0.3,
            flatten_height: None,
            paint_layer: 0,
            dragging: false,
            is_vertex_painting: false,
            vertex_strength: 1.0,
            vertex_color_a: [1.0, 1.0, 1.0],
            vertex_color_b: [1.0, 1.0, 1.0],
        }
    }
}

/// Persisted vertex color brush settings, saved to / loaded from disk.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct TerrainVertexColorSettings {
    #[serde(default)]
    pub vertex_color_a: [f32; 3],
    #[serde(default)]
    pub vertex_color_b: [f32; 3],
    #[serde(default)]
    pub vertex_strength: f32,
}

impl Default for TerrainVertexColorSettings {
    fn default() -> Self {
        Self {
            vertex_color_a: [1.0, 1.0, 1.0],
            vertex_color_b: [1.0, 1.0, 1.0],
            vertex_strength: 1.0,
        }
    }
}

impl TerrainVertexColorSettings {
    pub const PATH: &'static str = "res/.editor/terrain_vertex_colors.yaml";

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

/// Updated every frame by `terrain_input` with the brush's current world-space hit point.
/// Cleared when the tool is inactive or the cursor leaves the viewport.
#[derive(Resource, Clone, Default)]
pub struct TerrainBrushGizmo {
    pub hit_pos: Option<Vector3<f32>>,
}

#[update(mode = "all")]
pub fn toggle_terrain_state(world: &mut World) -> Result<()> {
    if !world.has_resource::<TerrainToolState>() {
        return Ok(());
    }
    if let Ok(ctx) = world.get_resource::<EguiContext>() {
        if ctx.0.egui_wants_keyboard_input() {
            return Ok(());
        }
    }
    let toggle = world
        .get_resource::<InputManager>()?
        .is_keybind_active("TerrainMode");

    let state = world.get_resource_mut::<TerrainToolState>()?;

    if toggle {
        state.active = !state.active;
    }

    Ok(())
}
