use anyhow::Result;
use std::path::Path;

use cgmath::Vector3;

use crate::{
    objects::{
        Object,
        cell::CellCoord,
        tags::skips_serilization::SkipsSerilization,
        world::World,
    },
    terrain::{
        TerrainChunkMap,
        chunk::{NeedsTerrainRebuild, TerrainChunk},
    },
};

/// Saves all terrain chunks to binary files under `dir`.
/// Each cell is written as `{cx}_{cz}.terrain`.
pub fn save_terrain_cells(world: &World, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;

    let terrain_ids: Vec<_> = world
        .get_objects_with_component_with_ids::<TerrainChunk>()
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    for id in terrain_ids {
        if let Some(obj) = world.get_object(id) {
            if let Ok(chunk) = obj.get_component::<TerrainChunk>() {
                let filename = cell_filename(chunk.cell_coord);
                write_terrain_cell(chunk, &dir.join(&filename))?;
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

    let entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                == Some("terrain")
        })
        .collect();

    for entry in entries {
        let path = entry.path();
        let coord = match parse_cell_filename(&path) {
            Some(c) => c,
            None => continue,
        };

        let chunk = match read_terrain_cell(&path, coord) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[terrain] Failed to load {}: {}", path.display(), e);
                continue;
            }
        };

        let existing_id = world
            .get_resource::<TerrainChunkMap>()
            .ok()
            .and_then(|m| m.0.get(&coord).copied())
            .filter(|&id| world.get_object(id).is_some());

        if let Some(obj_id) = existing_id {
            if let Some(obj) = world.get_object_mut(obj_id) {
                if let Ok(existing) = obj.get_component_mut::<TerrainChunk>() {
                    *existing = chunk;
                }
                obj.add_tag(NeedsTerrainRebuild);
            }
        } else {
            let mut obj = Object::default();
            obj.name = format!("Terrain ({},{})", coord.x, coord.z);
            obj.add_component(chunk);
            obj.add_tag(NeedsTerrainRebuild);
            obj.add_tag(SkipsSerilization);

            // Place the object in its actual cell so the streaming system
            // doesn't unload it because the camera is far from (0,0,0).
            let obj_id = world.add_object_to_cell(coord, obj);
            if let Ok(map) = world.get_resource_mut::<TerrainChunkMap>() {
                map.0.insert(coord, obj_id);
            }
        }
    }
    Ok(())
}

// ── Binary format ────────────────────────────────────────────────────────────
// u32  resolution (LE)
// f32  heights[(resolution+1)^2] (LE, row-major x+z*(resolution+1))
// u8×4 texture_weights[(resolution+1)^2] (flat RGBA bytes)
// ─────────────────────────────────────────────────────────────────────────────

fn write_terrain_cell(chunk: &TerrainChunk, path: &Path) -> Result<()> {
    let count = ((chunk.resolution + 1) as usize).pow(2);
    let mut bytes: Vec<u8> = Vec::with_capacity(4 + count * 4 + count * 4);

    bytes.extend_from_slice(&chunk.resolution.to_le_bytes());
    for &h in &chunk.heights {
        bytes.extend_from_slice(&h.to_le_bytes());
    }
    for w in &chunk.texture_weights {
        bytes.extend_from_slice(w);
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

fn read_terrain_cell(path: &Path, coord: CellCoord) -> Result<TerrainChunk> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 4 {
        anyhow::bail!("terrain file too small");
    }

    let resolution = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let side = (resolution + 1) as usize;
    let count = side * side;
    let expected = 4 + count * 4 + count * 4;
    if bytes.len() < expected {
        anyhow::bail!(
            "terrain file truncated: expected {} bytes, got {}",
            expected,
            bytes.len()
        );
    }

    let mut heights = vec![0.0f32; count];
    for i in 0..count {
        let o = 4 + i * 4;
        heights[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }

    let mut texture_weights = vec![[255u8, 0, 0, 0]; count];
    let base = 4 + count * 4;
    for i in 0..count {
        let o = base + i * 4;
        texture_weights[i] = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
    }

    Ok(TerrainChunk {
        cell_coord: coord,
        resolution,
        heights,
        texture_weights,
    })
}

fn cell_filename(coord: CellCoord) -> String {
    // Use 'n' prefix for negative components to avoid filesystem issues with leading '-'.
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
