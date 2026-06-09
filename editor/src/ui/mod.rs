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

use preferences_panel::EditorPreferences;

#[start(mode = "all")]
pub fn init(world: &mut World) -> Result<()> {
    world.remove_resource::<WindowLayout>();
    world.insert_resource(load_layout());
    dbg!(world.get_resource::<WindowLayout>().unwrap());

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
