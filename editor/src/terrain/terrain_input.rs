use anyhow::Result;
use apostasy_core::{
    cgmath::{SquareMatrix, Vector3, Vector4},
    objects::{
        cell::{CellCoord, CELL_SIZE, world_to_cell},
        components::transform::Transform,
        world::World,
    },
    rendering::components::camera::{Camera, EditorCamera, get_perspective_projection, get_view_matrix},
    terrain::{
        TerrainChunkMap, TerrainSettings,
        chunk::{NeedsTerrainRebuild, TerrainChunk},
    },
    ui::ui_context::ViewportSize,
    update,
    winit::event::MouseButton,
};

use crate::{
    terrain::{TerrainTool, TerrainToolState},
    ui::viewport_panel::ViewportInfo,
};

#[update(mode = "editor")]
pub fn terrain_input(world: &mut World) -> Result<()> {
    if !world.has_resource::<TerrainToolState>() {
        return Ok(());
    }
    let active = world.get_resource::<TerrainToolState>().map(|s| s.active).unwrap_or(false);
    if !active {
        return Ok(());
    }

    let viewport_hovered = world
        .get_resource::<ViewportInfo>()
        .map(|v| v.is_hovered)
        .unwrap_or(false);
    if !viewport_hovered {
        return Ok(());
    }

    let input = world.get_resource::<apostasy_core::objects::resources::input_manager::InputManager>()?.clone();
    let mouse_held = input.mouse_held.contains(&MouseButton::Left);
    let mouse_pressed = input.mouse_pressed.contains(&MouseButton::Left);
    let mouse_released = input.mouse_released.contains(&MouseButton::Left);

    // Update dragging flag
    {
        if let Ok(state) = world.get_resource_mut::<TerrainToolState>() {
            if mouse_released {
                state.dragging = false;
                // Clear flatten height when drag ends so next click re-samples
                if state.tool == TerrainTool::Flatten {
                    state.flatten_height = None;
                }
            } else if mouse_pressed {
                state.dragging = true;
            }
        }
    }

    if !mouse_held {
        return Ok(());
    }

    let (ray_origin, ray_dir) = match compute_ray(world) {
        Some(r) => r,
        None => return Ok(()),
    };

    // For initial creation and general hit: intersect with terrain heightmap or y=0 plane
    let hit_pos = match intersect_terrain_or_plane(world, ray_origin, ray_dir) {
        Some(p) => p,
        None => return Ok(()),
    };

    let state = world.get_resource::<TerrainToolState>()?.clone();

    let affected_cells = cells_in_radius(hit_pos, state.brush_radius);
    let resolution = world.get_resource::<TerrainSettings>().map(|s| s.resolution).unwrap_or(128);

    // Ensure all affected cells have terrain objects
    for &cell in &affected_cells {
        ensure_terrain_chunk(world, cell, resolution);
    }

    // Sample flatten height on first click (before applying brush)
    if mouse_pressed && state.tool == TerrainTool::Flatten {
        let sampled = sample_height_at(world, hit_pos);
        if let Ok(s) = world.get_resource_mut::<TerrainToolState>() {
            s.flatten_height = Some(sampled);
        }
    }

    let flatten_target = world.get_resource::<TerrainToolState>()?.flatten_height;

    // Apply brush to each affected chunk
    for &cell in &affected_cells {
        let chunk_map = world.get_resource::<TerrainChunkMap>()?.clone();
        if let Some(&obj_id) = chunk_map.0.get(&cell) {
            apply_brush(world, obj_id, hit_pos, &state, flatten_target, resolution);
        }
    }

    Ok(())
}

fn compute_ray(world: &World) -> Option<(Vector3<f32>, Vector3<f32>)> {
    let input = world.get_resource::<apostasy_core::objects::resources::input_manager::InputManager>().ok()?;
    let viewport = world.get_resource::<ViewportSize>().ok()?;

    let mouse_physical = input.mouse_position;
    let ppp = {
        let ctx = world.get_resource::<apostasy_core::ui::ui_context::EguiContext>().ok()?;
        ctx.0.pixels_per_point()
    };
    let mouse_logical_x = mouse_physical.x as f32 / ppp;
    let mouse_logical_y = mouse_physical.y as f32 / ppp;

    // Position within viewport
    let vx = mouse_logical_x - viewport.logical_x;
    let vy = mouse_logical_y - viewport.logical_y;

    if vx < 0.0 || vy < 0.0 || vx > viewport.logical_width || vy > viewport.logical_height {
        return None;
    }

    let ndc_x = (vx / viewport.logical_width) * 2.0 - 1.0;
    let ndc_y = (vy / viewport.logical_height) * 2.0 - 1.0;

    let cam_objs = world.get_objects_with_tag::<EditorCamera>();
    let cam_obj = cam_objs.first()?;
    let cam_t = cam_obj.get_component::<Transform>().ok()?;
    let cam_c = cam_obj.get_component::<Camera>().ok()?;
    let aspect = viewport.aspect_logical();

    let proj = get_perspective_projection(cam_c, aspect);
    let view = get_view_matrix(cam_t);
    let inv_vp = (proj * view).invert()?;

    let near = inv_vp * Vector4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far  = inv_vp * Vector4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near = Vector3::new(near.x / near.w, near.y / near.w, near.z / near.w);
    let far  = Vector3::new(far.x / far.w, far.y / far.w, far.z / far.w);

    let dir = far - near;
    let len = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
    if len < 1e-6 { return None; }
    Some((near, Vector3::new(dir.x / len, dir.y / len, dir.z / len)))
}

/// Intersects the ray with the terrain heightmap; falls back to y=0 plane.
fn intersect_terrain_or_plane(world: &World, origin: Vector3<f32>, dir: Vector3<f32>) -> Option<Vector3<f32>> {
    // Step along ray looking for terrain intersection (coarse search)
    let step = 1.0f32;
    let max_dist = 2000.0f32;
    let mut t = 0.0f32;
    let mut last_above = true;

    while t < max_dist {
        let p = origin + dir * t;
        let terrain_h = sample_height_at(world, p);
        let above = p.y > terrain_h;
        if t > 0.0 && above != last_above {
            // Binary search refinement
            let mut lo = t - step;
            let mut hi = t;
            for _ in 0..8 {
                let mid = (lo + hi) * 0.5;
                let pm = origin + dir * mid;
                if pm.y > sample_height_at(world, pm) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let hit = origin + dir * ((lo + hi) * 0.5);
            return Some(hit);
        }
        last_above = above;
        t += step;
    }

    // Fallback: intersect y=0 plane
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let t_plane = -origin.y / dir.y;
    if t_plane < 0.0 {
        return None;
    }
    Some(origin + dir * t_plane)
}

/// Samples terrain height at a world XZ position.
fn sample_height_at(world: &World, pos: Vector3<f32>) -> f32 {
    let cell = world_to_cell(pos);
    let chunk_map = match world.get_resource::<TerrainChunkMap>() {
        Ok(m) => m,
        Err(_) => return 0.0,
    };
    let obj_id = match chunk_map.0.get(&cell) {
        Some(&id) => id,
        None => return 0.0,
    };
    let obj = match world.get_object(obj_id) {
        Some(o) => o,
        None => return 0.0,
    };
    let chunk = match obj.get_component::<TerrainChunk>() {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    let r = chunk.resolution as f32;
    let cell_size = CELL_SIZE as f32;
    let (ox, oz) = chunk.world_origin();
    let local_x = ((pos.x - ox) / cell_size * r).clamp(0.0, r);
    let local_z = ((pos.z - oz) / cell_size * r).clamp(0.0, r);
    let xi = local_x as usize;
    let zi = local_z as usize;
    let fx = local_x.fract();
    let fz = local_z.fract();
    let side = chunk.resolution as usize;
    let xi1 = (xi + 1).min(side);
    let zi1 = (zi + 1).min(side);
    let h00 = chunk.height_at(xi, zi);
    let h10 = chunk.height_at(xi1, zi);
    let h01 = chunk.height_at(xi, zi1);
    let h11 = chunk.height_at(xi1, zi1);
    h00 * (1.0 - fx) * (1.0 - fz)
        + h10 * fx * (1.0 - fz)
        + h01 * (1.0 - fx) * fz
        + h11 * fx * fz
}

/// Returns all cell coords that overlap a circle of `radius` centered at `pos`.
fn cells_in_radius(pos: Vector3<f32>, radius: f32) -> Vec<CellCoord> {
    let cell_size = CELL_SIZE as f32;
    let min_x = ((pos.x - radius) / cell_size).floor() as i32;
    let max_x = ((pos.x + radius) / cell_size).ceil() as i32;
    let min_z = ((pos.z - radius) / cell_size).floor() as i32;
    let max_z = ((pos.z + radius) / cell_size).ceil() as i32;
    let mut cells = Vec::new();
    for cx in min_x..=max_x {
        for cz in min_z..=max_z {
            cells.push(Vector3::new(cx, 0, cz));
        }
    }
    cells
}

/// Ensures a terrain chunk object exists for `cell`. Creates it if missing.
fn ensure_terrain_chunk(world: &mut World, cell: CellCoord, resolution: u32) {
    let exists = world
        .get_resource::<TerrainChunkMap>()
        .ok()
        .and_then(|m| m.0.get(&cell).copied())
        .is_some();
    if exists {
        return;
    }

    let mut obj = apostasy_core::objects::Object::default();
    obj.name = format!("Terrain ({},{})", cell.x, cell.z);
    let chunk = TerrainChunk::new(cell, resolution);
    obj.add_component(chunk);
    obj.add_tag(NeedsTerrainRebuild);

    let id = world.add_object(obj);
    if let Ok(map) = world.get_resource_mut::<TerrainChunkMap>() {
        map.0.insert(cell, id);
    }
}

fn apply_brush(
    world: &mut World,
    obj_id: apostasy_core::objects::cell::ObjectId,
    hit_pos: Vector3<f32>,
    state: &TerrainToolState,
    flatten_target: Option<f32>,
    resolution: u32,
) {
    let obj = match world.get_object_mut(obj_id) {
        Some(o) => o,
        None => return,
    };
    let chunk = match obj.get_component_mut::<TerrainChunk>() {
        Ok(c) => c,
        Err(_) => return,
    };

    let r = resolution as usize;
    let cell_size = CELL_SIZE as f32;
    let (ox, oz) = chunk.world_origin();
    let radius = state.brush_radius;
    let strength = state.brush_strength;

    for z in 0..=(r) {
        for x in 0..=(r) {
            let step = cell_size / r as f32;
            let wx = ox + x as f32 * step;
            let wz = oz + z as f32 * step;
            let dx = wx - hit_pos.x;
            let dz = wz - hit_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            if dist >= radius {
                continue;
            }
            let weight = gaussian_weight(dist, radius) * strength;
            let h = chunk.height_at_mut(x, z);

            match state.tool {
                TerrainTool::Raise => *h += weight,
                TerrainTool::Lower => *h -= weight,
                TerrainTool::Smooth => {
                    // Smoothing is applied as a separate pass below
                }
                TerrainTool::Flatten => {
                    if let Some(target) = flatten_target {
                        *h += (*h - target) * -weight;
                    }
                }
                TerrainTool::Paint => {
                    let side = r + 1;
                    let idx = x + z * side;
                    if idx < chunk.texture_weights.len() {
                        let layer = state.paint_layer.min(3);
                        let w = (weight * 255.0) as u8;
                        blend_paint_weight(&mut chunk.texture_weights[idx], layer, w);
                    }
                }
            }
        }
    }

    // Smooth pass: replace heights with local average
    if state.tool == TerrainTool::Smooth {
        let side = r + 1;
        let old_heights = chunk.heights.clone();
        for z in 0..=r {
            for x in 0..=r {
                let step = cell_size / r as f32;
                let wx = ox + x as f32 * step;
                let wz = oz + z as f32 * step;
                let dx = wx - hit_pos.x;
                let dz = wz - hit_pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist >= radius {
                    continue;
                }
                let weight = gaussian_weight(dist, radius) * strength;
                let avg = neighbor_avg(&old_heights, x, z, side);
                let h = &mut chunk.heights[x + z * side];
                *h = *h * (1.0 - weight) + avg * weight;
            }
        }
    }

    // Mark dirty
    if let Some(obj) = world.get_object_mut(obj_id) {
        obj.add_tag(NeedsTerrainRebuild);
    }
}

fn gaussian_weight(dist: f32, radius: f32) -> f32 {
    let t = dist / radius;
    (-3.0 * t * t).exp()
}

fn neighbor_avg(heights: &[f32], x: usize, z: usize, side: usize) -> f32 {
    let mut sum = 0.0f32;
    let mut count = 0u32;
    for dz in -1i32..=1 {
        for dx in -1i32..=1 {
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx >= 0 && nz >= 0 && (nx as usize) < side && (nz as usize) < side {
                sum += heights[nx as usize + nz as usize * side];
                count += 1;
            }
        }
    }
    if count > 0 { sum / count as f32 } else { heights[x + z * side] }
}

/// Blends a paint weight into the RGBA weight array for the given layer, renormalizing.
fn blend_paint_weight(weights: &mut [u8; 4], layer: usize, amount: u8) {
    let add = amount as u16;
    let old = weights[layer] as u16;
    let new_val = (old + add).min(255) as u8;
    let diff = new_val - weights[layer];
    weights[layer] = new_val;
    // Reduce other channels proportionally
    let total_other: u16 = weights.iter().enumerate()
        .filter(|(i, _)| *i != layer)
        .map(|(_, &w)| w as u16)
        .sum();
    if total_other > 0 && diff > 0 {
        let reduce = diff as u16;
        for i in 0..4 {
            if i != layer && weights[i] > 0 {
                let take = (weights[i] as u16 * reduce / total_other) as u8;
                weights[i] = weights[i].saturating_sub(take);
            }
        }
    }
}
