use apostasy_macros::{Component, Tag};
use cgmath::{Deg, Matrix4, PerspectiveFov, Point3};

use crate::objects::components::transform::Transform;

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
