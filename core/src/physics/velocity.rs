use anyhow::Result;
use apostasy_macros::{Component, fixed_update, update};
use cgmath::{Vector3, Zero};

use crate::{
    log,
    objects::{components::transform::Transform, systems::DeltaTime, tags::Player, world::World},
    physics::collider::Collider,
};

#[derive(Component, Clone, Debug)]
pub struct Velocity {
    pub angular_velocity: Vector3<f32>,
    pub linear_velocity: Vector3<f32>,
    pub mass: f32,
    pub is_grounded: bool,
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            angular_velocity: Vector3::zero(),
            linear_velocity: Vector3::zero(),
            mass: 1.0,
            is_grounded: false,
        }
    }
}

impl Velocity {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}
#[update]
fn velocity_process(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    for node in world.get_objects_with_component_mut::<Velocity>() {
        // if node.get_component::<Collider>().is_ok() {
        //     continue;
        // }
        let linear = node.get_component::<Velocity>()?.linear_velocity;
        let transform = node.get_component_mut::<Transform>()?;
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
