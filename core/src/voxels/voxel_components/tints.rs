use apostasy_macros::{Component, Inspect};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The type of tint a voxel has, colours defined in biomes.
/// Serialized as an int (`0` = Foliage, anything else = Water) to match voxel yaml.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TintType {
    #[default]
    Foliage,
    Water,
}

impl Serialize for TintType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(match self {
            TintType::Foliage => 0,
            TintType::Water => 1,
        })
    }
}

impl<'de> Deserialize<'de> for TintType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match i64::deserialize(deserializer)? {
            0 => TintType::Foliage,
            _ => TintType::Water,
        })
    }
}

/// Defines if a voxel has a tint, takes a TintType
#[derive(Component, Inspect, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[component(serde)]
pub struct HasTint(pub TintType);
