use apostasy_macros::{Component, Inspect};

/// Per-vertex texture paint data.
/// Stored as a separate component so it can be added/removed independently
/// from the heightmap data.
///
/// Each vertex stores two texture hex IDs and a blend weight between them.
/// When only one texture is painted, `tex1_hex` is `u32::MAX` and `blend` is 0.
///
/// Hex IDs are stable identifiers (8 hex chars → u32) assigned by `TerrainTextureLibrary`.
/// They are resolved to array layer indices at mesh-build time, making the paint
/// data independent of texture ordering in the library.
#[derive(Debug, Inspect, Component, Clone)]
pub struct TerrainPaintData {
    pub resolution: u32,
    pub vertices: Vec<VertexPaint>,
}

impl Default for TerrainPaintData {
    fn default() -> Self {
        Self {
            resolution: 0,
            vertices: Vec::new(),
        }
    }
}

impl TerrainPaintData {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn new(resolution: u32) -> Self {
        let side = (resolution + 1) as usize;
        let count = side * side;
        Self {
            resolution,
            vertices: vec![VertexPaint::default(); count],
        }
    }

    pub fn with_default_texture(resolution: u32, default_hex: u32) -> Self {
        let side = (resolution + 1) as usize;
        let count = side * side;
        Self {
            resolution,
            vertices: vec![VertexPaint::new(default_hex); count],
        }
    }

    #[inline]
    pub fn vertex_at(&self, x: usize, z: usize) -> &VertexPaint {
        let side = (self.resolution + 1) as usize;
        &self.vertices[x + z * side]
    }

    #[inline]
    pub fn vertex_at_mut(&mut self, x: usize, z: usize) -> &mut VertexPaint {
        let side = (self.resolution + 1) as usize;
        &mut self.vertices[x + z * side]
    }
}

/// Paint data for a single vertex.
#[derive(Clone, Copy, Debug, Default)]
pub struct VertexPaint {
    /// Primary texture hex ID.
    pub tex0_hex: u32,
    /// Secondary texture hex ID, or `u32::MAX` if unused.
    pub tex1_hex: u32,
    /// Blend weight 0.0–1.0 (0 = fully tex0, 1 = fully tex1).
    pub blend: f32,
}

impl VertexPaint {
    pub fn new(hex: u32) -> Self {
        Self {
            tex0_hex: hex,
            tex1_hex: u32::MAX,
            blend: 0.0,
        }
    }

    /// Returns true if this vertex only has a single texture.
    pub fn is_single(&self) -> bool {
        self.tex1_hex == u32::MAX
    }

    /// Returns true if both textures are the same (effectively single).
    pub fn is_effectively_single(&self) -> bool {
        self.tex1_hex == u32::MAX || self.tex0_hex == self.tex1_hex || self.blend <= 0.0
    }
}
