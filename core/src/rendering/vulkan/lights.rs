use ash::vk;

use crate::rendering::lighting::gpu_light::{
    CSM_CASCADE_COUNT, GpuLight, MAX_LIGHTS, PointShadowData, ShadowData,
};
use crate::rendering::vulkan::VulkanRenderer;

impl VulkanRenderer {
    pub(crate) fn set_lights(
        &mut self,
        lights: &[GpuLight],
        shadow_data: Option<ShadowData>,
        point_shadow_data: Option<PointShadowData>,
        shadow_distance: f32,
        camera_pos: [f32; 3],
        camera_dir: [f32; 3],
    ) {
        let count = lights.len().min(MAX_LIGHTS) as u32;
        // SSBO layout (std430, 336-byte header):
        //   offset   0: uint  count
        //   offset   4: uint  shadow_enabled  (0=off, 1=spot, 2=directional CSM)
        //   offset   8: uint  cascade_count
        //   offset  12: float shadow_distance
        //   offset  16: vec4  camera_world_pos
        //   offset  32: vec4  camera_world_dir
        //   offset  48: mat4  light_space[4]         (256 bytes)
        //   offset 304: float cascade_splits[4]      (16 bytes)
        //   offset 320: uint  shadow_light_index
        //   offset 324: uint  point_shadow_enabled
        //   offset 328: uint  point_shadow_light_index
        //   offset 332: float point_shadow_far
        //   offset 336: GpuLight[]
        const LIGHTS_OFFSET: usize = 336;

        unsafe {
            let ptr = self
                .context
                .device
                .map_memory(
                    self.light_ssbo_memory,
                    0,
                    vk::WHOLE_SIZE,
                    vk::MemoryMapFlags::empty(),
                )
                .unwrap() as *mut u8;

            let (shadow_enabled, cascade_count, shadow_light_index) = match &shadow_data {
                None => (0u32, 0u32, 0u32),
                Some(d) if d.cascade_count == 1 => (1u32, 1u32, d.shadow_light_index),
                Some(d) => (2u32, CSM_CASCADE_COUNT as u32, d.shadow_light_index),
            };

            let (point_shadow_enabled, point_shadow_light_index, point_shadow_far) =
                match &point_shadow_data {
                    None => (0u32, 0u32, 1.0f32),
                    Some(d) => (1u32, d.light_index, d.far),
                };

            (ptr as *mut u32).write(count);
            (ptr.add(4) as *mut u32).write(shadow_enabled);
            (ptr.add(8) as *mut u32).write(cascade_count);
            (ptr.add(12) as *mut f32).write(shadow_distance);
            (ptr.add(16) as *mut [f32; 4]).write([
                camera_pos[0],
                camera_pos[1],
                camera_pos[2],
                0.0,
            ]);
            (ptr.add(32) as *mut [f32; 4]).write([
                camera_dir[0],
                camera_dir[1],
                camera_dir[2],
                0.0,
            ]);

            if let Some(ref d) = shadow_data {
                for i in 0..CSM_CASCADE_COUNT {
                    let mat = d.matrices.get(i).copied().unwrap_or([[0.0f32; 4]; 4]);
                    (ptr.add(48 + i * 64) as *mut [[f32; 4]; 4]).write(mat);
                }
                (ptr.add(304) as *mut [f32; 4]).write(d.splits);
            } else {
                std::ptr::write_bytes(ptr.add(48), 0, 256 + 16);
            }

            (ptr.add(320) as *mut u32).write(shadow_light_index);
            (ptr.add(324) as *mut u32).write(point_shadow_enabled);
            (ptr.add(328) as *mut u32).write(point_shadow_light_index);
            (ptr.add(332) as *mut f32).write(point_shadow_far);

            (ptr.add(LIGHTS_OFFSET) as *mut GpuLight)
                .copy_from_nonoverlapping(lights.as_ptr(), count as usize);

            self.context.device.unmap_memory(self.light_ssbo_memory);
        }
    }
}
