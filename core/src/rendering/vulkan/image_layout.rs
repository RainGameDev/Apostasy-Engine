use ash::vk;

#[derive(Clone, Copy)]
pub struct ImageLayoutState {
    pub access_mask: vk::AccessFlags,
    pub layout: vk::ImageLayout,
    pub stage_mask: vk::PipelineStageFlags,
    pub queue_family_index: u32,
}

#[derive(Clone, Copy)]
pub struct ImageLayouts {
    pub undefined: ImageLayoutState,
    pub renderable: ImageLayoutState,
    pub present: ImageLayoutState,
    pub depth: ImageLayoutState,
    /// For depth images written as a depth attachment — covers EARLY and LATE fragment tests
    /// so barriers correctly wait for depth writes (which complete at LATE_FRAGMENT_TESTS).
    pub depth_write: ImageLayoutState,
    pub shader_read_only: ImageLayoutState,
    /// Correct layout for sampling a depth image in a shader.
    pub depth_stencil_read_only: ImageLayoutState,
}

impl Default for ImageLayouts {
    fn default() -> Self {
        let undefined = ImageLayoutState {
            layout: vk::ImageLayout::UNDEFINED,
            access_mask: vk::AccessFlags::empty(),
            stage_mask: vk::PipelineStageFlags::TOP_OF_PIPE,
            queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        };
        let renderable = ImageLayoutState {
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        };

        let present = ImageLayoutState {
            layout: vk::ImageLayout::PRESENT_SRC_KHR,
            access_mask: vk::AccessFlags::empty(),
            stage_mask: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        };
        let depth = ImageLayoutState {
            layout: vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        };
        let depth_write = ImageLayoutState {
            layout: vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            stage_mask: vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        };
        let shader_read_only = ImageLayoutState {
            layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            access_mask: vk::AccessFlags::SHADER_READ,
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        };
        let depth_stencil_read_only = ImageLayoutState {
            layout: vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL,
            access_mask: vk::AccessFlags::SHADER_READ,
            stage_mask: vk::PipelineStageFlags::FRAGMENT_SHADER,
            queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        };
        Self {
            undefined,
            renderable,
            present,
            depth,
            depth_write,
            shader_read_only,
            depth_stencil_read_only,
        }
    }
}
