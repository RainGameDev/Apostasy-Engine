use ash::vk::{Buffer, DeviceMemory};

#[derive(Clone, Debug)]
pub struct GpuModel {
    pub meshes: Vec<Mesh>,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertex_buffer: Buffer,
    pub vertex_buffer_memory: DeviceMemory,
    pub index_buffer: Buffer,
    pub index_buffer_memory: DeviceMemory,
    pub index_count: u32,
    pub material_name: String,
}

impl GpuMesh for Mesh {
    fn get_vertex_buffer(&self) -> Buffer {
        self.vertex_buffer.clone()
    }
    fn get_index_buffer(&self) -> Buffer {
        self.index_buffer.clone()
    }
    fn get_index_count(&self) -> u32 {
        self.index_count
    }
}

pub trait GpuMesh {
    fn get_vertex_buffer(&self) -> Buffer;
    fn get_index_buffer(&self) -> Buffer;
    fn get_index_count(&self) -> u32;
}
