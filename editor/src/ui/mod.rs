use anyhow::Result;
use apostasy_core::{
    ecs::world::World,
    rendering::shared::{UpdateRenderer, anti_alisaing::AntiAliasing, shadow_settings::ShadowDistance},
    start,
    ui::{FontRegistry, ui_context::ViewportSize},
};

pub mod shared;
pub use self::shared::*;

pub mod style;
pub use self::style::{EditorStyle, Theme};

pub mod asset_editor;
pub mod assets_panel;
pub mod cell_panel;
pub mod gizmo;
pub mod inspector_panel;
pub mod keybind_widget;
pub mod preferences_panel;
pub mod top_bar;
pub mod ui_manager;
pub mod viewport_panel;

use asset_editor::AssetEditorState;
use assets_panel::DataWindowState;
use cell_panel::CellSearchState;
use inspector_panel::InspectorPanelState;
use preferences_panel::EditorPreferences;
use viewport_panel::{EditorGraphics, ViewportInfo};

#[start(mode = "all")]
pub fn init(world: &mut World) -> Result<()> {
    world.remove_resource::<WindowLayout>();
    let editor_layouts = load_layouts();
    let layout = editor_layouts
        .layouts
        .get(&editor_layouts.current)
        .cloned()
        .unwrap_or_default();

    world.insert_resource(ViewportInfo {
        open: layout.viewport_open,
        ..Default::default()
    });
    world.insert_resource(DataWindowState {
        open: layout.data_window_open,
        ..Default::default()
    });
    world.insert_resource(CellSearchState {
        open: layout.cell_open,
        ..Default::default()
    });
    world.insert_resource(InspectorPanelState {
        visible: layout.inspector_visible,
        ..Default::default()
    });
    world.insert_resource(AssetEditorState {
        open: layout.asset_editor_open,
        ..Default::default()
    });
    world.insert_resource(layout);
    world.insert_resource(editor_layouts);

    let prefs = EditorPreferences::load();
    let mut style = EditorStyle::from_theme(prefs.theme);
    style.font_size = prefs.font_size;
    world.insert_resource(style);

    if !prefs.active_font.is_empty()
        && let Ok(reg) = world.get_resource_mut::<FontRegistry>()
    {
        reg.set_active(prefs.active_font);
    }

    use crate::ecs::editor_camera::EditorCameraSettings;
    world.insert_resource(EditorCameraSettings {
        move_speed: prefs.camera_speed,
    });
    if let Ok(sd) = world.get_resource_mut::<ShadowDistance>() {
        sd.distance = prefs.shadow_distance;
        sd.cascade_count = prefs.cascade_count;
        sd.bias_constant = prefs.bias_constant;
        sd.bias_slope = prefs.bias_slope;
        sd.shadow_map_size = prefs.shadow_map_size;
    }

    let graphics = EditorGraphics::load();
    if let Ok(vs) = world.get_resource_mut::<ViewportSize>() {
        vs.supersample = graphics.supersample;
    }
    if let Ok(aa) = world.get_resource_mut::<AntiAliasing>() {
        aa.amount = graphics.anti_aliasing;
    }
    world.insert_resource(UpdateRenderer);

    Ok(())
}
