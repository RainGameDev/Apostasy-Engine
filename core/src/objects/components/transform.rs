use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::{Deg, Euler, Quaternion, Rotation, Vector3};

use crate::objects::{scene::ObjectId, world::World};

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
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
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
}

#[update]
pub fn transform_update(world: &mut World) -> Result<()> {
    let scene = &mut world.scene;

    for (_, object) in scene.objects.iter_mut() {
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

    let ids: Vec<ObjectId> = scene.objects.keys().collect();

    for id in ids {
        let ancestors = scene.get_ancestors(id);

        let parent_global = ancestors.iter().rev().find_map(|&ancestor_id| {
            let obj = scene.objects.get(ancestor_id)?;
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

        let Some(obj) = scene.objects.get_mut(id) else {
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

    Ok(())
}
