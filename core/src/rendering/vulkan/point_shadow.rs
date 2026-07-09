use anyhow::Result;
use ash::vk;

use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ShadowPointModelPushConstants, ShadowPointVoxelPushConstants,
};
use crate::rendering::vulkan::VulkanRenderer;
use crate::rendering::vulkan::shadow::{
    create_depth_array_target, transition_depth_layers, write_shadow_descriptor,
};

impl VulkanRenderer {
    pub(crate) fn rebuild_point_shadow_map(&mut self, size: u32) -> Result<()> {
        if size == self.point_shadow_map_size {
            return Ok(());
        }
        unsafe {
            self.context.device.device_wait_idle()?;

            for view in &self.point_shadow_face_views {
                self.context.device.destroy_image_view(*view, None);
            }
            self.context
                .device
                .destroy_image_view(self.point_shadow_cube_view, None);
            self.context
                .device
                .destroy_image(self.point_shadow_image, None);
            self.context
                .device
                .free_memory(self.point_shadow_image_memory, None);
        }

        let target = create_depth_array_target(&self.context, size, 6, true)?;
        write_shadow_descriptor(
            &self.context,
            self.light_descriptor_set,
            2,
            target.array_view,
            self.point_shadow_sampler,
        );

        self.point_shadow_image = target.image;
        self.point_shadow_image_memory = target.memory;
        self.point_shadow_cube_view = target.array_view;
        self.point_shadow_face_views = target.layer_views.try_into().unwrap();
        self.point_shadow_map_size = size;
        Ok(())
    }

    pub(crate) fn begin_point_shadow_pass(
        &mut self,
        face: usize,
        bias_constant: f32,
        bias_slope: f32,
    ) -> Result<()> {
        // On face 0, transition all 6 layers from read-only to depth-write at once.
        if face == 0 {
            transition_depth_layers(&self.context, self.cmd(), self.point_shadow_image, 6, true);
        }

        let s = self.point_shadow_map_size;
        self.context.begin_depth_only_rendering(
            self.cmd(),
            self.point_shadow_face_views[face],
            vk::Rect2D::default().extent(vk::Extent2D {
                width: s,
                height: s,
            }),
        );

        self.set_shadow_pass_state(s, bias_constant, bias_slope);
        Ok(())
    }

    pub(crate) fn end_point_shadow_pass(&mut self, face: usize) -> Result<()> {
        unsafe {
            self.context.device.cmd_end_rendering(self.cmd());
        }

        // After the last face, transition all 6 layers back to shader-readable.
        if face == 5 {
            transition_depth_layers(&self.context, self.cmd(), self.point_shadow_image, 6, false);
        }
        Ok(())
    }

    pub(crate) fn shadow_point_model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointModelPushConstants,
    ) -> Result<()> {
        self.depth_pass_draw(
            "shadow_point_model",
            self.shadow_point_model_pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            &pc.return_renderable(),
            &*mesh,
        );
        Ok(())
    }

    pub(crate) fn shadow_point_voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        pc: &ShadowPointVoxelPushConstants,
    ) -> Result<()> {
        self.depth_pass_draw(
            "shadow_point_voxel",
            self.shadow_point_voxel_pipeline_layout,
            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
            &pc.return_renderable(),
            &*mesh,
        );
        Ok(())
    }
}
