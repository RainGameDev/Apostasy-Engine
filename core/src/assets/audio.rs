use std::path::PathBuf;
use walkdir::WalkDir;

const AUDIO_EXTS: &[&str] = &["ogg", "mp3", "wav", "flac", "aac", "aiff", "alac"];

fn candidate_roots() -> Vec<PathBuf> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    vec![
        PathBuf::from("res"),
        PathBuf::from(manifest).join("res"),
        PathBuf::from(manifest).join("../game/res"),
        PathBuf::from(manifest).join("../editor/res"),
    ]
}

/// Resolves a res-relative audio path (e.g. `"audio/test.flac"`) to an
/// absolute path by trying each candidate res/ root in order. Returns the
/// first existing file found, or `None` if none matched.
pub fn resolve_audio_path(rel: &str) -> Option<PathBuf> {
    for root in candidate_roots() {
        let candidate = root.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    // Also accept absolute paths or paths that exist as-is.
    let p = PathBuf::from(rel);
    if p.exists() { Some(p) } else { None }
}

/// Returns all audio files found in res/ directories, as paths relative to res/
/// (e.g. "audio/footstep.ogg"). Searches CWD/res, the crate's own res/, and
/// the sibling game/editor package's res/. Skips .editor subdirectories.
pub fn list_available_audio() -> Vec<String> {
    let roots = candidate_roots();

    let mut names = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for root in &roots {
        let root = match root.canonicalize() {
            Ok(r) => r,
            Err(_) => continue,
        };

        let walker = WalkDir::new(&root)
            .into_iter()
            .filter_entry(|e| e.file_name().to_string_lossy() != ".editor");

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
            if !AUDIO_EXTS.contains(&ext.as_str()) {
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
