use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::{InnerSpace, Quaternion, Rotation3, Vector3, Zero};

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

    pub inertia_tensor: Vector3<f32>,
    // static friction coefficient
    pub mu_static: f32,
    // kinetic friction coefficient
    pub mu_kinetic: f32,
    // bounciness, 0 = no bounce
    pub restitution: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

impl Default for Velocity {
    /// Note: Default goes to cuboid
    fn default() -> Self {
        let half = Vector3::new(0.5, 0.5, 0.5);
        Self {
            angular_velocity: Vector3::zero(),
            linear_velocity: Vector3::zero(),
            mass: 1.0,
            is_grounded: false,
            process: true,

            inertia_tensor: compute_inertia_cuboid(1.0, half),
            restitution: 0.1,
            mu_static: 0.9,
            mu_kinetic: 0.8,
            linear_damping: 0.999,
            angular_damping: 0.998,
        }
    }
}

impl Velocity {
    pub fn static_object() -> Self {
        Self {
            angular_velocity: Vector3::zero(),
            linear_velocity: Vector3::zero(),
            mass: 0.0,
            is_grounded: false,
            process: false,

            inertia_tensor: Vector3::zero(),
            restitution: 0.4,
            mu_static: 0.9,
            mu_kinetic: 0.8,
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }

    /// Note: Default goes to cuboid
    pub fn default_sphere() -> Self {
        Self {
            angular_velocity: Vector3::zero(),
            linear_velocity: Vector3::zero(),
            mass: 1.0,
            is_grounded: false,
            process: true,

            inertia_tensor: compute_inertia_sphere(1.0, 1.0),
            restitution: 0.3,
            mu_static: 0.3,
            mu_kinetic: 0.2,
            linear_damping: 0.999,
            angular_damping: 0.995,
        }
    }
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

#[update(priority = 20)]
fn velocity_process(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    for object in world.get_objects_with_component_mut::<Velocity>() {
        let (linear, angular, grounded) = {
            let vel = object.get_component_mut::<Velocity>()?;
            if vel.mass == 0.0 || !vel.process {
                continue;
            }
            (
                vel.linear_velocity,
                vel.angular_velocity,
                vel.is_grounded,
            )
        };

        let transform = object.get_component_mut::<Transform>()?;
        transform.local_position += linear * delta;

        if angular.magnitude2() > 0.01 {
            let angle = angular.magnitude();
            let axis = angular / angle;
            let dq = Quaternion::from_axis_angle(axis, cgmath::Rad(angle * delta));
            transform.local_rotation = (dq * transform.local_rotation).normalize();
        }

        let vel = object.get_component_mut::<Velocity>()?;
        if grounded {
            let tangential = Vector3::new(vel.linear_velocity.x, 0.0, vel.linear_velocity.z);
            if tangential.magnitude() < 0.2 {
                vel.linear_velocity.x = 0.0;
                vel.linear_velocity.z = 0.0;
            } else {
                let grounded_damping = 0.9_f32.powf(delta);
                vel.linear_velocity.x *= grounded_damping;
                vel.linear_velocity.z *= grounded_damping;
            }
        }

        vel.linear_velocity *= vel.linear_damping.powf(delta);
        vel.angular_velocity *= vel.angular_damping.powf(delta);
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

pub fn compute_inertia_sphere(mass: f32, radius: f32) -> Vector3<f32> {
    let i = 2.0 / 5.0 * mass * radius * radius;
    Vector3::new(i, i, i)
}

pub fn compute_inertia_cuboid(mass: f32, half: Vector3<f32>) -> Vector3<f32> {
    let (hx, hy, hz) = (half.x, half.y, half.z);
    Vector3::new(
        mass / 3.0 * (hy * hy + hz * hz),
        mass / 3.0 * (hx * hx + hz * hz),
        mass / 3.0 * (hx * hx + hy * hy),
    )
}
