use apostasy_macros::{Component, Tag};
use cgmath::{Deg, Matrix4, PerspectiveFov, Point3};

use crate::{
    objects::{component::Inspect, components::transform::Transform},
    ui::{DRAG_SIZE, LABEL_WIDTH},
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
                ui.add_sized(DRAG_SIZE, egui::DragValue::new(&mut self.near).speed(0.1));
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Far"));
                ui.add_sized(DRAG_SIZE, egui::DragValue::new(&mut self.far).speed(0.1));
            });
            ui.separator();
        });
    }
}

impl Camera {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        if let Some(v) = value.get("fov_y").and_then(|v| v.as_f64()) {
            self.fov_y = v as f32;
        }
        if let Some(v) = value.get("near").and_then(|v| v.as_f64()) {
            self.near = v as f32;
        }
        if let Some(v) = value.get("far").and_then(|v| v.as_f64()) {
            self.far = v as f32;
        }
        if let Some(v) = value.get("is_main").and_then(|v| v.as_bool()) {
            self.is_main = v;
        }
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

inventory::submit!(crate::objects::tag::TagRegistration {
    type_name: "GameCamera",
    create: || Box::new(GameCamera),
});

#[derive(Tag, Clone)]
pub struct EditorCamera;

inventory::submit!(crate::objects::tag::TagRegistration {
    type_name: "EditorCamera",
    create: || Box::new(EditorCamera),
});

#[derive(Tag, Clone)]
pub struct ActiveCamera;

inventory::submit!(crate::objects::tag::TagRegistration {
    type_name: "ActiveCamera",
    create: || Box::new(ActiveCamera),
});
