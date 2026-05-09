use apostasy_macros::Component;

/// The type of tint a voxel has, colours defined in biomes
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TintType {
    #[default]
    Foliage,
    Water,
}

/// Defines if a voxel has a tint, takes a TintType
#[derive(Component, Clone, Debug, Default)]
pub struct HasTint(pub TintType);

impl HasTint {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        self.0 = if value.as_i64().unwrap() == 0 {
            TintType::Foliage
        } else {
            TintType::Water
        };
        Ok(())
    }
}
