use std::any::{Any, TypeId};

use anyhow::Result;
use apostasy_core::{
    egui::{self, Margin, Rect, Stroke, Window},
    log_warn,
    objects::{
        component::{BoxedComponent, Component, InspectorRegistry},
        fmt_key,
        world::World,
    },
    ui::{DRAG_SIZE, ui_context::EguiContext},
    update,
};
use apostasy_macros::Resource;

use crate::ui::{EditorStyle, cell_panel::CellSearchState};

#[derive(Resource, Default, Clone)]
pub struct ComponentPickerState {
    pub open: bool,
    pub search: String,
    pub copied_component: Option<BoxedComponent>,
}

#[derive(Resource, Clone)]
pub struct InspectorPanelState {
    pub visible: bool,
    pub window_pos: Option<egui::Pos2>,
    pub window_size: Option<egui::Vec2>,
}

impl Default for InspectorPanelState {
    fn default() -> Self {
        Self {
            visible: true,
            window_pos: None,
            window_size: None,
        }
    }
}

#[update(mode = "editor")]
pub fn inspector(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let style = world.get_resource::<EditorStyle>().cloned().unwrap_or_default();

    if !world.has_resource::<ComponentPickerState>() {
        world.insert_resource(ComponentPickerState::default());
    }
    if !world.has_resource::<InspectorPanelState>() {
        world.insert_resource(InspectorPanelState::default());
    }

    let inspector_state = world.get_resource_mut::<InspectorPanelState>()?;
    let mut visible = inspector_state.visible;
    let window_pos = inspector_state.window_pos;
    let window_size = inspector_state.window_size;

    let selected_id = world
        .get_resource::<CellSearchState>()
        .ok()
        .and_then(|state| state.selected_obj);

    let label_text = selected_id
        .and_then(|id| {
            world
                .get_object(id)
                .map(|obj| format!("Inspector: {} ({})", obj.name, fmt_key(id)))
        })
        .unwrap_or_else(|| "Inspector".to_string());

    let fns: Vec<(TypeId, fn(&mut dyn Any, &mut egui::Ui))> = if let Some(id) = selected_id {
        let registry = world.get_resource::<InspectorRegistry>()?;
        world
            .get_object(id)
            .unwrap()
            .get_components()
            .into_iter()
            .filter_map(|c: &Box<dyn Component + Send + Sync>| {
                let type_id = std::any::Any::type_id(c.as_ref().as_any());
                registry
                    .inspectors
                    .get(&type_id)
                    .copied()
                    .map(|f| (type_id, f))
            })
            .collect()
    } else {
        Vec::new()
    };

    // Collect all registered component names once
    let all_component_names: Vec<&'static str> =
        inventory::iter::<apostasy_core::objects::component::ComponentRegistration>()
            .map(|r| r.type_name)
            .collect();

    // Collect names already on this object
    let existing_component_names: Vec<&str> = if let Some(id) = selected_id {
        world
            .get_object(id)
            .unwrap()
            .get_components()
            .into_iter()
            .map(|c| c.type_name().split("::").last().unwrap_or(c.type_name()))
            .collect()
    } else {
        Vec::new()
    };

    let picker_state = world.get_resource::<ComponentPickerState>().unwrap();
    let picker_open = picker_state.open;
    let mut new_picker_open = picker_open;
    let mut new_search = picker_state.search.clone();
    let copied_component = picker_state.copied_component.clone();
    let mut component_to_add: Option<String> = None;
    let mut component_to_remove: Option<TypeId> = None;
    let mut component_to_copy: Option<BoxedComponent> = None;
    let mut to_paste_component = false;

    let screen_height = ctx.input(|i| {
        i.raw
            .screen_rect
            .map(|rect| rect.height())
            .unwrap_or(1080.0)
    });
    let max_height = (screen_height - 100.0).max(260.0);
    let max_size = egui::vec2(680.0, max_height);
    let default_pos = window_pos.unwrap_or(egui::pos2(100.0, 100.0));
    let default_size = window_size
        .unwrap_or(egui::vec2(340.0, 520.0))
        .min(max_size);

    let mut window = Window::new(&label_text)
        .id(egui::Id::new("inspector_window"))
        .open(&mut visible)
        .order(egui::Order::Foreground)
        .frame(style.window_frame(&ctx).inner_margin(Margin {
            left: 8,
            right: 8,
            bottom: 8,
            top: 0,
        }));

    if let (Some(pos), Some(size)) = (window_pos, window_size) {
        window = window.default_rect(Rect::from_min_size(pos, size.min(max_size)));
    } else {
        window = window.default_pos(default_pos).default_size(default_size);
    }

    let window = window
        .max_size([680.0, max_height])
        .resizable(true)
        .movable(true)
        .show(&ctx, |ui| {
            ui.add_space(8.0);
            let footer_height = 80.0;
            let scroll_height = (ui.available_height() - footer_height).max(0.0);
            egui::ScrollArea::vertical()
                .id_salt("inspector_scroll")
                .auto_shrink([false; 2])
                .max_height(scroll_height)
                .show(ui, |ui| {
                    ui.heading(label_text.clone());
                    ui.add_space(8.0);

                    if let Some(id) = selected_id {
                        if let Some(obj) = world.get_object_mut(id) {
                            for (component, (type_id, f)) in
                                obj.get_components_mut().into_iter().zip(fns)
                            {
                                egui::Frame::new()
                                    .fill(style.panel_bg)
                                    .stroke(Stroke::new(1.0, style.div_col))
                                    .corner_radius(4.0)
                                    .inner_margin(4.0)
                                    .show(ui, |ui| {
                                        let name_full = component
                                            .type_name()
                                            .split("::")
                                            .collect::<Vec<&str>>();
                                        let final_name = name_full.last().unwrap().to_string();

                                        ui.horizontal(|ui| {
                                            ui.label(final_name);

                                            ui.button("󰍜").context_menu(|ui| {
                                                ui.set_min_width(196.0);
                                                ui.separator();
                                                if ui
                                                    .add_sized(
                                                        DRAG_SIZE,
                                                        egui::Button::new("Remove Component"),
                                                    )
                                                    .clicked()
                                                {
                                                    component_to_remove = Some(type_id);
                                                }

                                                if ui
                                                    .add_sized(
                                                        DRAG_SIZE,
                                                        egui::Button::new("Copy Component"),
                                                    )
                                                    .clicked()
                                                {
                                                    component_to_copy = Some(component.clone());
                                                }
                                                if ui
                                                    .add_sized(
                                                        DRAG_SIZE,
                                                        egui::Button::new("Cut Component"),
                                                    )
                                                    .clicked()
                                                {
                                                    component_to_remove = Some(type_id);
                                                    component_to_copy = Some(component.clone());
                                                }
                                                if ui
                                                    .add_sized(
                                                        DRAG_SIZE,
                                                        egui::Button::new("Paste Component"),
                                                    )
                                                    .clicked()
                                                    && copied_component.is_some()
                                                {
                                                    to_paste_component = true;
                                                }
                                                ui.separator();
                                            });
                                        });
                                        ui.separator();
                                        ui.indent("indent", |ui| {
                                            if ui.button("- Remove Component").clicked() {
                                                component_to_remove = Some(type_id);
                                            }
                                            f(component.as_any_mut(), ui);
                                        });
                                    });
                            }
                        }
                    } else {
                        ui.label(egui::RichText::new("No object selected").italics().weak());
                    }
                });

            ui.separator();

            if selected_id.is_some() && ui.button("+ Add Component").clicked() {
                new_picker_open = !new_picker_open;
                if new_picker_open {
                    new_search.clear();
                }
            }

            if new_picker_open {
                egui::Frame::popup(&ctx.global_style())
                    .fill(style.dark_bg)
                    .show(ui, |ui| {
                        let popup_width = ui.available_width().max(280.0);
                        ui.set_min_width(popup_width);
                        ui.set_min_height(85.0);
                        ui.set_max_height(85.0);

                        let search_resp = ui.text_edit_singleline(&mut new_search);
                        if picker_open != new_picker_open {
                            search_resp.request_focus();
                        }
                        ui.add_space(4.0);

                        let query = new_search.to_lowercase();

                        egui::ScrollArea::vertical()
                            .max_height(52.0)
                            .show(ui, |ui| {
                                let mut any_shown = false;

                                for &name in &all_component_names {
                                    if !query.is_empty() && !name.to_lowercase().contains(&query) {
                                        continue;
                                    }

                                    let name = name.split("::").collect::<Vec<&str>>();
                                    let name = name.last().unwrap();
                                    let already_present = existing_component_names.contains(name);

                                    ui.add_enabled_ui(!already_present, |ui| {
                                        let resp = ui.selectable_label(false, *name);
                                        if resp.clicked() && !already_present {
                                            component_to_add = Some(name.to_string());
                                            new_picker_open = false;
                                        }
                                        if already_present {
                                            resp.on_disabled_hover_text("Already on this object");
                                        }
                                    });

                                    any_shown = true;
                                }

                                if !any_shown {
                                    ui.label(
                                        egui::RichText::new("No components found").italics().weak(),
                                    );
                                }
                                ui.allocate_space(ui.available_size());
                            });

                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            new_picker_open = false;
                        }
                    });
            }
        });

    if let Some(response) = window {
        if let Ok(state) = world.get_resource_mut::<InspectorPanelState>() {
            state.window_pos = Some(response.response.rect.min);
            state.window_size = Some(response.response.rect.size());
        }
    }

    if let Some(type_id) = component_to_remove {
        if let Some(id) = selected_id {
            if let Some(obj) = world.get_object_mut(id) {
                obj.remove_component_by_type_id(type_id);
            }
        }
    }

    if to_paste_component {
        if let Some(id) = selected_id {
            if let Some(obj) = world.get_object_mut(id) {
                dbg!(component_to_copy.clone());
                obj.add_boxed_component(copied_component.clone().unwrap());
            }
        }
    }

    if let Ok(state) = world.get_resource_mut::<ComponentPickerState>() {
        state.open = new_picker_open;
        state.search = new_search;
        if component_to_copy.is_some() {
            state.copied_component = component_to_copy;
        }
    }

    if let Some(name) = component_to_add {
        if let Some(id) = selected_id {
            if let Some(obj) = world.get_object_mut(id) {
                if let Err(e) = obj.add_component_by_name(&name) {
                    log_warn!("Failed to add component '{}': {}", name, e);
                }
            }
        }
    }

    if let Ok(state) = world.get_resource_mut::<InspectorPanelState>() {
        state.visible = visible;
    }

    Ok(())
}
