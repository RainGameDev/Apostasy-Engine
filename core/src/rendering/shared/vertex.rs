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
    /// 6 blend weights for terrain splatting (used by terrain shader).
    /// Unused by GLTF meshes (set to 0).
    pub weights: [f32; 6],
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
            // Weights — locations 3 through 8
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(3)
                .format(vk::Format::R32_SFLOAT)
                .offset(32),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(4)
                .format(vk::Format::R32_SFLOAT)
                .offset(36),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(5)
                .format(vk::Format::R32_SFLOAT)
                .offset(40),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(6)
                .format(vk::Format::R32_SFLOAT)
                .offset(44),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(7)
                .format(vk::Format::R32_SFLOAT)
                .offset(48),
            vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(8)
                .format(vk::Format::R32_SFLOAT)
                .offset(52),
        ]
    }
}
