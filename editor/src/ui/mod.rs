use anyhow::Result;
use apostasy_core::egui::Color32;
use apostasy_core::objects::world::World;
use apostasy_core::ui::ui_context::EguiContext;
use apostasy_core::{start, update};

pub mod shared;
pub use self::shared::*;

const DARK_BG: Color32 = Color32::from_rgb(18, 18, 18);
const PANEL_BG: Color32 = Color32::from_rgb(24, 24, 24);
const HEADER_BG: Color32 = Color32::from_rgb(30, 30, 30);
const ROW_ALT: Color32 = Color32::from_rgb(28, 28, 28);
const DIV_COL: Color32 = Color32::from_rgb(60, 60, 60);
const TEXT_COL: Color32 = Color32::WHITE;
const DIM_COL: Color32 = Color32::from_rgb(170, 170, 170);
const SEL_BG: Color32 = Color32::from_rgb(40, 80, 140);
const HOVER_BG: Color32 = Color32::from_rgb(38, 38, 50);

#[update(mode = "editor")]
pub fn update_screen_size(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let screen = ctx.content_rect();

    if !world.has_resource::<ScreenSize>() {
        world.insert_resource(ScreenSize::default());
    }
    let s = world.get_resource_mut::<ScreenSize>()?;
    let width_changed = (s.w - screen.width()).abs() > f32::EPSILON
        || (s.h - screen.height()).abs() > f32::EPSILON;
    s.w = screen.width();
    s.h = screen.height();

    if width_changed {
        if let Ok(object_window_resource) = world.get_resource_mut::<ObjectWindowState>() {
            object_window_resource.needs_layout_restore = true;
        }
        if let Ok(cell_search_state) = world.get_resource_mut::<CellSearchState>() {
            cell_search_state.needs_layout_restore = true;
        }
        if let Ok(viewport_info) = world.get_resource_mut::<ViewportInfo>() {
            viewport_info.needs_layout_restore = true;
        }
    }

    Ok(())
}

pub mod assets_panel;
pub mod cell_panel;
pub mod inspector_panel;
pub mod top_bar;
pub mod ui_manager;
pub mod viewport_panel;

use crate::ui::assets_panel::ObjectWindowState;
use crate::ui::cell_panel::CellSearchState;
use crate::ui::viewport_panel::ViewportInfo;

#[start(mode = "all")]
pub fn init(world: &mut World) -> Result<()> {
    world.remove_resource::<WindowLayout>();
    world.insert_resource(load_layout());
    dbg!(world.get_resource::<WindowLayout>().unwrap());
    Ok(())
}
