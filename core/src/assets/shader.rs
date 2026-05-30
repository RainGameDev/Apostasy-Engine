use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use crate::log;

const SHADER_DIRECTORIES: &[&str] = &["res/shaders", "../core/res/shaders", "core/res/shaders"];

pub fn load_shader_bytes(name: &str) -> Result<Vec<u8>> {
    let requested = Path::new(name);
    let source_path = resolve_shader_path(name);
    let spv_path = if requested.extension().and_then(|e| e.to_str()) == Some("spv") {
        source_path.clone()
    } else {
        resolve_shader_path(&format!("{}.spv", name))
    };

    if requested.extension().and_then(|e| e.to_str()) == Some("spv") {
        let spv = source_path.ok_or_else(|| {
            anyhow::anyhow!(
                "Shader '{}' was not found in app or core shader directories",
                name
            )
        })?;

        log!("Loading shader SPIR-V: {}", spv.display());
        return fs::read(&spv)
            .with_context(|| format!("Failed to read SPIR-V shader file {}", spv.display()));
    }

    if let Some(source_path) = source_path {
        log!("Compiling shader source: {}", source_path.display());
        return compile_shader(&source_path);
    }

    let spv = spv_path.ok_or_else(|| {
        anyhow::anyhow!(
            "Shader '{}' was not found in app or core shader directories",
            name
        )
    })?;

    log!("Loading fallback SPIR-V: {}", spv.display());
    fs::read(&spv).with_context(|| format!("Failed to read SPIR-V shader file {}", spv.display()))
}

pub fn resolve_shader_path(name: &str) -> Option<PathBuf> {
    let requested = Path::new(name);
    let shader_paths = if requested.extension().and_then(|e| e.to_str()) == Some("spv") {
        vec![name.to_string()]
    } else {
        vec![name.to_string(), format!("{}.spv", name)]
    };

    for dir in SHADER_DIRECTORIES {
        for shader_name in &shader_paths {
            let candidate = Path::new(dir).join(shader_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn source_is_older_than_spv(source: Option<&Path>, spv: &Path) -> bool {
    if let Some(source) = source {
        if let (Ok(source_meta), Ok(spv_meta)) = (fs::metadata(source), fs::metadata(spv)) {
            if let (Ok(source_time), Ok(spv_time)) = (source_meta.modified(), spv_meta.modified()) {
                return source_time <= spv_time;
            }
        }
    }
    false
}

fn compile_shader(path: &Path) -> Result<Vec<u8>> {
    let stage = shader_kind_from_path(path)?;
    let stage_arg = match stage {
        ShaderKind::Vertex => "vert",
        ShaderKind::Fragment => "frag",
    };

    let output_path = std::env::temp_dir().join(format!(
        "{}-{}-{}.spv",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("shader"),
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let status = Command::new("glslangValidator")
        .arg("-V")
        .arg("-S")
        .arg(stage_arg)
        .arg("-o")
        .arg(&output_path)
        .arg(path)
        .status()
        .with_context(|| {
            format!("Failed to execute glslangValidator. Is it installed and on PATH?")
        })?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "glslangValidator failed to compile {} (exit {})",
            path.display(),
            status.code().map_or(-1, |c| c)
        ));
    }

    let bytes = fs::read(&output_path)
        .with_context(|| format!("Failed to read generated SPIR-V {}", output_path.display()))?;

    let _ = fs::remove_file(&output_path);

    Ok(bytes)
}

pub enum ShaderKind {
    Vertex,
    Fragment,
}

pub fn shader_kind_from_path(path: &Path) -> Result<ShaderKind> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("vert") => Ok(ShaderKind::Vertex),
        Some("frag") => Ok(ShaderKind::Fragment),
        Some(ext) => anyhow::bail!(
            "Unsupported shader extension '{}', expected .vert or .frag",
            ext
        ),
        None => anyhow::bail!("Shader path {} has no extension", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_paths_does_not_crash() {
        let _ = resolve_shader_path("voxel.vert");
    }
}
