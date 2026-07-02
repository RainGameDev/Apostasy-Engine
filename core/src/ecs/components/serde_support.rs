use anyhow::Result;
use cgmath::{Quaternion, Vector3};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Deep-merges `patch` onto `base`: mappings merge key-wise, everything else
/// (scalars, sequences) is replaced wholesale.
pub fn deep_merge(base: &mut serde_yaml::Value, patch: &serde_yaml::Value) {
    match (base, patch) {
        (serde_yaml::Value::Mapping(base), serde_yaml::Value::Mapping(patch)) => {
            for (key, value) in patch {
                match base.get_mut(key) {
                    Some(slot) => deep_merge(slot, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}

/// Applies a partial yaml `patch` onto `current`. Fields absent from the patch
/// keep their current values (merge semantics, not replace).
pub fn apply_patch<T: Serialize + DeserializeOwned>(
    current: &mut T,
    patch: &serde_yaml::Value,
) -> Result<()> {
    let mut base = serde_yaml::to_value(&*current)?;
    deep_merge(&mut base, patch);
    *current = serde_yaml::from_value(base)?;
    Ok(())
}

/// Serializes a component to yaml with its `type` name injected, for scene
/// persistence. Returns `None` for non-mapping shapes (scalar/newtype
/// components are asset-definition data, not scene data).
pub fn serialize_component<T: Serialize>(
    component: &T,
    type_name: &str,
) -> Option<serde_yaml::Value> {
    let mut value = serde_yaml::to_value(component).ok()?;
    if let serde_yaml::Value::Mapping(map) = &mut value {
        map.insert("type".into(), type_name.into());
        Some(value)
    } else {
        None
    }
}

/// Deserializes `Option<String>` treating an empty string as `None`
/// (legacy scene files wrote `""` for absent values).
pub fn empty_str_as_none<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.filter(|s| !s.is_empty()))
}

/// Serde for `Vector3<f32>` as a `[x, y, z]` yaml sequence.
pub mod vec3_seq {
    use super::*;

    pub fn serialize<S: Serializer>(
        v: &Vector3<f32>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        [v.x, v.y, v.z].serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Vector3<f32>, D::Error> {
        let [x, y, z] = <[f32; 3]>::deserialize(deserializer)?;
        Ok(Vector3::new(x, y, z))
    }
}

/// Serde for `Quaternion<f32>` as a `[x, y, z, w]` yaml sequence.
pub mod quat_xyzw {
    use super::*;

    pub fn serialize<S: Serializer>(
        q: &Quaternion<f32>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        [q.v.x, q.v.y, q.v.z, q.s].serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Quaternion<f32>, D::Error> {
        let [x, y, z, w] = <[f32; 4]>::deserialize(deserializer)?;
        Ok(Quaternion::new(w, x, y, z))
    }
}

/// Serde for `Vector3<f32>` as an `{ r, g, b }` yaml mapping (light colors).
pub mod rgb {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Rgb {
        r: f32,
        g: f32,
        b: f32,
    }

    pub fn serialize<S: Serializer>(
        v: &Vector3<f32>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        Rgb { r: v.x, g: v.y, b: v.z }.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Vector3<f32>, D::Error> {
        let c = Rgb::deserialize(deserializer)?;
        Ok(Vector3::new(c.r, c.g, c.b))
    }
}
