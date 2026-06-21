pub mod transform;

use std::any::{Any, TypeId};

use apostasy_macros::Resource;
use hashbrown::HashMap;

use crate::ecs::{Entity, sets::SparseSet};

pub type BoxedComponent = Box<dyn Component + Send + Sync>;

/// A trait that defines a component that can be attached to an entity.
pub trait Component: Send + Sync + 'static + ComponentContainer + std::fmt::Debug {
    fn name() -> &'static str
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
}

/// Workaround for object safety — allows cloning a boxed component.
pub trait ComponentContainer {
    fn clone_box(&self) -> BoxedComponent;
}

impl<T: Component + Clone + Send + Sync + 'static> ComponentContainer for T {
    fn clone_box(&self) -> BoxedComponent {
        Box::new(self.clone())
    }
}

impl Clone for BoxedComponent {
    fn clone(&self) -> BoxedComponent {
        self.clone_box()
    }
}

/// Contains all stored and registered components.
/// Components are registered on startup via `inventory`.
pub struct ComponentRegistration {
    pub type_name: &'static str,
    pub create: fn() -> BoxedComponent,
    pub deserialize: fn(&mut BoxedComponent, &serde_yaml::Value) -> anyhow::Result<()>,
}

inventory::collect!(ComponentRegistration);

/// Takes in a type name and returns the registered component, if it exists.
pub fn get_component_registration(type_name: &str) -> Option<&'static ComponentRegistration> {
    inventory::iter::<ComponentRegistration>()
        .find(|r| r.type_name.to_lowercase() == type_name.to_lowercase())
}

/// Struct defining an inspectable component.
pub struct InspectEntry {
    pub type_id: fn() -> TypeId,
    pub inspect_fn: fn(&mut dyn Any, &mut crate::egui::Ui),
}

type InspectFn = fn(&mut dyn Any, &mut crate::egui::Ui);
inventory::collect!(InspectEntry);

/// Registry of all components that implement [`Inspect`].
#[derive(Resource, Clone)]
pub struct InspectorRegistry {
    pub inspectors: HashMap<TypeId, InspectFn>,
}

impl InspectorRegistry {
    pub fn build() -> Self {
        let mut inspectors = HashMap::new();
        for entry in inventory::iter::<InspectEntry> {
            inspectors.insert((entry.type_id)(), entry.inspect_fn);
        }
        Self { inspectors }
    }

    pub fn get(&self, type_id: TypeId) -> Option<fn(&mut dyn Any, &mut egui::Ui)> {
        self.inspectors.get(&type_id).copied()
    }
}

/// Allows a component to be inspected and edited in the editor.
pub trait Inspect: 'static {
    fn inspect(&mut self, _ui: &mut egui::Ui) {}
}

/// Type-erased storage for a single component type, backed by a [`SparseSet`].
pub trait ComponentStorage: Any + Send + Sync {
    fn remove(&mut self, entity: Entity);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Returns true if this storage holds a component for the given entity.
    fn contains_entity(&self, entity: Entity) -> bool;
    /// Clones the component at `src` into `dst_storage` at `dst_entity`.
    /// Both storages must be of the same concrete type.
    fn clone_entity_into(&self, src: Entity, dst_entity: Entity, dst: &mut dyn ComponentStorage);
    /// Creates an empty storage of the same component type.
    fn make_empty(&self) -> Box<dyn ComponentStorage>;
}

impl<T: Component + Clone + 'static> ComponentStorage for SparseSet<T> {
    fn remove(&mut self, entity: Entity) {
        SparseSet::remove(self, entity); // explicit to avoid ambiguity with trait method
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn contains_entity(&self, entity: Entity) -> bool {
        self.contains(entity)
    }

    fn clone_entity_into(&self, src: Entity, dst_entity: Entity, dst: &mut dyn ComponentStorage) {
        if let Some(value) = self.get(src) {
            if let Some(typed) = dst.as_any_mut().downcast_mut::<SparseSet<T>>() {
                typed.insert(dst_entity, value.clone());
            }
        }
    }

    fn make_empty(&self) -> Box<dyn ComponentStorage> {
        Box::new(SparseSet::<T>::new())
    }
}
