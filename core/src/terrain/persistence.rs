use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use cgmath::Vector3;

use crate::{
    objects::{Object, cell::CellCoord, tags::skips_serilization::SkipsSerilization, world::World},
    terrain::{
        TerrainAtlasNeedsRebuild, TerrainChunkMap, TerrainSettings,
        chunk::{NeedsTerrainRebuild, TerrainChunk},
    },
};

const FILE_MAGIC: u32 = 0x41525448; // "TRHA" — TeRrain HeAder
const FILE_VERSION: u32 = 1;

/// Saves all terrain chunks to binary files under `dir`.
/// Each cell is written as `{cx}_{cz}.terrain`.
pub fn save_terrain_cells(world: &World, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;

    let texture_layers: Vec<String> = world
        .get_resource::<TerrainSettings>()
        .ok()
        .map(|s| s.texture_layers.clone())
        .unwrap_or_default();

    let terrain_ids: Vec<_> = world
        .get_objects_with_component_with_ids::<TerrainChunk>()
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    for id in terrain_ids {
        if let Some(obj) = world.get_object(id) {
            if let Ok(chunk) = obj.get_component::<TerrainChunk>() {
                let filename = cell_filename(chunk.cell_coord);
                write_terrain_cell(chunk, &texture_layers, &dir.join(&filename))?;
            }
        }
    }
    Ok(())
}

/// Loads all `.terrain` files from `dir`, creating or updating terrain objects in `world`.
/// Registers each chunk in `TerrainChunkMap` and tags it with `NeedsTerrainRebuild`.
pub fn load_terrain_cells(world: &mut World, dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    // read all files, collect chunk data
    let mut loaded: Vec<(TerrainChunk, Vec<String>)> = Vec::new();

    let entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("terrain"))
        .collect();

    for entry in &entries {
        let path = entry.path();
        let coord = match parse_cell_filename(&path) {
            Some(c) => c,
            None => continue,
        };
        match read_terrain_cell(&path, coord) {
            Ok((chunk, tex_table)) => loaded.push((chunk, tex_table)),
            Err(e) => {
                eprintln!("[terrain] Failed to load {}: {}", path.display(), e);
            }
        }
    }

    if loaded.is_empty() {
        return Ok(());
    }

    // collect ALL unique texture paths, sort alphabetically for stable order.
    let mut all_paths: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (_, table) in &loaded {
        for p in table {
            if seen.insert(p.clone()) {
                all_paths.push(p.clone());
            }
        }
    }
    all_paths.sort();

    // build mapping from texture path -> new sorted index.
    let mapping: HashMap<&str, usize> = all_paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.as_str(), i))
        .collect();

    // remap each chunk's texture_index from its local table -> global sorted order.
    for (chunk, table) in &mut loaded {
        if table.is_empty() {
            continue; // old-format file with no texture table – keep indices as-is
        }
        for val in &mut chunk.texture_index {
            let floor = *val as usize;
            let fract = val.fract();
            let ceil = floor + 1;

            let idx = floor.min(table.len().saturating_sub(1));
            let new_floor = mapping.get(table[idx].as_str()).copied();
            let new_val = match new_floor {
                Some(nf) if fract < 0.001 || ceil >= table.len() => nf as f32,
                Some(nf) => {
                    let nc = mapping.get(table[ceil].as_str()).copied();
                    match nc {
                        Some(nc) if nc == nf + 1 => nf as f32 + fract,
                        _ => {
                            // textures not adjacent in the sorted atlas – snap to closest
                            if fract < 0.5 {
                                nf as f32
                            } else {
                                nc.unwrap_or(nf) as f32
                            }
                        }
                    }
                }
                None => *val,
            };
            *val = new_val;
        }
    }

    // update TerrainSettings with the sorted texture list.
    if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
        settings.texture_layers = all_paths;
    }
    world.insert_resource(TerrainAtlasNeedsRebuild);

    // place chunks in the world (existing logic, one-phase).
    for (chunk, _) in &loaded {
        let coord = chunk.cell_coord;

        let existing_id = world
            .get_resource::<TerrainChunkMap>()
            .ok()
            .and_then(|m| m.0.get(&coord).copied())
            .filter(|&id| world.get_object(id).is_some());

        if let Some(obj_id) = existing_id {
            if let Some(obj) = world.get_object_mut(obj_id) {
                if let Ok(existing) = obj.get_component_mut::<TerrainChunk>() {
                    *existing = chunk.clone();
                }
                obj.add_tag(NeedsTerrainRebuild);
            }
        } else {
            let mut obj = Object::default();
            obj.name = format!("Terrain ({},{})", coord.x, coord.z);
            obj.add_component(chunk.clone());
            obj.add_tag(NeedsTerrainRebuild);
            obj.add_tag(SkipsSerilization);

            let obj_id = world.add_object_to_cell(coord, obj);
            if let Ok(map) = world.get_resource_mut::<TerrainChunkMap>() {
                map.0.insert(coord, obj_id);
            }
        }
    }

    Ok(())
}

// Binary format (new)
// u32  magic      = 0x41525448  ("TRHA")
// u32  version    = 1
// u32  resolution (LE)
// u32  layer_count (LE)
// [for each layer:]
//   u16  path_len (LE)
//   u8[path_len] path (UTF-8)
// f32  heights[(resolution+1)^2] (LE)
// f32  texture_index[(resolution+1)^2] (LE) — indices into the layer table above
//
// Old format (no header):
// u32  resolution (LE)
// f32  heights[...] (LE)
// f32  texture_index[...] (LE, positional indices into TerrainSettings.texture_layers)

fn write_terrain_cell(chunk: &TerrainChunk, texture_layers: &[String], path: &Path) -> Result<()> {
    let count = ((chunk.resolution + 1) as usize).pow(2);

    let layer_count = texture_layers.len() as u32;
    let layers_bytes: usize = texture_layers.iter().map(|s| 2 + s.len()).sum();

    let mut bytes: Vec<u8> =
        Vec::with_capacity(4 + 4 + 4 + 4 + layers_bytes + count * 4 + count * 4);

    bytes.extend_from_slice(&FILE_MAGIC.to_le_bytes());
    bytes.extend_from_slice(&FILE_VERSION.to_le_bytes());
    bytes.extend_from_slice(&chunk.resolution.to_le_bytes());
    bytes.extend_from_slice(&layer_count.to_le_bytes());

    for path_str in texture_layers {
        let path_bytes = path_str.as_bytes();
        let path_len = path_bytes.len() as u16;
        bytes.extend_from_slice(&path_len.to_le_bytes());
        bytes.extend_from_slice(path_bytes);
    }

    for &h in &chunk.heights {
        bytes.extend_from_slice(&h.to_le_bytes());
    }
    for &ti in &chunk.texture_index {
        bytes.extend_from_slice(&ti.to_le_bytes());
    }

    std::fs::write(path, bytes)?;
    Ok(())
}

/// Reads a terrain cell file. Returns the chunk and its per-file texture table
/// (empty vec for old-format files).
fn read_terrain_cell(path: &Path, coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 4 {
        anyhow::bail!("terrain file too small");
    }

    let first = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

    if first == FILE_MAGIC {
        read_new_format(&bytes, coord)
    } else {
        read_old_format(&bytes, coord)
    }
}

fn read_new_format(bytes: &[u8], coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
    if bytes.len() < 12 {
        anyhow::bail!("new-format terrain file too small");
    }
    let _version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let resolution = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let side = (resolution + 1) as usize;
    let count = side * side;

    let mut offset = 12;

    let layer_count = u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]);
    offset += 4;

    let mut texture_layers = Vec::with_capacity(layer_count as usize);
    for _ in 0..layer_count {
        if offset + 2 > bytes.len() {
            anyhow::bail!("truncated layer path length");
        }
        let path_len = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2;
        if offset + path_len > bytes.len() {
            anyhow::bail!("truncated layer path data");
        }
        let path = String::from_utf8_lossy(&bytes[offset..offset + path_len]).into_owned();
        offset += path_len;
        texture_layers.push(path);
    }

    let expected_data = count * 4 + count * 4;
    if offset + expected_data > bytes.len() {
        anyhow::bail!("truncated height/texture data");
    }

    let mut heights = vec![0.0f32; count];
    for i in 0..count {
        let o = offset + i * 4;
        heights[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }
    offset += count * 4;

    let mut texture_index = vec![0.0f32; count];
    for i in 0..count {
        let o = offset + i * 4;
        texture_index[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }

    Ok((
        TerrainChunk {
            cell_coord: coord,
            resolution,
            heights,
            texture_index,
        },
        texture_layers,
    ))
}

/// Fallback for the original format: no texture table, just indices.
fn read_old_format(bytes: &[u8], coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
    let resolution = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let side = (resolution + 1) as usize;
    let count = side * side;
    let expected = 4 + count * 4 + count * 4;
    if bytes.len() < expected {
        anyhow::bail!(
            "old-format terrain file truncated: expected {} bytes, got {}",
            expected,
            bytes.len()
        );
    }

    let mut heights = vec![0.0f32; count];
    for i in 0..count {
        let o = 4 + i * 4;
        heights[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }

    let mut texture_index = vec![0.0f32; count];
    let base = 4 + count * 4;
    for i in 0..count {
        let o = base + i * 4;
        texture_index[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }

    Ok((
        TerrainChunk {
            cell_coord: coord,
            resolution,
            heights,
            texture_index,
        },
        Vec::new(), // empty texture table for old format
    ))
}

fn cell_filename(coord: CellCoord) -> String {
    let cx = if coord.x < 0 {
        format!("n{}", -coord.x)
    } else {
        format!("{}", coord.x)
    };
    let cz = if coord.z < 0 {
        format!("n{}", -coord.z)
    } else {
        format!("{}", coord.z)
    };
    format!("{}_{}.terrain", cx, cz)
}

fn parse_cell_filename(path: &Path) -> Option<CellCoord> {
    let stem = path.file_stem()?.to_str()?;
    let (lhs, rhs) = stem.split_once('_')?;

    let parse_coord = |s: &str| -> Option<i32> {
        if let Some(digits) = s.strip_prefix('n') {
            digits.parse::<i32>().ok().map(|v| -v)
        } else {
            s.parse::<i32>().ok()
        }
    };

    let cx = parse_coord(lhs)?;
    let cz = parse_coord(rhs)?;
    Some(Vector3::new(cx, 0, cz))
}
