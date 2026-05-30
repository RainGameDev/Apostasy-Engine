use anyhow::Result;
use apostasy_core::{
    egui::{Color32, ComboBox, Image, Label, RichText, Sense, Slider, Window},
    objects::world::World,
    rendering::shared::{
        UpdateRenderer,
        anti_alisaing::{AntiAliasing, AntiAliasingAmount},
    },
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
};

#[update]
pub fn viewport(world: &mut World) -> Result<()> {
    let anti_aliasing = world.get_resource_mut::<AntiAliasing>().unwrap();

    let aa_before = anti_aliasing.amount;
    let mut aa_selected = anti_aliasing.amount;

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let viewport_texture = world.get_resource::<ViewportTexture>().ok().map(|r| r.0);
    let viewport_size = world.get_resource_mut::<ViewportSize>().unwrap();

    let vp = Window::new("Viewport")
        .default_pos([100.0, 100.0])
        .default_size([960.0, 540.0])
        .resizable(true)
        .movable(true)
        .title_bar(false)
        .show(&ctx, |ui| {
            ui.colored_label(Color32::from_rgb(100, 100, 150), "Viewport");
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
        let size = window_rect.size();

        // store logical size
        viewport_size.logical_width = size.x;
        viewport_size.logical_height = size.y;

        // compute physical pixels using egui's device pixel ratio and supersample
        let pixels_per_point = ctx.pixels_per_point();
        let ss = viewport_size.supersample;
        let mut pixel_w = (size.x * pixels_per_point * ss).ceil();
        let mut pixel_h = (size.y * pixels_per_point * ss).ceil();

        // clamp to safe maximum to avoid too-large textures (tweak or query device limits)
        const MAX_DIM: f32 = 8192.0;
        pixel_w = pixel_w.clamp(1.0, MAX_DIM);
        pixel_h = pixel_h.clamp(1.0, MAX_DIM);

        viewport_size.pixel_width = pixel_w;
        viewport_size.pixel_height = pixel_h;
        world.get_resource_mut::<AntiAliasing>().unwrap().amount = aa_selected;
    }

    if aa_before != aa_selected {
        world.get_resource_mut::<AntiAliasing>().unwrap().amount = aa_selected;
        world.insert_resource(UpdateRenderer);
    }

    Ok(())
}
