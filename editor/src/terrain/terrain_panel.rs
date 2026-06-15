use anyhow::Result;
use apostasy_core::{
    egui::{DragValue, RichText, Slider, Window},
    objects::world::World,
    terrain::TerrainSettings,
    update,
};

use crate::terrain::{TerrainTool, TerrainToolState};

#[update(mode = "editor")]
pub fn terrain_panel(world: &mut World) -> Result<()> {
    if !world.has_resource::<TerrainToolState>() {
        world.insert_resource(TerrainToolState::default());
    }

    let active = world
        .get_resource::<TerrainToolState>()
        .map(|s| s.active)
        .unwrap_or(false);
    if !active {
        return Ok(());
    }

    let ctx = world
        .get_resource::<apostasy_core::ui::ui_context::EguiContext>()?
        .0
        .clone();

    let resolution = world
        .get_resource::<TerrainSettings>()
        .map(|s| s.resolution)
        .unwrap_or(128);

    let mut state = world.get_resource::<TerrainToolState>()?.clone();
    let mut new_resolution = resolution;
    let mut resolution_changed = false;

    Window::new("Terrain")
        .resizable(false)
        .collapsible(false)
        .default_width(200.0)
        .show(&ctx, |ui| {
            ui.label(RichText::new("Tool").strong());
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tool, TerrainTool::Raise, "Raise");
                ui.selectable_value(&mut state.tool, TerrainTool::Lower, "Lower");
                ui.selectable_value(&mut state.tool, TerrainTool::Smooth, "Smooth");
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.tool, TerrainTool::Flatten, "Flatten");
                ui.selectable_value(&mut state.tool, TerrainTool::Paint, "Paint");
            });

            ui.separator();

            ui.label(RichText::new("Brush").strong());
            ui.horizontal(|ui| {
                ui.label("Radius");
                ui.add(Slider::new(&mut state.brush_radius, 1.0..=64.0).suffix("u"));
            });
            ui.horizontal(|ui| {
                ui.label("Strength");
                ui.add(Slider::new(&mut state.brush_strength, 0.01..=1.0));
            });

            if state.tool == TerrainTool::Flatten {
                if let Some(h) = state.flatten_height {
                    ui.label(format!("Target height: {:.2}", h));
                } else {
                    ui.label("Click to sample height");
                }
            }

            if state.tool == TerrainTool::Paint {
                ui.separator();
                ui.label(RichText::new("Layer").strong());
                ui.horizontal(|ui| {
                    for i in 0..4usize {
                        ui.selectable_value(&mut state.paint_layer, i, format!("{}", i));
                    }
                });
            }

            ui.separator();
            ui.label(RichText::new("Settings").strong());
            ui.horizontal(|ui| {
                ui.label("Resolution");
                if ui.add(DragValue::new(&mut new_resolution).range(4_u32..=512)).changed() {
                    resolution_changed = true;
                }
            });
        });

    if let Ok(s) = world.get_resource_mut::<TerrainToolState>() {
        *s = state;
    }

    if resolution_changed {
        if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
            settings.resolution = new_resolution.max(4);
        }
    }

    Ok(())
}
