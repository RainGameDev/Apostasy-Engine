use ash::vk;

#[derive(Clone, PartialEq)]
pub struct RenderingSettings {
    pub depth_settings: DepthSettings,
    pub rasterization_settings: RasterizationSettings,
    pub image_settings: ImageSettings,
    pub debug_settings: DebugSettings,

    pub default_vertex_shader: String,
    pub default_fragment_shader: String,
}

impl Default for RenderingSettings {
    fn default() -> Self {
        Self {
            depth_settings: DepthSettings::default(),
            rasterization_settings: RasterizationSettings::default(),
            image_settings: ImageSettings::default(),
            debug_settings: DebugSettings::default(),

            default_vertex_shader: "shader.vert".to_string(),
            default_fragment_shader: "shader.frag".to_string(),
        }
    }
}

/// The settings for a depth test
#[derive(Clone, Copy, PartialEq)]
pub struct DepthSettings {
    pub depth_test_enabled: bool,
    pub depth_compare_op: vk::CompareOp,
}

impl Default for DepthSettings {
    fn default() -> Self {
        Self {
            depth_test_enabled: true,
            depth_compare_op: vk::CompareOp::LESS,
        }
    }
}

/// The settings for rasterization
#[derive(Clone, Copy, PartialEq)]
pub struct RasterizationSettings {
    pub polygon_mode: vk::PolygonMode,
    pub cull_mode: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    pub line_width: f32,
}

impl Default for RasterizationSettings {
    fn default() -> Self {
        Self {
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            line_width: 1.0,
        }
    }
}

/// The settings for image sampling
#[derive(Clone, Copy, PartialEq)]
pub struct ImageSettings {
    pub filter_mode: vk::Filter,
    pub address_mode: vk::SamplerAddressMode,
    pub anisotropy_enabled: bool,
    pub anisotropy_amount: u8,
    pub mip_map_mode: vk::SamplerMipmapMode,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            filter_mode: vk::Filter::NEAREST,
            address_mode: vk::SamplerAddressMode::REPEAT,
            anisotropy_enabled: false,
            anisotropy_amount: 16,
            mip_map_mode: vk::SamplerMipmapMode::LINEAR,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct DebugSettings {
    pub collision_debug_enabled: bool,
    pub debug_line_width: f32,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self {
            collision_debug_enabled: false,
            debug_line_width: 1.0,
        }
    }
}
