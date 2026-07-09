use apostasy_macros::Resource;

pub mod anti_aliasing;
pub mod culling;
pub mod frustum;
pub mod material;
pub mod model;
pub mod push_constants;
pub mod rendering_settings;
pub mod shadow_settings;
pub mod texture;
pub mod vertex;
pub mod wireframe;

#[derive(Resource, Clone)]
pub struct UpdateRenderer;
