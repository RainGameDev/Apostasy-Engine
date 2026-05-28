use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::{InnerSpace, Vector3, Zero};

use crate::{
    log,
    objects::{components::transform::Transform, systems::DeltaTime, tags::Player, world::World},
};

#[derive(Component, Clone, Debug)]
pub struct Velocity {
    pub angular_velocity: Vector3<f32>,
    pub linear_velocity: Vector3<f32>,
    pub mass: f32,
    pub is_grounded: bool,
    pub process: bool,
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            angular_velocity: Vector3::zero(),
            linear_velocity: Vector3::zero(),
            mass: 1.0,
            is_grounded: false,
            process: true,
        }
    }
}

impl Velocity {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }

    /// Recomputes angular_velocity from the tangential component of linear_velocity
    /// given a contact normal and sphere radius.
    pub fn sync_angular_from_linear(&mut self, radius: f32, normal: Vector3<f32>) {
        let v_tangential = self.linear_velocity - normal * self.linear_velocity.dot(normal);
        self.angular_velocity = v_tangential.cross(normal) * (1.0 / radius);
    }

    /// Recomputes the tangential part of linear_velocity from angular_velocity,
    /// preserving any velocity along the normal.
    pub fn sync_linear_from_angular(&mut self, radius: f32, normal: Vector3<f32>) {
        let v_normal = normal * self.linear_velocity.dot(normal);
        let v_tangential = self.angular_velocity.cross(normal) * radius;
        self.linear_velocity = v_normal + v_tangential;
    }
}

#[update]
fn velocity_process(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    for object in world.get_objects_with_component_mut::<Velocity>() {
        // Read-only checks first to avoid overlapping borrows
        let process = object.get_component::<Velocity>()?.process;
        if !process {
            continue;
        }

        let linear = object.get_component::<Velocity>()?.linear_velocity;
        let transform = object.get_component_mut::<Transform>()?;
        transform.local_position += linear * delta;
    }

    Ok(())
}

// #[fixed_update]
pub fn physics_debug(world: &mut World, _: f32) -> Result<()> {
    let player = world.get_object_with_tag::<Player>()?;
    let transform = player.get_component::<Transform>()?;
    let velocity = player.get_component::<Velocity>()?;

    log!(
        "local={:.2},{:.2},{:.2} global={:.2},{:.2},{:.2} vel={:.2},{:.2},{:.2} grounded={}",
        transform.local_position.x,
        transform.local_position.y,
        transform.local_position.z,
        transform.global_position.x,
        transform.global_position.y,
        transform.global_position.z,
        velocity.linear_velocity.x,
        velocity.linear_velocity.y,
        velocity.linear_velocity.z,
        velocity.is_grounded
    );

    Ok(())
}
