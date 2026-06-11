use std::any::TypeId;

use anyhow::{Error, Result};

use crate::{
    log_warn,
    objects::{
        cell::ObjectId,
        component::{Component, get_component_registration},
        tag::Tag,
    },
};

pub mod cell;
pub mod cell_streaming;
pub mod component;
pub mod components;
pub mod query;
pub mod resource;
pub mod resources;
pub mod systems;
pub mod tag;
pub mod tags;
pub mod world;
pub mod worldspace;
pub mod worldspace_serializer;

use crate::objects::component::BoxedComponent;

#[derive(Clone)]
pub struct Object {
    pub id: ObjectId,
    pub name: String,
    pub components: Vec<BoxedComponent>,
    pub tags: Vec<Box<dyn Tag + Send + Sync>>,
    pub parent: Option<ObjectId>,
    pub children: Vec<ObjectId>,
}

impl Default for Object {
    fn default() -> Self {
        Object::new()
    }
}

impl Object {
    pub fn new() -> Self {
        Self {
            name: "Object".to_string(),
            id: ObjectId::default(),
            children: Vec::new(),
            tags: Vec::new(),
            parent: None,
            components: Vec::new(),
        }
    }

    pub fn set_name(&mut self, name: &str) -> Self {
        self.name = name.to_string();
        self.clone()
    }

    // ========== ========== Tags ========== ==========

    pub fn has_tag<T: Tag + 'static>(&self) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.as_any().downcast_ref::<T>().is_some())
    }

    pub fn get_tag<T: Tag + 'static>(&self) -> Result<&T> {
        self.tags
            .iter()
            .find(|c| c.as_any().type_id() == TypeId::of::<T>())
            .and_then(|c| c.as_any().downcast_ref())
            .ok_or_else(|| Error::msg(format!("No Tag of type: {}", T::name())))
    }

    pub fn add_tag<T: Tag + 'static>(&mut self, tag: T) -> Self {
        if self.get_tag::<T>().is_ok() {
            return self.clone();
        }
        self.tags.push(Box::new(tag));
        self.clone()
    }

    pub fn remove_tag<T: Tag + 'static>(&mut self) {
        if let Some(i) = self
            .tags
            .iter()
            .position(|c| c.as_any().type_id() == TypeId::of::<T>())
        {
            self.tags.remove(i);
        }
    }

    // ========== ========== Components ========== ==========

    pub fn has_component<T: Component + 'static>(&self) -> bool {
        self.components
            .iter()
            .any(|component| component.as_any().downcast_ref::<T>().is_some())
    }

    pub fn remove_component<T: Component + 'static>(&mut self) {
        if let Some(i) = self
            .components
            .iter()
            .position(|c| c.as_any().type_id() == TypeId::of::<T>())
        {
            self.components.remove(i);
        }
    }

    pub fn get_component<T: Component + 'static>(&self) -> Result<&T> {
        self.components
            .iter()
            .find(|c| c.as_any().type_id() == TypeId::of::<T>())
            .and_then(|c| c.as_any().downcast_ref())
            .ok_or_else(|| Error::msg(format!("No Component of type: {}", T::name())))
    }

    pub fn get_component_mut<T: Component + 'static>(&mut self) -> Result<&mut T> {
        self.components
            .iter_mut()
            .find(|c| c.as_any().type_id() == TypeId::of::<T>())
            .and_then(|c| c.as_any_mut().downcast_mut())
            .ok_or_else(|| Error::msg(format!("No Component of type: {}", T::name())))
    }

    pub fn get_components(&self) -> Vec<&Box<dyn Component + Send + Sync>> {
        self.components.iter().collect()
    }

    pub fn get_components_mut(&mut self) -> Vec<&mut Box<dyn Component + Send + Sync>> {
        self.components.iter_mut().collect()
    }

    pub fn add_boxed_component(&mut self, component: Box<dyn Component>) -> Self {
        // if self.get_component::<T>().is_ok() {
        //     log_warn!("You can only have one of any component on an entity");
        //     return self.clone();
        // }
        self.components.push(component);
        self.clone()
    }

    pub fn add_component<T: Component + 'static>(&mut self, component: T) -> Self {
        if self.get_component::<T>().is_ok() {
            log_warn!("You can only have one of any component on an entity");
            return self.clone();
        }
        self.components.push(Box::new(component));
        self.clone()
    }

    pub fn add_component_by_name(&mut self, component_name: &str) -> Result<()> {
        let mut component_name = component_name.to_string();
        component_name = component_name.replace(" ", "");
        component_name = component_name.replace("_", "");

        let registration = get_component_registration(component_name.to_lowercase().as_str())
            .ok_or_else(|| {
                log_warn!("Component '{}' is not registered", component_name);
                anyhow::anyhow!(
                    "Component '{}' is not registered",
                    component_name.to_lowercase()
                )
            })?;

        let component = (registration.create)();
        let new_type_name = component.type_name();

        if self
            .components
            .iter()
            .any(|c| c.type_name() == new_type_name)
        {
            log_warn!("You can only have one of any component on an entity");
            return Ok(());
        }

        self.components.push(component);
        Ok(())
    }
    pub fn remove_component_by_type_id(&mut self, type_id: TypeId) {
        if let Some(i) = self
            .components
            .iter()
            .position(|c| c.as_any().type_id() == type_id)
        {
            self.components.remove(i);
        }
    }
}

/// Formats an object ID to a short display value: `cell_x,cell_z:index`.
pub fn fmt_key(id: ObjectId) -> String {
    let key = format!("{:?}", id.key);
    let index = key
        .trim_start_matches("DefaultKey(")
        .trim_start_matches("KeyData(")
        .split('v')
        .next()
        .unwrap_or(&key)
        .trim_end_matches(')');
    format!("{},{}:{}", id.cell.x, id.cell.z, index)
}
