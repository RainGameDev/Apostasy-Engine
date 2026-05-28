use std::collections::HashSet;
use std::io;

use anyhow::Result;
use apostasy_macros::Resource;
use ash::Device;
use ash::Entry;
use ash::Instance;
use ash::khr::*;
use ash::vk;
use ash::vk::ApplicationInfo;
use ash::vk::AttachmentLoadOp;
use ash::vk::AttachmentStoreOp;
use ash::vk::BlendFactor;
use ash::vk::BlendOp;
use ash::vk::Buffer;
use ash::vk::BufferCopy;
use ash::vk::BufferCreateInfo;
use ash::vk::BufferUsageFlags;
use ash::vk::ClearColorValue;
use ash::vk::ClearDepthStencilValue;
use ash::vk::ClearValue;
use ash::vk::ColorComponentFlags;
use ash::vk::CommandBuffer;
use ash::vk::CommandBufferAllocateInfo;
use ash::vk::CommandBufferBeginInfo;
use ash::vk::CommandBufferLevel;
use ash::vk::CommandBufferUsageFlags;
use ash::vk::CommandPool;
use ash::vk::CompareOp;
use ash::vk::CullModeFlags;
use ash::vk::DependencyFlags;
use ash::vk::DeviceCreateInfo;
use ash::vk::DeviceMemory;
use ash::vk::DeviceQueueCreateInfo;
use ash::vk::DeviceSize;
use ash::vk::DynamicState;
use ash::vk::Extent2D;
use ash::vk::Extent3D;
use ash::vk::Fence;
use ash::vk::Format;
use ash::vk::FrontFace;
use ash::vk::GraphicsPipelineCreateInfo;
use ash::vk::Image;
use ash::vk::ImageAspectFlags;
use ash::vk::ImageCreateInfo;
use ash::vk::ImageLayout;
use ash::vk::ImageMemoryBarrier;
use ash::vk::ImageSubresourceRange;
use ash::vk::ImageTiling;
use ash::vk::ImageType;
use ash::vk::ImageUsageFlags;
use ash::vk::ImageView;
use ash::vk::ImageViewCreateInfo;
use ash::vk::ImageViewType;
use ash::vk::InstanceCreateInfo;
use ash::vk::MemoryAllocateInfo;
use ash::vk::MemoryMapFlags;
use ash::vk::MemoryPropertyFlags;
use ash::vk::Offset2D;
use ash::vk::PhysicalDeviceBufferDeviceAddressFeatures;
use ash::vk::PhysicalDeviceDynamicRenderingFeatures;
use ash::vk::Pipeline;
use ash::vk::PipelineCache;
use ash::vk::PipelineColorBlendAttachmentState;
use ash::vk::PipelineColorBlendStateCreateInfo;
use ash::vk::PipelineDepthStencilStateCreateInfo;
use ash::vk::PipelineDynamicStateCreateInfo;
use ash::vk::PipelineInputAssemblyStateCreateInfo;
use ash::vk::PipelineLayout;
use ash::vk::PipelineMultisampleStateCreateInfo;
use ash::vk::PipelineRasterizationStateCreateInfo;
use ash::vk::PipelineRenderingCreateInfo;
use ash::vk::PipelineShaderStageCreateInfo;
use ash::vk::PipelineVertexInputStateCreateInfo;
use ash::vk::PipelineViewportStateCreateInfo;
use ash::vk::PolygonMode;
use ash::vk::PrimitiveTopology;
use ash::vk::Queue;
use ash::vk::Rect2D;
use ash::vk::RenderPass;
use ash::vk::RenderingAttachmentInfo;
use ash::vk::RenderingInfo;
use ash::vk::SampleCountFlags;
use ash::vk::ShaderModule;
use ash::vk::ShaderModuleCreateInfo;
use ash::vk::ShaderStageFlags;
use ash::vk::SharingMode;
use ash::vk::SubmitInfo;
use ash::vk::Viewport;
use hashbrown::HashMap;
use winit::raw_window_handle::HasDisplayHandle;
use winit::raw_window_handle::HasWindowHandle;
use winit::window::Window;

use crate::rendering::shared::vertex::Vertex;
use crate::rendering::shared::vertex::VertexDefinition;
use crate::rendering::vulkan::device::PhysicalDevice;
use crate::rendering::vulkan::image_layout::ImageLayoutState;
use crate::rendering::vulkan::queue_family::QueueFamilies;
use crate::rendering::vulkan::queue_family::QueueFamily;
use crate::rendering::vulkan::queue_family::QueueFamilyPicker;
use crate::rendering::vulkan::surface::Surface;
use crate::voxels::meshes::VoxelVertex;

pub struct RenderingContextAttributes<'window> {
    pub compatability_window: &'window Window,
    pub queue_family_picker: QueueFamilyPicker,
}

#[derive(Clone, Resource)]
pub struct VulkanRenderingContext {
    pub buffer_graveyard: Vec<(vk::Buffer, vk::DeviceMemory)>,
    pub command_pool: CommandPool,
    pub queues: HashMap<u32, Queue>,
    pub device: Device,
    pub physical_device: PhysicalDevice,
    pub queue_family_indices: HashSet<u32>,
    pub queue_families: QueueFamilies,
    pub surface_extension: surface::Instance,
    pub instance: Instance,
    pub entry: Entry,
    pub swapchain_extension: swapchain::Device,
}

pub struct GraphicsPipelineSettings {
    pub vertex_shader: ShaderModule,
    pub fragment_shader: ShaderModule,
    pub vertex_bindings: Vec<vk::VertexInputBindingDescription>,
    pub vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    pub primitive_topology: PrimitiveTopology,
    pub cull_mode: CullModeFlags,
    pub front_face: FrontFace,
    pub polygon_mode: PolygonMode,
    pub line_width: f32,
    pub depth_test_enable: bool,
    pub depth_write_enable: bool,
    pub depth_compare_op: CompareOp,
    pub blend_attachment: PipelineColorBlendAttachmentState,
    pub image_extent: Extent2D,
    pub image_format: Format,
    pub depth_format: Option<Format>,
    pub pipeline_layout: PipelineLayout,
    pub dynamic_states: Vec<DynamicState>,
}

impl GraphicsPipelineSettings {
    pub fn new(
        vertex_shader: ShaderModule,
        fragment_shader: ShaderModule,
        image_extent: Extent2D,
        image_format: Format,
        depth_format: Option<Format>,
        pipeline_layout: PipelineLayout,
        vertex_bindings: Vec<vk::VertexInputBindingDescription>,
        vertex_attributes: Vec<vk::VertexInputAttributeDescription>,
    ) -> Self {
        Self {
            vertex_shader,
            fragment_shader,
            vertex_bindings,
            vertex_attributes,
            primitive_topology: PrimitiveTopology::TRIANGLE_LIST,
            cull_mode: CullModeFlags::NONE,
            front_face: FrontFace::CLOCKWISE,
            polygon_mode: PolygonMode::FILL,
            line_width: 1.0,
            depth_test_enable: true,
            depth_write_enable: true,
            depth_compare_op: CompareOp::LESS,
            blend_attachment: PipelineColorBlendAttachmentState::default()
                .color_write_mask(ColorComponentFlags::RGBA)
                .blend_enable(true)
                .src_color_blend_factor(BlendFactor::SRC_ALPHA)
                .dst_color_blend_factor(BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(BlendOp::ADD)
                .src_alpha_blend_factor(BlendFactor::ONE)
                .dst_alpha_blend_factor(BlendFactor::ZERO)
                .alpha_blend_op(BlendOp::ADD),
            image_extent,
            image_format,
            depth_format,
            pipeline_layout,
            dynamic_states: vec![DynamicState::VIEWPORT, DynamicState::SCISSOR],
        }
    }

    pub fn wireframe(mut self) -> Self {
        self.polygon_mode = PolygonMode::LINE;
        self.line_width = 1.0;
        self
    }
}

impl VulkanRenderingContext {
    pub fn new(attributes: RenderingContextAttributes) -> Result<VulkanRenderingContext> {
        unsafe {
            let entry = Entry::load()?;

            let raw_display_handle = attributes.compatability_window.display_handle()?.as_raw();
            let raw_window_handle = attributes.compatability_window.window_handle()?.as_raw();

            let instance = entry.create_instance(
                &InstanceCreateInfo::default()
                    .application_info(&ApplicationInfo::default().api_version(vk::API_VERSION_1_3))
                    .enabled_extension_names(ash_window::enumerate_required_extensions(
                        raw_display_handle,
                    )?),
                None,
            )?;

            let surface_extension = ash::khr::surface::Instance::new(&entry, &instance);
            let compatability_surface = ash_window::create_surface(
                &entry,
                &instance,
                raw_display_handle,
                raw_window_handle,
                None,
            )?;

            let mut physical_devices = instance
                .enumerate_physical_devices()?
                .into_iter()
                .map(|handle| {
                    let properties = instance.get_physical_device_properties(handle);
                    let features = instance.get_physical_device_features(handle);
                    let memory_properties = instance.get_physical_device_memory_properties(handle);
                    let queue_family_properties =
                        instance.get_physical_device_queue_family_properties(handle);

                    let queue_families = queue_family_properties
                        .into_iter()
                        .enumerate()
                        .map(|(index, properties)| QueueFamily {
                            index: index as u32,
                            properties,
                        })
                        .collect::<Vec<_>>();

                    PhysicalDevice {
                        handle,
                        properties,
                        features,
                        memory_properties,
                        queue_families,
                    }
                })
                .collect::<Vec<_>>();

            physical_devices.retain(|device| {
                surface_extension
                    .get_physical_device_surface_support(device.handle, 0, compatability_surface)
                    .unwrap_or(false)
            });

            surface_extension.destroy_surface(compatability_surface, None);

            let (physical_device, queue_family) =
                (attributes.queue_family_picker)(physical_devices)?;

            let queue_family_indices = HashSet::from([
                queue_family.graphics,
                queue_family.present,
                queue_family.transfer,
                queue_family.compute,
            ]);

            let queue_create_infos = queue_family_indices
                .iter()
                .copied()
                .map(|index| {
                    DeviceQueueCreateInfo::default()
                        .queue_family_index(index)
                        .queue_priorities(&[1.0])
                })
                .collect::<Vec<_>>();

            let device = instance.create_device(
                physical_device.handle,
                &DeviceCreateInfo::default()
                    .queue_create_infos(&queue_create_infos)
                    .enabled_extension_names(&[ash::khr::swapchain::NAME.as_ptr()])
                    .push_next(
                        &mut PhysicalDeviceDynamicRenderingFeatures::default()
                            .dynamic_rendering(true),
                    )
                    .push_next(
                        &mut PhysicalDeviceBufferDeviceAddressFeatures::default()
                            .buffer_device_address(true),
                    ),
                None,
            )?;

            let swapchain_extension = ash::khr::swapchain::Device::new(&instance, &device);

            let queues = queue_family_indices
                .iter()
                .map(|&index| (index, device.get_device_queue(index, 0)))
                .collect::<HashMap<_, _>>();

            let command_pool = device.create_command_pool(
                &ash::vk::CommandPoolCreateInfo::default()
                    .queue_family_index(queue_family.graphics)
                    .flags(ash::vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )?;

            Ok(Self {
                queues,
                device,
                queue_family_indices,
                queue_families: queue_family,
                physical_device,
                surface_extension,
                instance,
                entry,
                swapchain_extension,
                command_pool,
                buffer_graveyard: Vec::new(),
            })
        }
    }

    // unsafe because the window should outlive the surface
    pub fn create_surface(&self, window: &Window) -> Result<Surface> {
        unsafe {
            let raw_display_handle = window.display_handle()?.as_raw();
            let raw_window_handle = window.window_handle()?.as_raw();
            let handle = ash_window::create_surface(
                &self.entry,
                &self.instance,
                raw_display_handle,
                raw_window_handle,
                None,
            )?;

            let capabilities = self
                .surface_extension
                .get_physical_device_surface_capabilities(self.physical_device.handle, handle)?;

            let formats = self
                .surface_extension
                .get_physical_device_surface_formats(self.physical_device.handle, handle)?;

            let present_modes = self
                .surface_extension
                .get_physical_device_surface_present_modes(self.physical_device.handle, handle)?;

            Ok(Surface {
                handle,
                capabilities,
                formats,
                present_modes,
            })
        }
    }
    pub fn find_memory_type(&self, filter: u32, properties: MemoryPropertyFlags) -> Result<u32> {
        // First, try to find an exact match
        for i in 0..self.physical_device.memory_properties.memory_type_count {
            if (filter & (1 << i)) != 0
                && (self.physical_device.memory_properties.memory_types[i as usize].property_flags
                    & properties)
                    == properties
            {
                return Ok(i);
            }
        }

        // Fallback: find any memory type that matches the filter
        for i in 0..self.physical_device.memory_properties.memory_type_count {
            if (filter & (1 << i)) != 0 {
                return Ok(i);
            }
        }

        Err(anyhow::anyhow!(
            "Failed to find suitable memory type with filter: {}",
            filter
        ))
    }
    pub fn create_image(
        &self,
        extent: Extent2D,
        format: Format,
        tiling: ImageTiling,
        usage: ImageUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<(Image, DeviceMemory)> {
        let image_info = ImageCreateInfo::default()
            .image_type(ImageType::TYPE_2D)
            .extent(Extent3D {
                width: extent.width,
                height: extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(tiling)
            .initial_layout(ImageLayout::UNDEFINED)
            .usage(usage)
            .samples(SampleCountFlags::TYPE_1)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let image = unsafe { self.device.create_image(&image_info, None).unwrap() };
        let mem_reqs = unsafe { self.device.get_image_memory_requirements(image) };

        let alloc_info = MemoryAllocateInfo::default()
            .allocation_size(mem_reqs.size)
            .memory_type_index(self.find_memory_type(mem_reqs.memory_type_bits, properties)?);

        let memory = unsafe { self.device.allocate_memory(&alloc_info, None).unwrap() };
        unsafe { self.device.bind_image_memory(image, memory, 0).unwrap() };
        Ok((image, memory))
    }
    pub fn create_image_view(
        &self,
        image: Image,
        format: Format,
        aspect_flags: ImageAspectFlags,
    ) -> Result<ImageView> {
        let image_view = unsafe {
            self.device.create_image_view(
                &ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(
                        ImageSubresourceRange::default()
                            .aspect_mask(aspect_flags)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1),
                    ),
                None,
            )
        }?;
        Ok(image_view)
    }

    pub fn create_shader_module(&self, code: &[u8]) -> Result<ShaderModule, vk::Result> {
        let mut code = io::Cursor::new(code);
        let code = ash::util::read_spv(&mut code).unwrap();
        let create_info = ShaderModuleCreateInfo::default().code(&code);
        let shader_module = unsafe { self.device.create_shader_module(&create_info, None) }?;
        Ok(shader_module)
    }

    pub fn create_graphics_pipeline_with_settings(
        &self,
        settings: GraphicsPipelineSettings,
    ) -> Result<Pipeline> {
        let entry_point = std::ffi::CString::new("main").unwrap();
        let attachment_formats = [settings.image_format];
        let mut render_info = PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&attachment_formats);
        if let Some(depth_format) = settings.depth_format {
            render_info = render_info.depth_attachment_format(depth_format);
        }

        unsafe {
            Ok(self
                .device
                .create_graphics_pipelines(
                    PipelineCache::null(),
                    &[GraphicsPipelineCreateInfo::default()
                        .stages(&[
                            PipelineShaderStageCreateInfo::default()
                                .stage(ShaderStageFlags::VERTEX)
                                .module(settings.vertex_shader)
                                .name(&entry_point),
                            PipelineShaderStageCreateInfo::default()
                                .stage(ShaderStageFlags::FRAGMENT)
                                .module(settings.fragment_shader)
                                .name(&entry_point),
                        ])
                        .vertex_input_state(
                            &PipelineVertexInputStateCreateInfo::default()
                                .vertex_binding_descriptions(&settings.vertex_bindings)
                                .vertex_attribute_descriptions(&settings.vertex_attributes),
                        )
                        .input_assembly_state(
                            &PipelineInputAssemblyStateCreateInfo::default()
                                .topology(settings.primitive_topology),
                        )
                        .viewport_state(
                            &PipelineViewportStateCreateInfo::default()
                                .viewports(&[Viewport {
                                    x: 0.0,
                                    y: 0.0,
                                    width: settings.image_extent.width as f32,
                                    height: settings.image_extent.height as f32,
                                    min_depth: 0.0,
                                    max_depth: 1.0,
                                }])
                                .scissors(&[Rect2D {
                                    offset: Offset2D { x: 0, y: 0 },
                                    extent: settings.image_extent,
                                }]),
                        )
                        .rasterization_state(
                            &PipelineRasterizationStateCreateInfo::default()
                                .depth_clamp_enable(false)
                                .rasterizer_discard_enable(false)
                                .polygon_mode(settings.polygon_mode)
                                .cull_mode(settings.cull_mode)
                                .front_face(settings.front_face)
                                .depth_bias_enable(false)
                                .line_width(settings.line_width),
                        )
                        .multisample_state(
                            &PipelineMultisampleStateCreateInfo::default()
                                .rasterization_samples(SampleCountFlags::TYPE_1)
                                .sample_shading_enable(false),
                        )
                        .color_blend_state(
                            &PipelineColorBlendStateCreateInfo::default()
                                .attachments(&[settings.blend_attachment]),
                        )
                        .dynamic_state(
                            &PipelineDynamicStateCreateInfo::default()
                                .dynamic_states(&settings.dynamic_states),
                        )
                        .depth_stencil_state(
                            &PipelineDepthStencilStateCreateInfo::default()
                                .depth_test_enable(settings.depth_test_enable)
                                .depth_write_enable(settings.depth_write_enable)
                                .depth_compare_op(settings.depth_compare_op),
                        )
                        .layout(settings.pipeline_layout)
                        .render_pass(RenderPass::null())
                        .push_next(&mut render_info)],
                    None,
                )
                .unwrap()
                .into_iter()
                .next()
                .unwrap())
        }
    }

    pub fn create_graphics_pipeline(
        &self,
        vertex_shader: ShaderModule,
        fragment_shader: ShaderModule,
        image_extent: Extent2D,
        image_format: Format,
        depth_format: Format,
        pipeline_layout: PipelineLayout,
        _pipeline_chache: PipelineCache,
    ) -> Result<Pipeline> {
        let settings = GraphicsPipelineSettings::new(
            vertex_shader,
            fragment_shader,
            image_extent,
            image_format,
            Some(depth_format),
            pipeline_layout,
            vec![Vertex::get_binding_description()],
            Vertex::get_attribute_descriptions(),
        );
        self.create_graphics_pipeline_with_settings(settings)
    }

    pub fn create_voxel_graphics_pipeline(
        &self,
        vertex_shader: ShaderModule,
        fragment_shader: ShaderModule,
        image_extent: Extent2D,
        image_format: Format,
        depth_format: Format,
        pipeline_layout: PipelineLayout,
        _pipeline_chache: PipelineCache,
    ) -> Result<Pipeline> {
        let settings = GraphicsPipelineSettings::new(
            vertex_shader,
            fragment_shader,
            image_extent,
            image_format,
            Some(depth_format),
            pipeline_layout,
            vec![VoxelVertex::get_binding_description()],
            VoxelVertex::get_attribute_descriptions(),
        );
        self.create_graphics_pipeline_with_settings(settings)
    }

    pub fn create_water_graphics_pipeline(
        &self,
        vertex_shader: ShaderModule,
        fragment_shader: ShaderModule,
        image_extent: Extent2D,
        image_format: Format,
        depth_format: Format,
        pipeline_layout: PipelineLayout,
        _pipeline_chache: PipelineCache,
    ) -> Result<Pipeline> {
        let settings = GraphicsPipelineSettings::new(
            vertex_shader,
            fragment_shader,
            image_extent,
            image_format,
            Some(depth_format),
            pipeline_layout,
            vec![VoxelVertex::get_binding_description()],
            VoxelVertex::get_attribute_descriptions(),
        );
        self.create_graphics_pipeline_with_settings(settings)
    }

    pub fn create_wireframe_pipeline(
        &self,
        vertex_shader: ShaderModule,
        fragment_shader: ShaderModule,
        image_extent: Extent2D,
        image_format: Format,
        depth_format: Format,
        pipeline_layout: PipelineLayout,
        _pipeline_chache: PipelineCache,
    ) -> Result<Pipeline> {
        let settings = GraphicsPipelineSettings::new(
            vertex_shader,
            fragment_shader,
            image_extent,
            image_format,
            Some(depth_format),
            pipeline_layout,
            vec![Vertex::get_binding_description()],
            Vertex::get_attribute_descriptions(),
        )
        .wireframe();
        self.create_graphics_pipeline_with_settings(settings)
    }
    pub fn create_voxel_wireframe_pipeline(
        &self,
        vertex_shader: ShaderModule,
        fragment_shader: ShaderModule,
        image_extent: Extent2D,
        image_format: Format,
        _depth_format: Format,
        pipeline_layout: PipelineLayout,
        _pipeline_chache: PipelineCache,
    ) -> Result<Pipeline> {
        let settings = GraphicsPipelineSettings::new(
            vertex_shader,
            fragment_shader,
            image_extent,
            image_format,
            None,
            pipeline_layout,
            vec![VoxelVertex::get_binding_description()],
            VoxelVertex::get_attribute_descriptions(),
        )
        .wireframe();
        self.create_graphics_pipeline_with_settings(settings)
    }

    pub fn transition_image_layout(
        &self,
        command_buffer: CommandBuffer,
        image: Image,
        old_state: ImageLayoutState,
        new_state: ImageLayoutState,
        aspect_mask: ImageAspectFlags,
    ) {
        let image_memory_barrier = ImageMemoryBarrier::default()
            .old_layout(old_state.layout)
            .new_layout(new_state.layout)
            .image(image)
            .src_access_mask(old_state.access_mask)
            .dst_access_mask(new_state.access_mask)
            .subresource_range(
                ImageSubresourceRange::default()
                    .aspect_mask(aspect_mask)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1),
            );

        unsafe {
            self.device.cmd_pipeline_barrier(
                command_buffer,
                old_state.stage_mask,
                new_state.stage_mask,
                DependencyFlags::empty(),
                &[],
                &[],
                &[image_memory_barrier],
            );
        }
    }

    pub fn begin_rendering(
        &self,
        command_buffer: CommandBuffer,
        view: ImageView,
        depth_view: ImageView,
        clear_color: ClearColorValue,
        render_area: Rect2D,
    ) {
        unsafe {
            self.device.cmd_begin_rendering(
                command_buffer,
                &RenderingInfo::default()
                    .layer_count(1)
                    .color_attachments(&[RenderingAttachmentInfo::default()
                        .image_view(view)
                        .image_layout(ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .clear_value(ClearValue { color: clear_color })
                        .load_op(AttachmentLoadOp::CLEAR)
                        .store_op(AttachmentStoreOp::STORE)])
                    .depth_attachment(
                        &RenderingAttachmentInfo::default()
                            .image_view(depth_view)
                            .image_layout(ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                            .clear_value(ClearValue {
                                depth_stencil: ClearDepthStencilValue {
                                    depth: 1.0,
                                    stencil: 0,
                                },
                            })
                            .load_op(AttachmentLoadOp::CLEAR)
                            .store_op(AttachmentStoreOp::STORE),
                    )
                    .render_area(render_area),
            );
        }
    }

    /// Creates a texture descriptor set
    pub fn create_texture_descriptor_set(
        &self,
        descriptor_pool: vk::DescriptorPool,
        descriptor_set_layout: vk::DescriptorSetLayout,
        image_view: vk::ImageView,
        sampler: vk::Sampler,
    ) -> vk::DescriptorSet {
        unsafe {
            let set = self
                .device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(descriptor_pool)
                        .set_layouts(&[descriptor_set_layout]),
                )
                .unwrap()[0];

            let image_info = vk::DescriptorImageInfo::default()
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                .image_view(image_view)
                .sampler(sampler);

            self.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::default()
                    .dst_set(set)
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[image_info])],
                &[],
            );

            set
        }
    }

    pub fn begin_single_time_commands(&self, command_pool: CommandPool) -> CommandBuffer {
        let alloc_info = CommandBufferAllocateInfo::default()
            .level(CommandBufferLevel::PRIMARY)
            .command_pool(command_pool)
            .command_buffer_count(1);

        let cmd_buf = unsafe { self.device.allocate_command_buffers(&alloc_info).unwrap()[0] };
        let begin_info =
            CommandBufferBeginInfo::default().flags(CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device
                .begin_command_buffer(cmd_buf, &begin_info)
                .unwrap()
        };
        cmd_buf
    }

    pub fn end_single_time_commands(
        &self,
        cmd_buf: CommandBuffer,
        queue: Queue,
        command_pool: CommandPool,
    ) {
        unsafe {
            if let Err(e) = self.device.end_command_buffer(cmd_buf) {
                eprintln!("Failed to end command buffer: {}", e);
                return;
            }

            let buffer = &[cmd_buf];
            let submit_info = SubmitInfo::default().command_buffers(buffer);
            if let Err(e) = self
                .device
                .queue_submit(queue, &[submit_info], Fence::null())
            {
                eprintln!("Failed to submit queue: {}", e);
                self.device.free_command_buffers(command_pool, &[cmd_buf]);
                return;
            }

            if let Err(e) = self.device.queue_wait_idle(queue) {
                eprintln!("Failed to wait for queue idle: {}", e);
                self.device.free_command_buffers(command_pool, &[cmd_buf]);
                return;
            }

            self.device.free_command_buffers(command_pool, &[cmd_buf]);
        }
    }
    pub fn create_buffer(
        &self,
        size: DeviceSize,
        usage: BufferUsageFlags,
        properties: MemoryPropertyFlags,
    ) -> Result<(Buffer, DeviceMemory)> {
        let buffer_info = BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let buffer = unsafe { self.device.create_buffer(&buffer_info, None)? };
        let mem_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        let alloc_info = MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(
                self.find_memory_type(mem_requirements.memory_type_bits, properties)?,
            );

        let memory = unsafe { self.device.allocate_memory(&alloc_info, None)? };

        unsafe { self.device.bind_buffer_memory(buffer, memory, 0)? };

        Ok((buffer, memory))
    }

    pub fn create_vertex_buffer<T: VertexDefinition>(
        &self,
        vertices: &[T],
        command_pool: CommandPool,
    ) -> Result<(Buffer, vk::DeviceMemory)> {
        let buffer_size = (size_of::<T>() * vertices.len()) as DeviceSize;

        // Create staging buffer
        let (staging_buffer, staging_memory) = self.create_buffer(
            buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            let data_ptr =
                self.device
                    .map_memory(staging_memory, 0, buffer_size, MemoryMapFlags::empty())?
                    as *mut T;
            data_ptr.copy_from_nonoverlapping(vertices.as_ptr(), vertices.len());
            self.device.unmap_memory(staging_memory);
        }

        // Create device local buffer
        let buffer_info = BufferCreateInfo::default()
            .size(buffer_size)
            .usage(BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let buffer = unsafe { self.device.create_buffer(&buffer_info, None)? };
        let mem_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        let alloc_info = MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(self.find_memory_type(
                mem_requirements.memory_type_bits,
                MemoryPropertyFlags::DEVICE_LOCAL,
            )?);

        let buffer_memory = unsafe { self.device.allocate_memory(&alloc_info, None)? };
        unsafe { self.device.bind_buffer_memory(buffer, buffer_memory, 0)? };

        // Copy staging -> device local
        let cmd = self.begin_single_time_commands(command_pool);
        unsafe {
            let copy_region = BufferCopy::default().size(buffer_size);
            self.device
                .cmd_copy_buffer(cmd, staging_buffer, buffer, &[copy_region]);
        }
        let queue = self.queues[&self.queue_families.transfer];
        self.end_single_time_commands(cmd, queue, command_pool);

        unsafe {
            self.device.destroy_buffer(staging_buffer, None);
            self.device.free_memory(staging_memory, None);
        }

        Ok((buffer, buffer_memory))
    }

    /// Creates an index buffer from a slice of indices
    pub fn create_index_buffer(
        &self,
        indices: &[u32],
        command_pool: CommandPool,
    ) -> Result<(Buffer, vk::DeviceMemory)> {
        let buffer_size = (std::mem::size_of::<u32>() * indices.len()) as DeviceSize;

        // staging
        let (staging_buffer, staging_memory) = self.create_buffer(
            buffer_size,
            BufferUsageFlags::TRANSFER_SRC,
            MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        )?;

        unsafe {
            let data_ptr =
                self.device
                    .map_memory(staging_memory, 0, buffer_size, MemoryMapFlags::empty())?
                    as *mut u32;
            data_ptr.copy_from_nonoverlapping(indices.as_ptr(), indices.len());
            self.device.unmap_memory(staging_memory);
        }

        // device local
        let buffer_info = BufferCreateInfo::default()
            .size(buffer_size)
            .usage(BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::INDEX_BUFFER)
            .sharing_mode(SharingMode::EXCLUSIVE);

        let buffer = unsafe { self.device.create_buffer(&buffer_info, None)? };
        let mem_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        let alloc_info = MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(self.find_memory_type(
                mem_requirements.memory_type_bits,
                MemoryPropertyFlags::DEVICE_LOCAL,
            )?);

        let buffer_memory = unsafe { self.device.allocate_memory(&alloc_info, None)? };
        unsafe { self.device.bind_buffer_memory(buffer, buffer_memory, 0)? };

        // copy
        let cmd = self.begin_single_time_commands(command_pool);
        unsafe {
            let copy_region = BufferCopy::default().size(buffer_size);
            self.device
                .cmd_copy_buffer(cmd, staging_buffer, buffer, &[copy_region]);
        }
        let queue = self.queues[&self.queue_families.transfer];
        self.end_single_time_commands(cmd, queue, command_pool);

        unsafe {
            self.device.destroy_buffer(staging_buffer, None);
            self.device.free_memory(staging_memory, None);
        }

        Ok((buffer, buffer_memory))
    }
}
