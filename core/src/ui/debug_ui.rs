use anyhow::Result;
use apostasy_macros::update;
use cgmath::Vector3;

use crate::{
    objects::{components::transform::Transform, systems::DeltaTime, tags::Player, world::World},
    rendering::shared::frustrum::ObjectsDrawing,
    ui::ui_context::EguiContext,
    voxels::{VoxelTransform, biome::BiomeRegistry, chunk::Chunk},
};

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    let transform = world
        .get_object_with_tag::<Player>()
        .unwrap()
        .get_component::<Transform>()
        .unwrap()
        .global_position;

    let chunk_transform = Vector3::new(
        transform.x as i32 / 32,
        transform.y as i32 / 32,
        transform.z as i32 / 32,
    );

    let chunks = world.get_objects_with_component::<Chunk>();
    let mut biome = "None".to_string();
    let registry = world.get_resource::<BiomeRegistry>()?;
    for chunk in chunks {
        let position = chunk.get_component::<VoxelTransform>()?.position;

        if position == chunk_transform {
            biome = registry
                .id_to_name
                .get(&chunk.get_component::<Chunk>()?.biome)
                .unwrap()
                .to_string();
        }
    }

    egui::Area::new(egui::Id::new("crosshair"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(&ctx, |ui| {
            ui.label(
                egui::RichText::new("+")
                    .size(24.0)
                    .color(egui::Color32::WHITE),
            );
        });

    egui::Window::new("Debug")
        .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
        .show(&ctx, |ui| {
            if let Ok(dt) = world.get_resource::<DeltaTime>() {
                ui.label(format!("FPS: {:.0}", 1.0 / dt.0));
            }
            ui.label(format!("Objects: {}", world.object_count()));
            ui.label(format!(
                "Objects  Drawing: {}",
                world.get_resource::<ObjectsDrawing>().unwrap().0
            ));

            ui.separator();

            ui.label(format!(
                "Chunks: {}",
                world.get_objects_with_component::<Chunk>().len()
            ));

            ui.label(format!("Biome: {}", biome));

            ui.separator();
            ui.label(format!(
                "Player position: {:?}",
                world
                    .get_object_with_tag::<Player>()
                    .unwrap()
                    .get_component::<Transform>()
                    .unwrap()
                    .global_position
            ));

            let transform = world
                .get_object_with_tag::<Player>()
                .unwrap()
                .get_component::<Transform>()
                .unwrap()
                .global_position;

            let chunk_transform = Vector3::new(
                (transform.x / 32.0).floor(),
                (transform.y / 32.0).floor(),
                (transform.z / 32.0).floor(),
            );

            ui.label(format!("Player chunk position: {:?}", chunk_transform));
        });

    Ok(())
}
