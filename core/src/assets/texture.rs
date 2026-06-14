use walkdir::WalkDir;

const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "tga", "bmp", "webp"];

/// Returns all image files found in res/ directories, as paths relative to res/
/// (e.g. "textures/brick.png"). Searches CWD/res, the crate's own res/, and
/// the sibling game package's res/. Skips .editor subdirectories.
pub fn list_available_textures() -> Vec<String> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let candidate_roots: &[std::path::PathBuf] = &[
        std::path::PathBuf::from("res"),
        std::path::PathBuf::from(manifest).join("res"),
        std::path::PathBuf::from(manifest).join("../game/res"),
        std::path::PathBuf::from(manifest).join("../editor/res"),
    ];

    let mut names = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for root in candidate_roots {
        let root = match root.canonicalize() {
            Ok(r) => r,
            Err(_) => continue,
        };

        let walker = WalkDir::new(&root).into_iter().filter_entry(|e| {
            e.file_name().to_string_lossy() != ".editor"
        });

        for entry in walker.filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !IMAGE_EXTS.contains(&ext.as_str()) {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(&root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if seen.insert(rel_str.clone()) {
                    names.push(rel_str);
                }
            }
        }
    }

    names.sort();
    names
}
