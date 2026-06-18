use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use apostasy_macros::Resource;
use hashbrown::HashMap;
use image::DynamicImage;

use crate::log_warn;

/// A registered terrain texture, identified by its stable hex hash
/// so that reordering textures in the library never corrupts saved paint data.
#[derive(Clone, Debug)]
pub struct TerrainTextureDef {
    pub hex_id: u32,
    pub name: String,
    pub path: String,
    /// Loaded image (kept in CPU memory for thumbnail generation & atlas building).
    pub image: DynamicImage,
}

/// Global resource that owns all terrain textures known to the editor.
#[derive(Resource, Clone, Debug)]
pub struct TerrainTextureLibrary {
    pub textures: Vec<TerrainTextureDef>,
    /// hex_id → index into `textures`
    pub hex_to_index: HashMap<u32, usize>,
    /// name → index
    pub name_to_index: HashMap<String, usize>,
}

impl Default for TerrainTextureLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl TerrainTextureLibrary {
    pub fn new() -> Self {
        Self {
            textures: Vec::new(),
            hex_to_index: HashMap::new(),
            name_to_index: HashMap::new(),
        }
    }

    /// Scan `res/textures/terrain/` and load all images into the library.
    pub fn load_from_disk(&mut self) {
        let candidate_roots = [
            Path::new("res/"),
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("res/"),
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../editor/res/"),
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../game/res/"),
        ];

        let terrain_dir = "textures/terrain";
        let mut discovered: Vec<String> = Vec::new();

        for root in &candidate_roots {
            let dir = root.join(terrain_dir);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if is_image_file(&path) {
                        discovered.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }

        discovered.sort();
        discovered.dedup();

        for path in &discovered {
            self.add_texture(path);
        }
    }

    /// Add a single texture by its absolute file path.
    pub fn add_texture(&mut self, path: &str) -> u32 {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        if let Some(&idx) = self.name_to_index.get(&stem) {
            return self.textures[idx].hex_id;
        }

        let hex_id = make_hex_for_path(&stem);

        let img = match image::open(path) {
            Ok(img) => img,
            Err(e) => {
                log_warn!("Failed to load terrain texture {}: {}", path, e);
                return 0;
            }
        };

        let idx = self.textures.len();
        self.textures.push(TerrainTextureDef {
            hex_id,
            name: stem,
            path: path.to_string(),
            image: img,
        });
        self.hex_to_index.insert(hex_id, idx);
        self.name_to_index
            .insert(self.textures[idx].name.clone(), idx);

        hex_id
    }

    /// Look up the array layer index for a hex ID. Returns 0 (fallback) if not found.
    pub fn layer_index(&self, hex_id: u32) -> u32 {
        self.hex_to_index
            .get(&hex_id)
            .map(|&i| i as u32)
            .unwrap_or(0)
    }

    /// Total number of textures.
    pub fn count(&self) -> u32 {
        self.textures.len() as u32
    }

    /// Get a texture definition by hex_id.
    pub fn get_by_hex(&self, hex_id: u32) -> Option<&TerrainTextureDef> {
        let idx = self.hex_to_index.get(&hex_id)?;
        self.textures.get(*idx)
    }
}

fn is_image_file(path: &Path) -> bool {
    const EXTS: &[&str] = &["png", "jpg", "jpeg", "tga", "bmp", "hdr"];
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    EXTS.contains(&ext.as_str())
}

fn make_hex_for_path(stem: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    stem.hash(&mut hasher);
    hasher.finish() as u32
}
