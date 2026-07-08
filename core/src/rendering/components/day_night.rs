use anyhow::Result;
use apostasy_macros::{Component, update};

use crate::{
    ecs::components::transform::Transform,
    ecs::{component::Inspect, systems::DeltaTime, world::World},
    rendering::components::lighting::{Light, LightType},
    rendering::components::skybox::Skybox,
    ui::{DRAG_SIZE, LABEL_WIDTH},
};

#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(serde)]
#[serde(default)]
pub struct DayNightCycle {
    /// Time of day in hours, 0–24. 6 = sunrise, 12 = noon, 18 = sunset.
    pub time_of_day: f32,
    /// Real seconds for a full 24 hour cycle.
    pub day_length: f32,
    /// Advance `time_of_day` automatically each frame.
    pub advance: bool,
    /// Directional light intensity at full day.
    pub day_intensity: f32,
    /// Directional light intensity at full night.
    pub night_intensity: f32,
}

impl Default for DayNightCycle {
    fn default() -> Self {
        Self {
            time_of_day: 12.0,
            day_length: 600.0,
            advance: true,
            day_intensity: 1.0,
            night_intensity: 0.05,
        }
    }
}

impl Inspect for DayNightCycle {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        let row_h = 20.0;
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Time"));
            ui.add(egui::Slider::new(&mut self.time_of_day, 0.0..=24.0));
        });
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Day length"));
            ui.add_sized(
                DRAG_SIZE,
                egui::DragValue::new(&mut self.day_length)
                    .speed(1.0)
                    .range(1.0..=86400.0)
                    .suffix(" s"),
            );
        });
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Advance"));
            ui.checkbox(&mut self.advance, "");
        });
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Day intensity"));
            ui.add_sized(
                DRAG_SIZE,
                egui::DragValue::new(&mut self.day_intensity).speed(0.01),
            );
        });
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Night intensity"));
            ui.add_sized(
                DRAG_SIZE,
                egui::DragValue::new(&mut self.night_intensity).speed(0.01),
            );
        });
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[update(mode = "all", priority = 2)]
pub fn day_night_update(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    for id in world.get_entities_with_component::<DayNightCycle>() {
        let Some(cycle) = world.get_component_mut::<DayNightCycle>(id) else {
            continue;
        };
        if cycle.advance && cycle.day_length > 0.0 {
            cycle.time_of_day =
                (cycle.time_of_day + delta * 24.0 / cycle.day_length).rem_euclid(24.0);
        }
        let time_of_day = cycle.time_of_day;
        let day_intensity = cycle.day_intensity;
        let night_intensity = cycle.night_intensity;

        let pitch = -(time_of_day - 6.0) / 24.0 * 360.0;
        let sun_elevation = -pitch.to_radians().sin();

        let blend = 1.0 - smoothstep(-0.15, 0.1, sun_elevation);

        if let Some(transform) = world.get_component_mut::<Transform>(id) {
            transform.local_euler_angles.x = pitch;
        }
        if let Some(skybox) = world.get_component_mut::<Skybox>(id) {
            skybox.blend = blend;
        }
        if let Some(light) = world.get_component_mut::<Light>(id)
            && matches!(light.light_type, LightType::Directional)
        {
            light.intensity = day_intensity + (night_intensity - day_intensity) * blend;
        }
    }

    Ok(())
}
