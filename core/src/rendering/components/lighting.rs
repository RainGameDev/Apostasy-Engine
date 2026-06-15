use std::fmt::{self, Display};

use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::Vector3;
use egui::ComboBox;
use rand::random_range;
use serde_yaml::Value;

use crate::{
    objects::{
        component::Inspect,
        systems::DeltaTime,
        world::{self, World},
    },
    ui::LABEL_WIDTH,
};

#[derive(Clone, Debug, Copy, PartialEq, Default)]
pub enum LightType {
    Point {
        radius: f32,
    },
    #[default]
    Directional,
    Spot {
        length: f32,
        angle: f32,
    },
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
#[derive(Component, Clone, Debug)]
pub struct Light {
    /// What type of light is defined.
    pub light_type: LightType,
    /// The colour emitted by the light.
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
    pub current_intensity_target: f32,
}

impl Default for Light {
    fn default() -> Self {
        Light {
            light_type: LightType::default(),
            color: Vector3::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            is_emitting: true,
            is_flickering: false,
            intensity_min: 0.0,
            intensity_max: 0.0,
            current_intensity_target: 0.0,
        }
    }
}

impl Light {
    pub fn deserialize(&mut self, value: &Value) -> Result<()> {
        if let Some(v) = value.get("light_type") {
            match v.as_str().unwrap_or_default() {
                "Point" => {
                    let radius = value.get("radius").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    self.light_type = LightType::Point { radius };
                }
                "Directional" => self.light_type = LightType::Directional,
                "Spot" => {
                    let length =
                        value.get("length").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
                    let angle = value.get("angle").and_then(|v| v.as_f64()).unwrap_or(45.0) as f32;
                    self.light_type = LightType::Spot { length, angle };
                }
                _ => self.light_type = LightType::default(),
            }
        }

        if let Some(color) = value.get("color") {
            if let (Some(r), Some(g), Some(b)) = (
                color.get("r").and_then(|v| v.as_f64()),
                color.get("g").and_then(|v| v.as_f64()),
                color.get("b").and_then(|v| v.as_f64()),
            ) {
                self.color = Vector3::new(r as f32, g as f32, b as f32);
            }
        }

        if let Some(v) = value.get("intensity").and_then(|v| v.as_f64()) {
            self.intensity = v as f32;
        }

        if let Some(v) = value.get("is_emitting").and_then(|v| v.as_bool()) {
            self.is_emitting = v;
        }

        if let Some(v) = value.get("is_flickering").and_then(|v| v.as_bool()) {
            self.is_flickering = v;
        }
        if let Some(v) = value.get("intensity_min").and_then(|v| v.as_f64()) {
            self.intensity_min = v as f32;
        }
        if let Some(v) = value.get("intensity_max").and_then(|v| v.as_f64()) {
            self.intensity_max = v as f32;
        }

        Ok(())
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
            }
        });
    }
}

#[update]
pub fn light_flicker(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>().unwrap().0.clone();
    let lights = world.get_objects_with_component_mut::<Light>();
    for light in lights {
        let light = light.get_component_mut::<Light>()?;

        if light.is_flickering {
            if light.intensity <= light.current_intensity_target - 0.1 {
                light.current_intensity_target =
                    random_range(light.intensity_min..=light.intensity_max);
            }

            light.intensity += delta;
            println!("Flickering");
        }
    }
    Ok(())
}
