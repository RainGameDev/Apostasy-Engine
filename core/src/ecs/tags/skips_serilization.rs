use apostasy_macros::Tag;

#[derive(Tag, Clone)]
pub struct SkipsSerilization;

inventory::submit!(crate::ecs::tag::TagRegistration {
    type_name: "SkipsSerilization",
    singleton: false,
    hidden: false,
    type_id: std::any::TypeId::of::<SkipsSerilization>,
    create: || Box::new(SkipsSerilization),
    add_to_world: |world, id| world.add_tag::<SkipsSerilization>(id),
});
