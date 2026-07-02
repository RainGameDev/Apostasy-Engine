use anyhow::Result;
use apostasy_macros::{Component, update};
use cgmath::{InnerSpace, Quaternion, Rotation3, Vector3, Zero};

use crate::{
    log,
    ecs::{
        component::Inspect,
        components::{serde_support::vec3_seq, transform::Transform},
        systems::DeltaTime,
        tags::Player,
        world::World,
    },
    ui::{DRAG_SIZE, LABEL_WIDTH},
};

/// Rigid body physics state for an entity.
/// Set `mass` to `0.0` or `process` to `false` to make the entity immovable.
#[derive(Component, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[component(serde)]
#[serde(default)]
pub struct Velocity {
    #[serde(with = "vec3_seq")]
    pub angular_velocity: Vector3<f32>,
    #[serde(with = "vec3_seq")]
    pub linear_velocity: Vector3<f32>,
    pub mass: f32,
    /// Set by the collision system each frame, `true` if resting on a surface.
    pub is_grounded: bool,
    /// Whether physics simulation is applied to this entity each frame.
    pub process: bool,

    #[serde(with = "vec3_seq")]
    pub inertia_tensor: Vector3<f32>,
    /// Static friction coefficient.
    pub mu_static: f32,
    /// Kinetic friction coefficient.
    pub mu_kinetic: f32,
    /// Bounciness; `0.0` = no bounce.
    pub restitution: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
}

impl Inspect for Velocity {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Mass"));
                ui.add_sized(DRAG_SIZE, egui::DragValue::new(&mut self.mass).speed(0.1));
            });
            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Friction"));
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.mu_kinetic).speed(0.1),
                );
            });
            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Damping"));
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.angular_damping).speed(0.1),
                );
            });

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Bounciness"));
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut self.restitution).speed(0.1),
                );
            });

            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Process"));
                ui.style_mut().spacing.icon_width = 20.0;
                ui.style_mut().spacing.icon_width_inner = 14.0;
                ui.add_space(DRAG_SIZE.x / 3.0);
                ui.add(egui::Checkbox::new(&mut self.process, ""));
            });

            ui.separator();
        });
    }
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
    /// Creates a zero-mass, non-processing velocity for immovable entities.
    pub fn static_entity() -> Self {
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

    /// Default velocity preset for a sphere collider with high damping and no bounce.
    pub fn default_sphere() -> Self {
        Self {
            angular_velocity: Vector3::zero(),
            linear_velocity: Vector3::zero(),
            mass: 1.0,
            is_grounded: false,
            process: true,

            inertia_tensor: compute_inertia_sphere(1.0, 1.0),
            restitution: 0.0,
            mu_static: 1.0,
            mu_kinetic: 1.0,
            linear_damping: 1.0,
            angular_damping: 1.0,
        }
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
    let noclip = world.get_resource::<crate::physics::Noclip>().map(|n| n.0).unwrap_or(false);

    let ids = world.get_entities_with_component::<Velocity>();
    for id in ids {
        if noclip && !world.has_tag::<Player>(id) {
            continue;
        }

        let (linear, angular, grounded) = {
            let vel = match world.get_component_mut::<Velocity>(id) {
                Some(v) => v,
                None => continue,
            };
            if vel.mass == 0.0 || !vel.process {
                continue;
            }
            (vel.linear_velocity, vel.angular_velocity, vel.is_grounded)
        };

        if let Some(transform) = world.get_component_mut::<Transform>(id) {
            transform.local_position += linear * delta;

            if angular.magnitude2() > 0.01 {
                let angle = angular.magnitude();
                let axis = angular / angle;
                let dq = Quaternion::from_axis_angle(axis, cgmath::Rad(angle * delta));
                transform.local_rotation = (dq * transform.local_rotation).normalize();
            }
        }

        if let Some(vel) = world.get_component_mut::<Velocity>(id) {
            if grounded {
                let tangential = Vector3::new(vel.linear_velocity.x, 0.0, vel.linear_velocity.z);
                let speed = tangential.magnitude();
                if speed < 0.2 {
                    vel.linear_velocity.x = 0.0;
                    vel.linear_velocity.z = 0.0;
                } else {
                    let friction_acc = vel.mu_kinetic * 9.8;
                    let friction_delta = friction_acc * delta;
                    let new_speed = (speed - friction_delta).max(0.0);
                    let tangential_dir = tangential / speed;
                    let new_tangential = tangential_dir * new_speed;
                    vel.linear_velocity.x = new_tangential.x;
                    vel.linear_velocity.z = new_tangential.z;
                }
            }

            vel.linear_velocity *= vel.linear_damping.powf(delta);
            vel.angular_velocity *= vel.angular_damping.powf(delta);
        }
    }

    Ok(())
}

// #[fixed_update]
pub fn physics_debug(world: &mut World, _: f32) -> Result<()> {
    let player_id = world.get_entity_with_tag::<Player>()?;
    let transform = world.get_component::<Transform>(player_id).ok_or_else(|| anyhow::anyhow!("no transform"))?;
    let velocity = world.get_component::<Velocity>(player_id).ok_or_else(|| anyhow::anyhow!("no velocity"))?;

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

/// Computes the diagonal inertia tensor for a solid sphere.
pub fn compute_inertia_sphere(mass: f32, radius: f32) -> Vector3<f32> {
    let i = 2.0 / 5.0 * mass * radius * radius;
    Vector3::new(i, i, i)
}

/// Computes the diagonal inertia tensor for a solid cuboid given its half-extents.
pub fn compute_inertia_cuboid(mass: f32, half: Vector3<f32>) -> Vector3<f32> {
    let (hx, hy, hz) = (half.x, half.y, half.z);
    Vector3::new(
        mass / 3.0 * (hy * hy + hz * hz),
        mass / 3.0 * (hx * hx + hz * hz),
        mass / 3.0 * (hx * hx + hy * hy),
    )
}
