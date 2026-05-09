use apostasy_macros::Resource;
use egui::Context;

#[derive(Resource, Clone)]
pub struct EguiContext(pub Context);
