use apostasy_macros::{Component, Inspect, Tag};
use cgmath::{Deg, Matrix4, PerspectiveFov, Point3};
use egui::Stroke;

use crate::{
    objects::{component::Inspect, components::transform::Transform},
    ui::{DIV_COL, DRAG_SIZE, LABEL_WIDTH, PANEL_BG},
};

#[derive(Component, Clone, Debug)]
pub struct Camera {
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
    pub is_main: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            fov_y: 90.0,
            near: 0.001,
            far: 10000.0,
            is_main: false,
        }
    }
}

impl Inspect for Camera {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(PANEL_BG)
            .stroke(Stroke::new(1.0, DIV_COL))
            .corner_radius(4.0)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.label("Camera");
                ui.separator();
                ui.indent("transform_indent", |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("FOV"));
                            ui.add_sized(
                                DRAG_SIZE,
                                egui::DragValue::new(&mut self.fov_y)
                                    .speed(0.1)
                                    .range(1.0..=170.0),
                            );
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Near"));
                            ui.add_sized(
                                DRAG_SIZE,
                                egui::DragValue::new(&mut self.near).speed(0.1),
                            );
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Far"));
                            ui.add_sized(DRAG_SIZE, egui::DragValue::new(&mut self.far).speed(0.1));
                        });
                        ui.separator();
                    });
                });
            });
    }
}

impl Camera {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn get_perspective_projection(camera: &Camera, aspect: f32) -> Matrix4<f32> {
    let mut proj: Matrix4<f32> = PerspectiveFov::to_perspective(&PerspectiveFov {
        fovy: Deg(camera.fov_y).into(),
        aspect,
        near: camera.near,
        far: camera.far,
    })
    .into();

    proj[1][1] *= -1.0;

    proj
}

pub fn get_view_matrix(transform: &Transform) -> Matrix4<f32> {
    let eye = Point3::new(
        transform.global_position.x,
        transform.global_position.y,
        transform.global_position.z,
    );

    let forward = transform.calculate_global_forward();

    let look = Point3::new(
        transform.global_position.x + forward.x,
        transform.global_position.y + forward.y,
        transform.global_position.z + forward.z,
    );

    let up = transform.calculate_global_up();

    Matrix4::look_at_rh(eye, look, up)
}

#[derive(Tag, Clone)]
pub struct GameCamera;

#[derive(Tag, Clone)]
pub struct EditorCamera;

#[derive(Tag, Clone)]
pub struct ActiveCamera;
