use apostasy_macros::Tag;

#[derive(Tag, Clone)]
pub struct SkipsSerilization;

inventory::submit!(crate::objects::tag::TagRegistration {
    type_name: "SkipsSerilization",
    singleton: false,
    hidden: false,
    create: || Box::new(SkipsSerilization),
});
