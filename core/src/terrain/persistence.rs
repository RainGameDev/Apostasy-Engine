use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use cgmath::Vector3;
use serde::{Deserialize, Serialize};

use crate::{
    objects::{Object, cell::CellCoord, tags::skips_serilization::SkipsSerilization, world::World},
    terrain::{
        TerrainAtlasNeedsRebuild, TerrainChunkMap, TerrainSettings,
        chunk::{NeedsTerrainRebuild, TerrainChunk, MAX_ACTIVE_LAYERS},
    },
};

const FILE_MAGIC: u32 = 0x41525448; // "TRHA"
const FILE_VERSION: u32 = 4;
/// Layer count used in the v2 binary format (before the 32-layer expansion).
const V2_ACTIVE_LAYERS: usize = 6;

#[derive(Serialize, Deserialize)]
struct TextureLayerList {
    texture_layers: Vec<String>,
}

/// Saves the terrain texture layer list as a sidecar YAML file.
pub fn save_terrain_settings(world: &World, dir: &Path) -> Result<()> {
    let layers = world
        .get_resource::<TerrainSettings>()
        .ok()
        .map(|s| s.texture_layers.clone())
        .unwrap_or_default();
    let list = TextureLayerList {
        texture_layers: layers,
    };
    let yaml = serde_yaml::to_string(&list)?;
    std::fs::write(dir.join("texture_layers.yaml"), yaml)?;
    Ok(())
}

/// Loads the texture layer list from the sidecar YAML file, if it exists.
pub fn load_terrain_settings(dir: &Path) -> Vec<String> {
    let path = dir.join("texture_layers.yaml");
    if !path.exists() {
        return Vec::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_yaml::from_str::<TextureLayerList>(&content) {
            Ok(list) => list.texture_layers,
            Err(e) => {
                eprintln!("[terrain] Failed to parse texture_layers.yaml: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            eprintln!("[terrain] Failed to read texture_layers.yaml: {}", e);
            Vec::new()
        }
    }
}

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

    // Also persist the texture list as a sidecar file so it survives
    // even when no terrain chunks exist yet.
    save_terrain_settings(world, dir)?;

    Ok(())
}

/// Loads all `.terrain` files from `dir`, creating or updating terrain objects in `world`.
pub fn load_terrain_cells(world: &mut World, dir: &Path) -> Result<()> {
    if !dir.exists() {
        // Still try to load standalone texture list even without terrain dir.
        let saved = load_terrain_settings(dir);
        if !saved.is_empty()
            && let Ok(settings) = world.get_resource_mut::<TerrainSettings>()
        {
            settings.texture_layers = saved;
            world.insert_resource(TerrainAtlasNeedsRebuild);
        }
        return Ok(());
    }

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

    // Start with the standalone texture list, then merge in any paths from chunks.
    let mut all_paths = load_terrain_settings(dir);
    let mut seen: std::collections::HashSet<String> = all_paths.iter().cloned().collect();
    for (_, table) in &loaded {
        for p in table {
            if seen.insert(p.clone()) {
                all_paths.push(p.clone());
            }
        }
    }
    all_paths.sort();

    // Build mapping from texture path -> new sorted index.
    let mapping: HashMap<&str, usize> = all_paths
        .iter()
        .enumerate()
        .map(|(i, p)| (p.as_str(), i))
        .collect();

    // Remap each chunk's active_layer_ids from local table -> global sorted order.
    for (chunk, table) in &mut loaded {
        if table.is_empty() {
            continue;
        }
        for id in &mut chunk.active_layer_ids {
            if *id < table.len() as u32 {
                let old_path = &table[*id as usize];
                if let Some(&new_id) = mapping.get(old_path.as_str()) {
                    *id = new_id as u32;
                }
            }
        }
    }

    // Update TerrainSettings with the sorted texture list.
    if let Ok(settings) = world.get_resource_mut::<TerrainSettings>() {
        settings.texture_layers = all_paths;
    }
    world.insert_resource(TerrainAtlasNeedsRebuild);

    // Place chunks in the world.
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

// Binary format (version 4):
// u32  magic      = "TRHA"
// u32  version    = 4
// u32  resolution
// u32  layer_count
// [for each layer:]
//   u16  path_len
//   u8[path_len] path (UTF-8)
// f32  heights[(resolution+1)^2]
// u32  active_layer_count
// u32  active_layer_ids[32]
// f32  vertex_weights[(resolution+1)^2][32]
// f32  vertex_colors[(resolution+1)^2][3]   <-- added in v4

fn write_terrain_cell(chunk: &TerrainChunk, texture_layers: &[String], path: &Path) -> Result<()> {
    let count = ((chunk.resolution + 1) as usize).pow(2);

    let layer_count = texture_layers.len() as u32;
    let layers_bytes: usize = texture_layers.iter().map(|s| 2 + s.len()).sum();

    let mut bytes: Vec<u8> = Vec::with_capacity(
        4 + 4 + 4 + 4 + layers_bytes + count * 4 // heights
        + 4 + MAX_ACTIVE_LAYERS as usize * 4 + count * MAX_ACTIVE_LAYERS as usize * 4 // weights
        + count * 3 * 4, // vertex colors
    );

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

    // Heights
    for &h in &chunk.heights {
        bytes.extend_from_slice(&h.to_le_bytes());
    }

    // Active layer IDs
    bytes.extend_from_slice(&(chunk.active_layer_count as u32).to_le_bytes());
    for &id in &chunk.active_layer_ids {
        bytes.extend_from_slice(&id.to_le_bytes());
    }

    // Vertex weights
    for w in &chunk.vertex_weights {
        for &v in w {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    // Vertex colors (RGB per vertex)
    for c in &chunk.vertex_colors {
        for &v in c {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }

    std::fs::write(path, bytes)?;
    Ok(())
}

fn read_terrain_cell(path: &Path, coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 4 {
        anyhow::bail!("terrain file too small");
    }

    let first = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

    if first == FILE_MAGIC {
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        match version {
            1 => read_v1_format(&bytes, coord),
            2 => read_v2_format(&bytes, coord),
            3 => read_v3_format(&bytes, coord),
            _ => read_v4_format(&bytes, coord),
        }
    } else {
        read_old_format(&bytes, coord)
    }
}

/// Version 2: 6-layer format — read with the old V2_ACTIVE_LAYERS count and expand to 32.
fn read_v2_format(bytes: &[u8], coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
    if bytes.len() < 12 {
        anyhow::bail!("v2 terrain file too small");
    }
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

    // Heights
    let expected_heights = count * 4;
    if offset + expected_heights > bytes.len() {
        anyhow::bail!("truncated height data");
    }
    let mut heights = vec![0.0f32; count];
    for i in 0..count {
        let o = offset + i * 4;
        heights[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }
    offset += count * 4;

    // Active layer count + IDs (v2 stores exactly V2_ACTIVE_LAYERS u32s)
    if offset + 4 > bytes.len() {
        anyhow::bail!("truncated layer count");
    }
    let active_layer_count = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
    offset += 4;

    let mut active_layer_ids = [0u32; MAX_ACTIVE_LAYERS as usize];
    for i in 0..V2_ACTIVE_LAYERS {
        if offset + 4 > bytes.len() {
            anyhow::bail!("truncated layer IDs");
        }
        active_layer_ids[i] = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
        offset += 4;
    }

    // Vertex weights (v2 stores V2_ACTIVE_LAYERS floats per vertex)
    let expected_weights = count * V2_ACTIVE_LAYERS * 4;
    if offset + expected_weights > bytes.len() {
        anyhow::bail!("truncated weight data");
    }
    let mut vertex_weights = vec![[0.0f32; MAX_ACTIVE_LAYERS as usize]; count];
    for i in 0..count {
        for j in 0..V2_ACTIVE_LAYERS {
            let o = offset + (i * V2_ACTIVE_LAYERS + j) * 4;
            vertex_weights[i][j] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        }
    }

    Ok((
        TerrainChunk {
            cell_coord: coord,
            resolution,
            heights,
            active_layer_ids,
            active_layer_count: active_layer_count as u8,
            vertex_weights,
            vertex_colors: vec![[1.0, 1.0, 1.0]; count],
        },
        texture_layers,
    ))
}

/// Version 3: 32-layer format.
fn read_v3_format(bytes: &[u8], coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
    if bytes.len() < 12 {
        anyhow::bail!("v3 terrain file too small");
    }
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

    // Heights
    let expected_heights = count * 4;
    if offset + expected_heights > bytes.len() {
        anyhow::bail!("truncated height data");
    }
    let mut heights = vec![0.0f32; count];
    for i in 0..count {
        let o = offset + i * 4;
        heights[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }
    offset += count * 4;

    // Active layer count + IDs
    if offset + 4 > bytes.len() {
        anyhow::bail!("truncated layer count");
    }
    let active_layer_count = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
    offset += 4;

    let mut active_layer_ids = [0u32; MAX_ACTIVE_LAYERS as usize];
    for i in 0..MAX_ACTIVE_LAYERS as usize {
        if offset + 4 > bytes.len() {
            anyhow::bail!("truncated layer IDs");
        }
        active_layer_ids[i] = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
        offset += 4;
    }

    // Vertex weights
    let expected_weights = count * MAX_ACTIVE_LAYERS as usize * 4;
    if offset + expected_weights > bytes.len() {
        anyhow::bail!("truncated weight data");
    }
    let mut vertex_weights = vec![[0.0f32; MAX_ACTIVE_LAYERS as usize]; count];
    for i in 0..count {
        for j in 0..MAX_ACTIVE_LAYERS as usize {
            let o = offset + (i * MAX_ACTIVE_LAYERS as usize + j) * 4;
            vertex_weights[i][j] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        }
    }

    Ok((
        TerrainChunk {
            cell_coord: coord,
            resolution,
            heights,
            active_layer_ids,
            active_layer_count: active_layer_count as u8,
            vertex_weights,
            vertex_colors: vec![[1.0, 1.0, 1.0]; count],
        },
        texture_layers,
    ))
}

/// Version 4: 32-layer format + vertex colors.
fn read_v4_format(bytes: &[u8], coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
    if bytes.len() < 12 {
        anyhow::bail!("v4 terrain file too small");
    }
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

    // Heights
    let expected_heights = count * 4;
    if offset + expected_heights > bytes.len() {
        anyhow::bail!("truncated height data");
    }
    let mut heights = vec![0.0f32; count];
    for i in 0..count {
        let o = offset + i * 4;
        heights[i] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    }
    offset += count * 4;

    // Active layer count + IDs
    if offset + 4 > bytes.len() {
        anyhow::bail!("truncated layer count");
    }
    let active_layer_count = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
    offset += 4;

    let mut active_layer_ids = [0u32; MAX_ACTIVE_LAYERS as usize];
    for i in 0..MAX_ACTIVE_LAYERS as usize {
        if offset + 4 > bytes.len() {
            anyhow::bail!("truncated layer IDs");
        }
        active_layer_ids[i] = u32::from_le_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]]);
        offset += 4;
    }

    // Vertex weights
    let expected_weights = count * MAX_ACTIVE_LAYERS as usize * 4;
    if offset + expected_weights > bytes.len() {
        anyhow::bail!("truncated weight data");
    }
    let mut vertex_weights = vec![[0.0f32; MAX_ACTIVE_LAYERS as usize]; count];
    for i in 0..count {
        for j in 0..MAX_ACTIVE_LAYERS as usize {
            let o = offset + (i * MAX_ACTIVE_LAYERS as usize + j) * 4;
            vertex_weights[i][j] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
        }
    }
    offset += expected_weights;

    // Vertex colors (RGB per vertex)
    let expected_colors = count * 3 * 4;
    let mut vertex_colors = vec![[1.0f32; 3]; count];
    if offset + expected_colors <= bytes.len() {
        for i in 0..count {
            for j in 0..3 {
                let o = offset + (i * 3 + j) * 4;
                vertex_colors[i][j] = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
            }
        }
    }

    Ok((
        TerrainChunk {
            cell_coord: coord,
            resolution,
            heights,
            active_layer_ids,
            active_layer_count: active_layer_count as u8,
            vertex_weights,
            vertex_colors,
        },
        texture_layers,
    ))
}

/// Version 1: old texture_index format — convert to new weight system.
fn read_v1_format(bytes: &[u8], coord: CellCoord) -> Result<(TerrainChunk, Vec<String>)> {
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
        let path_len = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2;
        let path = String::from_utf8_lossy(&bytes[offset..offset + path_len]).into_owned();
        offset += path_len;
        texture_layers.push(path);
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

    // Convert old single-index format to new weight format.
    // Find all unique rounded layer indices used in this chunk.
    let mut unique_layers: Vec<u32> = Vec::new();
    for &ti in &texture_index {
        let layer = ti.round() as i32;
        if layer >= 0 && !unique_layers.contains(&(layer as u32)) {
            unique_layers.push(layer as u32);
        }
    }
    unique_layers.sort();
    // Default to layer 0 if nothing found.
    if unique_layers.is_empty() {
        unique_layers.push(0);
    }

    // Cap to MAX_ACTIVE_LAYERS.
    let active_count = unique_layers.len().min(MAX_ACTIVE_LAYERS as usize);
    let mut active_layer_ids = [0u32; MAX_ACTIVE_LAYERS as usize];
    for (i, &id) in unique_layers.iter().enumerate().take(active_count) {
        active_layer_ids[i] = id;
    }

    // Build lookup: old layer index → new slot.
    let mut layer_to_slot: HashMap<u32, usize> = HashMap::new();
    for (i, &id) in unique_layers.iter().enumerate().take(active_count) {
        layer_to_slot.insert(id, i);
    }

    let mut vertex_weights = vec![[0.0f32; MAX_ACTIVE_LAYERS as usize]; count];
    for (vi, &ti) in texture_index.iter().enumerate() {
        let layer = ti.round() as u32;
        if let Some(&slot) = layer_to_slot.get(&layer) {
            vertex_weights[vi][slot] = 1.0;
        } else {
            vertex_weights[vi][0] = 1.0; // fallback to base
        }
    }

    Ok((
        TerrainChunk {
            cell_coord: coord,
            resolution,
            heights,
            active_layer_ids,
            active_layer_count: active_count as u8,
            vertex_weights,
            vertex_colors: vec![[1.0, 1.0, 1.0]; count],
        },
        texture_layers,
    ))
}

/// Fallback for the original format: no texture table, single-index format.
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

    // Convert to new format — all vertices become slot 0 (base layer = texture_index[0] rounded).
    let base_layer = texture_index
        .iter()
        .map(|v| v.round() as u32)
        .reduce(|a, b| if a == b { a } else { 0 })
        .unwrap_or(0);

    let mut active_layer_ids = [0u32; MAX_ACTIVE_LAYERS as usize];
    active_layer_ids[0] = base_layer;

    let mut vertex_weights = vec![[0.0f32; MAX_ACTIVE_LAYERS as usize]; count];
    for (vi, &ti) in texture_index.iter().enumerate() {
        let layer = ti.round() as u32;
        vertex_weights[vi][0] = if layer == base_layer { 1.0 } else { 0.0 };
        // Try to find the layer in active_layer_ids; if it matches base, fine.
        // Otherwise the vertex gets 0 weight (will show as 0-contribution in shader).
    }

    Ok((
        TerrainChunk {
            cell_coord: coord,
            resolution,
            heights,
            active_layer_ids,
            active_layer_count: 1,
            vertex_weights,
            vertex_colors: vec![[1.0, 1.0, 1.0]; count],
        },
        Vec::new(),
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
