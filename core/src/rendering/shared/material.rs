use crate::rendering::shared::texture::GpuTexture;

#[derive(Clone, Debug)]
pub struct GpuMaterial {
    pub albedo: Option<GpuTexture>,
    pub normal: Option<GpuTexture>,
    pub color: [f32; 4],
    pub shader: Option<String>,
    pub descriptor_set: ash::vk::DescriptorSet,
}
