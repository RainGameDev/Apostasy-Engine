use std::any::{Any, TypeId};

use apostasy_macros::Resource;
use hashbrown::HashMap;

pub type BoxedComponent = Box<dyn Component + Send + Sync>;

/// A trait that defines a component that can be attached to an object.
pub trait Component: Send + Sync + 'static + ComponentContainer + std::fmt::Debug {
    fn name() -> &'static str
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
}

/// Wrapper for a workaround of object safety.
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
/// Components are registered on startup.
pub struct ComponentRegistration {
    pub type_name: &'static str,
    pub create: fn() -> BoxedComponent,
    pub deserialize: fn(&mut BoxedComponent, &serde_yaml::Value) -> anyhow::Result<()>,
}

inventory::collect!(ComponentRegistration);

/// Takes in type [`type_name`] and returns the registered component.
pub fn get_component_registration(type_name: &str) -> Option<&'static ComponentRegistration> {
    inventory::iter::<ComponentRegistration>()
        .find(|r| r.type_name.to_lowercase() == type_name.to_lowercase())
}

/// Struct defining an inspectable component
pub struct InspectEntry {
    pub type_id: fn() -> TypeId,
    pub inspect_fn: fn(&mut dyn Any, &mut crate::egui::Ui),
}

type InspectFn = fn(&mut dyn Any, &mut crate::egui::Ui);
inventory::collect!(InspectEntry);

/// A hashmap that contains all components that impliment [`Inspect`].
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

/// A trait that defines how a component can be inspected in the editor.
/// ```
/// impl Inspect for Transform {
///     fn inspect(&mut self, ui: &mut egui::Ui) {
///         ui.vertical(|ui| {
///             ui.horizontal(|ui| {
///                 ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Position"));
///                 ui.add_sized(
///                     DRAG_SIZE,
///                     egui::DragValue::new(&mut self.local_position.x).speed(0.1),
///                 );
///                 ui.add_sized(
///                     DRAG_SIZE,
///                     egui::DragValue::new(&mut self.local_position.y).speed(0.1),
///                 );
///                 ui.add_sized(
///                     DRAG_SIZE,
///                     egui::DragValue::new(&mut self.local_position.z).speed(0.1),
///                 );
///             });
///         });
///     }
/// }```
///
pub trait Inspect: 'static {
    fn inspect(&mut self, _ui: &mut egui::Ui) {}
}
