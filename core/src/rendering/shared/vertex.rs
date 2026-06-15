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
    /// First texture-layer endpoint (flat-interpolated on terrain)
    pub tex_layer_a: f32,
    /// Second texture-layer endpoint (flat-interpolated on terrain)
    pub tex_layer_b: f32,
    /// Blend weight between layer_a and layer_b (smooth-interpolated)
    pub tex_blend: f32,
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
            // Position
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(0),
            // Normal
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(12),
            // Tex Coord
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(2)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(24),
            // tex_layer_a — flat for terrain shader
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(3)
                .format(vk::Format::R32_SFLOAT)
                .offset(32),
            // tex_layer_b — flat for terrain shader
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(4)
                .format(vk::Format::R32_SFLOAT)
                .offset(36),
            // tex_blend — smooth for terrain shader
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(5)
                .format(vk::Format::R32_SFLOAT)
                .offset(40),
        ]
    }
}
