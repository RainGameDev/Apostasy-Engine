use apostasy_macros::Tag;

#[derive(Tag, Clone)]
pub struct SkipsSerilization;

inventory::submit!(crate::objects::tag::TagRegistration {
    type_name: "SkipsSerilization",
    create: || Box::new(SkipsSerilization),
});
