use apostasy_macros::Resource;

#[derive(Resource, Clone)]
pub struct ShadowDistance {
    pub distance: f32,
    /// Number of shadow cascades to use (1–4). 1 disables CSM (single map).
    pub cascade_count: usize,
    /// Vulkan depth bias constant factor (added to every shadow fragment).
    pub bias_constant: f32,
    /// Vulkan depth bias slope factor (scales with polygon slope relative to light).
    pub bias_slope: f32,
    /// Shadow map texture size per cascade (must be a power of two: 512, 1024, 2048, 4096).
    pub shadow_map_size: u32,
}

impl Default for ShadowDistance {
    fn default() -> Self {
        Self {
            distance: 128.0,
            cascade_count: 4,
            bias_constant: 2.0,
            bias_slope: 2.0,
            shadow_map_size: 2048,
        }
    }
}
