use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SHADER_DIRECTORIES: &[&str] = &["res/shaders", "../core/res/shaders", "core/res/shaders"];

pub fn load_shader_bytes(name: &str) -> Result<Vec<u8>> {
    let requested = Path::new(name);
    let source_path = resolve_shader_path(name);
    let spv_path = if requested.extension().and_then(|e| e.to_str()) == Some("spv") {
        source_path.clone()
    } else {
        resolve_shader_path(&format!("{}.spv", name))
    };

    if let Some(spv) = &spv_path {
        if source_path.is_none() || source_is_older_than_spv(source_path.as_deref(), spv) {
            eprintln!("Loading shader SPIR-V: {}", spv.display());
            return fs::read(spv)
                .with_context(|| format!("Failed to read SPIR-V shader file {}", spv.display()));
        }
    }

    let source_path = source_path.ok_or_else(|| {
        anyhow::anyhow!(
            "Shader '{}' was not found in app or core shader directories",
            name
        )
    })?;

    eprintln!("Loading shader source: {}", source_path.display());

    match compile_shader(&source_path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if spv_path.is_some() => {
            let spv = spv_path.unwrap();
            eprintln!(
                "WARNING: GLSL compile failed, falling back to SPIR-V {}: {}",
                spv.display(),
                err
            );
            fs::read(&spv)
                .with_context(|| format!("Failed to read SPIR-V shader file {}", spv.display()))
        }
        Err(err) => Err(err),
    }
}

fn resolve_shader_path(name: &str) -> Option<PathBuf> {
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

fn source_is_older_than_spv(source: Option<&Path>, spv: &Path) -> bool {
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
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
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

enum ShaderKind {
    Vertex,
    Fragment,
}

fn shader_kind_from_path(path: &Path) -> Result<ShaderKind> {
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
