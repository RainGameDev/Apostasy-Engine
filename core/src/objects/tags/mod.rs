use apostasy_macros::Tag;

pub mod skips_serilization;

#[derive(Tag, Clone)]
pub struct Player;

inventory::submit!(crate::objects::tag::TagRegistration {
    type_name: "Player",
    singleton: false,
    hidden: false,
    create: || Box::new(Player),
});
