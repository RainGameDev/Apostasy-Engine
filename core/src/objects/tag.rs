use std::any::Any;

pub trait Tag: TagContainer {
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

pub trait TagContainer {
    fn clone_box(&self) -> Box<dyn Tag>;
}

impl<T> TagContainer for T
where
    T: 'static + Tag + Clone,
{
    fn clone_box(&self) -> Box<dyn Tag> {
        Box::new(self.clone())
    }
}
impl Clone for Box<dyn Tag> {
    fn clone(&self) -> Box<dyn Tag> {
        self.clone_box()
    }
}

pub struct TagRegistration {
    pub type_name: &'static str,
    // pub serialize: fn(&dyn Tag) -> serde_yaml::Value,
    // pub deserialize: fn(serde_yaml::Value) -> Box<dyn Tag>,
    pub create: fn() -> Box<dyn Tag>,
}

inventory::collect!(TagRegistration);

pub fn get_tag_registration(type_name: &str) -> Option<&'static TagRegistration> {
    inventory::iter::<TagRegistration>()
        .find(|r| r.type_name.to_lowercase() == type_name.to_lowercase())
}
