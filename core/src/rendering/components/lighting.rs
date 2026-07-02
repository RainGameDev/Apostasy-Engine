use std::fmt::{self, Display};

use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::Vector3;
use egui::ComboBox;
use rand::{RngExt, rng};

use crate::{
    ecs::{
        component::Inspect, components::serde_support::rgb, systems::DeltaTime, world::World,
    },
    ui::LABEL_WIDTH,
};

#[derive(Clone, Debug, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "light_type")]
pub enum LightType {
    Point {
        #[serde(default = "default_radius")]
        radius: f32,
    },
    #[default]
    Directional,
    Spot {
        #[serde(default = "default_length")]
        length: f32,
        #[serde(default = "default_angle")]
        angle: f32,
    },
}

fn default_radius() -> f32 {
    1.0
}
fn default_length() -> f32 {
    10.0
}
fn default_angle() -> f32 {
    45.0
}

impl Display for LightType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LightType::Point { .. } => write!(f, "Point"),
            LightType::Directional => write!(f, "Directional"),
            LightType::Spot { .. } => write!(f, "Spot"),
        }
    }
}

/// A component defining a light source.
#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(serde)]
pub struct Light {
    /// What type of light is defined.
    #[serde(flatten)]
    pub light_type: LightType,
    /// The colour emitted by the light.
    #[serde(with = "rgb", default = "default_color")]
    pub color: Vector3<f32>,
    /// How strong the light is.
    pub intensity: f32,
    /// Whether the light is currently active.
    pub is_emitting: bool,

    /// Is this light flickering?
    pub is_flickering: bool,
    /// The min amount of intensity.
    pub intensity_min: f32,
    /// The max amount of intensity.
    pub intensity_max: f32,
    #[serde(skip)]
    pub current_intensity_target: f32,

    /// The min amount of intensity.
    pub radius_min: f32,
    /// The max amount of intensity.
    pub radius_max: f32,
    #[serde(skip)]
    pub current_radius_target: f32,
}

fn default_color() -> Vector3<f32> {
    Vector3::new(1.0, 1.0, 1.0)
}

impl Default for Light {
    fn default() -> Self {
        Light {
            light_type: LightType::default(),
            color: Vector3::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            is_emitting: true,
            is_flickering: false,
            intensity_min: 1.0,
            intensity_max: 2.0,
            current_intensity_target: 0.0,
            radius_min: 1.0,
            radius_max: 21.0,
            current_radius_target: 1.0,
        }
    }
}


impl Inspect for Light {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Type"));
                ComboBox::from_id_salt("light_type")
                    .selected_text(self.light_type.to_string())
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(self.light_type, LightType::Point { .. }),
                                "Point",
                            )
                            .clicked()
                        {
                            self.light_type = LightType::Point { radius: 1.0 };
                        }
                        if ui
                            .selectable_label(
                                self.light_type == LightType::Directional,
                                "Directional",
                            )
                            .clicked()
                        {
                            self.light_type = LightType::Directional;
                        }
                        if ui
                            .selectable_label(
                                matches!(self.light_type, LightType::Spot { .. }),
                                "Spot",
                            )
                            .clicked()
                        {
                            self.light_type = LightType::Spot {
                                length: 10.0,
                                angle: 45.0,
                            };
                        }
                    });
            });

            match &mut self.light_type {
                LightType::Point { radius } => {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Radius"));
                        ui.add(
                            egui::DragValue::new(radius)
                                .speed(0.1)
                                .range(0.0..=f32::MAX),
                        );
                    });
                }
                LightType::Spot { length, angle } => {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Length"));
                        ui.add(
                            egui::DragValue::new(length)
                                .speed(0.1)
                                .range(0.0..=f32::MAX),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Angle"));
                        ui.add(
                            egui::DragValue::new(angle)
                                .speed(0.5)
                                .range(0.0..=180.0)
                                .suffix("°"),
                        );
                    });
                }
                LightType::Directional => {}
            }

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Color"));
                let mut rgb = [self.color.x, self.color.y, self.color.z];
                if ui.color_edit_button_rgb(&mut rgb).changed() {
                    self.color = Vector3::new(rgb[0], rgb[1], rgb[2]);
                }
            });

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Intensity"));
                ui.add(
                    egui::DragValue::new(&mut self.intensity)
                        .speed(0.05)
                        .range(0.0..=f32::MAX),
                );
            });

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Emitting"));
                ui.checkbox(&mut self.is_emitting, "");
            });

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Flickering"));
                ui.checkbox(&mut self.is_flickering, "");
            });

            if self.is_flickering {
                ui.horizontal(|ui| {
                    ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Min Intensity"));
                    ui.add(
                        egui::DragValue::new(&mut self.intensity_min)
                            .speed(0.05)
                            .range(0.0..=f32::MAX),
                    );
                });

                ui.horizontal(|ui| {
                    ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Max Intensity"));
                    ui.add(
                        egui::DragValue::new(&mut self.intensity_max)
                            .speed(0.05)
                            .range(0.0..=f32::MAX),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Min Radius"));
                    ui.add(
                        egui::DragValue::new(&mut self.radius_min)
                            .speed(0.05)
                            .range(0.0..=f32::MAX),
                    );
                });

                ui.horizontal(|ui| {
                    ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Max Radius"));
                    ui.add(
                        egui::DragValue::new(&mut self.radius_max)
                            .speed(0.05)
                            .range(0.0..=f32::MAX),
                    );
                });

                ui.horizontal(|ui| {
                    ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Target"));
                    ui.add(
                        egui::DragValue::new(&mut self.current_intensity_target)
                            .speed(0.05)
                            .range(0.0..=f32::MAX),
                    );
                });
            }
        });
    }
}

#[update(mode = "all")]
pub fn light_flicker(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;
    let light_ids = world.get_entities_with_component::<Light>();

    for id in light_ids {
        let light = match world.get_component_mut::<Light>(id) {
            Some(l) => l,
            None => continue,
        };

        if light.is_flickering {
            if (light.intensity - light.current_intensity_target).abs() < 0.1 {
                light.current_intensity_target =
                    rng().random_range(light.intensity_min..light.intensity_max);
            }

            light.intensity = lerp(light.intensity, light.current_intensity_target, delta * 3.0);
            light.intensity = light
                .intensity
                .clamp(light.intensity_min, light.intensity_max);

            match &mut light.light_type {
                LightType::Point { radius } => {
                    if (*radius - light.current_radius_target).abs() < 0.1 {
                        light.current_radius_target =
                            rng().random_range(light.radius_min..light.radius_max);
                    }

                    *radius = lerp(*radius, light.current_radius_target, delta * 3.0);
                }

                _ => {}
            }
        }
    }
    Ok(())
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    let t_clamped = t.clamp(0.0, 1.0);
    start + (end - start) * t_clamped
}
