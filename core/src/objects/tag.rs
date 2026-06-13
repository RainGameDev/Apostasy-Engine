use std::any::Any;

/// A trait that defines a tag that can be attached to an object.
pub trait Tag: TagContainer + Send + Sync {
    fn name() -> &'static str
    where
        Self: Sized;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
    fn type_name_static() -> &'static str
    where
        Self: Sized;
}

impl PartialEq for dyn Tag {
    fn eq(&self, other: &Self) -> bool {
        self.type_name() == other.type_name()
    }
}

impl Eq for dyn Tag {}

/// Wrapper for a workaround of object saftey.
pub trait TagContainer {
    fn clone_box(&self) -> Box<dyn Tag + Send + Sync>;
}

impl<T> TagContainer for T
where
    T: 'static + Tag + Clone + Send + Sync,
{
    fn clone_box(&self) -> Box<dyn Tag + Send + Sync> {
        Box::new(self.clone())
    }
}
impl Clone for Box<dyn Tag + Send + Sync> {
    fn clone(&self) -> Box<dyn Tag + Send + Sync> {
        self.clone_box()
    }
}

/// Contains all stored and registered tags.
/// Tags are registered on startup.
pub struct TagRegistration {
    pub type_name: &'static str,
    pub singleton: bool,
    pub hidden: bool,
    pub create: fn() -> Box<dyn Tag + Send + Sync>,
}

inventory::collect!(TagRegistration);

/// Takes in type [`type_name`] and returns the registered tag.
pub fn get_tag_registration(type_name: &str) -> Option<&'static TagRegistration> {
    inventory::iter::<TagRegistration>()
        .find(|r| r.type_name.to_lowercase() == type_name.to_lowercase())
}
