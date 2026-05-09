use ash::vk::{CommandBuffer, Fence, Semaphore};

pub struct VulkanFrame {
    pub command_buffer: CommandBuffer,
    pub image_available_semaphore: Semaphore,
    pub render_finished_semaphore: Semaphore,
    pub in_flight_fence: Fence,
}
