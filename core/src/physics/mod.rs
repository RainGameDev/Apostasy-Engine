use anyhow::Result;
use apostasy_macros::{Component, Inspect, fixed_update};

use crate::{objects::world::World, physics::velocity::Velocity};

pub mod collider;
pub mod collision_system;
pub mod raycast;
pub mod velocity;

#[derive(Component, Inspect, Clone, Debug)]
pub struct Gravity {
    pub strength: f32,
}

impl Default for Gravity {
    fn default() -> Self {
        Self { strength: 9.81 }
    }
}

impl Gravity {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        if let Some(v) = value.get("strength").and_then(|v| v.as_f64()) {
            self.strength = v as f32;
        }
        Ok(())
    }
}
#[fixed_update(priority = 10)]
pub fn apply_gravity(world: &mut World, delta: f32) -> Result<()> {
    for object in world.get_objects_with_component_mut::<Velocity>() {
        if let Ok(gravity) = object.get_component::<Gravity>().cloned() {
            let velocity = object.get_component_mut::<Velocity>()?;
            if velocity.is_grounded {
                if velocity.linear_velocity.y < 0.0 {
                    velocity.linear_velocity.y = 0.0;
                }
            } else {
                let gravity = gravity.strength;
                let mut fall_accel = gravity;
                if velocity.linear_velocity.y < 0.0 {
                    fall_accel *= 1.8;
                }
                velocity.linear_velocity.y -= fall_accel * delta;
                velocity.linear_velocity.y = velocity.linear_velocity.y.max(-50.0);
            }
        }
    }
    Ok(())
}
