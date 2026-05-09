#[derive(Clone, Debug)]
pub struct GpuTexture {
    pub name: String,
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub memory: vk::DeviceMemory,
    pub sampler: vk::Sampler,
    pub descriptor_set: vk::DescriptorSet,
}
use ash::vk;
