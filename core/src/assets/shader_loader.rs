use anyhow::Result;

pub fn load_shader_bytes(name: &str) -> Result<Vec<u8>> {
    crate::assets::shader::load_shader_bytes(name)
}
