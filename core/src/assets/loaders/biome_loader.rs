use std::sync::{Arc, RwLock};

use anyhow::{Error, Result};
use hashbrown::HashMap;

use crate::{
    assets::loader::AssetLoader,
    voxels::biome::{BiomeDefinition, BiomeRegistry, StructureDefinition, TerrainShaping},
};

pub struct BiomeLoader {
    pub registry: Arc<RwLock<BiomeRegistry>>,
}

impl AssetLoader for BiomeLoader {
    fn class_name(&self) -> &'static str {
        "Biome"
    }

    fn load(&mut self, raw: &serde_yaml::Value) -> Result<()> {
        let name: String = raw["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'name'"))?
            .to_string();

        let namespace: String = raw["namespace"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'namespace'"))?
            .to_string();

        {
            let registry = self.registry.read().unwrap();
            for reg in registry.defs.iter() {
                if reg.name == name && reg.namespace == namespace {
                    return Err(Error::msg(format!(
                        "Biome with the name: {} exists in namespace {} already",
                        name, namespace
                    )));
                }
            }
        }

        let surface_voxels = raw["voxel"]["surface"]
            .as_sequence()
            .ok_or_else(|| anyhow::anyhow!("Missing 'voxel.surface'"))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>();

        let subsurface_voxels = raw["voxel"]["subsurface"]
            .as_sequence()
            .ok_or_else(|| anyhow::anyhow!("Missing 'voxel.subsurface'"))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>();

        let underground_voxels = raw["voxel"]["underground"]
            .as_sequence()
            .ok_or_else(|| anyhow::anyhow!("Missing 'voxel.underground'"))?
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>();

        let amplitude = raw["amplitude"]
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'amplitude'"))?;

        let octaves = raw["octaves"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'octaves'"))? as u32;

        let frequency = raw["frequency"]
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'frequency'"))?;

        let humidity = raw["humidity"]
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'humidity'"))?;

        let temperature = raw["temperature"]
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("Missing 'temperature'"))?;

        // ── terrain_shaping block (all fields optional, falls back to a named
        //    preset or field-by-field defaults if the block is absent) ────────
        let terrain_shaping = parse_terrain_shaping(&raw["terrain_shaping"])?;

        // ── Structures ────────────────────────────────────────────────────────
        let mut structures: Vec<StructureDefinition> = Vec::new();
        if let Some(structures_seq) = raw["structures"].as_sequence() {
            for struct_yaml in structures_seq {
                let structure_type = struct_yaml["type"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Structure missing 'type'"))?
                    .to_string();

                let density = struct_yaml["density"]
                    .as_f64()
                    .ok_or_else(|| anyhow::anyhow!("Structure missing 'density'"))?;

                let asset = struct_yaml["asset"].as_str().map(|s| s.to_string());

                let mut voxels: HashMap<String, String> = HashMap::new();
                if let Some(voxels_map) = struct_yaml["voxels"].as_mapping() {
                    for (key, value) in voxels_map {
                        if let (Some(k), Some(v)) = (key.as_str(), value.as_str()) {
                            voxels.insert(k.to_string(), v.to_string());
                        }
                    }
                }

                let mut parameters: HashMap<String, serde_yaml::Value> = HashMap::new();
                if let Some(params_map) = struct_yaml["parameters"].as_mapping() {
                    for (key, value) in params_map {
                        if let Some(k) = key.as_str() {
                            parameters.insert(k.to_string(), value.clone());
                        }
                    }
                }

                structures.push(StructureDefinition {
                    structure_type,
                    density,
                    asset,
                    voxels,
                    parameters,
                });
            }
        }

        // ── Ambient graphics ──────────────────────────────────────────────────
        let water_color = parse_color(&raw["ambient_graphics"]["water_color"], (63, 118, 228))?;
        let foliage_color = parse_color(&raw["ambient_graphics"]["foliage_color"], (77, 140, 61))?;

        let def = BiomeDefinition {
            name: name.clone(),
            namespace: namespace.clone(),
            class: "Biome".to_string(),

            surface_voxels,
            subsurface_voxels,
            underground_voxels,

            amplitude,
            frequency,
            octaves,

            humidity,
            temperature,
            structures,
            water_color,
            foliage_color,

            terrain_shaping,
        };

        let mut registry = self.registry.write().unwrap();
        // Second duplicate check under write lock to avoid a TOCTOU race.
        for reg in registry.defs.iter() {
            if reg.name == name && reg.namespace == namespace {
                return Err(Error::msg(format!(
                    "Biome with the name: {} exists in namespace {} already",
                    name, namespace
                )));
            }
        }

        let id = registry.defs.len() as u16;
        let full_name = format!("{}:Biomes:{}", namespace, name);
        registry.defs.push(def);
        registry.name_to_id.insert(full_name.clone(), id);
        registry.id_to_name.insert(id, full_name);

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// terrain_shaping parser
//
// Supports two authoring styles in YAML:
//
// Style A – named preset (quickest):
//   terrain_shaping:
//     preset: flat          # flat | rolling | hilly | mountainous | ocean
//
// Style B – explicit fields (full control), any subset may be omitted:
//   terrain_shaping:
//     ridge_strength:      0.03
//     peak_strength:       0.03
//     valley_strength:     0.0
//     continental_scale:   10.0
//     height_curve:        1.0
//
// Style B fields override the chosen preset, so you can start from a preset
// and tweak individual values:
//   terrain_shaping:
//     preset: rolling
//     ridge_strength: 0.05   # slightly flatter ridges than the rolling preset
//
// If the block is absent entirely, TerrainShaping::flat() is used as the
// safest default (no unexpected hills in new biomes).
// ─────────────────────────────────────────────────────────────────────────────
fn parse_terrain_shaping(raw: &serde_yaml::Value) -> Result<TerrainShaping> {
    // Block absent → safe flat default.
    if raw.is_null() || raw.as_mapping().map(|m| m.is_empty()).unwrap_or(false) {
        return Ok(TerrainShaping::flat());
    }

    // Start from a preset if one is named, otherwise start from the default.
    let mut shaping = match raw["preset"].as_str() {
        Some("flat") => TerrainShaping::flat(),
        Some("rolling") => TerrainShaping::rolling(),
        Some("hilly") => TerrainShaping::hilly(),
        Some("mountainous") => TerrainShaping::mountainous(),
        Some("ocean") => TerrainShaping::ocean(),
        Some(other) => {
            return Err(anyhow::anyhow!(
                "Unknown terrain_shaping preset '{}'. \
                 Valid values: flat, rolling, hilly, mountainous, ocean",
                other
            ));
        }
        None => TerrainShaping::flat(),
    };

    // Override individual fields when present.
    if let Some(v) = raw["ridge_strength"].as_f64() {
        shaping.ridge_strength = v;
    }
    if let Some(v) = raw["peak_strength"].as_f64() {
        shaping.peak_strength = v;
    }
    if let Some(v) = raw["valley_strength"].as_f64() {
        shaping.valley_strength = v;
    }
    if let Some(v) = raw["continental_scale"].as_f64() {
        shaping.continental_scale = v;
    }
    if let Some(v) = raw["height_curve"].as_f64() {
        shaping.height_curve = v;
    }

    Ok(shaping)
}

fn parse_color(value: &serde_yaml::Value, default: (u8, u8, u8)) -> Result<(u8, u8, u8)> {
    if let Some(seq) = value.as_sequence() {
        if seq.len() >= 3 {
            let r = seq[0].as_u64().unwrap_or(default.0 as u64) as u8;
            let g = seq[1].as_u64().unwrap_or(default.1 as u64) as u8;
            let b = seq[2].as_u64().unwrap_or(default.2 as u64) as u8;
            return Ok((r, g, b));
        }
    }
    if let Some(map) = value.as_mapping() {
        let r = map
            .get(&serde_yaml::Value::String("r".to_string()))
            .and_then(|v| v.as_u64())
            .unwrap_or(default.0 as u64) as u8;
        let g = map
            .get(&serde_yaml::Value::String("g".to_string()))
            .and_then(|v| v.as_u64())
            .unwrap_or(default.1 as u64) as u8;
        let b = map
            .get(&serde_yaml::Value::String("b".to_string()))
            .and_then(|v| v.as_u64())
            .unwrap_or(default.2 as u64) as u8;
        return Ok((r, g, b));
    }
    Ok(default)
}
