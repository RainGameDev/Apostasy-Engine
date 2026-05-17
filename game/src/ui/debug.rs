use std::collections::VecDeque;

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
    pub samples: VecDeque<f32>,
    pub max_samples: usize,
}

impl Default for FpsTracker {
    fn default() -> Self {
        Self {
            samples: VecDeque::new(),
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
    if world.get_resource::<HasInitGeneration>().is_err() {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();

    // Single player lookup
    let player_obj = world.get_object_with_tag::<Player>().unwrap();
    let transform = player_obj
        .get_component::<Transform>()
        .unwrap()
        .global_position;
    let velocity = player_obj.get_component::<Velocity>().unwrap();
    let linear_velocity = velocity.linear_velocity;
    let is_grounded = velocity.is_grounded;

    let chunk_pos = Vector3::new(
        (transform.x as i32) / 32,
        (transform.y as i32) / 32,
        (transform.z as i32) / 32,
    );
    let chunk_pos_display = Vector3::new(
        (transform.x / 32.0).floor(),
        (transform.y / 32.0).floor(),
        (transform.z / 32.0).floor(),
    );

    let registry = world.get_resource::<BiomeRegistry>()?;
    let seed = world.get_resource::<ChunkLoader>()?.seed;

    let chunks = world.get_objects_with_component::<Chunk>();
    let chunk_count = chunks.len();
    let biome = chunks
        .iter()
        .find(|c| {
            c.get_component::<VoxelTransform>()
                .map_or(false, |t| t.position == chunk_pos)
        })
        .and_then(|c| c.get_component::<Chunk>().ok())
        .and_then(|c| registry.id_to_name.get(&c.biome))
        .map(|n| n.to_string())
        .unwrap_or_else(|| "None".to_string());

    let dt = world.get_resource::<DeltaTime>()?;
    let fps = 1.0 / dt.0;
    if !(1.0..=5000.0).contains(&fps) || !fps.is_finite() {
        return Ok(());
    }

    let tracker = world.get_resource_mut::<FpsTracker>()?;
    if tracker.samples.len() == tracker.max_samples {
        tracker.samples.pop_front();
    }
    tracker.samples.push_back(fps);

    let sample_count = tracker.samples.len();
    let avg = tracker.samples.iter().sum::<f32>() / sample_count as f32;

    let mut sorted: Vec<f32> = tracker.samples.iter().copied().collect();
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

    let one_pct_idx = (sample_count as f32 * 0.01).ceil() as usize;
    let point1_idx = (sample_count as f32 * 0.001).ceil() as usize;
    let one_pct_low = sorted[..one_pct_idx.max(1)].iter().sum::<f32>() / one_pct_idx.max(1) as f32;
    let point1_low = sorted[..point1_idx.max(1)].iter().sum::<f32>() / point1_idx.max(1) as f32;

    let object_count = world.object_count();
    let objects_drawing = world.get_resource::<ObjectsDrawing>()?.0;

    egui::Window::new("Debug")
        .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
        .show(&ctx, |ui| {
            ui.separator();
            ui.label(format!("FPS: {:.0}", fps));
            ui.label(format!("Avg FPS: {:.0}", avg));
            ui.label(format!("1% low: {:.0}", one_pct_low));
            ui.label(format!("0.1% low: {:.0}", point1_low));
            ui.separator();
            ui.label(format!("Objects: {}", object_count));
            ui.label(format!("Objects Drawing: {}", objects_drawing));
            ui.separator();
            ui.label(format!("Chunks: {}", chunk_count));
            ui.label(format!("Biome: {}", biome));
            ui.label(format!("Seed: {}", seed));
            ui.separator();
            ui.label(format!("Player position: {:?}", transform));
            ui.label(format!(
                "Player velocity: {:.2} voxels/s",
                linear_velocity.magnitude()
            ));
            ui.label(format!("Player grounded: {}", is_grounded));
            ui.label(format!("Player chunk position: {:?}", chunk_pos_display));
        });

    Ok(())
}
