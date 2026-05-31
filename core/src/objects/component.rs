use std::any::{Any, TypeId};

use apostasy_macros::Resource;
use hashbrown::HashMap;

pub type BoxedComponent = Box<dyn Component + Send + Sync>;

pub trait Component: Send + Sync + 'static + ComponentContainer + std::fmt::Debug {
    fn name() -> &'static str
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
}

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

pub struct ComponentRegistration {
    pub type_name: &'static str,
    pub create: fn() -> BoxedComponent,
    pub deserialize: fn(&mut BoxedComponent, &serde_yaml::Value) -> anyhow::Result<()>,
}

inventory::collect!(ComponentRegistration);

pub fn get_component_registration(type_name: &str) -> Option<&'static ComponentRegistration> {
    inventory::iter::<ComponentRegistration>()
        .find(|r| r.type_name.to_lowercase() == type_name.to_lowercase())
}

/// Struct defining an inspectable compone
pub struct InspectEntry {
    pub type_id: fn() -> TypeId,
    pub inspect_fn: fn(&mut dyn Any, &mut crate::egui::Ui),
}

type InspectFn = fn(&mut dyn Any, &mut crate::egui::Ui);
inventory::collect!(InspectEntry);

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

pub trait Inspect: 'static {
    fn inspect(&mut self, _ui: &mut egui::Ui) {}
}
