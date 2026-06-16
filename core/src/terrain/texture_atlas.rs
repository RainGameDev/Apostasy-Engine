use std::path::Path;

use anyhow::Result;
use ash::vk::{self, SampleCountFlags};
use image::DynamicImage;

use crate::{log_warn, rendering::vulkan::rendering_context::VulkanRenderingContext};

/// Scans a directory for image files (png, jpg, jpeg, tga, bmp, hdr).
/// Returns full absolute paths sorted alphabetically.
/// Useful for auto-discovering terrain textures from a `textures/terrain` folder.
pub fn discover_terrain_textures(dir: &Path) -> Vec<String> {
    const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "tga", "bmp", "hdr"];
    let mut paths: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if IMAGE_EXTS.contains(&ext.as_str()) {
                paths.push(path.to_string_lossy().to_string());
            }
        }
    }
    paths.sort();
    paths
}

pub fn load_terrain_texture(path: &str) -> DynamicImage {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = vec![
        Path::new("res/").join(path),
        manifest.join("res/").join(path),
        manifest.join("../game/res/").join(path),
    ];
    candidates.push(Path::new(path).to_path_buf());

    for candidate in &candidates {
        if candidate.exists() {
            if let Ok(img) = image::open(candidate) {
                return img;
            }
        }
    }

    log_warn!("Terrain texture not found: {}", path);
    DynamicImage::new_rgba8(1, 1)
}
