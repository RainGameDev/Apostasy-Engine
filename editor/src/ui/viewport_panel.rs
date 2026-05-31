use anyhow::Result;
use apostasy_core::{
    egui::{self, Color32, ComboBox, Id, Image, Label, Margin, RichText, Sense, Slider, Window},
    objects::world::World,
    rendering::shared::{
        UpdateRenderer,
        anti_alisaing::{AntiAliasing, AntiAliasingAmount},
    },
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
};
use apostasy_macros::Resource;

#[derive(Resource, Clone, Default)]
pub struct ViewportInfo {
    pub is_hovered: bool,
}

use crate::ui::DARK_BG;
#[update]
pub fn viewport(world: &mut World) -> Result<()> {
    if !world.has_resource::<ViewportInfo>() {
        world.insert_resource(ViewportInfo::default());
    }

    let anti_aliasing = world.get_resource_mut::<AntiAliasing>().unwrap();

    let aa_before = anti_aliasing.amount;
    let mut aa_selected = anti_aliasing.amount;

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let viewport_texture = world.get_resource::<ViewportTexture>().ok().map(|r| r.0);
    let viewport_size = world.get_resource_mut::<ViewportSize>().unwrap();

    let mut frame_rect_out = None;

    let vp = Window::new("Viewport")
        .default_size([960.0, 540.0])
        .resizable(true)
        .movable(true)
        .title_bar(true)
        .drag_area(egui::WindowDrag::TitleBar)
        .frame(
            egui::Frame::window(&ctx.global_style())
                .fill(DARK_BG)
                .inner_margin(Margin::same(0)),
        )
        .show(&ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Resolution scale");
                ui.add(Slider::new(&mut viewport_size.supersample, 1.0..=4.0).text("SSAA"));

                ComboBox::from_label("MSAA")
                    .selected_text(format!("{:?}", aa_selected))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut aa_selected, AntiAliasingAmount::X0, "None");
                        ui.selectable_value(&mut aa_selected, AntiAliasingAmount::X2, "X2");
                        ui.selectable_value(&mut aa_selected, AntiAliasingAmount::X4, "X4");
                        ui.selectable_value(&mut aa_selected, AntiAliasingAmount::X8, "X8");
                    });
            });

            ui.separator();

            let available_size = ui.available_size();
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
        let window_rect = response.response.rect;
        let current_size = window_rect.size();

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

        world.get_resource_mut::<AntiAliasing>().unwrap().amount = aa_selected;

        let viewport_info = world.get_resource_mut::<ViewportInfo>().unwrap();
        viewport_info.is_hovered = response.response.hovered();
    }

    if aa_before != aa_selected {
        world.get_resource_mut::<AntiAliasing>().unwrap().amount = aa_selected;
        world.insert_resource(UpdateRenderer);
    }

    Ok(())
}
