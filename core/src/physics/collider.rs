use apostasy_macros::Component;
use cgmath::Vector3;

#[derive(Component, Debug, Clone)]
pub struct Collider {
    pub half_extents: Vector3<f32>,
}

impl Default for Collider {
    fn default() -> Self {
        Self {
            half_extents: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

impl Collider {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn player() -> Self {
        Self {
            half_extents: Vector3::new(0.2, 1.0, 0.2),
        }
    }
}
