use apostasy_core::{
    anyhow::Result,
    egui::{self, Color32, LayerId, Rect},
    init_core,
    ecs::world::World,
    rendering::RenderingBackend,
    ui::ui_context::{EguiContext, ViewportSize, ViewportTexture},
    update,
};

fn main() {
    init_core(RenderingBackend::Vulkan, vec![]).unwrap();
}

#[update]
pub fn viewport(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let viewport_texture = world.get_resource::<ViewportTexture>().ok().map(|r| r.0);
    let viewport_size = world.get_resource_mut::<ViewportSize>().unwrap();

    let screen_rect = ctx.input(|i| i.viewport_rect());
    let pixels_per_point = ctx.pixels_per_point();
    let ss = viewport_size.supersample;

    viewport_size.logical_x = screen_rect.min.x;
    viewport_size.logical_y = screen_rect.min.y;
    viewport_size.logical_width = screen_rect.width();
    viewport_size.logical_height = screen_rect.height();
    viewport_size.pixel_width = (screen_rect.width() * pixels_per_point * ss)
        .ceil()
        .clamp(1.0, 8192.0);
    viewport_size.pixel_height = (screen_rect.height() * pixels_per_point * ss)
        .ceil()
        .clamp(1.0, 8192.0);

    if let Some(texture_id) = viewport_texture {
        let painter = ctx.layer_painter(LayerId::background());
        painter.image(
            texture_id,
            screen_rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    Ok(())
}
