use apostasy_macros::Component;

use crate::objects::component::Inspect;

/// Attaches a WASM script to an object.
/// The script at `path` must export `on_start` and/or `on_update`.
#[derive(Component, Clone, Debug, Default)]
pub struct ScriptComponent {
    pub path: String,
}

impl Inspect for ScriptComponent {}

impl ScriptComponent {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        if let Some(v) = value.get("path").and_then(|v| v.as_str()) {
            self.path = v.to_string();
        }
        Ok(())
    }
}
