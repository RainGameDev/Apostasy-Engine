use apostasy_macros::{Component, Inspect, Resource};
use hashbrown::HashMap;
use serde_yaml::Value;

/// All of an entity's lua defined componentes, keyed by name.
/// Stored as `serde_yaml::Value` so it can bridge lua tables and the engine's existing component
/// seraliation
#[derive(Component, Inspect, Clone, Debug, Default)]
pub struct ScriptComponents {
    pub map: HashMap<String, Value>,
}

impl ScriptComponents {
    /// Adds a component of name.
    pub fn get(&self, name: &str) -> Option<&serde_yaml::Value> {
        self.map.get(name)
    }
    /// Sets a component of name.
    pub fn set(&mut self, name: &str, value: serde_yaml::Value) {
        self.map.insert(name.to_string(), value);
    }
    /// Removes a component of name.
    pub fn remove(&mut self, name: &str) {
        self.map.remove(name);
    }
    /// Checks if theres a component of name.
    pub fn has(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }
}

/// Schemas for every lua created component.
/// Name -> Default tables.
/// for example:
/// ```lua
///     register_component("health", {current = 100, max = 100})
/// ```
#[derive(Resource, Clone, Default)]
pub struct LuaComponentRegistry {
    pub defaults: HashMap<String, Value>,
}

impl LuaComponentRegistry {
    pub fn register(&mut self, name: &str, default: Value) {
        self.defaults.insert(name.to_string(), default);
    }
    pub fn is_registered(&self, name: &str) -> bool {
        self.defaults.contains_key(name)
    }
    pub fn default_for(&self, name: &str) -> Option<&Value> {
        self.defaults.get(name)
    }
}
