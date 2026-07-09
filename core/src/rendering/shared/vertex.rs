use ash::vk;

pub trait VertexDefinition {
    fn get_binding_description() -> vk::VertexInputBindingDescription;
    fn get_attribute_descriptions() -> Vec<vk::VertexInputAttributeDescription>;
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    /// 32 blend weights for terrain.
    /// Passed as 8 × vec4 vertex attributes.
    pub weights: [f32; 32],
    /// Per-vertex RGB tint color. Multiplied over albedo in the terrain shader.
    /// Defaults to white [1,1,1] (no tint).
    pub color: [f32; 3],
    pub tangent: [f32; 4],
}

impl VertexDefinition for Vertex {
    fn get_binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
    }

    fn get_attribute_descriptions() -> Vec<vk::VertexInputAttributeDescription> {
        vec![
            // Position — location 0
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            // Normal — location 1
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(12),
            // Tex Coord — location 2
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(2)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(24),
            // Weights — 8 × vec4, locations 3–10, offsets 32–144
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(3)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(32),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(4)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(48),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(5)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(64),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(6)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(80),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(7)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(96),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(8)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(112),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(9)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(128),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(10)
                .format(vk::Format::R32G32B32A32_SFLOAT)
                .offset(144),
            // Color (RGB tint) — location 11, offset 160
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(11)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(160),
            // Tangent noraml map   location 12, offset 172
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(12)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(172),
        ]
    }
}
