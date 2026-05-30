use apostasy_macros::Resource;

pub mod anti_alisaing;
pub mod culling;
pub mod frustrum;
pub mod model;
pub mod push_constants;
pub mod rendering_settings;
pub mod texture;
pub mod vertex;

#[derive(Resource, Clone)]
pub struct UpdateRenderer;
