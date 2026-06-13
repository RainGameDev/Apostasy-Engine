use apostasy_macros::Resource;

#[derive(Resource, Clone)]
pub struct ShadowDistance {
    pub distance: f32,
}

impl Default for ShadowDistance {
    fn default() -> Self {
        Self { distance: 128.0 }
    }
}
