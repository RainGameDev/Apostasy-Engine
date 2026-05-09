use anyhow::{Context, Result};
use naga::back::spv::{Options as SpvOptions, PipelineOptions, write_vec};
use naga::front::glsl::{Frontend, Options as GlslOptions};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{ResourceBinding, ShaderStage};
use std::fs;
use std::path::{Path, PathBuf};

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
    let source = fs::read_to_string(&source_path).with_context(|| {
        format!(
            "Failed to read shader source file {}",
            source_path.display()
        )
    })?;

    match compile_shader(&source_path, &source) {
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

fn compile_shader(path: &Path, source: &str) -> Result<Vec<u8>> {
    let stage = shader_kind_from_path(path)?;
    let preprocessed_source = preprocess_shader_source(source);

    let mut frontend = Frontend::default();
    let options = GlslOptions::from(stage);
    let mut module = frontend
        .parse(&options, &preprocessed_source)
        .with_context(|| format!("Failed to parse GLSL shader {}", path.display()))?;

    apply_explicit_bindings(source, &mut module);

    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let module_info = validator
        .validate(&module)
        .with_context(|| format!("Failed to validate shader module {}", path.display()))?;

    let writer_options = SpvOptions::default();
    let pipeline_options = PipelineOptions {
        shader_stage: stage,
        entry_point: "main".into(),
    };

    let words = write_vec(
        &module,
        &module_info,
        &writer_options,
        Some(&pipeline_options),
    )
    .with_context(|| format!("Failed to write SPIR-V for shader {}", path.display()))?;

    Ok(words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<u8>>())
}

fn preprocess_shader_source(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut remainder = source;

    while let Some(layout_start) = remainder.find("layout(") {
        output.push_str(&remainder[..layout_start]);
        let after_layout = &remainder[layout_start + "layout(".len()..];
        if let Some(layout_end) = find_matching_paren(after_layout) {
            let contents = &after_layout[..layout_end];
            if contents.contains("binding") && !contents.contains("uniform") {
                remainder = &after_layout[layout_end + 1..];
                continue;
            }
            output.push_str("layout(");
            output.push_str(contents);
            output.push(')');
            remainder = &after_layout[layout_end + 1..];
        } else {
            output.push_str(&remainder[layout_start..]);
            remainder = "";
        }
    }

    output.push_str(remainder);
    // Remove "flat" qualifiers which naga doesn't support
    let mut result = output.replace("flat ", "");

    result
}

fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 1;
    for (idx, c) in s.chars().enumerate() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn apply_explicit_bindings(source: &str, module: &mut naga::Module) {
    for line in source.lines() {
        if let Some(binding) = parse_uniform_binding(line) {
            if let Some(name) = parse_uniform_name(line) {
                for (_, var) in module.global_variables.iter_mut() {
                    if var.name.as_deref() == Some(name) {
                        var.binding = Some(ResourceBinding { group: 0, binding });
                    }
                }
            }
        }
    }
}

fn parse_uniform_binding(line: &str) -> Option<u32> {
    if !line.contains("layout") || !line.contains("uniform") {
        return None;
    }

    let binding_start = line.find("binding")?;
    let after_eq = line[binding_start..].find('=')? + binding_start + 1;
    let after_eq = &line[after_eq..];
    let end = after_eq.find(')').unwrap_or_else(|| after_eq.len());

    after_eq[..end]
        .trim()
        .trim_end_matches(|c: char| c.is_whitespace())
        .parse::<u32>()
        .ok()
}

fn parse_uniform_name(line: &str) -> Option<&str> {
    let uniform_idx = line.find("uniform")?;
    let remainder = &line[uniform_idx + "uniform".len()..].trim_start();
    let mut parts = remainder.split_whitespace();
    let type_part = parts.next()?;
    let name_part = parts.next()?;
    if name_part.ends_with('{') || name_part.ends_with(';') {
        Some(name_part.trim_end_matches(|c| c == '{' || c == ';'))
    } else {
        // For "type name;"
        Some(name_part)
    }
}

fn shader_kind_from_path(path: &Path) -> Result<ShaderStage> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("vert") => Ok(ShaderStage::Vertex),
        Some("frag") => Ok(ShaderStage::Fragment),
        Some(ext) => anyhow::bail!(
            "Unsupported shader extension '{}', expected .vert or .frag",
            ext
        ),
        None => anyhow::bail!("Shader path {} has no extension", path.display()),
    }
}
