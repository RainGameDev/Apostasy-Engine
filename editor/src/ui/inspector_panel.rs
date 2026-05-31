use std::any::Any;

use anyhow::Result;
use apostasy_core::{
    egui::{self, Margin, Window},
    log_warn,
    objects::{
        component::{Component, InspectorRegistry},
        fmt_key,
        world::World,
    },
    ui::ui_context::EguiContext,
    update,
};
use apostasy_macros::Resource;

use crate::ui::{DARK_BG, scenes_panel::CellSearchState};

#[derive(Resource, Default, Clone)]
pub struct ComponentPickerState {
    pub open: bool,
    pub search: String,
}

#[update]
pub fn inspector(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    if !world.has_resource::<ComponentPickerState>() {
        world.insert_resource(ComponentPickerState::default());
    }

    if let Ok(cell_search_state) = world.get_resource::<CellSearchState>() {
        let Some(id) = cell_search_state.selected_obj else {
            return Ok(());
        };

        // Collect the inspect fns for this object's components before borrowing object mutably
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

        // Collect all registered component names once
        let all_component_names: Vec<&'static str> =
            inventory::iter::<apostasy_core::objects::component::ComponentRegistration>()
                .map(|r| r.type_name)
                .collect();

        // Collect names already on this object
        let existing_component_names: Vec<&'static str> = world
            .get_object(id)
            .unwrap()
            .get_components()
            .into_iter()
            .map(|c| c.type_name())
            .collect();

        let label_text = format!("Inspector: {} ({})", obj_name, fmt_key(id));

        // Read picker state
        let (picker_open, search_text) = {
            let state = world.get_resource::<ComponentPickerState>().unwrap();
            (state.open, state.search.clone())
        };

        let mut new_picker_open = picker_open;
        let mut new_search = search_text.clone();
        let mut component_to_add: Option<String> = None;

        Window::new(label_text)
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(DARK_BG)
                    .inner_margin(Margin::same(8)),
            )
            .default_pos([100.0, 100.0])
            .movable(true)
            .show(&ctx, |ui| {
                // existing component editors
                let obj = world.get_object_mut(id).unwrap();
                for (component, f) in obj.get_components_mut().into_iter().zip(fns) {
                    f(component.as_any_mut(), ui);
                }

                ui.separator();

                if ui.button("+ Add Component").clicked() {
                    new_picker_open = !picker_open;
                    if new_picker_open {
                        new_search.clear();
                    }
                }

                if new_picker_open {
                    egui::Frame::popup(&ctx.global_style())
                        .fill(DARK_BG)
                        .show(ui, |ui| {
                            ui.set_min_width(200.0);

                            // Search bar
                            let search_resp = ui.text_edit_singleline(&mut new_search);
                            if picker_open != new_picker_open {
                                // request focus
                                search_resp.request_focus();
                            }
                            ui.add_space(4.0);

                            let query = new_search.to_lowercase();

                            egui::ScrollArea::vertical()
                                .max_height(200.0)
                                .show(ui, |ui| {
                                    let mut any_shown = false;

                                    for &name in &all_component_names {
                                        if !query.is_empty()
                                            && !name.to_lowercase().contains(&query)
                                        {
                                            continue;
                                        }

                                        let already_present = existing_component_names
                                            .iter()
                                            .any(|&e| e.to_lowercase() == name.to_lowercase());

                                        ui.add_enabled_ui(!already_present, |ui| {
                                            let resp = ui.selectable_label(false, name);
                                            if resp.clicked() && !already_present {
                                                component_to_add = Some(name.to_string());
                                                new_picker_open = false;
                                            }
                                            if already_present {
                                                resp.on_disabled_hover_text(
                                                    "Already on this object",
                                                );
                                            }
                                        });

                                        any_shown = true;
                                    }

                                    if !any_shown {
                                        ui.label(
                                            egui::RichText::new("No components found")
                                                .italics()
                                                .weak(),
                                        );
                                    }
                                    ui.allocate_space(ui.available_size());
                                });

                            // Close on Escape
                            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                new_picker_open = false;
                            }
                        });
                }
            });

        // Apply picker state changes
        if let Ok(state) = world.get_resource_mut::<ComponentPickerState>() {
            state.open = new_picker_open;
            state.search = new_search;
        }

        // Add component outside of any borrow
        if let Some(name) = component_to_add
            && let Some(obj) = world.get_object_mut(id)
            && let Err(e) = obj.add_component_by_name(&name)
        {
            log_warn!("Failed to add component '{}': {}", name, e);
        }
    }

    Ok(())
}
