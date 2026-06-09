use anyhow::Result;
use apostasy_core::{
    egui::{self, Color32, ComboBox, Image, Label, RichText, Sense, Slider, Window},
    objects::world::World,
    rendering::shared::{
        UpdateRenderer,
        anti_alisaing::{AntiAliasing, AntiAliasingAmount},
    },
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
};
use apostasy_macros::Resource;

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
use crate::ui::shared::{WindowLayout, save_layout};

#[update(mode = "editor")]
pub fn viewport(world: &mut World) -> Result<()> {
    if !world.has_resource::<ViewportInfo>() {
        world.insert_resource(ViewportInfo::default());
    }

    if !world.has_resource::<WindowLayout>() {
        return Ok(());
    }

    if !world
        .get_resource::<ViewportInfo>()
        .map(|v| v.open)
        .unwrap_or(true)
    {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let style = world
        .get_resource::<EditorStyle>()
        .cloned()
        .unwrap_or_default();
    let layout = world.get_resource::<WindowLayout>().ok();
    let state = if let Some(layout) = layout {
        layout.viewport.clone()
    } else {
        return Ok(());
    };

    let pos = state.to_pos();
    let size = state.to_size();

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

    let available_options = world
        .get_resource::<AntiAliasing>()
        .unwrap()
        .available_options
        .clone();
    let anti_aliasing = world.get_resource_mut::<AntiAliasing>().unwrap();
    let aa_before = anti_aliasing.amount;
    let mut aa_selected = anti_aliasing.amount;

    let mut frame_rect_out = None;
    let viewport_texture = world.get_resource::<ViewportTexture>().ok().map(|r| r.0);
    let viewport_size = world.get_resource_mut::<ViewportSize>().unwrap();

    let vp = window
        .resizable(true)
        .movable(true)
        .title_bar(true)
        .drag_area(egui::WindowDrag::TitleBar)
        .frame(style.window_frame(&ctx))
        .show(&ctx, |ui| {
            let bar_h = style.header_height();
            let (bar_rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), bar_h),
                egui::Sense::hover(),
            );
            let mut bar = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(bar_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            bar.add_space(6.0);
            bar.label("Resolution scale");
            bar.add(Slider::new(&mut viewport_size.supersample, 1.0..=4.0).text("SSAA"));
            ComboBox::from_label("MSAA")
                .selected_text(format!("{:?}", aa_selected))
                .show_ui(&mut bar, |ui| {
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

            let available_size = ui.available_size();
            ui.separator();
            if available_size.x <= 0.0 || available_size.y <= 0.0 {
                return;
            }

            let (frame_rect, _) = ui.allocate_exact_size(available_size, Sense::hover());
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
        });

    if let Some(response) = vp {
        let rect = response.response.rect;
        let current_size = rect.size();

        viewport_size.logical_width = current_size.x;
        viewport_size.logical_height = current_size.y;

        let pixels_per_point = ctx.pixels_per_point();
        let ss = viewport_size.supersample;
        let mut pixel_w = (current_size.x * pixels_per_point * ss).ceil();
        let mut pixel_h = (current_size.y * pixels_per_point * ss).ceil();

        const MAX_DIM: f32 = 8192.0;
        pixel_w = pixel_w.clamp(1.0, MAX_DIM);
        pixel_h = pixel_h.clamp(1.0, MAX_DIM);

        viewport_size.pixel_width = pixel_w;
        viewport_size.pixel_height = pixel_h;

        if let Some(frame_rect) = frame_rect_out {
            viewport_size.logical_x = frame_rect.min.x;
            viewport_size.logical_y = frame_rect.min.y;
            viewport_size.logical_width = frame_rect.width();
            viewport_size.logical_height = frame_rect.height();
        }

        let viewport_info = world.get_resource_mut::<ViewportInfo>()?;
        viewport_info.is_hovered = response.response.hovered();

        let layout = world.get_resource_mut::<WindowLayout>()?;
        layout.viewport.update_from_rect(rect);
        save_layout(layout);
    }

    if aa_before != aa_selected {
        world.get_resource_mut::<AntiAliasing>().unwrap().amount = aa_selected;
        world.insert_resource(UpdateRenderer);
    }

    Ok(())
}
