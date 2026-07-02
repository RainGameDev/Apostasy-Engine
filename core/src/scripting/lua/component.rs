use apostasy_macros::{Component, Resource};
use hashbrown::HashMap;
use serde_yaml::Value;

use crate::ecs::component::Inspect;
use crate::ui::{DRAG_SIZE, LABEL_WIDTH};

/// All of an entity's lua defined componentes, keyed by name.
/// Stored as `serde_yaml::Value` so it can bridge lua tables and the engine's existing component
/// seraliation
#[derive(Component, Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[component(serde, skip_if = "is_empty")]
#[serde(default)]
pub struct ScriptComponents {
    #[serde(rename = "components", with = "sorted_map")]
    pub map: HashMap<String, Value>,
}

/// Serde for the script component map; keys are sorted on write so saved
/// scenes diff cleanly (HashMap order isn't stable).
mod sorted_map {
    use super::*;
    use serde::ser::SerializeMap;

    pub fn serialize<S: serde::Serializer>(
        map: &HashMap<String, Value>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut names: Vec<&String> = map.keys().collect();
        names.sort();
        let mut out = serializer.serialize_map(Some(names.len()))?;
        for name in names {
            out.serialize_entry(name, &map[name])?;
        }
        out.end()
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<HashMap<String, Value>, D::Error> {
        use serde::Deserialize as _;
        let entries = std::collections::BTreeMap::<String, Value>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

impl Inspect for ScriptComponents {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        if self.map.is_empty() {
            ui.weak("(no script components)");
            return;
        }
        // Stable alphabetical order — HashMap iteration order isn't deterministic.
        let mut names: Vec<String> = self.map.keys().cloned().collect();
        names.sort();
        for name in names {
            let value = self.map.get_mut(&name).expect("key just collected");
            egui::CollapsingHeader::new(&name)
                .default_open(true)
                .show(ui, |ui| inspect_value(ui, value));
        }
        ui.separator();
    }
}

/// Renders a `Value`. Mappings become labelled rows; scalars are edited in place.
fn inspect_value(ui: &mut egui::Ui, value: &mut Value) {
    match value {
        Value::Mapping(map) => {
            let keys: Vec<Value> = map.keys().cloned().collect();
            for k in keys {
                let label = k
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("{k:?}"));
                let field = map.get_mut(&k).expect("key just collected");
                ui.horizontal(|ui| {
                    ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new(label));
                    inspect_scalar(ui, field);
                });
            }
        }
        other => {
            inspect_scalar(ui, other);
        }
    }
}

/// Editable widget for a single scalar `Value`. Nested tables shown read-only.
fn inspect_scalar(ui: &mut egui::Ui, v: &mut Value) {
    match v {
        Value::Bool(b) => {
            ui.checkbox(b, "");
        }
        Value::String(s) => {
            ui.add_sized(DRAG_SIZE, egui::TextEdit::singleline(s));
        }
        Value::Number(_) => {
            // Preserve integer-ness so values round-trip back to Lua cleanly.
            if let Some(i) = v.as_i64() {
                let mut x = i;
                if ui.add_sized(DRAG_SIZE, egui::DragValue::new(&mut x)).changed() {
                    *v = Value::from(x);
                }
            } else {
                let mut f = v.as_f64().unwrap_or(0.0);
                if ui
                    .add_sized(DRAG_SIZE, egui::DragValue::new(&mut f).speed(0.1))
                    .changed()
                {
                    *v = Value::from(f);
                }
            }
        }
        Value::Null => {
            ui.weak("nil");
        }
        nested => {
            ui.weak(format!("{nested:?}"));
        }
    }
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

    /// True when no script components are attached; such entities skip scene persistence.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
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
