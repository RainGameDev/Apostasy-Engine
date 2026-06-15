use anyhow::Result;
use apostasy_core::{
    cgmath::{SquareMatrix, Vector3, Vector4},
    objects::{
        cell::{CELL_SIZE, CellCoord, ObjectId, world_to_cell},
        components::transform::Transform,
        tags::skips_serilization::SkipsSerilization,
        world::World,
    },
    rendering::components::camera::{
        Camera, EditorCamera, get_perspective_projection, get_view_matrix,
    },
    terrain::{
        TerrainChunkMap, TerrainSettings,
        chunk::{NeedsTerrainRebuild, TerrainChunk},
    },
    ui::ui_context::ViewportSize,
    update,
    winit::event::MouseButton,
};

use crate::{
    terrain::{TerrainBrushGizmo, TerrainTool, TerrainToolState},
    ui::viewport_panel::ViewportInfo,
};

#[update(mode = "editor")]
pub fn terrain_input(world: &mut World) -> Result<()> {
    if !world.has_resource::<TerrainBrushGizmo>() {
        world.insert_resource(TerrainBrushGizmo::default());
    }

    let clear_gizmo = |world: &mut World| {
        if let Ok(g) = world.get_resource_mut::<TerrainBrushGizmo>() {
            g.hit_pos = None;
        }
    };

    if !world.has_resource::<TerrainToolState>() {
        clear_gizmo(world);
        return Ok(());
    }
    let active = world
        .get_resource::<TerrainToolState>()
        .map(|s| s.active)
        .unwrap_or(false);
    if !active {
        clear_gizmo(world);
        return Ok(());
    }

    let viewport_hovered = world
        .get_resource::<ViewportInfo>()
        .map(|v| v.is_hovered)
        .unwrap_or(false);
    if !viewport_hovered {
        clear_gizmo(world);
        return Ok(());
    }

    let input = world
        .get_resource::<apostasy_core::objects::resources::input_manager::InputManager>()?
        .clone();
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

    // Compute hit position for gizmo regardless of mouse state
    let (ray_origin, ray_dir) = match compute_ray(world) {
        Some(r) => r,
        None => {
            clear_gizmo(world);
            return Ok(());
        }
    };

    // For initial creation and general hit: intersect with terrain heightmap or y=0 plane
    let hit_pos = match intersect_terrain_or_plane(world, ray_origin, ray_dir) {
        Some(p) => p,
        None => {
            clear_gizmo(world);
            return Ok(());
        }
    };

    if let Ok(g) = world.get_resource_mut::<TerrainBrushGizmo>() {
        g.hit_pos = Some(hit_pos);
    }

    if !mouse_held {
        return Ok(());
    }

    let state = world.get_resource::<TerrainToolState>()?.clone();

    let affected_cells = cells_in_radius(hit_pos, state.brush_radius);
    let resolution = world
        .get_resource::<TerrainSettings>()
        .map(|s| s.resolution)
        .unwrap_or(128);

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

    // Ensure shared border vertices are equal across adjacent chunks
    stitch_chunk_borders(world, &affected_cells, resolution);

    Ok(())
}

fn compute_ray(world: &World) -> Option<(Vector3<f32>, Vector3<f32>)> {
    let input = world
        .get_resource::<apostasy_core::objects::resources::input_manager::InputManager>()
        .ok()?;
    let viewport = world.get_resource::<ViewportSize>().ok()?;

    let mouse_physical = input.mouse_position;
    let ppp = {
        let ctx = world
            .get_resource::<apostasy_core::ui::ui_context::EguiContext>()
            .ok()?;
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
    let far = inv_vp * Vector4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near = Vector3::new(near.x / near.w, near.y / near.w, near.z / near.w);
    let far = Vector3::new(far.x / far.w, far.y / far.w, far.z / far.w);

    let dir = far - near;
    let len = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
    if len < 1e-6 {
        return None;
    }
    Some((near, Vector3::new(dir.x / len, dir.y / len, dir.z / len)))
}

/// Intersects the ray with the terrain heightmap; falls back to y=0 plane.
fn intersect_terrain_or_plane(
    world: &World,
    origin: Vector3<f32>,
    dir: Vector3<f32>,
) -> Option<Vector3<f32>> {
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
    h00 * (1.0 - fx) * (1.0 - fz) + h10 * fx * (1.0 - fz) + h01 * (1.0 - fx) * fz + h11 * fx * fz
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
    obj.add_tag(SkipsSerilization);

    let id = world.add_object(obj);
    if let Ok(map) = world.get_resource_mut::<TerrainChunkMap>() {
        map.0.insert(cell, id);
    }

    // Seed borders from existing neighbors so this chunk starts seamless
    init_chunk_borders(world, cell, id, resolution);
}

/// Copies neighbor edge heights into a freshly created chunk's border vertices.
fn init_chunk_borders(world: &mut World, cell: CellCoord, new_id: ObjectId, resolution: u32) {
    let r = resolution as usize;
    let side = r + 1;

    let chunk_map = match world.get_resource::<TerrainChunkMap>() {
        Ok(m) => m.clone(),
        Err(_) => return,
    };

    // Each block collects owned Vec<f32> — world borrow ends before the next block.
    let left_col: Option<Vec<f32>> = {
        chunk_map
            .0
            .get(&Vector3::new(cell.x - 1, 0, cell.z))
            .and_then(|&nid| world.get_object(nid))
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
            .map(|c| (0..side).map(|z| c.heights[r + z * side]).collect())
    };
    let right_col: Option<Vec<f32>> = {
        chunk_map
            .0
            .get(&Vector3::new(cell.x + 1, 0, cell.z))
            .and_then(|&nid| world.get_object(nid))
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
            .map(|c| (0..side).map(|z| c.heights[z * side]).collect())
    };
    let top_row: Option<Vec<f32>> = {
        chunk_map
            .0
            .get(&Vector3::new(cell.x, 0, cell.z - 1))
            .and_then(|&nid| world.get_object(nid))
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
            .map(|c| (0..side).map(|x| c.heights[x + r * side]).collect())
    };
    let bottom_row: Option<Vec<f32>> = {
        chunk_map
            .0
            .get(&Vector3::new(cell.x, 0, cell.z + 1))
            .and_then(|&nid| world.get_object(nid))
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
            .map(|c| (0..side).map(|x| c.heights[x]).collect())
    };

    if let Some(obj) = world.get_object_mut(new_id) {
        if let Ok(chunk) = obj.get_component_mut::<TerrainChunk>() {
            if let Some(col) = left_col {
                for z in 0..side {
                    chunk.heights[z * side] = col[z];
                }
            }
            if let Some(col) = right_col {
                for z in 0..side {
                    chunk.heights[r + z * side] = col[z];
                }
            }
            if let Some(row) = top_row {
                for x in 0..side {
                    chunk.heights[x] = row[x];
                }
            }
            if let Some(row) = bottom_row {
                for x in 0..side {
                    chunk.heights[x + r * side] = row[x];
                }
            }
        }
    }
}

/// Averages shared border vertices between all adjacent chunk pairs in `cells`.
/// Also covers adjacent chunks that exist but weren't painted (handles legacy data).
fn stitch_chunk_borders(world: &mut World, cells: &[CellCoord], resolution: u32) {
    let chunk_map = match world.get_resource::<TerrainChunkMap>() {
        Ok(m) => m.clone(),
        Err(_) => return,
    };

    for &cell in cells {
        let right = Vector3::new(cell.x + 1, 0, cell.z);
        if chunk_map.0.contains_key(&right) {
            stitch_x_border(world, &chunk_map, cell, right, resolution);
        }

        let bottom = Vector3::new(cell.x, 0, cell.z + 1);
        if chunk_map.0.contains_key(&bottom) {
            stitch_z_border(world, &chunk_map, cell, bottom, resolution);
        }
    }

    stitch_corners(world, &chunk_map, cells, resolution);
}

fn stitch_x_border(
    world: &mut World,
    chunk_map: &TerrainChunkMap,
    left_cell: CellCoord,
    right_cell: CellCoord,
    resolution: u32,
) {
    let r = resolution as usize;
    let side = r + 1;
    let (&left_id, &right_id) = match (chunk_map.0.get(&left_cell), chunk_map.0.get(&right_cell)) {
        (Some(l), Some(r)) => (l, r),
        _ => return,
    };

    let avg: Vec<f32> = {
        let l_col: Vec<f32> = match world
            .get_object(left_id)
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
        {
            Some(c) => (0..side).map(|z| c.heights[r + z * side]).collect(),
            None => return,
        };
        let r_col: Vec<f32> = match world
            .get_object(right_id)
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
        {
            Some(c) => (0..side).map(|z| c.heights[z * side]).collect(),
            None => return,
        };
        l_col
            .iter()
            .zip(&r_col)
            .map(|(a, b)| (a + b) * 0.5)
            .collect()
    };

    if let Some(obj) = world.get_object_mut(left_id) {
        if let Ok(chunk) = obj.get_component_mut::<TerrainChunk>() {
            for z in 0..side {
                chunk.heights[r + z * side] = avg[z];
            }
        }
        obj.add_tag(NeedsTerrainRebuild);
    }
    if let Some(obj) = world.get_object_mut(right_id) {
        if let Ok(chunk) = obj.get_component_mut::<TerrainChunk>() {
            for z in 0..side {
                chunk.heights[z * side] = avg[z];
            }
        }
        obj.add_tag(NeedsTerrainRebuild);
    }
}

fn stitch_z_border(
    world: &mut World,
    chunk_map: &TerrainChunkMap,
    top_cell: CellCoord,
    bottom_cell: CellCoord,
    resolution: u32,
) {
    let r = resolution as usize;
    let side = r + 1;
    let (&top_id, &bot_id) = match (chunk_map.0.get(&top_cell), chunk_map.0.get(&bottom_cell)) {
        (Some(t), Some(b)) => (t, b),
        _ => return,
    };

    let avg: Vec<f32> = {
        let t_row: Vec<f32> = match world
            .get_object(top_id)
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
        {
            Some(c) => (0..side).map(|x| c.heights[x + r * side]).collect(),
            None => return,
        };
        let b_row: Vec<f32> = match world
            .get_object(bot_id)
            .and_then(|o| o.get_component::<TerrainChunk>().ok())
        {
            Some(c) => (0..side).map(|x| c.heights[x]).collect(),
            None => return,
        };
        t_row
            .iter()
            .zip(&b_row)
            .map(|(a, b)| (a + b) * 0.5)
            .collect()
    };

    if let Some(obj) = world.get_object_mut(top_id) {
        if let Ok(chunk) = obj.get_component_mut::<TerrainChunk>() {
            for x in 0..side {
                chunk.heights[x + r * side] = avg[x];
            }
        }
        obj.add_tag(NeedsTerrainRebuild);
    }
    if let Some(obj) = world.get_object_mut(bot_id) {
        if let Ok(chunk) = obj.get_component_mut::<TerrainChunk>() {
            for x in 0..side {
                chunk.heights[x] = avg[x];
            }
        }
        obj.add_tag(NeedsTerrainRebuild);
    }
}

/// Fixes the single corner vertex shared by up to 4 chunks after border stitching.
fn stitch_corners(
    world: &mut World,
    chunk_map: &TerrainChunkMap,
    cells: &[CellCoord],
    resolution: u32,
) {
    let r = resolution as usize;
    let side = r + 1;

    for &cell in cells {
        // Shares the (r,r) corner with three neighbors at (0,r), (r,0), and (0,0).
        let neighbor_slots: [(CellCoord, usize); 3] = [
            (Vector3::new(cell.x + 1, 0, cell.z), r * side), // (x=0, z=r)
            (Vector3::new(cell.x, 0, cell.z + 1), r),        // (x=r, z=0)
            (Vector3::new(cell.x + 1, 0, cell.z + 1), 0),    // (x=0, z=0)
        ];
        let corner_idx = r + r * side;

        // Read phase: collect (id, index, height) for all present chunks
        let mut entries: Vec<(ObjectId, usize, f32)> = Vec::with_capacity(4);
        if let Some(&id) = chunk_map.0.get(&cell) {
            if let Some(h) = world
                .get_object(id)
                .and_then(|o| o.get_component::<TerrainChunk>().ok())
                .map(|c| c.heights[corner_idx])
            {
                entries.push((id, corner_idx, h));
            }
        }
        for &(nc, idx) in &neighbor_slots {
            if let Some(&id) = chunk_map.0.get(&nc) {
                if let Some(h) = world
                    .get_object(id)
                    .and_then(|o| o.get_component::<TerrainChunk>().ok())
                    .map(|c| c.heights[idx])
                {
                    entries.push((id, idx, h));
                }
            }
        }

        if entries.len() < 2 {
            continue;
        }
        let avg = entries.iter().map(|(_, _, h)| h).sum::<f32>() / entries.len() as f32;

        // Write phase
        for (id, idx, _) in entries {
            if let Some(obj) = world.get_object_mut(id) {
                if let Ok(chunk) = obj.get_component_mut::<TerrainChunk>() {
                    chunk.heights[idx] = avg;
                }
                obj.add_tag(NeedsTerrainRebuild);
            }
        }
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
                    if idx < chunk.texture_index.len() && weight > 0.01 {
                        let layer = state.paint_layer as f32;
                        let cur = chunk.texture_index[idx];
                        // Smoothly blend toward the target index
                        let blend = (weight * 0.5).min(0.5);
                        chunk.texture_index[idx] = cur * (1.0 - blend) + layer * blend;
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
    if count > 0 {
        sum / count as f32
    } else {
        heights[x + z * side]
    }
}


