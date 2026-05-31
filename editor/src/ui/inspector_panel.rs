use std::any::Any;

use anyhow::Result;
use apostasy_core::{
    egui::{self, Margin, Window},
    objects::{
        component::{Component, InspectorRegistry},
        fmt_key,
        world::World,
    },
    ui::ui_context::EguiContext,
    update,
};

use crate::ui::{DARK_BG, scenes_panel::CellSearchState};
#[update]
pub fn inspector(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if let Ok(cell_search_state) = world.get_resource::<CellSearchState>() {
        let Some(id) = cell_search_state.selected_obj else {
            return Ok(());
        };

        // collect the inspect fns for this object's components before borrowing object mutably
        let fns: Vec<fn(&mut dyn Any, &mut egui::Ui)> = {
            let registry = world.get_resource::<InspectorRegistry>()?;
            world
                .get_object(id)
                .unwrap()
                .get_components()
                .into_iter()
                .filter_map(|c: &Box<dyn Component + Send + Sync>| {
                    let type_id = std::any::Any::type_id(c.as_ref().as_any());
                    registry.inspectors.get(&type_id).copied()
                })
                .collect()
        };
        let obj_name = world.get_object(id).unwrap().name.clone();

        let label_text = format!("Inspector: {} ({})", obj_name, fmt_key(id));
        Window::new(label_text)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(DARK_BG)
                    .inner_margin(Margin::same(8)),
            )
            .default_pos([100.0, 100.0])
            .movable(true)
            .show(&ctx, |ui| {
                let obj = world.get_object_mut(id).unwrap();
                for (component, f) in obj.get_components_mut().into_iter().zip(fns.into_iter()) {
                    f(component.as_any_mut(), ui);
                    // ui.separator();
                }
            });
    }

    Ok(())
}
