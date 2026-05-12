use apostasy_macros::{Resource, update};

use apostasy_core::{
    anyhow::Result,
    cgmath::{InnerSpace, Vector3},
    egui,
    objects::{components::transform::Transform, systems::DeltaTime, tags::Player, world::World},
    physics::velocity::Velocity,
    rendering::shared::frustrum::ObjectsDrawing,
    start,
    ui::ui_context::EguiContext,
    voxels::{VoxelTransform, biome::BiomeRegistry, chunk::Chunk},
};

use crate::{states::HasInitGeneration, world::chunk_loader::ChunkLoader};

#[derive(Resource, Debug, Clone)]
pub struct FpsTracker {
    pub samples: Vec<f32>,
    pub max_samples: usize,
}

impl Default for FpsTracker {
    fn default() -> Self {
        Self {
            samples: Vec::new(),
            max_samples: 1000,
        }
    }
}

#[start]

pub fn hud_start(world: &mut World) -> Result<()> {
    world.insert_resource(FpsTracker::default());
    Ok(())
}

#[update]
pub fn hud(world: &mut World) -> Result<()> {
    if !world.get_resource::<HasInitGeneration>().is_ok() {
        return Ok(());
    }

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
    let seed = world.get_resource::<ChunkLoader>()?.seed;

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
    let dt = world.get_resource::<DeltaTime>()?;
    let fps = 1.0 / dt.0;

    if fps < 1.0 || fps > 5000.0 || !fps.is_finite() {
        return Ok(());
    } else {
        let tracker = world.get_resource_mut::<FpsTracker>()?;
        tracker.samples.push(fps);
        if tracker.samples.len() > tracker.max_samples {
            tracker.samples.remove(0);
        }

        let samples = tracker.samples.clone();
        let avg = samples.iter().sum::<f32>() / samples.len() as f32;

        let mut sorted = samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let one_percent_idx = (sorted.len() as f32 * 0.01).ceil() as usize;
        let point_one_idx = (sorted.len() as f32 * 0.001).ceil() as usize;

        let one_percent_low =
            sorted[..one_percent_idx.max(1)].iter().sum::<f32>() / one_percent_idx.max(1) as f32;
        let point_one_low =
            sorted[..point_one_idx.max(1)].iter().sum::<f32>() / point_one_idx.max(1) as f32;

        egui::Window::new("Debug")
            .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
            .show(&ctx, |ui| {
                ui.separator();

                ui.label(format!("FPS: {:.0}", fps));
                ui.label(format!("Avg FPS: {:.0}", avg));
                ui.label(format!("1% low: {:.0}", one_percent_low));
                ui.label(format!("0.1% low: {:.0}", point_one_low));

                ui.separator();

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

                ui.label(format!("Seed : {}", seed));

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

                let velocity = world
                    .get_object_with_tag::<Player>()
                    .unwrap()
                    .get_component::<Velocity>()
                    .unwrap()
                    .linear_velocity;

                ui.label(format!(
                    "Player velocity: {:.2} voxels/s",
                    velocity.magnitude()
                ));

                let grounded = world
                    .get_object_with_tag::<Player>()
                    .unwrap()
                    .get_component::<Velocity>()
                    .unwrap()
                    .is_grounded;
                ui.label(format!("Player grounded: {} ", grounded));

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
}
