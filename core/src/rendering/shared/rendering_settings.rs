use ash::vk::{
    self, BlendFactor, BlendOp, ColorComponentFlags, DynamicState, Extent2D, Format,
    PipelineColorBlendAttachmentState, PrimitiveTopology, ShaderModule,
    VertexInputAttributeDescription, VertexInputBindingDescription,
};

#[derive(Clone)]
pub struct PipelineOptions {
    pub image_extent: Extent2D,
    pub image_format: Format,
    pub depth_format: Option<Format>,
    pub vertex_shader: ShaderModule,
    pub fragment_shader: ShaderModule,
    pub vertex_bindings: Vec<VertexInputBindingDescription>,
    pub vertex_attributes: Vec<VertexInputAttributeDescription>,
}

#[derive(Clone)]
pub struct RenderingSettings {
    pub depth_settings: DepthSettings,
    pub rasterization_settings: RasterizationSettings,
    pub image_settings: ImageSettings,
    pub debug_settings: DebugSettings,
    pub color_blend_settings: ColorBlendSettings,
    pub dynamic_state_settings: DynamicStateSettings,
    pub primitive_topology_settings: PrimitiveTopologySettings,

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
            color_blend_settings: ColorBlendSettings::default(),
            dynamic_state_settings: DynamicStateSettings::default(),
            primitive_topology_settings: PrimitiveTopologySettings::default(),

            default_vertex_shader: "shader.vert".to_string(),
            default_fragment_shader: "shader.frag".to_string(),
        }
    }
}

/// The settings for the primitive topology
#[derive(Clone, Copy)]
pub struct PrimitiveTopologySettings {
    pub primitive_topology: PrimitiveTopology,
}

impl Default for PrimitiveTopologySettings {
    fn default() -> Self {
        Self {
            primitive_topology: PrimitiveTopology::TRIANGLE_LIST,
        }
    }
}

/// The settings for the blending
#[derive(Clone, Copy)]
pub struct ColorBlendSettings {
    pub blend_attachment: PipelineColorBlendAttachmentState,
}

impl Default for ColorBlendSettings {
    fn default() -> Self {
        Self {
            blend_attachment: PipelineColorBlendAttachmentState::default()
                .color_write_mask(ColorComponentFlags::RGBA)
                .blend_enable(true)
                .src_color_blend_factor(BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(BlendOp::ADD)
                .src_alpha_blend_factor(BlendFactor::ONE)
                .dst_alpha_blend_factor(BlendFactor::ZERO)
                .alpha_blend_op(BlendOp::ADD),
        }
    }
}

/// The settings for the dynamic states
#[derive(Clone)]
pub struct DynamicStateSettings {
    pub dynamic_states: Vec<DynamicState>,
}

impl Default for DynamicStateSettings {
    fn default() -> Self {
        Self {
            dynamic_states: vec![DynamicState::VIEWPORT, DynamicState::SCISSOR],
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
