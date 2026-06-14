use crate::rendering::shared::texture::GpuTexture;

#[derive(Clone, Debug)]
pub struct GpuMaterial {
    pub albedo: Option<GpuTexture>,
    pub color: [f32; 4],
}
