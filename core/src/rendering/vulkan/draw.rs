use anyhow::Result;
use ash::vk;

use crate::rendering::shared::model::GpuMesh;
use crate::rendering::shared::push_constants::{
    ModelPushConstants, PushConstants, VoxelPushConstants,
};
use crate::rendering::vulkan::VulkanRenderer;
use crate::voxels::texture_atlas::VoxelTextureAtlas;

impl VulkanRenderer {
    fn default_material_ds(&self) -> vk::DescriptorSet {
        self.default_white_material.descriptor_set
    }

    fn resolve_material_ds(&self, requested: Option<vk::DescriptorSet>) -> vk::DescriptorSet {
        requested
            .filter(|ds| *ds != vk::DescriptorSet::null())
            .unwrap_or_else(|| self.default_material_ds())
    }

    /// Binds the mesh's vertex/index buffers and issues the indexed draw.
    pub(crate) fn bind_mesh_and_draw(&self, mesh: &dyn GpuMesh) {
        let cmd = self.cmd();
        unsafe {
            self.context
                .device
                .cmd_bind_vertex_buffers(cmd, 0, &[mesh.get_vertex_buffer()], &[0]);
            self.context.device.cmd_bind_index_buffer(
                cmd,
                mesh.get_index_buffer(),
                0,
                vk::IndexType::UINT32,
            );
            self.context
                .device
                .cmd_draw_indexed(cmd, mesh.get_index_count(), 1, 0, 0, 0);
        }
    }

    /// Draws a mesh with the model pipeline layout: set 0 = lights, set 1 = `set1_ds`.
    fn model_pass_draw(
        &self,
        pipeline: vk::Pipeline,
        set1_ds: vk::DescriptorSet,
        data: &[u8],
        mesh: &dyn GpuMesh,
    ) {
        let cmd = self.cmd();
        unsafe {
            self.context
                .device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            self.context.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.light_descriptor_set, set1_ds],
                &[],
            );
            self.context.device.cmd_push_constants(
                cmd,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                data,
            );
        }
        self.bind_mesh_and_draw(mesh);
    }

    /// Draws a mesh with a voxel-style layout: set 0 = atlas, set 1 = lights.
    fn voxel_pass_draw(
        &self,
        pipeline: vk::Pipeline,
        layout: vk::PipelineLayout,
        atlas_ds: vk::DescriptorSet,
        data: &[u8],
        mesh: &dyn GpuMesh,
    ) {
        let cmd = self.cmd();
        unsafe {
            self.context
                .device
                .cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
            self.context.device.cmd_push_constants(
                cmd,
                layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                data,
            );
            self.context.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                0,
                &[atlas_ds],
                &[],
            );
            self.context.device.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                layout,
                1,
                &[self.light_descriptor_set],
                &[],
            );
        }
        self.bind_mesh_and_draw(mesh);
    }

    /// Depth-only draw for shadow passes: bind pipeline, push constants, draw.
    pub(crate) fn depth_pass_draw(
        &self,
        pipeline_key: &str,
        layout: vk::PipelineLayout,
        stages: vk::ShaderStageFlags,
        data: &[u8],
        mesh: &dyn GpuMesh,
    ) {
        let cmd = self.cmd();
        unsafe {
            self.context.device.cmd_bind_pipeline(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.get_pipeline(pipeline_key),
            );
            self.context
                .device
                .cmd_push_constants(cmd, layout, stages, 0, data);
        }
        self.bind_mesh_and_draw(mesh);
    }

    pub(crate) fn model_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        albedo_descriptor_set: Option<vk::DescriptorSet>,
        shader_override: Option<&str>,
        wireframe: bool,
    ) -> Result<()> {
        let material_ds = self.resolve_material_ds(albedo_descriptor_set);
        let pipeline = match shader_override {
            Some(name) => self.pipeline_manager.get_or_create_model_pipeline(
                name,
                &self.context.clone(),
                wireframe,
            )?,
            None => self.get_pipeline(if wireframe {
                "model::wireframe"
            } else {
                "model"
            }),
        };

        let mut data = push_constants.return_renderable();
        data.extend(model_push_constants.return_renderable());
        self.model_pass_draw(pipeline, material_ds, &data, &*mesh);
        Ok(())
    }

    pub(crate) fn collider_debug_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
    ) -> Result<()> {
        let mut data = push_constants.return_renderable();
        data.extend(model_push_constants.return_renderable());
        self.model_pass_draw(
            self.get_pipeline("model::collider_debug"),
            self.default_material_ds(),
            &data,
            &*mesh,
        );
        Ok(())
    }

    pub(crate) fn skybox_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        push_constants: PushConstants,
        model_push_constants: &ModelPushConstants,
        sky_descriptor_set: vk::DescriptorSet,
        additive: bool,
    ) -> Result<()> {
        let pipeline = self.get_pipeline(if additive {
            "skybox::additive"
        } else {
            "skybox"
        });
        let mut data = push_constants.return_renderable();
        data.extend(model_push_constants.return_renderable());
        self.model_pass_draw(pipeline, sky_descriptor_set, &data, &*mesh);
        Ok(())
    }

    pub(crate) fn voxel_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
        wireframe: bool,
    ) -> Result<()> {
        let mut data = push_constants.return_renderable();
        data.extend(voxel_push_constants.return_renderable());
        self.voxel_pass_draw(
            self.get_pipeline(if wireframe {
                "voxel::wireframe"
            } else {
                "voxel"
            }),
            self.voxel_pipeline_layout,
            atlas.descriptor_set,
            &data,
            &*mesh,
        );
        Ok(())
    }

    pub(crate) fn water_render(
        &mut self,
        mesh: Box<dyn GpuMesh>,
        atlas: &VoxelTextureAtlas,
        push_constants: &PushConstants,
        voxel_push_constants: &VoxelPushConstants,
    ) -> Result<()> {
        let mut data = push_constants.return_renderable();
        data.extend(voxel_push_constants.return_renderable());
        self.voxel_pass_draw(
            self.get_pipeline("water"),
            self.water_pipeline_layout,
            atlas.descriptor_set,
            &data,
            &*mesh,
        );
        Ok(())
    }
}
