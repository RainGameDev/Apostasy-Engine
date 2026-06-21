use std::any::TypeId;

use anyhow::Result;
use apostasy_core::{
    egui::{self, Margin, Rect, Stroke, Window},
    log_warn,
    ecs::{
        component::{BoxedComponent, Component, InspectorRegistry},
        tag::TagRegistration,
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
    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();

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

    let entity_name = selected_id
        .and_then(|id| world.get_name(id).map(|n| n.to_string()));

    let label_text = entity_name
        .as_ref()
        .map(|name| format!("Inspector: {} ({:?})", name, selected_id.unwrap()))
        .unwrap_or_else(|| "Inspector".to_string());

    // Collect component TypeIds for the selected entity
    let component_type_ids: Vec<TypeId> = selected_id
        .map(|id| world.get_entity_component_type_ids(id))
        .unwrap_or_default();

    // Build (TypeId, inspect_fn) pairs from InspectorRegistry
    let fns: Vec<(TypeId, fn(&mut dyn std::any::Any, &mut egui::Ui))> = if let Some(_id) = selected_id {
        let registry = world.get_resource::<InspectorRegistry>()?;
        component_type_ids.iter()
            .filter_map(|&type_id| {
                registry.inspectors.get(&type_id).copied().map(|f| (type_id, f))
            })
            .collect()
    } else {
        Vec::new()
    };

    // Collect all registered component names once
    let all_component_names: Vec<&'static str> =
        inventory::iter::<apostasy_core::ecs::component::ComponentRegistration>()
            .map(|r| r.type_name)
            .collect();

    // Collect names already on this entity
    let existing_component_type_names: Vec<&'static str> = component_type_ids.iter()
        .filter_map(|&type_id| selected_id.and_then(|id| world.get_component_type_name(id, type_id)))
        .collect();

    let existing_component_names: Vec<&str> = existing_component_type_names.iter()
        .map(|full_name| full_name.split("::").last().unwrap_or(full_name))
        .collect();

    // Tag info
    let tag_type_ids: Vec<TypeId> = selected_id
        .map(|id| world.get_entity_tag_type_ids(id))
        .unwrap_or_default();

    let is_editor_camera = tag_type_ids.iter().any(|type_id| {
        inventory::iter::<TagRegistration>()
            .find(|r| (r.type_id)() == *type_id)
            .map(|r| r.type_name.eq_ignore_ascii_case("EditorCamera"))
            .unwrap_or(false)
    });

    let has_camera_component = existing_component_names.iter().any(|&n| n == "Camera");

    let all_tag_names: Vec<&'static str> = inventory::iter::<TagRegistration>()
        .filter(|r| !r.hidden)
        .filter(|r| {
            !r.type_name.eq_ignore_ascii_case("ActiveCamera")
                || (has_camera_component && !is_editor_camera)
        })
        .map(|r| r.type_name)
        .collect();

    let existing_tag_names: Vec<&'static str> = tag_type_ids.iter()
        .filter_map(|&type_id| {
            inventory::iter::<TagRegistration>()
                .find(|r| (r.type_id)() == type_id)
                .map(|r| r.type_name)
        })
        .collect();

    let picker_state = world.get_resource::<ComponentPickerState>().unwrap();
    let picker_open = picker_state.open;
    let mut new_picker_open = picker_open;
    let mut new_search = picker_state.search.clone();
    let copied_component = picker_state.copied_component.clone();
    let mut component_to_add: Option<String> = None;
    let mut component_to_remove: Option<TypeId> = None;
    let mut component_to_copy: Option<BoxedComponent> = None;
    let mut to_paste_component = false;
    let mut tag_to_add: Option<String> = None;
    let mut tag_to_remove: Option<String> = None;
    let mut pending_name: Option<(apostasy_core::ecs::cell::ObjectId, String)> = None;

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
            bottom: 0,
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
                    ui.add_space(8.0);

                    if let Some(id) = selected_id {
                        // Name field
                        {
                            let mut name_buf = world.get_name(id).unwrap_or("Entity").to_string();
                            ui.horizontal(|ui| {
                                ui.label("Name");

                                let btn_w = 72.0;
                                let text_w =
                                    (ui.available_width() - btn_w - ui.spacing().item_spacing.x)
                                        .max(0.0);
                                let resp = ui.add_sized(
                                    egui::vec2(text_w, DRAG_SIZE.y),
                                    egui::TextEdit::singleline(&mut name_buf)
                                        .hint_text("Entity name..."),
                                );
                                if resp.changed() {
                                    pending_name = Some((id, name_buf.clone()));
                                }

                                let tag_count = existing_tag_names.len();
                                let tags_label = if tag_count > 0 {
                                    format!("Tags ({})", tag_count)
                                } else {
                                    "Tags".to_string()
                                };
                                let tags_btn = ui.add_sized(
                                    egui::vec2(btn_w, DRAG_SIZE.y),
                                    egui::Button::new(&tags_label),
                                );

                                let popup_id = ui.id().with("tags_popup");
                                let popup_resp = egui::Popup::from_response(&tags_btn)
                                    .id(popup_id)
                                    .open_memory(
                                        tags_btn.clicked().then_some(egui::SetOpenCommand::Toggle),
                                    )
                                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                    .show(|ui| {
                                        ui.set_min_width(140.0);
                                        if all_tag_names.is_empty() {
                                            ui.label(
                                                egui::RichText::new("No tags registered")
                                                    .italics()
                                                    .weak(),
                                            );
                                        } else {
                                            for &type_name in &all_tag_names {
                                                let short_name = type_name
                                                    .split("::")
                                                    .last()
                                                    .unwrap_or(type_name);
                                                let has_tag = existing_tag_names
                                                    .iter()
                                                    .any(|&t| t == type_name);
                                                let mut checked = has_tag;
                                                if ui.checkbox(&mut checked, short_name).changed() {
                                                    if checked {
                                                        tag_to_add = Some(type_name.to_string());
                                                    } else {
                                                        tag_to_remove = Some(type_name.to_string());
                                                    }
                                                }
                                            }
                                        }
                                    });

                                if let Some(ref inner) = popup_resp {
                                    let pressed_outside = ui.input(|i| {
                                        i.pointer.any_pressed()
                                            && i.pointer
                                                .press_origin()
                                                .map(|p| !inner.response.rect.contains(p))
                                                .unwrap_or(false)
                                    });
                                    if pressed_outside {
                                        egui::Popup::close_id(ui.ctx(), popup_id);
                                    }
                                }
                            });
                        }
                        ui.add_space(6.0);

                        // Component inspector panels
                        for (type_id, inspect_fn) in &fns {
                            let type_name = world.get_component_type_name(id, *type_id)
                                .unwrap_or("Unknown");
                            let short_name = type_name.split("::").last().unwrap_or(type_name).to_string();
                            let copy_tid = *type_id;
                            let copy_fn = *inspect_fn;

                            egui::Frame::new()
                                .fill(style.panel_bg)
                                .stroke(Stroke::new(1.0, style.div_col))
                                .corner_radius(4.0)
                                .inner_margin(4.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(&short_name);

                                        ui.button("󰍜").context_menu(|ui| {
                                            ui.set_min_width(196.0);
                                            ui.separator();
                                            if ui.add_sized(DRAG_SIZE, egui::Button::new("Remove Component")).clicked() {
                                                component_to_remove = Some(copy_tid);
                                            }
                                            // Copy/cut/paste of boxed components needs type-erased cloning
                                            // which is not directly available here; skip for now.
                                            ui.separator();
                                        });
                                    });
                                    ui.separator();
                                    ui.indent("indent", |ui| {
                                        if ui.button("- Remove Component").clicked() {
                                            component_to_remove = Some(copy_tid);
                                        }
                                        world.with_component_any_mut(id, copy_tid, |any| {
                                            copy_fn(any, ui);
                                        });
                                    });
                                });
                        }

                        if fns.is_empty() && selected_id.is_some() {
                            ui.label(egui::RichText::new("No inspectable components").italics().weak());
                        }
                    } else {
                        ui.label(egui::RichText::new("No object selected").italics().weak());
                    }
                });

            ui.separator();

            let add_btn_resp = if selected_id.is_some() {
                Some(ui.button("+ Add Component"))
            } else {
                None
            };
            if let Some(ref resp) = add_btn_resp {
                if resp.clicked() {
                    new_picker_open = !new_picker_open;
                    if new_picker_open {
                        new_search.clear();
                    }
                }
            }

            ui.add_space(8.0);

            if new_picker_open {
                let anchor = add_btn_resp
                    .as_ref()
                    .map(|r| r.rect.left_bottom())
                    .unwrap_or_default();

                let area_resp = egui::Area::new(ui.id().with("component_picker_area"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(anchor)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(&ui.style())
                            .fill(style.dark_bg)
                            .show(ui, |ui| {
                                ui.set_width(280.0);

                                let search_resp = ui.text_edit_singleline(&mut new_search);
                                if picker_open != new_picker_open {
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

                                            let short = name.split("::").last().unwrap_or(name);
                                            let already_present = existing_component_names.contains(&short);

                                            ui.add_enabled_ui(!already_present, |ui| {
                                                let resp = ui.selectable_label(false, short);
                                                if resp.clicked() && !already_present {
                                                    component_to_add = Some(name.to_string());
                                                    new_picker_open = false;
                                                }
                                                if already_present {
                                                    resp.on_disabled_hover_text("Already on this entity");
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
                            });
                    });

                if ui.input(|i| i.key_pressed(egui::Key::Escape))
                    || (picker_open
                        && ui.input(|i| i.pointer.any_click())
                        && !area_resp.response.hovered())
                {
                    new_picker_open = false;
                }
            }
        });

    if let Some(response) = window
        && let Ok(state) = world.get_resource_mut::<InspectorPanelState>()
    {
        state.window_pos = Some(response.response.rect.min);
        state.window_size = Some(response.response.rect.size());
    }

    // Apply pending name change
    if let Some((id, name)) = pending_name {
        world.set_name(id, &name);
    }

    // Remove component
    if let Some(type_id) = component_to_remove
        && let Some(id) = selected_id
    {
        world.remove_component_by_type_id(id, type_id);
    }

    // Paste component
    if to_paste_component
        && let Some(id) = selected_id
        && let Some(comp) = copied_component.clone()
    {
        world.add_boxed_component(id, comp);
    }

    if let Ok(state) = world.get_resource_mut::<ComponentPickerState>() {
        state.open = new_picker_open;
        state.search = new_search;
        if component_to_copy.is_some() {
            state.copied_component = component_to_copy;
        }
    }

    if let Some(name) = component_to_add
        && let Some(id) = selected_id
    {
        if let Err(e) = world.add_component_by_name(id, &name) {
            log_warn!("Failed to add component '{}': {}", name, e);
        }
    }

    if let Some(ref type_name) = tag_to_add
        && let Some(id) = selected_id
    {
        let is_singleton = inventory::iter::<TagRegistration>()
            .find(|r| r.type_name.eq_ignore_ascii_case(type_name))
            .map(|r| r.singleton)
            .unwrap_or(false);

        if is_singleton {
            let tag_type_id = inventory::iter::<TagRegistration>()
                .find(|r| r.type_name.eq_ignore_ascii_case(type_name))
                .map(|r| (r.type_id)());
            if let Some(ttid) = tag_type_id {
                let ids_to_clear: Vec<_> = world.get_all_ids()
                    .into_iter()
                    .filter(|&oid| oid != id && world.get_entity_tag_type_ids(oid).contains(&ttid))
                    .collect();
                for oid in ids_to_clear {
                    world.remove_tag_by_name(oid, type_name);
                }
            }
        }

        if let Err(e) = world.add_tag_by_name(id, type_name) {
            log_warn!("Failed to add tag '{}': {}", type_name, e);
        }
    }

    if let Some(type_name) = tag_to_remove
        && let Some(id) = selected_id
    {
        world.remove_tag_by_name(id, &type_name);
    }

    if let Ok(state) = world.get_resource_mut::<InspectorPanelState>() {
        state.visible = visible;
    }

    Ok(())
}
