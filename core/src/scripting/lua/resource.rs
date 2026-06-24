use apostasy_macros::Resource;
use hashbrown::HashMap;
use serde_yaml::Value;

/// Global key/value state shared between Lua scripts (and readable from Rust).
///
/// Declared at script top-level with `register_resource(name, defaults)` and
/// accessed via `world:get_resource(name)` / `world:set_resource(name, table)`.
/// Unlike [`ScriptComponents`](super::component::ScriptComponents) this is a
/// single global blackboard, not per-entity.
#[derive(Resource, Clone, Default)]
pub struct LuaResources {
    pub map: HashMap<String, Value>,
}

impl LuaResources {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.map.get(name)
    }
    pub fn set(&mut self, name: &str, value: Value) {
        self.map.insert(name.to_string(), value);
    }
    pub fn has(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
    pub fn remove(&mut self, name: &str) {
        self.map.remove(name);
    }
}
