use std::sync::{Arc, RwLock};

use anyhow::{Error, Result};

use crate::{
    assets::loader::AssetLoader,
    items::{ItemDefinition, ItemId, ItemRegistry},
    log_warn,
    objects::component::{BoxedComponent, get_component_registration},
};

pub struct ItemLoader {
    pub registry: Arc<RwLock<ItemRegistry>>,
}

impl AssetLoader for ItemLoader {
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
            let registry = self.registry.read().unwrap();

            for reg in registry.defs.iter() {
                if reg.name == name && reg.namespace == namespace {
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

        let mut registry = self.registry.write().unwrap();

        for reg in registry.defs.iter() {
            if reg.name == name && reg.namespace == namespace {
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
        registry.defs.push(def);
        registry.name_to_id.insert(full_name.clone(), id);
        registry.id_to_name.insert(id, full_name);

        Ok(())
    }
}
