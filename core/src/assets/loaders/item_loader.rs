use parking_lot::RwLock;
use std::{
    any::Any,
    sync::Arc,
};

use anyhow::{Error, Result};

use crate::{
    assets::loader::YamlAssetLoader,
    items::{ItemDefinition, ItemId, ItemRegistry},
    log_warn,
    ecs::component::{BoxedComponent, get_component_registration},
};

#[derive(Clone)]
pub struct ItemLoader {
    pub registry: Arc<RwLock<ItemRegistry>>,
}

impl YamlAssetLoader for ItemLoader {
    fn class_name(&self) -> &'static str {
        "Item"
    }

    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()> {
        let name: String = raw["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'name'"))?
            .to_string();

        let namespace: String = raw["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'namespace'"))?
            .to_string();

        {
            let registry = self.registry.read();

            for reg in registry.defs.iter() {
                if reg.1.name == name && reg.1.namespace == namespace {
                    let msg = format!(
                        "Item with the name: {} exists in name space {} already",
                        name.to_string(),
                        namespace.to_string()
                    );

                    return Err(Error::msg(msg));
                }
            }
        }
        let mut components: Vec<BoxedComponent> = Vec::new();

        if let Some(comp_map) = raw["components"].as_mapping() {
            for (key, value) in comp_map {
                let component_name = key
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid component key"))?;

                if let Some(registration) = get_component_registration(component_name) {
                    let mut component = (registration.create)();
                    (registration.deserialize)(&mut component, value)?;
                    components.push(component);
                } else {
                    log_warn!("Unknown component: {}", component_name);
                }
            }
        }

        let def = ItemDefinition {
            name: name.clone(),
            namespace: namespace.clone(),
            class: "Item".to_string(),
            components,
        };

        let mut registry = self.registry.write();

        for reg in registry.defs.iter() {
            if reg.1.name == name && reg.1.namespace == namespace {
                let msg = format!(
                    "Item with the name: {} exists in name space {} already",
                    name.to_string(),
                    namespace.to_string()
                );

                return Err(Error::msg(msg));
            }
        }

        let id = registry.defs.len() as ItemId;
        let full_name = format!("{}:Item:{}", namespace, name);
        registry.defs.insert(full_name.clone(), def);
        registry.name_to_id.insert(full_name.clone(), id);
        registry.id_to_name.insert(id, full_name);

        Ok(())
    }
    fn clone_box(&self) -> Box<dyn YamlAssetLoader> {
        Box::new(self.clone())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn list_entries(&self) -> Vec<(String, String)> {
        let registry = self.registry.read();
        registry.defs.values().map(|d| (d.namespace.clone(), d.name.clone())).collect()
    }
}
