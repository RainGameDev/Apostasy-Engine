use anyhow::Result;
use apostasy_core::{
    cgmath::{SquareMatrix, Vector3, Vector4},
    ecs::{
        cell::{CELL_SIZE, CellCoord, ObjectId, world_to_cell},
        components::transform::Transform,
        resources::input_manager::{InputManager, KeyAction, KeyBind},
        tags::skips_serilization::SkipsSerilization,
        world::World,
    },
    rendering::components::camera::{
        Camera, EditorCamera, get_perspective_projection, get_view_matrix,
    },
    start,
    terrain::{
        TerrainChunkMap, TerrainSettings,
        chunk::{NeedsTerrainRebuild, TerrainChunk},
    },
    ui::ui_context::ViewportSize,
    update,
    winit::{
        event::MouseButton,
        keyboard::{KeyCode, PhysicalKey},
    },
};

use crate::{
    terrain::{TerrainBrushGizmo, TerrainTool, TerrainToolState},
    ui::viewport_panel::ViewportInfo,
};

#[start(mode = "editor")]
pub fn terrain_init_input(world: &mut World) -> Result<()> {
    let inputs = world.get_resource_mut::<InputManager>().unwrap();
    inputs.register_default_keybind(
        "TerrainSmooth",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyS), KeyAction::Press),
    );
    inputs.register_default_keybind(
        "TerrainFlatten",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyF), KeyAction::Press),
    );
    inputs.register_default_keybind(
        "TerrainColorEdit",
        KeyBind::new(PhysicalKey::Code(KeyCode::KeyC), KeyAction::Press),
    );
    Ok(())
}

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

    let input = world.get_resource::<InputManager>()?.clone();
    let middle_mouse = input.mouse_held.contains(&MouseButton::Middle);
    let mouse_held = input.mouse_held.contains(&MouseButton::Left);
    let mouse_pressed = input.mouse_pressed.contains(&MouseButton::Left);
    let mouse_released = input.mouse_released.contains(&MouseButton::Left);
    let right_click = input.mouse_held.contains(&MouseButton::Right);
    let shift_pressed = input.is_keybind_active("ShiftModifier");
    let toggle_smooth = input.is_keybind_active("TerrainSmooth");
    let toggle_flatten = input.is_keybind_active("TerrainFlatten");
    let toggle_color_edit = input.is_keybind_active("TerrainColorEdit");

    if let Ok(state) = world.get_resource_mut::<TerrainToolState>()
        && !middle_mouse
    {
        if toggle_flatten {
            if state.tool == TerrainTool::Flatten {
                state.tool = TerrainTool::Modify;
            } else {
                state.tool = TerrainTool::Flatten;
            }
        }

        if toggle_smooth {
            if state.tool == TerrainTool::Smooth {
                state.tool = TerrainTool::Modify;
            } else {
                state.tool = TerrainTool::Smooth;
            }
        }

        if toggle_color_edit {
            state.is_vertex_painting = !state.is_vertex_painting;
        }
    }

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
    if !mouse_held && !right_click {
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
            apply_brush(
                world,
                obj_id,
                hit_pos,
                &state,
                flatten_target,
                resolution,
                shift_pressed,
                right_click,
            );
        }
    }

    // Ensure shared border vertices are equal across adjacent chunks
    stitch_chunk_borders(world, &affected_cells, resolution);

    Ok(())
}

fn compute_ray(world: &World) -> Option<(Vector3<f32>, Vector3<f32>)> {
    let input = world
        .get_resource::<apostasy_core::ecs::resources::input_manager::InputManager>()
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

    let cam_id = world.get_entity_with_tag::<EditorCamera>().ok()?;
    let cam_t_owned = world.get_component::<Transform>(cam_id)?.clone();
    let cam_c_owned = world.get_component::<Camera>(cam_id)?.clone();
    let cam_t = &cam_t_owned;
    let cam_c = &cam_c_owned;
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
    let chunk = match world.get_component::<TerrainChunk>(obj_id) {
        Some(c) => c,
        None => return 0.0,
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

    let id = world.spawn();
    world.set_name(id, &format!("Terrain ({},{})", cell.x, cell.z));
    world.add_component(id, TerrainChunk::new(cell, resolution));
    world.add_tag::<NeedsTerrainRebuild>(id);
    world.add_tag::<SkipsSerilization>(id);
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
    let left_col: Option<Vec<f32>> = chunk_map
        .0
        .get(&Vector3::new(cell.x - 1, 0, cell.z))
        .and_then(|&nid| world.get_component::<TerrainChunk>(nid))
        .map(|c| (0..side).map(|z| c.heights[r + z * side]).collect());
    let right_col: Option<Vec<f32>> = chunk_map
        .0
        .get(&Vector3::new(cell.x + 1, 0, cell.z))
        .and_then(|&nid| world.get_component::<TerrainChunk>(nid))
        .map(|c| (0..side).map(|z| c.heights[z * side]).collect());
    let top_row: Option<Vec<f32>> = chunk_map
        .0
        .get(&Vector3::new(cell.x, 0, cell.z - 1))
        .and_then(|&nid| world.get_component::<TerrainChunk>(nid))
        .map(|c| (0..side).map(|x| c.heights[x + r * side]).collect());
    let bottom_row: Option<Vec<f32>> = chunk_map
        .0
        .get(&Vector3::new(cell.x, 0, cell.z + 1))
        .and_then(|&nid| world.get_component::<TerrainChunk>(nid))
        .map(|c| (0..side).map(|x| c.heights[x]).collect());

    if let Some(chunk) = world.get_component_mut::<TerrainChunk>(new_id) {
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
        let l_col: Vec<f32> = match world.get_component::<TerrainChunk>(left_id) {
            Some(c) => (0..side).map(|z| c.heights[r + z * side]).collect(),
            None => return,
        };
        let r_col: Vec<f32> = match world.get_component::<TerrainChunk>(right_id) {
            Some(c) => (0..side).map(|z| c.heights[z * side]).collect(),
            None => return,
        };
        l_col.iter().zip(&r_col).map(|(a, b)| (a + b) * 0.5).collect()
    };

    if let Some(chunk) = world.get_component_mut::<TerrainChunk>(left_id) {
        for z in 0..side {
            chunk.heights[r + z * side] = avg[z];
        }
    }
    world.add_tag::<NeedsTerrainRebuild>(left_id);

    if let Some(chunk) = world.get_component_mut::<TerrainChunk>(right_id) {
        for z in 0..side {
            chunk.heights[z * side] = avg[z];
        }
    }
    world.add_tag::<NeedsTerrainRebuild>(right_id);
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
        let t_row: Vec<f32> = match world.get_component::<TerrainChunk>(top_id) {
            Some(c) => (0..side).map(|x| c.heights[x + r * side]).collect(),
            None => return,
        };
        let b_row: Vec<f32> = match world.get_component::<TerrainChunk>(bot_id) {
            Some(c) => (0..side).map(|x| c.heights[x]).collect(),
            None => return,
        };
        t_row.iter().zip(&b_row).map(|(a, b)| (a + b) * 0.5).collect()
    };

    if let Some(chunk) = world.get_component_mut::<TerrainChunk>(top_id) {
        for x in 0..side {
            chunk.heights[x + r * side] = avg[x];
        }
    }
    world.add_tag::<NeedsTerrainRebuild>(top_id);

    if let Some(chunk) = world.get_component_mut::<TerrainChunk>(bot_id) {
        for x in 0..side {
            chunk.heights[x] = avg[x];
        }
    }
    world.add_tag::<NeedsTerrainRebuild>(bot_id);
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
        if let Some(&id) = chunk_map.0.get(&cell)
            && let Some(h) = world
                .get_component::<TerrainChunk>(id)
                .map(|c| c.heights[corner_idx])
        {
            entries.push((id, corner_idx, h));
        }
        for &(nc, idx) in &neighbor_slots {
            if let Some(&id) = chunk_map.0.get(&nc)
                && let Some(h) = world
                    .get_component::<TerrainChunk>(id)
                    .map(|c| c.heights[idx])
            {
                entries.push((id, idx, h));
            }
        }

        if entries.len() < 2 {
            continue;
        }
        let avg = entries.iter().map(|(_, _, h)| h).sum::<f32>() / entries.len() as f32;

        // Write phase
        for (id, idx, _) in entries {
            if let Some(chunk) = world.get_component_mut::<TerrainChunk>(id) {
                chunk.heights[idx] = avg;
            }
            world.add_tag::<NeedsTerrainRebuild>(id);
        }
    }
}

fn apply_brush(
    world: &mut World,
    obj_id: ObjectId,
    hit_pos: Vector3<f32>,
    state: &TerrainToolState,
    flatten_target: Option<f32>,
    resolution: u32,
    shift_pressed: bool,
    right_click_pressed: bool,
) {
    let chunk = match world.get_component_mut::<TerrainChunk>(obj_id) {
        Some(c) => c,
        None => return,
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

            // Vertex color painting takes priority; skip height/texture branches.
            if state.is_vertex_painting {
                let target = if right_click_pressed {
                    state.vertex_color_b
                } else {
                    state.vertex_color_a
                };
                let t = (gaussian_weight(dist, radius) * state.vertex_strength).clamp(0.0, 1.0);
                let idx = x + z * (r + 1);
                if let Some(c) = chunk.vertex_colors.get_mut(idx) {
                    for k in 0..3 {
                        c[k] = c[k] * (1.0 - t) + target[k] * t;
                    }
                }
                continue;
            }

            let h = chunk.height_at_mut(x, z);

            if !right_click_pressed {
                match state.tool {
                    TerrainTool::Modify => {
                        if shift_pressed {
                            *h -= weight
                        } else {
                            *h += weight
                        }
                    }
                    TerrainTool::Smooth => {
                        // Smoothing is applied as a separate pass below
                    }
                    TerrainTool::Flatten => {
                        if let Some(target) = flatten_target {
                            *h += (*h - target) * -weight;
                        }
                    }
                }
            }
            if right_click_pressed {
                let idx = x + z * (r + 1);
                if idx < chunk.vertex_weights.len() && weight > 0.01 {
                    paint_vertex(chunk, idx, state.paint_layer as u32, weight);
                }
            }
        }
    }

    // Smooth pass: replace heights with local average
    if state.tool == TerrainTool::Smooth && !right_click_pressed {
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
    world.add_tag::<NeedsTerrainRebuild>(obj_id);
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

/// Weight-based paint: add weight to the target layer, renormalize all other slots.
fn paint_vertex(chunk: &mut TerrainChunk, vertex_idx: usize, target_layer_id: u32, strength: f32) {
    let slot = match chunk.find_or_allocate_slot(target_layer_id) {
        Some(s) => s,
        None => {
            // All 6 slots are full — can't paint here.
            return;
        }
    };

    let weights = &mut chunk.vertex_weights[vertex_idx];
    let old_target = weights[slot];
    let new_target = (old_target + strength).min(1.0);

    // Renormalize: scale down other slots so total still sums to 1.0.
    let remaining_old = 1.0 - old_target;
    let remaining_new = 1.0 - new_target;
    let scale = if remaining_old > 0.0001 {
        remaining_new / remaining_old
    } else {
        0.0
    };

    for w in weights.iter_mut() {
        *w *= scale;
    }
    weights[slot] = new_target;
}
