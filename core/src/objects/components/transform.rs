use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::{Deg, Euler, InnerSpace, Matrix3, Quaternion, Rotation, Rotation3, Vector3};

use crate::{
    objects::{cell::ObjectId, component::Inspect, world::World},
    ui::{DRAG_SIZE, LABEL_WIDTH},
};

pub const UP: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);
pub const RIGHT: Vector3<f32> = Vector3::new(1.0, 0.0, 0.0);
pub const FORWARD: Vector3<f32> = Vector3::new(0.0, 0.0, -1.0);

#[derive(Component, Clone, Debug)]
pub struct Transform {
    pub local_position: Vector3<f32>,
    pub local_euler_angles: Vector3<f32>,
    pub local_rotation: Quaternion<f32>,
    pub local_scale: Vector3<f32>,
    pub global_position: Vector3<f32>,
    pub global_rotation: Quaternion<f32>,
    pub global_euler_angles: Vector3<f32>,
    pub global_scale: Vector3<f32>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            local_position: Vector3::new(0.0, 0.0, 0.0),
            local_euler_angles: Vector3::new(0.0, 0.0, 0.0),
            local_rotation: Quaternion::new(1.0, 0.0, 0.0, 0.0),
            local_scale: Vector3::new(1.0, 1.0, 1.0),
            global_position: Vector3::new(0.0, 0.0, 0.0),
            global_rotation: Quaternion::new(1.0, 0.0, 0.0, 0.0),
            global_euler_angles: Vector3::new(0.0, 0.0, 0.0),
            global_scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Transform {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        if let Some(seq) = value.get("local_position").and_then(|v| v.as_sequence()) {
            if seq.len() >= 3 {
                self.local_position = Vector3::new(
                    seq[0].as_f64().unwrap_or(0.0) as f32,
                    seq[1].as_f64().unwrap_or(0.0) as f32,
                    seq[2].as_f64().unwrap_or(0.0) as f32,
                );
            }
        }
        if let Some(seq) = value.get("local_euler_angles").and_then(|v| v.as_sequence()) {
            if seq.len() >= 3 {
                self.local_euler_angles = Vector3::new(
                    seq[0].as_f64().unwrap_or(0.0) as f32,
                    seq[1].as_f64().unwrap_or(0.0) as f32,
                    seq[2].as_f64().unwrap_or(0.0) as f32,
                );
            }
        }
        if let Some(seq) = value.get("local_scale").and_then(|v| v.as_sequence()) {
            if seq.len() >= 3 {
                self.local_scale = Vector3::new(
                    seq[0].as_f64().unwrap_or(1.0) as f32,
                    seq[1].as_f64().unwrap_or(1.0) as f32,
                    seq[2].as_f64().unwrap_or(1.0) as f32,
                );
            }
        }
        Ok(())
    }
    pub fn calculate_up(&self) -> Vector3<f32> {
        self.local_rotation.rotate_vector(UP)
    }

    pub fn calculate_forward(&self) -> Vector3<f32> {
        self.local_rotation.rotate_vector(FORWARD)
    }

    pub fn calculate_right(&self) -> Vector3<f32> {
        self.local_rotation.rotate_vector(RIGHT)
    }

    pub fn calculate_global_forward(&self) -> Vector3<f32> {
        self.global_rotation.rotate_vector(FORWARD)
    }

    pub fn calculate_global_up(&self) -> Vector3<f32> {
        self.global_rotation.rotate_vector(UP)
    }
    pub fn calculate_global_right(&self) -> Vector3<f32> {
        self.global_rotation.rotate_vector(RIGHT)
    }

    /// Orbits the object around `pivot` by `angle_deg` degrees along `axis` in world space
    /// Updates both position and orientation, roll is preserved (usually zero)
    pub fn rotate_around(&mut self, pivot: Vector3<f32>, axis: Vector3<f32>, angle_deg: f32) {
        let q = Quaternion::from_axis_angle(axis.normalize(), Deg(angle_deg));

        let offset = self.local_position - pivot;
        self.local_position = pivot + q.rotate_vector(offset);

        let new_q = (q * self.local_rotation).normalize();

        // Extract YXZ Euler angles matching the Ry*Rx*Rz composition in transform_update
        // Matrix is column-major: m[col][row]. Derived from expanding Ry Rx Rz
        //   m[2][1] = sin(x),  m[2][0] = -cx sy,  m[2][2] = cx cy
        //   m[0][1] = cx sz,   m[1][1] = cx cz
        let m = Matrix3::from(new_q);
        let sin_x = m[2][1].clamp(-1.0, 1.0);
        let x = sin_x.asin();
        let cos_x = x.cos();
        let (y, z) = if cos_x.abs() > 1e-6 {
            ((-m[2][0]).atan2(m[2][2]), m[0][1].atan2(m[1][1]))
        } else if sin_x > 0.0 {
            // Gimbal lock x ~= +90(degrees) y-z collapses, absorb into y
            (m[1][0].atan2(m[0][0]), 0.0)
        } else {
            // Gimbal lock x ~= -90(degrees) y+z collapses, absorb into y
            ((-m[1][0]).atan2(m[0][0]), 0.0)
        };
        self.local_rotation = new_q;
        self.local_euler_angles = Vector3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());
    }

    /// Rotates the position so it's forward axis points towards the target in worldspace
    /// Roll is always zeroed
    pub fn look_at(&mut self, target: Vector3<f32>) {
        let delta = target - self.global_position;
        let len = delta.magnitude();
        if len < f32::EPSILON {
            return;
        }
        let dir = delta / len;

        let pitch = dir.y.asin().to_degrees();
        let yaw = (-dir.x).atan2(-dir.z).to_degrees();
        self.local_euler_angles = Vector3::new(pitch, yaw, 0.0);
    }
}

impl Inspect for Transform {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Position"));
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_position.x).speed(0.1),
                );
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_position.y).speed(0.1),
                );
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_position.z).speed(0.1),
                );
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Rotation"));
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_euler_angles.x).speed(0.1),
                );
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_euler_angles.y).speed(0.1),
                );
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_euler_angles.z).speed(0.1),
                );
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Scale"));
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_scale.x).speed(0.1),
                );
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_scale.y).speed(0.1),
                );
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.local_scale.z).speed(0.1),
                );
            });
            ui.separator();
        });
    }
}

#[update(mode = "all", priority = 1)]
pub fn transform_update(world: &mut World) -> Result<()> {
    // Object hierarchies never span cells, so each cell can be solved
    // independently using only its own objects.
    for cell in world.worldspace.cells.values_mut() {
    let coord = cell.coord;

    for (_, object) in cell.objects.iter_mut() {
        let Some(transform) = object
            .components
            .iter_mut()
            .find_map(|c| c.as_any_mut().downcast_mut::<Transform>())
        else {
            continue;
        };

        transform.local_rotation = Quaternion::from(Euler {
            x: Deg(0.0),
            y: Deg(transform.local_euler_angles.y),
            z: Deg(0.0),
        }) * Quaternion::from(Euler {
            x: Deg(transform.local_euler_angles.x),
            y: Deg(0.0),
            z: Deg(0.0),
        }) * Quaternion::from(Euler {
            x: Deg(0.0),
            y: Deg(0.0),
            z: Deg(transform.local_euler_angles.z),
        });
        transform.global_rotation = transform.local_rotation;
        transform.global_position = transform.local_position;
        transform.global_scale = transform.local_scale;
        transform.global_euler_angles = transform.local_euler_angles;
    }

    let ids: Vec<ObjectId> = cell
        .objects
        .keys()
        .map(|key| ObjectId { cell: coord, key })
        .collect();

    for id in ids {
        let ancestors = cell.get_ancestors(id);

        let parent_global = ancestors.iter().rev().find_map(|&ancestor_id| {
            let obj = cell.objects.get(ancestor_id.key)?;
            let t = obj
                .components
                .iter()
                .find_map(|c| c.as_any().downcast_ref::<Transform>())?;
            Some((
                t.global_position,
                t.global_rotation,
                t.global_scale,
                t.global_euler_angles,
            ))
        });

        let Some((parent_pos, parent_rot, parent_scale, parent_euler)) = parent_global else {
            continue;
        };

        let Some(obj) = cell.objects.get_mut(id.key) else {
            continue;
        };

        let Some(transform) = obj
            .components
            .iter_mut()
            .find_map(|c| c.as_any_mut().downcast_mut::<Transform>())
        else {
            continue;
        };

        transform.global_position = parent_pos + parent_rot.rotate_vector(transform.local_position);

        transform.global_euler_angles = parent_euler + transform.local_euler_angles;
        transform.global_rotation = Quaternion::from(Euler {
            x: Deg(0.0),
            y: Deg(transform.global_euler_angles.y),
            z: Deg(0.0),
        }) * Quaternion::from(Euler {
            x: Deg(transform.global_euler_angles.x),
            y: Deg(0.0),
            z: Deg(0.0),
        }) * Quaternion::from(Euler {
            x: Deg(0.0),
            y: Deg(0.0),
            z: Deg(transform.global_euler_angles.z),
        });

        transform.global_scale = Vector3::new(
            parent_scale.x * transform.local_scale.x,
            parent_scale.y * transform.local_scale.y,
            parent_scale.z * transform.local_scale.z,
        );
    }
    }

    Ok(())
}
