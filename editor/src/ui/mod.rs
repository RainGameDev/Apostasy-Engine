use anyhow::Result;
use apostasy_core::objects::world::World;
use apostasy_core::start;
use apostasy_core::ui::FontRegistry;

pub mod shared;
pub use self::shared::*;

pub mod style;
pub use self::style::{EditorStyle, Theme};

pub mod assets_panel;
pub mod cell_panel;
pub mod inspector_panel;
pub mod preferences_panel;
pub mod top_bar;
pub mod ui_manager;
pub mod viewport_panel;

use assets_panel::ObjectWindowState;
use cell_panel::CellSearchState;
use inspector_panel::InspectorPanelState;
use preferences_panel::EditorPreferences;
use viewport_panel::ViewportInfo;

#[start(mode = "all")]
pub fn init(world: &mut World) -> Result<()> {
    world.remove_resource::<WindowLayout>();
    let layout = load_layout();

    world.insert_resource(ViewportInfo {
        open: layout.viewport_open,
        ..Default::default()
    });
    world.insert_resource(ObjectWindowState {
        open: layout.object_window_open,
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

    world.insert_resource(layout);

    let prefs = EditorPreferences::load();
    let mut style = EditorStyle::from_theme(prefs.theme);
    style.font_size = prefs.font_size;
    world.insert_resource(style);

    if !prefs.active_font.is_empty() {
        if let Ok(reg) = world.get_resource_mut::<FontRegistry>() {
            reg.set_active(prefs.active_font);
        }
    }

    Ok(())
}
