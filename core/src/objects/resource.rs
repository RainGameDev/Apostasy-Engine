use std::any::{Any, TypeId, type_name};

use anyhow::{Result, anyhow};
use hashbrown::HashMap;

pub trait Resource: ResourceContainer {
    fn name() -> &'static str
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
}

pub trait ResourceContainer {
    fn clone_box(&self) -> Box<dyn Resource>;
}

impl<T> ResourceContainer for T
where
    T: 'static + Resource + Clone,
{
    fn clone_box(&self) -> Box<dyn Resource> {
        Box::new(self.clone())
    }
}
impl Clone for Box<dyn Resource> {
    fn clone(&self) -> Box<dyn Resource> {
        self.clone_box()
    }
}

pub struct ResourceRegistration {
    pub type_name: &'static str,
    // pub serialize: fn(&dyn Resource) -> serde_yaml::Value,
    // pub deserialize: fn(serde_yaml::Value) -> Box<dyn Resource>,
    pub create: fn() -> Box<dyn Resource>,
}

inventory::collect!(ResourceRegistration);

pub fn get_resource_registration(type_name: &str) -> Option<&'static ResourceRegistration> {
    inventory::iter::<ResourceRegistration>()
        .find(|r| r.type_name.to_lowercase() == type_name.to_lowercase())
}

#[derive(Default)]
pub struct ResourceMap {
    pub(crate) map: HashMap<TypeId, Box<dyn Resource>>,
}

impl ResourceMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Insert a new resource into the map
    pub fn insert<T: Resource + 'static>(&mut self, resource: T) {
        if self.get::<T>().is_ok() {
            self.remove::<T>();
        }

        self.map.insert(TypeId::of::<T>(), Box::new(resource));
    }

    /// Get a resource from the map
    pub fn get<T: Resource + 'static>(&self) -> Result<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|r| r.as_any().downcast_ref::<T>())
            .ok_or_else(|| anyhow!("[ERROR!] Resource {} not found", type_name::<T>()))
    }

    /// Get a resource mutably from the map
    pub fn get_mut<T: Resource + 'static>(&mut self) -> Result<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|r| r.as_any_mut().downcast_mut::<T>())
            .ok_or_else(|| anyhow!("[ERROR!] Resource {} not found", type_name::<T>()))
    }

    /// Remove a resource from the map
    pub fn remove<T: Resource + 'static>(&mut self) {
        self.map.remove(&TypeId::of::<T>());
    }
}
