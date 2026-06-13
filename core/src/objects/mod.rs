use std::any::TypeId;

use anyhow::{Error, Result};

use crate::{
    log_warn,
    objects::{
        cell::ObjectId,
        component::{Component, get_component_registration},
        tag::{Tag, get_tag_registration},
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
pub mod worldspace_streaming;

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

    /// Checks if the object has a tag of type [`T`].
    pub fn has_tag<T: Tag + 'static>(&self) -> bool {
        self.tags
            .iter()
            .any(|tag| tag.as_any().downcast_ref::<T>().is_some())
    }

    /// Gets a tag of type [`T`] from the object.
    /// Returns an error if it doesn't.
    pub fn get_tag<T: Tag + 'static>(&self) -> Result<&T> {
        self.tags
            .iter()
            .find(|c| c.as_any().type_id() == TypeId::of::<T>())
            .and_then(|c| c.as_any().downcast_ref())
            .ok_or_else(|| Error::msg(format!("No Tag of type: {}", T::name())))
    }

    /// Adds tag of type [`T`] to the object.
    /// Note: You cannot have more than one of the same tag.
    pub fn add_tag<T: Tag + 'static>(&mut self, tag: T) -> Self {
        if self.get_tag::<T>().is_ok() {
            return self.clone();
        }
        self.tags.push(Box::new(tag));
        self.clone()
    }

    /// Removes tag of type [`T`] from the object if it exists.
    pub fn remove_tag<T: Tag + 'static>(&mut self) {
        if let Some(i) = self
            .tags
            .iter()
            .position(|c| c.as_any().type_id() == TypeId::of::<T>())
        {
            self.tags.remove(i);
        }
    }

    /// Adds tag via name of [`type_name`] if it exists.
    /// Note: returns early if it already has the object.
    /// Note: errors if a tag of type [`type_name`] doesnt exist.
    pub fn add_tag_by_name(&mut self, type_name: &str) -> Result<()> {
        let registration = get_tag_registration(type_name)
            .ok_or_else(|| anyhow::anyhow!("Tag '{}' is not registered", type_name))?;
        let already_has = self.tags.iter().any(|t| {
            t.type_name()
                .rsplit("::")
                .next()
                .unwrap_or(t.type_name())
                .eq_ignore_ascii_case(registration.type_name)
        });
        if already_has {
            return Ok(());
        }
        self.tags.push((registration.create)());
        Ok(())
    }

    /// Removes tag via name of [`type_name`] if it exists.
    /// Note: errors if a tag of type [`type_name`] doesnt exist.
    pub fn remove_tag_by_name(&mut self, type_name: &str) {
        if let Some(i) = self.tags.iter().position(|t| {
            t.type_name()
                .rsplit("::")
                .next()
                .unwrap_or(t.type_name())
                .eq_ignore_ascii_case(type_name)
        }) {
            self.tags.remove(i);
        }
    }

    // ========== ========== Components ========== ==========

    /// Checks if the object has a component of type [`T`].
    ///
    /// # Example
    /// ```
    /// let has = object.has_component::<Transform>();
    /// ```
    pub fn has_component<T: Component + 'static>(&self) -> bool {
        self.components
            .iter()
            .any(|component| component.as_any().downcast_ref::<T>().is_some())
    }

    /// Removes a component of type [`T`] from the object if it exists.
    ///
    /// # Example
    /// ```
    /// object.remove_component::<Transform>();
    /// ```
    pub fn remove_component<T: Component + 'static>(&mut self) {
        if let Some(i) = self
            .components
            .iter()
            .position(|c| c.as_any().type_id() == TypeId::of::<T>())
        {
            self.components.remove(i);
        }
    }

    /// Gets a component of type [`T`] from the object.
    /// Returns an error if it doesn't.
    ///
    /// # Example
    /// ```
    /// let transform = object.get_component::<Transform>()?;
    /// ```
    pub fn get_component<T: Component + 'static>(&self) -> Result<&T> {
        self.components
            .iter()
            .find(|c| c.as_any().type_id() == TypeId::of::<T>())
            .and_then(|c| c.as_any().downcast_ref())
            .ok_or_else(|| Error::msg(format!("No Component of type: {}", T::name())))
    }

    /// Gets a component of type [`T`] from the object mutably.
    /// Returns an error if it doesn't.
    ///
    /// # Example
    /// ```
    /// let transform = object.get_component_mut::<Transform>()?;
    /// transform.local_position.x += 1.0;
    /// ```
    pub fn get_component_mut<T: Component + 'static>(&mut self) -> Result<&mut T> {
        self.components
            .iter_mut()
            .find(|c| c.as_any().type_id() == TypeId::of::<T>())
            .and_then(|c| c.as_any_mut().downcast_mut())
            .ok_or_else(|| Error::msg(format!("No Component of type: {}", T::name())))
    }

    /// Returns all components on the object.
    ///
    /// # Example
    /// ```
    /// for component in object.get_components() {
    ///     println!("{}", component.type_name());
    /// }
    /// ```
    pub fn get_components(&self) -> Vec<&Box<dyn Component + Send + Sync>> {
        self.components.iter().collect()
    }

    /// Returns all components on the object mutably.
    ///
    /// # Example
    /// ```
    /// for component in object.get_components_mut() {
    ///     // mutate each component
    /// }
    /// ```
    pub fn get_components_mut(&mut self) -> Vec<&mut Box<dyn Component + Send + Sync>> {
        self.components.iter_mut().collect()
    }

    /// Adds a pre-boxed component to the object.
    /// Note: Does not check for duplicates, use [`add_component`] for that.
    ///
    /// # Example
    /// ```
    /// object.add_boxed_component(Box::new(Transform::default()));
    /// ```
    pub fn add_boxed_component(&mut self, component: Box<dyn Component>) -> Self {
        self.components.push(component);
        self.clone()
    }

    /// Adds a component of type [`T`] to the object.
    /// Note: You cannot have more than one of the same component.
    ///
    /// # Example
    /// ```
    /// object.add_component(Transform::default());
    /// ```
    pub fn add_component<T: Component + 'static>(&mut self, component: T) -> Self {
        if self.get_component::<T>().is_ok() {
            log_warn!("You can only have one of any component on an entity");
            return self.clone();
        }
        self.components.push(Box::new(component));
        self.clone()
    }

    /// Adds a component via name of [`component_name`] if it exists.
    /// Note: returns early if it already has the component.
    /// Note: errors if a component of type [`component_name`] doesnt exist.
    ///
    /// # Example
    /// ```
    /// object.add_component_by_name("Transform")?;
    /// ```
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

    /// Removes a component via name of [`component_name`] if it exists.
    ///
    /// # Example
    /// ```
    /// object.remove_component_by_name("Transform");
    /// ```
    pub fn remove_component_by_name(&mut self, component_name: &str) {
        let mut component_name = component_name.to_string();
        component_name = component_name.replace(" ", "");
        component_name = component_name.replace("_", "");

        if let Some(i) = self.components.iter().position(|c| {
            c.type_name()
                .rsplit("::")
                .next()
                .unwrap_or(c.type_name())
                .replace("_", "")
                .eq_ignore_ascii_case(&component_name)
        }) {
            self.components.remove(i);
        }
    }

    /// Removes a component by its [`TypeId`].
    ///
    /// # Example
    /// ```
    /// object.remove_component_by_type_id(TypeId::of::<Transform>());
    /// ```
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
