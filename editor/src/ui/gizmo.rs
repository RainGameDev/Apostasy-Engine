use apostasy_core::{
    cgmath::{InnerSpace, Matrix3, Matrix4, Rotation3, Vector3, Vector4},
    egui::{self, Color32, Pos2, Stroke, Vec2},
    objects::components::transform::Transform,
    physics::collider::{Collider, ColliderShape},
};
use apostasy_macros::Resource;

const SCREEN_RADIUS: f32 = 90.0;
const HIT_PX: f32 = 8.0;
const RING_SEGS: usize = 48;

#[derive(Clone, Default, PartialEq)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, PartialEq, Debug)]
pub enum GizmoAxis {
    X,
    Y,
    Z,
}

#[derive(Clone)]
pub struct GizmoDrag {
    axis: GizmoAxis,
    /// World-space direction of this axis captured at drag start (local or global)
    axis_world_dir: Vector3<f32>,
    start_mouse: Pos2,
    pub start_transform: Transform,
    screen_dir: Vec2,
    /// world-units/pixel for translation, degrees/pixel for rotate, scale/pixel for scale
    sensitivity: f32,
}

#[derive(Resource, Clone)]
pub struct GizmoState {
    pub mode: GizmoMode,
    /// true = axes follow object orientation, false = world-aligned axes
    pub local: bool,
    pub drag: Option<GizmoDrag>,
    /// True when cursor is near a gizmo handle, suppresses viewport deselect on raycast miss
    pub consuming: bool,
    pub snap_translate: bool,
    pub snap_translate_size: f32,
    pub snap_rotate: bool,
    pub snap_rotate_deg: f32,
    pub snap_scale: bool,
    pub snap_scale_size: f32,
    /// Set each frame by the viewport when Shift is held; XORs against each snap toggle.
    pub shift_snap_held: bool,
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            mode: GizmoMode::default(),
            local: false,
            drag: None,
            consuming: false,
            snap_translate: false,
            snap_translate_size: 1.0,
            snap_rotate: false,
            snap_rotate_deg: 15.0,
            snap_scale: false,
            snap_scale_size: 0.25,
            shift_snap_held: false,
        }
    }
}

fn project(pos: Vector3<f32>, vp: Matrix4<f32>, rect: egui::Rect) -> Option<Pos2> {
    let c = vp * Vector4::new(pos.x, pos.y, pos.z, 1.0);
    if c.w <= 0.0001 {
        return None;
    }
    let nx = c.x / c.w;
    let ny = c.y / c.w;
    Some(Pos2::new(
        (nx + 1.0) * 0.5 * rect.width() + rect.min.x,
        (ny + 1.0) * 0.5 * rect.height() + rect.min.y,
    ))
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let len_sq = ab.length_sq();
    if len_sq < 0.001 {
        return (p - a).length();
    }
    let t = ((ap.x * ab.x + ap.y * ab.y) / len_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// Returns two axes perpendicular to `axis` for drawing a ring
fn ring_perps_from_dir(axis: Vector3<f32>) -> (Vector3<f32>, Vector3<f32>) {
    let ref_vec = if axis.dot(Vector3::new(0.0, 1.0, 0.0)).abs() < 0.9 {
        Vector3::new(0.0, 1.0, 0.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };
    let p1 = axis.cross(ref_vec).normalize();
    let p2 = axis.cross(p1).normalize();
    (p1, p2)
}

/// YXZ-Euler matching `transform_update` systems convention (Ry * Rx * Rz).
/// For that matrix, m[2][1] = -sin(X) and m[2][0] = sin(Y)*cos(X).
fn quat_to_euler_deg(q: apostasy_core::cgmath::Quaternion<f32>) -> Vector3<f32> {
    let m = Matrix3::from(q);
    let sin_x = (-m[2][1]).clamp(-1.0, 1.0);
    let x = sin_x.asin();
    let cos_x = x.cos();
    let (y, z) = if cos_x.abs() > 1e-6 {
        (m[2][0].atan2(m[2][2]), m[0][1].atan2(m[1][1]))
    } else if sin_x > 0.0 {
        // X ≈ +90°: only Y-Z recoverable, set Z=0
        (m[1][0].atan2(m[0][0]), 0.0)
    } else {
        // X ≈ -90°: only Y+Z recoverable, set Z=0
        ((-m[1][0]).atan2(m[0][0]), 0.0)
    };
    Vector3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
}

fn lighten(c: Color32) -> Color32 {
    Color32::from_rgb(
        (c.r() as u32 + 80).min(255) as u8,
        (c.g() as u32 + 80).min(255) as u8,
        (c.b() as u32 + 80).min(255) as u8,
    )
}

/// Draw the gizmo and handle mouse interaction
/// Returns `(new_transform, updated_state)`
pub fn gizmo(
    ui: &mut egui::Ui,
    mut state: GizmoState,
    transform: &Transform,
    view_proj: Matrix4<f32>,
    frame_rect: egui::Rect,
) -> (Option<Transform>, GizmoState) {
    let origin = transform.global_position;

    let center = match project(origin, view_proj, frame_rect) {
        Some(p) => p,
        None => {
            state.consuming = false;
            return (None, state);
        }
    };

    if !frame_rect.expand(80.0).contains(center) {
        state.consuming = false;
        return (None, state);
    }

    // Measure pixels-per-world-unit along all three world axes and take the largest.
    // Using only X would blow up when the camera looks along X (near-zero projected extent);
    // taking the max ensures at least one axis is well-conditioned regardless of view angle.
    let gizmo_len: f32 = {
        let px = |offset: Vector3<f32>| {
            project(origin + offset, view_proj, frame_rect)
                .map(|p| (p - center).length())
                .unwrap_or(0.0)
        };
        let best = px(Vector3::new(1.0, 0.0, 0.0))
            .max(px(Vector3::new(0.0, 1.0, 0.0)))
            .max(px(Vector3::new(0.0, 0.0, 1.0)))
            .max(0.01);
        SCREEN_RADIUS / best
    };

    const AXES: [(GizmoAxis, Color32); 3] = [
        (GizmoAxis::X, Color32::from_rgb(220, 55, 55)),
        (GizmoAxis::Y, Color32::from_rgb(55, 200, 55)),
        (GizmoAxis::Z, Color32::from_rgb(55, 100, 220)),
    ];

    // World-space direction for each axis - rotated by object orientation in local mode
    let axis_dirs: [Vector3<f32>; 3] = if state.local {
        [
            (transform.local_rotation * Vector3::new(1.0, 0.0, 0.0)).normalize(),
            (transform.local_rotation * Vector3::new(0.0, 1.0, 0.0)).normalize(),
            (transform.local_rotation * Vector3::new(0.0, 0.0, 1.0)).normalize(),
        ]
    } else {
        [
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, 0.0, 1.0),
        ]
    };

    let tips: [Option<Pos2>; 3] =
        std::array::from_fn(|i| project(origin + axis_dirs[i] * gizmo_len, view_proj, frame_rect));

    let rings: [Vec<Pos2>; 3] = std::array::from_fn(|i| {
        let (p1, p2) = ring_perps_from_dir(axis_dirs[i]);
        (0..=RING_SEGS)
            .filter_map(|j| {
                let a = j as f32 / RING_SEGS as f32 * std::f32::consts::TAU;
                project(
                    origin + p1 * (gizmo_len * a.cos()) + p2 * (gizmo_len * a.sin()),
                    view_proj,
                    frame_rect,
                )
            })
            .collect()
    });

    let cursor = ui.input(|i| i.pointer.hover_pos());
    let pressed = ui.input(|i| i.pointer.primary_pressed());
    let released = ui.input(|i| !i.pointer.primary_down());

    if released {
        state.drag = None;
    }

    let was_consuming = state.consuming;

    // Hover: closest handle within HIT_PX when not dragging
    let hovered_axis: Option<GizmoAxis> = if state.drag.is_none() {
        cursor.filter(|&c| frame_rect.contains(c)).and_then(|c| {
            let mut min_dist = HIT_PX;
            let mut hit: Option<GizmoAxis> = None;
            for (i, (ax, _)) in AXES.iter().enumerate() {
                let d = match state.mode {
                    GizmoMode::Translate | GizmoMode::Scale => {
                        tips[i].map(|t| seg_dist(c, center, t)).unwrap_or(f32::MAX)
                    }
                    GizmoMode::Rotate => rings[i]
                        .windows(2)
                        .map(|w| seg_dist(c, w[0], w[1]))
                        .fold(f32::MAX, f32::min),
                };
                if d < min_dist {
                    min_dist = d;
                    hit = Some(ax.clone());
                }
            }
            hit
        })
    } else {
        state.drag.as_ref().map(|d| d.axis.clone())
    };

    let near_handle = cursor
        .filter(|&c| frame_rect.contains(c))
        .map(|c| match state.mode {
            GizmoMode::Translate | GizmoMode::Scale => tips.iter().any(|t| {
                t.map(|t| seg_dist(c, center, t) < HIT_PX * 2.0)
                    .unwrap_or(false)
            }),
            GizmoMode::Rotate => rings.iter().any(|pts| {
                pts.windows(2)
                    .any(|w| seg_dist(c, w[0], w[1]) < HIT_PX * 2.0)
            }),
        })
        .unwrap_or(false);
    state.consuming = near_handle || state.drag.is_some();

    let mut new_transform: Option<Transform> = None;

    // Update active drag
    if let Some(ref drag) = state.drag.clone() {
        if let Some(cur) = cursor {
            let delta = cur - drag.start_mouse;
            let proj = delta.x * drag.screen_dir.x + delta.y * drag.screen_dir.y;
            let mut t = drag.start_transform.clone();
            // Shift held temporarily inverts each snap toggle.
            let eff_snap_translate = state.snap_translate ^ state.shift_snap_held;
            let eff_snap_rotate = state.snap_rotate ^ state.shift_snap_held;
            let eff_snap_scale = state.snap_scale ^ state.shift_snap_held;

            match state.mode {
                GizmoMode::Translate => {
                    let d = drag.axis_world_dir * (proj * drag.sensitivity);
                    let mut pos = drag.start_transform.local_position + d;
                    if eff_snap_translate {
                        let s = state.snap_translate_size.max(0.001);
                        pos.x = (pos.x / s).round() * s;
                        pos.y = (pos.y / s).round() * s;
                        pos.z = (pos.z / s).round() * s;
                    }
                    t.local_position = pos;
                    t.global_position = pos;
                }
                GizmoMode::Rotate => {
                    let raw_deg = proj * drag.sensitivity;
                    let angle_deg = if eff_snap_rotate {
                        let s = state.snap_rotate_deg.max(0.1);
                        (raw_deg / s).round() * s
                    } else {
                        raw_deg
                    };
                    let q = apostasy_core::cgmath::Quaternion::from_axis_angle(
                        drag.axis_world_dir,
                        apostasy_core::cgmath::Deg(angle_deg),
                    );
                    let new_q = (q * drag.start_transform.local_rotation).normalize();
                    t.local_rotation = new_q;
                    t.local_euler_angles = quat_to_euler_deg(new_q);
                    // Write global_* immediately so renderer sees the change this frame
                    t.global_rotation = new_q;
                    t.global_euler_angles = t.local_euler_angles;
                }
                GizmoMode::Scale => {
                    let ds = proj * drag.sensitivity;
                    let snap = |raw: f32| -> f32 {
                        if eff_snap_scale {
                            let s = state.snap_scale_size.max(0.001);
                            ((raw / s).round() * s).max(0.001)
                        } else {
                            raw.max(0.001)
                        }
                    };
                    match drag.axis {
                        GizmoAxis::X => {
                            t.local_scale.x = snap(drag.start_transform.local_scale.x + ds);
                        }
                        GizmoAxis::Y => {
                            t.local_scale.y = snap(drag.start_transform.local_scale.y + ds);
                        }
                        GizmoAxis::Z => {
                            t.local_scale.z = snap(drag.start_transform.local_scale.z + ds);
                        }
                    }
                    t.global_scale = t.local_scale;
                }
            }
            new_transform = Some(t);
        }
    } else if pressed
        && was_consuming
        && let Some(cur) = cursor
        && frame_rect.contains(cur)
    {
        state.drag = AXES.iter().enumerate().find_map(|(i, (ax, _))| {
            let hit = match state.mode {
                GizmoMode::Translate | GizmoMode::Scale => tips[i]
                    .map(|t| seg_dist(cur, center, t) < HIT_PX)
                    .unwrap_or(false),
                GizmoMode::Rotate => rings[i]
                    .windows(2)
                    .any(|w| seg_dist(cur, w[0], w[1]) < HIT_PX),
            };
            if !hit {
                return None;
            }

            let (screen_dir, sensitivity) = match state.mode {
                GizmoMode::Translate | GizmoMode::Scale => {
                    let v = tips[i]? - center;
                    let l = v.length().max(0.001);
                    let sens = if state.mode == GizmoMode::Translate {
                        gizmo_len / l
                    } else {
                        0.01
                    };
                    (v / l, sens)
                }
                GizmoMode::Rotate => {
                    let radius = (cur - center).normalized();
                    (Vec2::new(-radius.y, radius.x), 1.0 / 1.5)
                }
            };

            Some(GizmoDrag {
                axis: ax.clone(),
                axis_world_dir: axis_dirs[i],
                start_mouse: cur,
                start_transform: transform.clone(),
                screen_dir,
                sensitivity,
            })
        });
    }

    // Draw
    let painter = ui.painter_at(frame_rect);

    match state.mode {
        GizmoMode::Translate => {
            for (i, (ax, base_col)) in AXES.iter().enumerate() {
                if let Some(tip) = tips[i] {
                    let active = state.drag.as_ref().map(|d| &d.axis == ax).unwrap_or(false);
                    let hov = hovered_axis.as_ref().map(|h| h == ax).unwrap_or(false);
                    let col = if active {
                        Color32::WHITE
                    } else if hov {
                        lighten(*base_col)
                    } else {
                        *base_col
                    };
                    let w = if active || hov { 3.0 } else { 2.0 };
                    painter.line_segment([center, tip], Stroke::new(w, col));
                    painter.arrow(tip, (tip - center).normalized() * 10.0, Stroke::new(w, col));
                }
            }
            painter.circle_filled(center, 5.0, Color32::WHITE);
        }
        GizmoMode::Rotate => {
            for (i, (ax, base_col)) in AXES.iter().enumerate() {
                let active = state.drag.as_ref().map(|d| &d.axis == ax).unwrap_or(false);
                let hov = hovered_axis.as_ref().map(|h| h == ax).unwrap_or(false);
                let col = if active {
                    Color32::WHITE
                } else if hov {
                    lighten(*base_col)
                } else {
                    *base_col
                };
                let w = if active || hov { 3.0 } else { 2.0 };
                for pair in rings[i].windows(2) {
                    painter.line_segment([pair[0], pair[1]], Stroke::new(w, col));
                }
            }
        }
        GizmoMode::Scale => {
            for (i, (ax, base_col)) in AXES.iter().enumerate() {
                if let Some(tip) = tips[i] {
                    let active = state.drag.as_ref().map(|d| &d.axis == ax).unwrap_or(false);
                    let hov = hovered_axis.as_ref().map(|h| h == ax).unwrap_or(false);
                    let col = if active {
                        Color32::WHITE
                    } else if hov {
                        lighten(*base_col)
                    } else {
                        *base_col
                    };
                    let w = if active || hov { 3.0 } else { 2.0 };
                    painter.line_segment([center, tip], Stroke::new(w, col));
                    painter.rect_filled(
                        egui::Rect::from_center_size(tip, Vec2::splat(9.0)),
                        1.0,
                        col,
                    );
                }
            }
            painter.rect_filled(
                egui::Rect::from_center_size(center, Vec2::splat(9.0)),
                1.0,
                Color32::WHITE,
            );
        }
    }

    (new_transform, state)
}

/// Draw a wireframe outline of the collider shape for the selected object.
pub fn collider_gizmo(
    ui: &mut egui::Ui,
    transform: &Transform,
    collider: &Collider,
    view_proj: Matrix4<f32>,
    frame_rect: egui::Rect,
) {
    let painter = ui.painter_at(frame_rect);
    let col = Color32::from_rgb(80, 220, 100);
    let stroke = Stroke::new(1.5, col);

    let rot = transform.global_rotation;
    let scale = transform.global_scale;
    let center = transform.global_position
        + rot
            * Vector3::new(
                collider.offset.x * scale.x,
                collider.offset.y * scale.y,
                collider.offset.z * scale.z,
            );

    match &collider.shape {
        ColliderShape::Cuboid { size } => {
            let ax = rot * Vector3::new(size.x * scale.x, 0.0, 0.0);
            let ay = rot * Vector3::new(0.0, size.y * scale.y, 0.0);
            let az = rot * Vector3::new(0.0, 0.0, size.z * scale.z);

            let corners: [Vector3<f32>; 8] = [
                center + ax + ay + az,
                center + ax + ay - az,
                center + ax - ay + az,
                center + ax - ay - az,
                center - ax + ay + az,
                center - ax + ay - az,
                center - ax - ay + az,
                center - ax - ay - az,
            ];

            let edges: [(usize, usize); 12] = [
                (0, 1),
                (2, 3),
                (4, 5),
                (6, 7),
                (0, 2),
                (1, 3),
                (4, 6),
                (5, 7),
                (0, 4),
                (1, 5),
                (2, 6),
                (3, 7),
            ];

            for (a, b) in edges {
                if let (Some(pa), Some(pb)) = (
                    project(corners[a], view_proj, frame_rect),
                    project(corners[b], view_proj, frame_rect),
                ) {
                    painter.line_segment([pa, pb], stroke);
                }
            }
        }
        ColliderShape::Sphere { radius } => {
            let rx = radius * scale.x;
            let ry = radius * scale.y;
            let rz = radius * scale.z;
            for axis in 0..3u8 {
                let pts: Vec<Pos2> = (0..=RING_SEGS)
                    .filter_map(|j| {
                        let a = j as f32 / RING_SEGS as f32 * std::f32::consts::TAU;
                        let p = match axis {
                            0 => center + rot * Vector3::new(0.0, ry * a.cos(), rz * a.sin()),
                            1 => center + rot * Vector3::new(rx * a.cos(), 0.0, rz * a.sin()),
                            _ => center + rot * Vector3::new(rx * a.cos(), ry * a.sin(), 0.0),
                        };
                        project(p, view_proj, frame_rect)
                    })
                    .collect();
                for pair in pts.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
            }
        }
        ColliderShape::Capsule { radius, height } => {
            let r = radius * scale.x.max(scale.z);
            let ry = radius * scale.y;
            let h = height * 0.5 * scale.y;
            let y_up = rot * Vector3::new(0.0, 1.0, 0.0);
            let top_c = center + y_up * h;
            let bot_c = center - y_up * h;

            // Cylinder rings at hemisphere junctions
            for ring_c in [top_c, bot_c] {
                let pts: Vec<Pos2> = (0..=RING_SEGS)
                    .filter_map(|j| {
                        let a = j as f32 / RING_SEGS as f32 * std::f32::consts::TAU;
                        project(
                            ring_c + rot * Vector3::new(r * a.cos(), 0.0, r * a.sin()),
                            view_proj,
                            frame_rect,
                        )
                    })
                    .collect();
                for pair in pts.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
            }

            // side lines
            for (cx, cz) in [(r, 0.0_f32), (-r, 0.0), (0.0, r), (0.0, -r)] {
                let off = rot * Vector3::new(cx, 0.0, cz);
                if let (Some(pt), Some(pb)) = (
                    project(top_c + off, view_proj, frame_rect),
                    project(bot_c + off, view_proj, frame_rect),
                ) {
                    painter.line_segment([pt, pb], stroke);
                }
            }

            // Hemispheres
            let half = RING_SEGS / 2;
            for (cap_c, sign) in [(top_c, 1.0_f32), (bot_c, -1.0)] {
                for plane in 0..2u8 {
                    let pts: Vec<Pos2> = (0..=half)
                        .filter_map(|j| {
                            let a = j as f32 / half as f32 * std::f32::consts::PI;
                            let local = match plane {
                                0 => Vector3::new(r * a.cos(), sign * ry * a.sin(), 0.0),
                                _ => Vector3::new(0.0, sign * ry * a.sin(), r * a.cos()),
                            };
                            project(cap_c + rot * local, view_proj, frame_rect)
                        })
                        .collect();
                    for pair in pts.windows(2) {
                        painter.line_segment([pair[0], pair[1]], stroke);
                    }
                }
            }
        }
        ColliderShape::Cylinder { radius, height } => {
            let r = radius * scale.x.max(scale.z);
            let h = height * 0.5 * scale.y;
            let y_up = rot * Vector3::new(0.0, 1.0, 0.0);
            let top_c = center + y_up * h;
            let bot_c = center - y_up * h;

            for ring_c in [top_c, bot_c] {
                let pts: Vec<Pos2> = (0..=RING_SEGS)
                    .filter_map(|j| {
                        let a = j as f32 / RING_SEGS as f32 * std::f32::consts::TAU;
                        project(
                            ring_c + rot * Vector3::new(r * a.cos(), 0.0, r * a.sin()),
                            view_proj,
                            frame_rect,
                        )
                    })
                    .collect();
                for pair in pts.windows(2) {
                    painter.line_segment([pair[0], pair[1]], stroke);
                }
            }

            for (cx, cz) in [(r, 0.0_f32), (-r, 0.0), (0.0, r), (0.0, -r)] {
                let off = rot * Vector3::new(cx, 0.0, cz);
                if let (Some(pt), Some(pb)) = (
                    project(top_c + off, view_proj, frame_rect),
                    project(bot_c + off, view_proj, frame_rect),
                ) {
                    painter.line_segment([pt, pb], stroke);
                }
            }
        }
    }
}
