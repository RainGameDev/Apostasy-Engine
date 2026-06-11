use anyhow::Result;
use apostasy_core::{
    cgmath::Vector3,
    egui::{self, Color32, ComboBox, Image, Label, RichText, Sense, Slider, Window},
    objects::{components::transform::Transform, scene::ObjectId, world::World},
    physics::collider::Collider,
    rendering::{
        components::camera::{Camera, EditorCamera, get_perspective_projection, get_view_matrix},
        shared::{UpdateRenderer, anti_alisaing::{AntiAliasing, AntiAliasingAmount}},
    },
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
};
use apostasy_macros::Resource;

#[derive(Resource, Default, Clone)]
pub struct ViewportContextMenu {
    pub hit_obj: Option<ObjectId>,
}

#[derive(Resource, Clone)]
pub struct ViewportInfo {
    pub is_hovered: bool,
    pub needs_layout_restore: bool,
    pub open: bool,
}

impl Default for ViewportInfo {
    fn default() -> Self {
        Self {
            is_hovered: false,
            needs_layout_restore: false,
            open: true,
        }
    }
}

use super::EditorStyle;
use crate::{
    objects::editor_camera::EditorCameraSettings,
    systems::object_focus::IsObjectFocused,
    ui::{
        cell_panel::CellSearchState,
        inspector_panel::InspectorPanelState,
        shared::{WindowLayout, save_layout},
    },
};

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct EditorGraphics {
    #[serde(default = "EditorGraphics::default_supersample")]
    pub supersample: f32,
    #[serde(default)]
    pub anti_aliasing: AntiAliasingAmount,
}

impl Default for EditorGraphics {
    fn default() -> Self {
        Self { supersample: 1.0, anti_aliasing: AntiAliasingAmount::X0 }
    }
}

impl EditorGraphics {
    pub const PATH: &'static str = "res/.editor/editor_graphics.yaml";

    fn default_supersample() -> f32 { 1.0 }

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

#[update(mode = "editor")]
pub fn viewport(world: &mut World) -> Result<()> {
    if !world.has_resource::<ViewportInfo>() {
        world.insert_resource(ViewportInfo::default());
    }

    let mut is_open = world
        .get_resource::<ViewportInfo>()
        .map(|v| v.open)
        .unwrap_or(true);
    if !world.has_resource::<WindowLayout>() || !world.has_resource::<EditorCameraSettings>() {
        return Ok(());
    }
    if !is_open {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();
    let state = world.get_resource::<WindowLayout>()?.viewport.clone();
    let (pos, size) = (state.to_pos(), state.to_size());

    let mut window = Window::new("Viewport")
        .default_pos(pos)
        .default_size(size)
        .resizable(true)
        .movable(true);

    let viewport_info = world.get_resource_mut::<ViewportInfo>()?;
    if viewport_info.needs_layout_restore {
        window = window.current_pos(pos).fixed_size(size).constrain(false);
        viewport_info.needs_layout_restore = false;
    }

    let (available_options, aa_before, mut aa_selected) = {
        let aa = world.get_resource::<AntiAliasing>().unwrap();
        (aa.available_options.clone(), aa.amount, aa.amount)
    };

    let mut camera_speed = world
        .get_resource::<EditorCameraSettings>()
        .unwrap()
        .move_speed;
    let mut frame_rect_out = None;
    let viewport_texture = world.get_resource::<ViewportTexture>().ok().map(|r| r.0);
    let supersample = world.get_resource::<ViewportSize>().map(|v| v.supersample).unwrap_or(1.0);

    if !world.has_resource::<ViewportContextMenu>() {
        world.insert_resource(ViewportContextMenu::default());
    }
    let ctx_obj_id: Option<ObjectId> = world
        .get_resource::<ViewportContextMenu>()
        .ok()
        .and_then(|r| r.hit_obj);
    let ctx_obj_name: Option<String> =
        ctx_obj_id.and_then(|id| world.get_object(id).map(|o| o.name.clone()));

    let mut pending_focus = false;
    let mut pending_inspect = false;
    let mut pending_copy = false;
    let mut pending_duplicate = false;
    let mut pending_delete = false;

    // Gizmo state and per-frame data (all extracted before show so world is free during show)
    if !world.has_resource::<crate::ui::gizmo::GizmoState>() {
        world.insert_resource(crate::ui::gizmo::GizmoState::default());
    }
    let mut gizmo_state = world
        .get_resource::<crate::ui::gizmo::GizmoState>()
        .ok()
        .cloned()
        .unwrap_or_default();
    let gizmo_data: Option<(ObjectId, Transform, apostasy_core::cgmath::Matrix4<f32>, Option<Collider>)> = {
        let sel_id = world
            .get_resource::<CellSearchState>()
            .ok()
            .and_then(|s| s.selected_obj);
        sel_id.and_then(|id| {
            let obj_t = world
                .get_object(id)?
                .get_component::<Transform>()
                .ok()?
                .clone();
            let collider = world
                .get_object(id)
                .and_then(|o| o.get_component::<Collider>().ok().cloned());
            let cam_t = world
                .get_objects_with_component::<Camera>()
                .first()?
                .get_component::<Transform>()
                .ok()?
                .clone();
            let cam_c = world
                .get_objects_with_component::<Camera>()
                .first()?
                .get_component::<Camera>()
                .ok()?
                .clone();
            let aspect = world
                .get_resource::<ViewportSize>()
                .map(|v| v.logical_width / v.logical_height)
                .unwrap_or(1.0);
            let view_proj = get_perspective_projection(&cam_c, aspect) * get_view_matrix(&cam_t);
            Some((id, obj_t, view_proj, collider))
        })
    };
    let mut new_gizmo_state_from_fn: Option<crate::ui::gizmo::GizmoState> = None;
    let mut gizmo_transform_out: Option<Transform> = None;

    let vp = window
        .open(&mut is_open)
        .resizable(true)
        .movable(true)
        .title_bar(true)
        .drag_area(egui::WindowDrag::TitleBar)
        .frame(style.window_frame(&ctx))
        .show(&ctx, |ui| {
            let bar_h = style.header_height();
            egui::ScrollArea::horizontal()
                .id_salt("viewport_bar_scroll")
                .max_height(bar_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_height(bar_h);
                    ui.horizontal_centered(|ui| {
                        ui.add_space(6.0);

                        ComboBox::from_label("MSAA")
                            .selected_text(format!("{:?}", aa_selected))
                            .show_ui(ui, |ui| {
                                let options = [
                                    (AntiAliasingAmount::X0, "None"),
                                    (AntiAliasingAmount::X2, "X2"),
                                    (AntiAliasingAmount::X4, "X4"),
                                    (AntiAliasingAmount::X8, "X8"),
                                ];
                                for (amount, label) in options {
                                    if available_options.contains(&amount) {
                                        ui.selectable_value(&mut aa_selected, amount, label);
                                    }
                                }
                            });

                        ui.add_space(8.0);
                        ui.label("Camera Speed");
                        ui.add(Slider::new(&mut camera_speed, 1.0..=256.0).text("M/s"));

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);
                        use crate::ui::gizmo::GizmoMode;
                        if ui.selectable_label(gizmo_state.mode == GizmoMode::Translate, "Move").clicked() {
                            gizmo_state.mode = GizmoMode::Translate;
                            gizmo_state.drag = None;
                        }
                        if ui.selectable_label(gizmo_state.mode == GizmoMode::Rotate, "Rotate").clicked() {
                            gizmo_state.mode = GizmoMode::Rotate;
                            gizmo_state.drag = None;
                        }
                        if ui.selectable_label(gizmo_state.mode == GizmoMode::Scale, "Scale").clicked() {
                            gizmo_state.mode = GizmoMode::Scale;
                            gizmo_state.drag = None;
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                        if ui.selectable_label(!gizmo_state.local, "Global").clicked() {
                            gizmo_state.local = false;
                            gizmo_state.drag = None;
                        }
                        if ui.selectable_label(gizmo_state.local, "Local").clicked() {
                            gizmo_state.local = true;
                            gizmo_state.drag = None;
                        }
                    });
                });

            // Some othershit idc

            ui.separator();
            let available_size = ui.available_size();
            if available_size.x <= 0.0 || available_size.y <= 0.0 {
                return;
            }

            let (frame_rect, frame_resp) = ui.allocate_exact_size(available_size, Sense::click());
            frame_rect_out = Some(frame_rect);
            ui.painter()
                .rect_filled(frame_rect, 4.0, Color32::from_gray(40));

            if let Some(texture_id) = viewport_texture {
                let image = Image::new((texture_id, available_size));
                ui.put(frame_rect, image);
            } else {
                let label =
                    Label::new(RichText::new("Viewport initializing...").color(Color32::WHITE));
                ui.put(frame_rect, label);
            }

            if let Some((_, ref obj_t, view_proj, ref maybe_collider)) = gizmo_data {
                let (new_t, gs) = crate::ui::gizmo::gizmo(ui, gizmo_state.clone(), obj_t, view_proj, frame_rect);
                gizmo_transform_out = new_t;
                new_gizmo_state_from_fn = Some(gs);
                if let Some(collider) = maybe_collider {
                    crate::ui::gizmo::collider_gizmo(ui, obj_t, collider, view_proj, frame_rect);
                }
            }

            if ctx_obj_id.is_some() {
                frame_resp.context_menu(|ui| {
                    ui.set_min_width(190.0);
                    ui.weak(ctx_obj_name.as_deref().unwrap_or("Object"));
                    ui.separator();
                    if ui.button("Teleport to Object").clicked() {
                        pending_focus = true;
                        ui.close();
                    }
                    if ui.button("Inspect Object").clicked() {
                        pending_inspect = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Copy Object").clicked() {
                        pending_copy = true;
                        ui.close();
                    }
                    if ui.button("Duplicate Object").clicked() {
                        pending_duplicate = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Delete Object").clicked() {
                        pending_delete = true;
                        ui.close();
                    }
                });
            }
        });

    if let Some(response) = vp {
        let rect = response.response.rect;
        let current_size = rect.size();
        let pixels_per_point = ctx.pixels_per_point();
        let mut pixel_w = (current_size.x * pixels_per_point * supersample).ceil();
        let mut pixel_h = (current_size.y * pixels_per_point * supersample).ceil();
        const MAX_DIM: f32 = 8192.0;
        pixel_w = pixel_w.clamp(1.0, MAX_DIM);
        pixel_h = pixel_h.clamp(1.0, MAX_DIM);

        {
            let viewport_size = world.get_resource_mut::<ViewportSize>().unwrap();
            viewport_size.logical_width = current_size.x;
            viewport_size.logical_height = current_size.y;
            viewport_size.pixel_width = pixel_w;
            viewport_size.pixel_height = pixel_h;
            if let Some(frame_rect) = frame_rect_out {
                viewport_size.logical_x = frame_rect.min.x;
                viewport_size.logical_y = frame_rect.min.y;
                viewport_size.logical_width = frame_rect.width();
                viewport_size.logical_height = frame_rect.height();
            }
        }

        let viewport_info = world.get_resource_mut::<ViewportInfo>()?;
        viewport_info.is_hovered = response.response.hovered();
        viewport_info.open = is_open;

        let layout = world.get_resource_mut::<WindowLayout>()?;
        layout.viewport.update_from_rect(rect);
        save_layout(layout);
    }

    if pending_focus {
        if let Some(id) = ctx_obj_id {
            let selected_transform = world
                .get_object(id)
                .and_then(|o| o.get_component::<Transform>().ok())
                .map(|t| t.clone());
            if let Some(selected_transform) = selected_transform {
                world.insert_resource(IsObjectFocused);
                if let Ok(editor_camera) = world.get_object_with_tag_mut::<EditorCamera>() {
                    if let Ok(transform) = editor_camera.get_component_mut::<Transform>() {
                        transform.local_position = selected_transform.global_position
                            - (transform.global_rotation * Vector3::new(0.0, 0.0, -10.0));
                        transform.global_position = transform.local_position;
                        transform.look_at(selected_transform.global_position);
                    }
                }
            }
        }
    }

    if pending_inspect {
        if let Some(id) = ctx_obj_id {
            if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
                s.selected_obj = Some(id);
            }
            if let Ok(s) = world.get_resource_mut::<InspectorPanelState>() {
                s.visible = true;
            }
        }
    }

    if pending_copy {
        if let Some(id) = ctx_obj_id {
            if let Some(obj) = world.get_object(id).cloned() {
                if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
                    s.copied_obj = Some(obj);
                }
            }
        }
    }

    if pending_duplicate {
        if let Some(id) = ctx_obj_id {
            if let Some(obj) = world.get_object(id).cloned() {
                world.add_object(obj);
            }
        }
    }

    // Write back gizmo state (drag + mode) and apply any transform produced by dragging
    {
        let final_state = new_gizmo_state_from_fn.unwrap_or(gizmo_state);
        if let Ok(s) = world.get_resource_mut::<crate::ui::gizmo::GizmoState>() {
            *s = final_state;
        }
    }
    if let (Some(new_t), Some((id, _, _, _))) = (gizmo_transform_out, gizmo_data) {
        if let Some(obj) = world.get_object_mut(id) {
            if let Ok(t) = obj.get_component_mut::<Transform>() {
                *t = new_t;
            }
        }
    }

    if pending_delete {
        if let Some(id) = ctx_obj_id {
            world.remove_object(id);
            if let Ok(s) = world.get_resource_mut::<CellSearchState>() {
                if s.selected_obj == Some(id) { s.selected_obj = None; }
                if s.clicked_obj == Some(id) { s.clicked_obj = None; }
                if s.renaming_obj == Some(id) { s.renaming_obj = None; }
            }
            if let Ok(s) = world.get_resource_mut::<ViewportContextMenu>() {
                s.hit_obj = None;
            }
        }
    }

    if aa_before != aa_selected {
        world.get_resource_mut::<AntiAliasing>().unwrap().amount = aa_selected;
        world.insert_resource(UpdateRenderer);
    }

    if aa_before != aa_selected {
        let ss = world.get_resource::<ViewportSize>().unwrap().supersample;
        EditorGraphics { supersample: ss, anti_aliasing: aa_selected }.save();
    }

    let prev_speed = world.get_resource::<EditorCameraSettings>().unwrap().move_speed;
    if (camera_speed - prev_speed).abs() > f32::EPSILON {
        world.get_resource_mut::<EditorCameraSettings>().unwrap().move_speed = camera_speed;
        crate::ui::preferences_panel::EditorPreferences::save_camera_speed(camera_speed);
    }

    Ok(())
}
