use std::sync::Arc;

use anyhow::Result;
use apostasy_macros::{Component, Inspect, update};
use cgmath::{InnerSpace, Quaternion, Vector3, Zero};
use egui::{ComboBox, DragAndDrop};

use crate::{
    ecs::{
        cell::EntityId,
        component::Inspect,
        components::{serde_support::vec3_seq, transform::Transform},
        tags::{ColliderDebugVisual, Player},
        world::World,
    },
    physics::{ColliderRenderDebug, PlayerColliderRenderDebug, velocity::Velocity},
    rendering::{components::model_renderer::ModelRenderer, shared::model::Bvh},
    ui::{DRAG_SIZE, LABEL_WIDTH},
};

/// The geometric shape used for collision detection.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "shape")]
pub enum ColliderShape {
    Cuboid {
        #[serde(with = "vec3_seq", default = "default_cuboid_size")]
        size: Vector3<f32>,
    },
    Sphere {
        #[serde(default = "default_radius")]
        radius: f32,
    },
    Capsule {
        #[serde(default = "default_capsule_radius")]
        radius: f32,
        #[serde(default = "default_height")]
        height: f32,
    },
    Cylinder {
        #[serde(default = "default_capsule_radius")]
        radius: f32,
        #[serde(default = "default_height")]
        height: f32,
    },
    Mesh {
        #[serde(skip)]
        triangles: Arc<Vec<[Vector3<f32>; 3]>>,
        #[serde(skip, default = "empty_bvh")]
        bvh: Arc<Bvh>,
        model_path: String,
    },
}

fn default_cuboid_size() -> Vector3<f32> {
    Vector3::new(1.0, 1.0, 1.0)
}
fn default_radius() -> f32 {
    1.0
}
fn default_capsule_radius() -> f32 {
    0.5
}
fn default_height() -> f32 {
    2.0
}
fn empty_bvh() -> Arc<Bvh> {
    Arc::new(Bvh::empty())
}
fn zero_vec3() -> Vector3<f32> {
    Vector3::zero()
}

impl ColliderShape {
    pub fn half_extents(&self) -> Vector3<f32> {
        match self {
            ColliderShape::Cuboid { size } => *size,
            ColliderShape::Sphere { radius } => Vector3::new(*radius, *radius, *radius),
            ColliderShape::Capsule { radius, height } => {
                Vector3::new(*radius, height * 0.5 + radius, *radius)
            }
            ColliderShape::Cylinder { radius, height } => {
                Vector3::new(*radius, height * 0.5, *radius)
            }
            ColliderShape::Mesh { bvh, .. } => {
                if bvh.nodes.is_empty() {
                    Vector3::new(0.0, 0.0, 0.0)
                } else {
                    (bvh.nodes[0].aabb_max - bvh.nodes[0].aabb_min) * 0.5
                }
            }
        }
    }

    #[allow(unused)]
    pub fn to_string(&self) -> String {
        match self {
            ColliderShape::Cuboid { size } => "Cube".into(),
            ColliderShape::Sphere { radius } => "Sphere".into(),
            ColliderShape::Capsule { radius, height } => "Capsule".into(),
            ColliderShape::Cylinder { radius, height } => "Cylinder".into(),
            ColliderShape::Mesh {
                triangles,
                bvh,
                model_path,
            } => "Mesh".into(),
        }
    }
}

/// Collision volume attached to an entity.
/// `is_static` entities are immovable, `is_area` entities detect overlaps but do not resolve them.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[component(serde)]
pub struct Collider {
    #[serde(flatten)]
    pub shape: ColliderShape,
    /// Local-space offset from the entity's origin to the collider center.
    #[serde(with = "vec3_seq", default = "zero_vec3")]
    pub offset: Vector3<f32>,
    #[serde(default)]
    pub is_static: bool,
    #[serde(default)]
    pub is_area: bool,
}

impl Inspect for Collider {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Shape"));
                ComboBox::from_id_salt("collider_shape")
                    .selected_text(self.shape.to_string())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.shape,
                            ColliderShape::Cuboid {
                                size: Vector3::new(1.0, 1.0, 1.0),
                            },
                            "Cube",
                        );
                        ui.selectable_value(
                            &mut self.shape,
                            ColliderShape::Sphere { radius: 1.0 },
                            "Sphere",
                        );
                        ui.selectable_value(
                            &mut self.shape,
                            ColliderShape::Capsule {
                                radius: 1.0,
                                height: 2.0,
                            },
                            "Capsule",
                        );
                        ui.selectable_value(
                            &mut self.shape,
                            ColliderShape::Cylinder {
                                radius: 1.0,
                                height: 2.0,
                            },
                            "Cylinder",
                        );
                        ui.selectable_value(
                            &mut self.shape,
                            ColliderShape::Mesh {
                                triangles: Arc::new(Vec::new()),
                                bvh: Arc::new(Bvh::empty()),
                                model_path: "".into(),
                            },
                            "Mesh",
                        );
                    });
            });
            ui.separator();
            match &mut self.shape {
                ColliderShape::Cuboid { size } => {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Size"));
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(&mut size.x)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(&mut size.y)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(&mut size.z)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                    });
                }
                ColliderShape::Sphere { radius } => {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Radius"));
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(radius)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                    });
                }
                ColliderShape::Capsule { radius, height } => {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Radius"));
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(radius)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Height"));
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(height)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                    });
                }
                ColliderShape::Cylinder { radius, height } => {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Radius"));
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(radius)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new("Height"));
                        ui.add_sized(
                            DRAG_SIZE,
                            egui::DragValue::new(height)
                                .speed(0.1)
                                .range(0.1..=f32::MAX),
                        );
                    });
                }

                ColliderShape::Mesh { model_path, .. } => {
                    let row_h = 20.0;
                    let has_model_drag = DragAndDrop::has_payload_of_type::<String>(ui.ctx());
                    let mut disp_model_path = model_path.clone();
                    let inner = ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Model"));
                        ui.add_sized(
                            [ui.available_width(), row_h],
                            egui::TextEdit::singleline(&mut disp_model_path)
                                .hint_text("drag a model here…"),
                        )
                    });
                    let te_resp = inner.inner;
                    let is_hovering = has_model_drag && ui.rect_contains_pointer(te_resp.rect);
                    if is_hovering {
                        ui.painter().rect_stroke(
                            te_resp.rect.expand(2.0),
                            3.0,
                            egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 200, 140)),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if let Some(payload) = te_resp.dnd_release_payload::<String>() {
                        if let Some(name) = payload.strip_prefix("model:") {
                            *model_path = name.to_string();
                        }
                    }
                }
            }
        });
    }
}

impl Default for Collider {
    fn default() -> Self {
        Self {
            shape: ColliderShape::Cuboid {
                size: Vector3::new(1.0, 1.0, 1.0),
            },
            offset: Vector3::new(0.0, 0.0, 0.0),
            is_static: false,
            is_area: false,
        }
    }
}

impl Collider {
    /// Creates a dynamic collider.
    pub fn new(shape: ColliderShape, offset: Vector3<f32>) -> Self {
        Self {
            shape,
            offset,
            is_static: false,
            is_area: false,
        }
    }

    /// Creates a static collider.
    pub fn new_static(shape: ColliderShape, offset: Vector3<f32>) -> Self {
        Self {
            shape,
            offset,
            is_static: true,
            is_area: false,
        }
    }

    /// Returns the world-space center of this collider (offset rotated by entity rotation).
    pub fn world_center(&self, position: Vector3<f32>, rotation: Quaternion<f32>) -> Vector3<f32> {
        position + rotate_vector(rotation, self.offset)
    }

    /// Returns the three local axes of this OBB in world space.
    pub fn world_axes(&self, rotation: Quaternion<f32>) -> [Vector3<f32>; 3] {
        [
            rotate_vector(rotation, Vector3::new(1.0, 0.0, 0.0)),
            rotate_vector(rotation, Vector3::new(0.0, 1.0, 0.0)),
            rotate_vector(rotation, Vector3::new(0.0, 0.0, 1.0)),
        ]
    }

    /// Returns the half-extents (collider_size is already treated as half-extents).
    pub fn half_extents(&self) -> Vector3<f32> {
        self.shape.half_extents()
    }

    /// Returns the minimum translation vector to separate this collider from `other`, or `None` if they are not overlapping.
    pub fn translation_vector_against(
        &self,
        pos_a: Vector3<f32>,
        rotation_a: Quaternion<f32>,
        other: &Collider,
        pos_b: Vector3<f32>,
        rotation_b: Quaternion<f32>,
    ) -> Option<Vector3<f32>> {
        let center_a = self.world_center(pos_a, rotation_a);
        let center_b = other.world_center(pos_b, rotation_b);
        let axes_a = self.world_axes(rotation_a);
        let axes_b = other.world_axes(rotation_b);
        let half_a = self.half_extents();
        let half_b = other.half_extents();

        let d = center_a - center_b;

        match (&self.shape, &other.shape) {
            (ColliderShape::Sphere { radius: ra }, ColliderShape::Sphere { radius: rb }) => {
                return sphere_vs_sphere(center_a, *ra, center_b, *rb);
            }

            (ColliderShape::Sphere { radius }, ColliderShape::Mesh { bvh, .. }) => {
                return sphere_vs_mesh(center_a, *radius, bvh, pos_b, rotation_b);
            }
            (ColliderShape::Capsule { radius, height }, ColliderShape::Mesh { bvh, .. }) => {
                let half_h = *height * 0.5;
                let up = rotate_vector(rotation_a, Vector3::new(0.0, 1.0, 0.0));
                return capsule_vs_mesh(
                    center_a + up * half_h,
                    center_a - up * half_h,
                    *radius,
                    bvh,
                    pos_b,
                    rotation_b,
                );
            }
            (ColliderShape::Mesh { bvh, .. }, ColliderShape::Sphere { radius }) => {
                return sphere_vs_mesh(center_b, *radius, bvh, pos_a, rotation_a).map(|v| -v);
            }
            (ColliderShape::Mesh { bvh, .. }, ColliderShape::Capsule { radius, height }) => {
                let half_h = *height * 0.5;
                let up = rotate_vector(rotation_b, Vector3::new(0.0, 1.0, 0.0));
                return capsule_vs_mesh(
                    center_b + up * half_h,
                    center_b - up * half_h,
                    *radius,
                    bvh,
                    pos_a,
                    rotation_a,
                )
                .map(|v| -v);
            }
            (
                ColliderShape::Cuboid { .. } | ColliderShape::Cylinder { .. },
                ColliderShape::Mesh { bvh, .. },
            ) => {
                return obb_vs_mesh(center_a, &axes_a, half_a, bvh, pos_b, rotation_b);
            }
            (
                ColliderShape::Mesh { bvh, .. },
                ColliderShape::Cuboid { .. } | ColliderShape::Cylinder { .. },
            ) => {
                return obb_vs_mesh(center_b, &axes_b, half_b, bvh, pos_a, rotation_a).map(|v| -v);
            }

            (ColliderShape::Sphere { radius }, _) => {
                return sphere_vs_obb(center_a, *radius, center_b, &axes_b, half_b);
            }
            (_, ColliderShape::Sphere { radius }) => {
                return sphere_vs_obb(center_b, *radius, center_a, &axes_a, half_a).map(|v| -v);
            }

            (ColliderShape::Mesh { bvh, .. }, ColliderShape::Mesh { bvh: bh2, .. }) => {
                return mesh_vs_mesh(bvh, pos_a, rotation_a, bh2, pos_b, rotation_b);
            }

            _ => {}
        }

        let mut min_overlap = f32::MAX;
        let mut min_axis = Vector3::new(0.0f32, 0.0, 0.0);
        let face_axes: [Vector3<f32>; 6] = [
            axes_a[0], axes_a[1], axes_a[2], axes_b[0], axes_b[1], axes_b[2],
        ];

        let mut edge_axes: Vec<Vector3<f32>> = Vec::new();
        for i in 0..3 {
            for j in 0..3 {
                let cross = axes_a[i].cross(axes_b[j]);
                if cross.magnitude2() > 1e-10 {
                    edge_axes.push(cross.normalize());
                }
            }
        }

        let all_axes: Vec<Vector3<f32>> = face_axes.iter().copied().chain(edge_axes).collect();

        for axis in &all_axes {
            if axis.magnitude2() < 1e-10 {
                continue;
            }
            let axis = axis.normalize();

            let proj_a = project_obb(axis, &axes_a, half_a);
            let proj_b = project_obb(axis, &axes_b, half_b);
            let dist = d.dot(axis).abs();
            let overlap = proj_a + proj_b - dist;

            if overlap <= 0.0 {
                return None; // Separating axis found, no collision
            }
            if overlap < min_overlap {
                min_overlap = overlap;
                // Ensure the MTV points from B toward A
                min_axis = if d.dot(axis) >= 0.0 { axis } else { -axis };
            }
        }

        Some(min_axis * min_overlap)
    }

    /// Returns `true` if `point` (world space) lies inside this collider.
    pub fn contains_point(
        &self,
        position: Vector3<f32>,
        point: Vector3<f32>,
        rotation: Quaternion<f32>,
    ) -> bool {
        let axes = self.world_axes(rotation);
        let half = self.half_extents();
        let center = self.world_center(position, rotation);
        let local = point - center;

        // Project the point onto each local axis and check against half-extent
        local.dot(axes[0]).abs() <= half.x
            && local.dot(axes[1]).abs() <= half.y
            && local.dot(axes[2]).abs() <= half.z
    }
}

fn project_obb(axis: Vector3<f32>, obb_axes: &[Vector3<f32>; 3], half: Vector3<f32>) -> f32 {
    axis.dot(obb_axes[0]).abs() * half.x
        + axis.dot(obb_axes[1]).abs() * half.y
        + axis.dot(obb_axes[2]).abs() * half.z
}

fn mesh_vs_mesh(
    bvh: &Bvh,
    mesh_pos: Vector3<f32>,
    mesh_rot: Quaternion<f32>,
    bvh_2: &Bvh,
    mesh_pos_2: Vector3<f32>,
    mesh_rot_2: Quaternion<f32>,
) -> Option<Vector3<f32>> {
    if bvh.nodes.is_empty() || bvh_2.nodes.is_empty() {
        return None;
    }

    let r = mesh_rot.conjugate() * mesh_rot_2;
    let t = mesh_rot.conjugate() * (mesh_pos_2 - mesh_pos);

    let rc = [
        r * Vector3::new(1.0, 0.0, 0.0),
        r * Vector3::new(0.0, 1.0, 0.0),
        r * Vector3::new(0.0, 0.0, 1.0),
    ];
    let transform_aabb = |min: Vector3<f32>, max: Vector3<f32>| {
        let center = r * ((min + max) * 0.5) + t;
        let e = (max - min) * 0.5;
        let extent = Vector3::new(
            rc[0].x.abs() * e.x + rc[1].x.abs() * e.y + rc[2].x.abs() * e.z,
            rc[0].y.abs() * e.x + rc[1].y.abs() * e.y + rc[2].y.abs() * e.z,
            rc[0].z.abs() * e.x + rc[1].z.abs() * e.y + rc[2].z.abs() * e.z,
        );
        (center - extent, center + extent)
    };

    let mut stack = vec![(0u32, 0u32)];
    let mut deepest: Option<Vector3<f32>> = None;
    let mut max_depth = 0.0f32;

    while let Some((idx, idx_2)) = stack.pop() {
        let node = &bvh.nodes[idx as usize];
        let node_2 = &bvh_2.nodes[idx_2 as usize];
        let (min_2, max_2) = transform_aabb(node_2.aabb_min, node_2.aabb_max);

        if !aabb_overlaps_aabb(node.aabb_min, node.aabb_max, min_2, max_2) {
            continue;
        }

        match (node.left == 0, node_2.left == 0) {
            (true, true) => {
                for i in node.tri_start..node.tri_start + node.tri_count {
                    let tri = &bvh.triangles[i as usize];
                    for x in node_2.tri_start..node_2.tri_start + node_2.tri_count {
                        let raw = &bvh_2.triangles[x as usize];
                        let tri_2 = [r * raw[0] + t, r * raw[1] + t, r * raw[2] + t];
                        if let Some((direction, depth)) = triangle_vs_triangle(tri, &tri_2)
                            && depth > max_depth
                        {
                            max_depth = depth;
                            deepest = Some(mesh_rot * direction * depth);
                        }
                    }
                }
            }
            (true, false) => {
                stack.push((idx, node_2.left));
                stack.push((idx, node_2.right));
            }
            (false, true) => {
                stack.push((node.left, idx_2));
                stack.push((node.right, idx_2));
            }
            (false, false) => {
                stack.push((node.left, node_2.left));
                stack.push((node.left, node_2.right));
                stack.push((node.right, node_2.left));
                stack.push((node.right, node_2.right));
            }
        }
    }

    deepest
}

fn obb_vs_mesh(
    center: Vector3<f32>,
    axes: &[Vector3<f32>; 3],
    half_extents: Vector3<f32>,
    bvh: &Bvh,
    mesh_pos: Vector3<f32>,
    mesh_rot: Quaternion<f32>,
) -> Option<Vector3<f32>> {
    if bvh.nodes.is_empty() {
        return None;
    }

    // Transform the OBB into the mesh local space.
    let inv_rot = mesh_rot.conjugate();
    let local_center = inv_rot * (center - mesh_pos);
    let local_axes = [inv_rot * axes[0], inv_rot * axes[1], inv_rot * axes[2]];

    let extent = Vector3::new(
        project_obb(Vector3::new(1.0, 0.0, 0.0), &local_axes, half_extents),
        project_obb(Vector3::new(0.0, 1.0, 0.0), &local_axes, half_extents),
        project_obb(Vector3::new(0.0, 0.0, 1.0), &local_axes, half_extents),
    );
    let obb_min = local_center - extent;
    let obb_max = local_center + extent;

    let mut stack = vec![0u32];
    let mut deepest: Option<Vector3<f32>> = None;
    let mut max_depth = 0.0f32;

    while let Some(idx) = stack.pop() {
        let node = &bvh.nodes[idx as usize];

        if !aabb_overlaps_aabb(node.aabb_min, node.aabb_max, obb_min, obb_max) {
            continue;
        }

        if node.left == 0 {
            for i in node.tri_start..node.tri_start + node.tri_count {
                let tri = &bvh.triangles[i as usize];
                if let Some((direction, depth)) =
                    obb_vs_triangle(local_center, &local_axes, half_extents, tri)
                    && depth > max_depth
                {
                    max_depth = depth;
                    deepest = Some(mesh_rot * direction * depth);
                }
            }
        } else {
            stack.push(node.left);
            stack.push(node.right);
        }
    }

    deepest
}

fn triangle_vs_triangle(
    tri: &[Vector3<f32>; 3],
    tri_2: &[Vector3<f32>; 3],
) -> Option<(Vector3<f32>, f32)> {
    let edges = [tri[1] - tri[0], tri[2] - tri[1], tri[0] - tri[2]];
    let edges_2 = [
        tri_2[1] - tri_2[0],
        tri_2[2] - tri_2[1],
        tri_2[0] - tri_2[2],
    ];
    let n = edges[0].cross(edges[1]);
    let n_2 = edges_2[0].cross(edges_2[1]);

    let mut axes = [Vector3::zero(); 17];
    axes[0] = n;
    axes[1] = n_2;
    let mut k = 2;
    for e in &edges {
        for e2 in &edges_2 {
            axes[k] = e.cross(*e2);
            k += 1;
        }
    }
    for e in &edges {
        axes[k] = n.cross(*e);
        k += 1;
    }
    for e2 in &edges_2 {
        axes[k] = n_2.cross(*e2);
        k += 1;
    }

    let mut min_overlap = f32::MAX;
    let mut min_axis = Vector3::zero();

    for axis in axes {
        if axis.magnitude2() < 1e-10 {
            continue;
        }
        let axis = axis.normalize();
        let (min_a, max_a) = project_triangle(axis, tri);
        let (min_b, max_b) = project_triangle(axis, tri_2);
        let overlap = (max_b - min_a).min(max_a - min_b);
        if overlap <= 0.0 {
            return None; // Separating axis found, no collision
        }
        if overlap < min_overlap {
            min_overlap = overlap;
            min_axis = axis;
        }
    }

    if min_overlap == f32::MAX {
        return None;
    }

    let centroid = (tri[0] + tri[1] + tri[2]) / 3.0;
    let centroid_2 = (tri_2[0] + tri_2[1] + tri_2[2]) / 3.0;
    if (centroid - centroid_2).dot(min_axis) < 0.0 {
        min_axis = -min_axis;
    }

    Some((min_axis, min_overlap))
}

fn project_triangle(axis: Vector3<f32>, tri: &[Vector3<f32>; 3]) -> (f32, f32) {
    let d0 = tri[0].dot(axis);
    let d1 = tri[1].dot(axis);
    let d2 = tri[2].dot(axis);
    (d0.min(d1).min(d2), d0.max(d1).max(d2))
}

fn obb_vs_triangle(
    center: Vector3<f32>,
    axes: &[Vector3<f32>; 3],
    half_extents: Vector3<f32>,
    tri: &[Vector3<f32>; 3],
) -> Option<(Vector3<f32>, f32)> {
    let tri_normal = triangle_normal(tri);
    if tri_normal.magnitude2() < 1e-10 {
        return None;
    }
    let tri_normal = tri_normal.normalize();

    // Reject test on the box's 3 local axes
    // for axis in axes {
    // if overlap_along_axis(*axis, center, axes, half_extents, tri) <= 0.0 {
    //     return None;
    // }
    // }

    // Reject and  resolve on the triangle's own face normal
    let plane_value = tri[0].dot(tri_normal);
    let box_center_proj = center.dot(tri_normal);
    let box_radius = project_obb(tri_normal, axes, half_extents);
    let overlap = box_radius - (box_center_proj - plane_value).abs();
    if overlap <= 0.0 {
        return None;
    }

    let min_axis = if box_center_proj >= plane_value {
        tri_normal
    } else {
        -tri_normal
    };

    Some((min_axis, overlap))
}

/// Rotates a vector by a quaternion: q * v * q^-1
fn rotate_vector(q: Quaternion<f32>, v: Vector3<f32>) -> Vector3<f32> {
    let qv = Vector3::new(q.v.x, q.v.y, q.v.z);
    let t = qv.cross(v) * 2.0;
    v + t * q.s + qv.cross(t)
}

/// Data produced when two colliders overlap during a frame.
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    pub node_a: String,
    pub node_b: String,
    pub translation_vector: Vector3<f32>,
    pub depth: f32,
    pub normal: Vector3<f32>,
}

#[derive(Debug, Clone, Default, Component, Inspect)]
pub struct CollisionEvents {
    pub events: Vec<CollisionEvent>,
}

impl CollisionEvents {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A snapshot of a collider and it's needed data.
#[derive(Clone)]
struct Snapshot {
    id: EntityId,
    name: String,
    position: Vector3<f32>,
    rotation: Quaternion<f32>,
    collider: Collider,
    is_static: bool,
}

fn build_snapshot(world: &World) -> Vec<Snapshot> {
    world
        .get_entities_with_component::<Collider>()
        .into_iter()
        .filter_map(|id| {
            let transform = world.get_component::<Transform>(id)?;
            let position = transform.global_position;
            let scale = transform.global_scale;
            let rotation = transform.global_rotation;
            let collider_raw = world.get_component::<Collider>(id)?;
            let is_static = collider_raw.is_static;
            let mut collider = collider_raw.clone();

            // Bake scale into collider shape so collision matches world-space mesh size
            collider.shape = match collider.shape {
                ColliderShape::Cuboid { size } => ColliderShape::Cuboid {
                    size: Vector3::new(size.x * scale.x, size.y * scale.y, size.z * scale.z),
                },
                ColliderShape::Sphere { radius } => ColliderShape::Sphere {
                    radius: radius * scale.x.max(scale.y).max(scale.z),
                },
                ColliderShape::Capsule { radius, height } => ColliderShape::Capsule {
                    radius: radius * scale.x.max(scale.z),
                    height: height * scale.y,
                },
                ColliderShape::Cylinder { radius, height } => ColliderShape::Cylinder {
                    radius: radius * scale.x.max(scale.z),
                    height: height * scale.y,
                },
                ColliderShape::Mesh { .. } => collider.shape.clone(),
            };

            Some(Snapshot {
                id,
                name: world.get_name(id).unwrap_or_default().to_string(),
                position,
                rotation,
                collider,
                is_static,
            })
        })
        .collect()
}

const COLLISION_SLOP: f32 = 0.01;

/// Detects collisions between all entities using OBB vs OBB
#[update(priority = 10)]
pub fn collision_detection_system(world: &mut World) -> Result<()> {
    // Reset grounded flag itll be set by active collisions
    for id in world.get_entities_with_component::<Velocity>() {
        if let Some(v) = world.get_component_mut::<Velocity>(id) {
            v.is_grounded = false;
        }
    }

    if world
        .get_resource::<crate::physics::Noclip>()
        .map(|n| n.0)
        .unwrap_or(false)
    {
        if let Some(ev_id) = world
            .get_entities_with_component::<CollisionEvents>()
            .into_iter()
            .next()
        {
            if let Some(ev) = world.get_component_mut::<CollisionEvents>(ev_id) {
                ev.events.clear();
            }
        }
        return Ok(());
    }

    let snapshot = build_snapshot(world);
    let n = snapshot.len();

    let mut events: Vec<CollisionEvent> = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            let a = &snapshot[i];
            let b = &snapshot[j];

            if let Some(translation_vector) = a.collider.translation_vector_against(
                a.position,
                a.rotation,
                &b.collider,
                b.position,
                b.rotation,
            ) {
                let center_a = a.collider.world_center(a.position, a.rotation);
                let center_b = b.collider.world_center(b.position, b.rotation);

                let depth = translation_vector.magnitude();
                let normal = if depth > 1e-10 {
                    translation_vector / depth
                } else {
                    Vector3::new(0.0, 1.0, 0.0)
                };
                let upness = normal.y.abs();

                let correction_depth = (depth - COLLISION_SLOP).max(0.0);
                let correction_vector = normal * correction_depth;

                let (r_a, r_b) = match (&a.collider.shape, &b.collider.shape) {
                    (ColliderShape::Sphere { radius }, _) => {
                        let contact_point = center_a - normal * *radius;
                        (contact_point - center_a, contact_point - center_b)
                    }
                    (_, ColliderShape::Sphere { radius }) => {
                        let contact_point = center_b + normal * *radius;
                        (contact_point - center_a, contact_point - center_b)
                    }
                    _ => (Vector3::zero(), Vector3::zero()),
                };

                events.push(CollisionEvent {
                    node_a: a.name.clone(),
                    node_b: b.name.clone(),
                    translation_vector,
                    depth,
                    normal,
                });

                let vel_a = world.get_component::<Velocity>(a.id).cloned();
                let vel_b = world.get_component::<Velocity>(b.id).cloned();

                if let (Some(mut va), Some(mut vb)) = (vel_a, vel_b) {
                    // Positional correction
                    match (a.is_static, b.is_static) {
                        (false, false) => {
                            apply_position_correction(world, a.id, correction_vector * 0.5);
                            apply_position_correction(world, b.id, -correction_vector * 0.5);
                            // Cancel velocity along normal for both
                            if let Some(v) = world.get_component_mut::<Velocity>(a.id) {
                                let vn = v.linear_velocity.dot(normal);
                                if vn < 0.0 {
                                    v.linear_velocity -= normal * vn;
                                }
                            }
                            if let Some(v) = world.get_component_mut::<Velocity>(b.id) {
                                let vn = v.linear_velocity.dot(-normal);
                                if vn < 0.0 {
                                    v.linear_velocity += normal * vn;
                                }
                            }
                        }
                        (true, false) => {
                            apply_position_correction(world, b.id, -correction_vector);
                            if let Some(v) = world.get_component_mut::<Velocity>(b.id) {
                                let vn = v.linear_velocity.dot(normal);
                                if vn > 0.0 {
                                    v.linear_velocity -= normal * vn;
                                }
                            }
                        }
                        (false, true) => {
                            apply_position_correction(world, a.id, correction_vector);
                            if let Some(v) = world.get_component_mut::<Velocity>(a.id) {
                                let vn = v.linear_velocity.dot(normal);
                                if vn < 0.0 {
                                    v.linear_velocity -= normal * vn;
                                }
                            }
                        }
                        (true, true) => {}
                    }
                    // zero out velocity for static entities so impulse math is correct
                    if a.is_static {
                        va.linear_velocity = Vector3::zero();
                        va.angular_velocity = Vector3::zero();
                        va.mass = 0.0;
                    }
                    if b.is_static {
                        vb.linear_velocity = Vector3::zero();
                        vb.angular_velocity = Vector3::zero();
                        vb.mass = 0.0;
                    }

                    resolve_impulse(&mut va, &mut vb, r_a, r_b, normal);

                    // skip static entities
                    if !a.is_static
                        && let Some(v) = world.get_component_mut::<Velocity>(a.id)
                    {
                        *v = va;
                    }

                    if !b.is_static
                        && let Some(v) = world.get_component_mut::<Velocity>(b.id)
                    {
                        *v = vb;
                    }

                    if upness > 0.5 {
                        if !a.is_static
                            && let Some(v) = world.get_component_mut::<Velocity>(a.id)
                        {
                            v.is_grounded = true;
                        }
                        if !b.is_static
                            && let Some(v) = world.get_component_mut::<Velocity>(b.id)
                        {
                            v.is_grounded = true;
                        }
                    }

                    // On near-up contacts, zero tiny tangential velocity to stop landing jitter
                    let upness = normal.y.abs();
                    if upness > 0.7 {
                        let tang_threshold = 0.5; // tuneable
                        if !a.is_static
                            && let Some(v) = world.get_component_mut::<Velocity>(a.id)
                        {
                            let tang = v.linear_velocity - normal * v.linear_velocity.dot(normal);
                            if tang.magnitude() < tang_threshold {
                                v.linear_velocity -= tang; // zero small tangential
                            }
                        }
                        if !b.is_static
                            && let Some(v) = world.get_component_mut::<Velocity>(b.id)
                        {
                            let tang = v.linear_velocity - normal * v.linear_velocity.dot(normal);
                            if tang.magnitude() < tang_threshold {
                                v.linear_velocity -= tang;
                            }
                        }
                    }
                }
            }
        }
    }

    // Store events on whichever entity holds the CollisionEvents component
    if let Some(ev_id) = world
        .get_entities_with_component::<CollisionEvents>()
        .into_iter()
        .next()
    {
        if let Some(ev) = world.get_component_mut::<CollisionEvents>(ev_id) {
            ev.events = events;
        }
    }

    Ok(())
}
/// Extra half-extent added to every axis of a collider debug visual so it renders
/// slightly larger than the actual collider, keeping it visibly distinct from
/// coincident mesh surfaces instead of z-fighting with them.
pub(crate) const DEBUG_VISUAL_SIZE_OFFSET: f32 = 0.05;

/// Where a collider debug visual's geometry should come from.
pub enum DebugVisualSource {
    /// One of the built-in primitive models (cube/sphere/cylinder), scaled to match.
    Primitive {
        model_path: &'static str,
        scale: Vector3<f32>,
    },
    /// A mesh collider baked from a named, already-loaded model — reuse that model
    /// directly as the debug visual instead of a primitive, scaled up slightly.
    NamedModel {
        model_path: String,
        scale: Vector3<f32>,
    },
    /// A mesh collider with no backing model (e.g. terrain) — render its raw
    /// collision triangles directly via `MeshColliderDebugSource`.
    Triangles(Arc<Vec<[Vector3<f32>; 3]>>),
}

/// Returns the debug-visual geometry source for a collider shape (plus a small size
/// offset), or `None` if it can't be visualized yet (e.g. an unresolved mesh collider).
fn debug_visual_for_shape(shape: &ColliderShape) -> Option<DebugVisualSource> {
    match shape {
        ColliderShape::Cuboid { .. } | ColliderShape::Sphere { .. } => {
            let half = shape.half_extents()
                + Vector3::new(
                    DEBUG_VISUAL_SIZE_OFFSET,
                    DEBUG_VISUAL_SIZE_OFFSET,
                    DEBUG_VISUAL_SIZE_OFFSET,
                );
            let model_path = if matches!(shape, ColliderShape::Sphere { .. }) {
                "m_default_sphere"
            } else {
                "m_default_cube"
            };
            Some(DebugVisualSource::Primitive {
                model_path,
                scale: half,
            })
        }
        ColliderShape::Cylinder { .. } | ColliderShape::Capsule { .. } => {
            let half = shape.half_extents()
                + Vector3::new(
                    DEBUG_VISUAL_SIZE_OFFSET,
                    DEBUG_VISUAL_SIZE_OFFSET,
                    DEBUG_VISUAL_SIZE_OFFSET,
                );
            Some(DebugVisualSource::Primitive {
                model_path: "m_default_cylinder",
                scale: Vector3::new(half.x, half.y * 2.0, half.z),
            })
        }
        ColliderShape::Mesh {
            triangles,
            bvh,
            model_path,
        } => {
            if triangles.is_empty() {
                return None;
            }
            if !model_path.is_empty() {
                // Inflate by DEBUG_VISUAL_SIZE_OFFSET relative to the baked (scaled)
                // bounding box, so the extra local_scale approximates a uniform
                // world-space size increase once combined with the owner's scale.
                let node = bvh.nodes.first()?;
                let half_extent = (node.aabb_max - node.aabb_min) * 0.5;
                let inflate = |h: f32| (h + DEBUG_VISUAL_SIZE_OFFSET) / h.max(1e-4);
                let scale = Vector3::new(
                    inflate(half_extent.x),
                    inflate(half_extent.y),
                    inflate(half_extent.z),
                );
                Some(DebugVisualSource::NamedModel {
                    model_path: model_path.clone(),
                    scale,
                })
            } else {
                Some(DebugVisualSource::Triangles(triangles.clone()))
            }
        }
    }
}

/// Marks a collider debug visual whose geometry is built at render time directly
/// from a mesh collider's raw collision triangles (no backing named model exists).
#[derive(Component, Inspect, Clone, Debug, Default)]
pub struct MeshColliderDebugSource {
    pub triangles: Vec<[Vector3<f32>; 3]>,
}

/// Spawns/updates wireframe debug visuals for every `Collider` while `ColliderRenderDebug` is enabled.
#[update(mode = "all")]
pub fn collider_debug_render_system(world: &mut World) -> Result<()> {
    let enabled = world
        .get_resource::<ColliderRenderDebug>()
        .map(|r| r.0)
        .unwrap_or(false);

    let player_enabled = world
        .get_resource::<PlayerColliderRenderDebug>()
        .map(|r| r.0)
        .unwrap_or(false);

    if !enabled && !player_enabled {
        for id in world.get_entities_with_tag::<ColliderDebugVisual>() {
            world.despawn(id);
        }
        return Ok(());
    }
    for owner_id in world.get_entities_with_component::<Collider>() {
        let is_player = world.has_tag::<Player>(owner_id);
        let should_render = if is_player { player_enabled } else { enabled };
        if !should_render {
            if let Some(debug_id) = world
                .get_children_ids(owner_id)
                .into_iter()
                .find(|id| world.has_tag::<ColliderDebugVisual>(*id))
            {
                world.despawn(debug_id);
            }
            continue;
        }

        let collider = match world.get_component::<Collider>(owner_id) {
            Some(c) => c.clone(),
            None => continue,
        };

        let Some(source) = debug_visual_for_shape(&collider.shape) else {
            continue;
        };
        let model_path: &str = match &source {
            DebugVisualSource::Primitive { model_path, .. } => model_path,
            DebugVisualSource::NamedModel { model_path, .. } => model_path.as_str(),
            DebugVisualSource::Triangles(_) => "",
        };
        let scale = match &source {
            DebugVisualSource::Primitive { scale, .. } => *scale,
            DebugVisualSource::NamedModel { scale, .. } => *scale,
            DebugVisualSource::Triangles(_) => Vector3::new(1.0, 1.0, 1.0),
        };

        let debug_id = world
            .get_children_ids(owner_id)
            .into_iter()
            .find(|id| world.has_tag::<ColliderDebugVisual>(*id))
            .unwrap_or_else(|| {
                let id = world.spawn().id();
                world.add_component(id, Transform::default());
                world.add_component(id, ModelRenderer::from_path(model_path));
                world.add_tag::<ColliderDebugVisual>(id);
                let _ = world.set_parent(id, Some(owner_id));
                id
            });

        if let Some(mr) = world.get_component_mut::<ModelRenderer>(debug_id) {
            if mr.model_path != model_path {
                *mr = ModelRenderer::from_path(model_path);
            }
            mr.is_wireframe = true;
        }
        if let Some(t) = world.get_component_mut::<Transform>(debug_id) {
            t.local_position = collider.offset;
            t.local_scale = scale;
        }

        if let DebugVisualSource::Triangles(triangles) = source {
            let needs_update = world
                .get_component::<MeshColliderDebugSource>(debug_id)
                .map(|src| src.triangles.len() != triangles.len())
                .unwrap_or(true);
            if needs_update {
                world.add_component(
                    debug_id,
                    MeshColliderDebugSource {
                        triangles: (*triangles).clone(),
                    },
                );
                // Force the renderer to rebuild the GPU mesh from the new triangles.
                if let Some(mr) = world.get_component_mut::<ModelRenderer>(debug_id) {
                    mr.model = None;
                }
            }
        }
    }

    Ok(())
}

fn apply_position_correction(world: &mut World, id: EntityId, offset: Vector3<f32>) {
    if offset.magnitude2() < 1e-6 {
        return;
    }
    // Detected overlap depth is already naturally bounded (radius for
    // sphere/capsule, box extent for OBB), so resolving it fully in one frame
    // is a bounded, sane snap. The old flat 0.25 clamp took several frames to
    // recover from post-fall tunneling, leaving the character visibly sunk
    // into the ground after landing; this ceiling only guards against
    // pathological/corrupted geometry, not ordinary contacts.
    let max_correction = 3.0;
    let offset = if offset.magnitude2() > max_correction * max_correction {
        offset.normalize() * max_correction
    } else {
        offset
    };
    // Compute local-space delta before taking mutable borrows on `world`.
    let local_delta = if let Some(parent_id) = world.get_parent_id(id) {
        if let Some(parent_t) = world.get_component::<Transform>(parent_id) {
            let pr = parent_t.global_rotation;
            let inv_pr = Quaternion::new(pr.s, -pr.v.x, -pr.v.y, -pr.v.z);
            rotate_vector(inv_pr, offset)
        } else {
            offset
        }
    } else {
        offset
    };

    if let Some(t) = world.get_component_mut::<Transform>(id) {
        // Apply correction in world space
        t.global_position += offset;
        // Apply the precomputed local delta
        t.local_position += local_delta;
    }
}

fn resolve_impulse(
    vel_a: &mut Velocity,
    vel_b: &mut Velocity,
    r_a: Vector3<f32>,
    r_b: Vector3<f32>,
    normal: Vector3<f32>,
) {
    let inv_mass_a = if vel_a.mass > 0.0 {
        1.0 / vel_a.mass
    } else {
        0.0
    };
    let inv_mass_b = if vel_b.mass > 0.0 {
        1.0 / vel_b.mass
    } else {
        0.0
    };
    let inv_i_a = vel_a
        .inertia_tensor
        .map(|v| if v > 0.0 { 1.0 / v } else { 0.0 });
    let inv_i_b = vel_b
        .inertia_tensor
        .map(|v| if v > 0.0 { 1.0 / v } else { 0.0 });

    // Velocity at contact point including angular contribution (v + ω × r)
    let va_contact = vel_a.linear_velocity + vel_a.angular_velocity.cross(r_a);
    let vb_contact = vel_b.linear_velocity + vel_b.angular_velocity.cross(r_b);
    let relative_vel = va_contact - vb_contact;

    let vel_along_normal = relative_vel.dot(normal);

    // Already separating
    if vel_along_normal > 0.0 {
        return;
    }

    // Angular contribution
    let ang_term = |inv_i: Vector3<f32>, r: Vector3<f32>| {
        let rxn = r.cross(normal);
        // apply diagonal inverse inertia tensor
        let i_inv_rxn = Vector3::new(inv_i.x * rxn.x, inv_i.y * rxn.y, inv_i.z * rxn.z);
        i_inv_rxn.cross(r).dot(normal)
    };

    let restitution = if vel_along_normal > -0.5 {
        0.0
    } else {
        vel_a.restitution.min(vel_b.restitution)
    };
    let j_denom = inv_mass_a + inv_mass_b + ang_term(inv_i_a, r_a) + ang_term(inv_i_b, r_b);
    if j_denom <= 1e-6 {
        return;
    }
    let j = -(1.0 + restitution) * vel_along_normal / j_denom;

    // Apply normal impulse
    vel_a.linear_velocity += normal * (j * inv_mass_a);
    vel_b.linear_velocity -= normal * (j * inv_mass_b);
    vel_a.angular_velocity += apply_inv_inertia(inv_i_a, r_a.cross(normal * j));
    vel_b.angular_velocity -= apply_inv_inertia(inv_i_b, r_b.cross(normal * j));

    // Friction
    // Recompute relative vel after normal impulse
    let va_contact = vel_a.linear_velocity + vel_a.angular_velocity.cross(r_a);
    let vb_contact = vel_b.linear_velocity + vel_b.angular_velocity.cross(r_b);
    let relative_vel = va_contact - vb_contact;

    let tangential = relative_vel - normal * relative_vel.dot(normal);

    // No sliding
    if tangential.magnitude2() < 1e-8 {
        return;
    }
    let tangent = tangential.normalize();

    // Same denominator structure but along tangent
    let ang_term_t = |inv_i: Vector3<f32>, r: Vector3<f32>| {
        let rxt = r.cross(tangent);
        let i_inv_rxt = Vector3::new(inv_i.x * rxt.x, inv_i.y * rxt.y, inv_i.z * rxt.z);
        i_inv_rxt.cross(r).dot(tangent)
    };

    let jt_denom = inv_mass_a + inv_mass_b + ang_term_t(inv_i_a, r_a) + ang_term_t(inv_i_b, r_b);
    let jt = -relative_vel.dot(tangent) / jt_denom;

    // clamp static vs kinetic
    let mu_s = (vel_a.mu_static * vel_b.mu_static).sqrt();
    let mu_k = (vel_a.mu_kinetic * vel_b.mu_kinetic).sqrt();

    let friction_impulse = if jt.abs() <= j * mu_s {
        tangent * jt
    } else {
        tangent * (j * mu_k * -jt.signum())
    };

    // Don't let friction increase tangential speed.
    let orig_va_contact = vel_a.linear_velocity + vel_a.angular_velocity.cross(r_a);
    let orig_vb_contact = vel_b.linear_velocity + vel_b.angular_velocity.cross(r_b);
    let orig_rel_tang = orig_va_contact
        - orig_vb_contact
        - normal * (orig_va_contact - orig_vb_contact).dot(normal);
    let orig_tang_mag = orig_rel_tang.magnitude();

    // Tentatively apply linear part of friction impulse to test effect on tangential velocity
    let test_va_lin = vel_a.linear_velocity + friction_impulse * inv_mass_a;
    let test_vb_lin = vel_b.linear_velocity - friction_impulse * inv_mass_b;
    let test_va_contact = test_va_lin + vel_a.angular_velocity.cross(r_a);
    let test_vb_contact = test_vb_lin + vel_b.angular_velocity.cross(r_b);
    let test_rel_tang = test_va_contact
        - test_vb_contact
        - normal * (test_va_contact - test_vb_contact).dot(normal);
    let test_tang_mag = test_rel_tang.magnitude();

    let final_friction = if test_tang_mag > orig_tang_mag + 1e-6 {
        // scale down impulse to not increase tangential speed
        if test_tang_mag.abs() > 1e-9 {
            friction_impulse * (orig_tang_mag / test_tang_mag)
        } else {
            Vector3::new(0.0, 0.0, 0.0)
        }
    } else {
        friction_impulse
    };

    vel_a.linear_velocity += final_friction * inv_mass_a;
    vel_b.linear_velocity -= final_friction * inv_mass_b;
    vel_a.angular_velocity += apply_inv_inertia(inv_i_a, r_a.cross(final_friction));
    vel_b.angular_velocity -= apply_inv_inertia(inv_i_b, r_b.cross(final_friction));
}

fn apply_inv_inertia(inv_i: Vector3<f32>, torque: Vector3<f32>) -> Vector3<f32> {
    Vector3::new(inv_i.x * torque.x, inv_i.y * torque.y, inv_i.z * torque.z)
}

fn sphere_vs_sphere(
    center_a: Vector3<f32>,
    ra: f32,
    center_b: Vector3<f32>,
    rb: f32,
) -> Option<Vector3<f32>> {
    let d = center_a - center_b;
    let dist2 = d.magnitude2();
    let sum = ra + rb;
    if dist2 >= sum * sum {
        return None;
    }
    let dist = dist2.sqrt();
    let normal = if dist > 1e-10 {
        d / dist
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    Some(normal * (sum - dist))
}

fn sphere_vs_obb(
    sphere_center: Vector3<f32>,
    radius: f32,
    obb_center: Vector3<f32>,
    obb_axes: &[Vector3<f32>; 3],
    obb_half: Vector3<f32>,
) -> Option<Vector3<f32>> {
    let d = sphere_center - obb_center;
    // Find closest point on OBB to sphere center
    let closest = obb_center
        + obb_axes[0] * d.dot(obb_axes[0]).clamp(-obb_half.x, obb_half.x)
        + obb_axes[1] * d.dot(obb_axes[1]).clamp(-obb_half.y, obb_half.y)
        + obb_axes[2] * d.dot(obb_axes[2]).clamp(-obb_half.z, obb_half.z);

    let diff = sphere_center - closest;
    let dist2 = diff.magnitude2();
    if dist2 >= radius * radius {
        return None;
    }
    let dist = dist2.sqrt();
    let normal = if dist > 1e-10 {
        diff / dist
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    Some(normal * (radius - dist))
}

fn capsule_vs_mesh(
    seg_a: Vector3<f32>,
    seg_b: Vector3<f32>,
    radius: f32,
    bvh: &Bvh,
    mesh_pos: Vector3<f32>,
    mesh_rot: Quaternion<f32>,
) -> Option<Vector3<f32>> {
    if bvh.nodes.is_empty() {
        return None;
    }
    let inv_rot = mesh_rot.conjugate();
    let local_a = inv_rot * (seg_a - mesh_pos);
    let local_b = inv_rot * (seg_b - mesh_pos);

    // AABB of the full capsule for BVH pruning
    let cap_min = Vector3::new(
        local_a.x.min(local_b.x) - radius,
        local_a.y.min(local_b.y) - radius,
        local_a.z.min(local_b.z) - radius,
    );
    let cap_max = Vector3::new(
        local_a.x.max(local_b.x) + radius,
        local_a.y.max(local_b.y) + radius,
        local_a.z.max(local_b.z) + radius,
    );

    let mut stack = vec![0u32];
    let mut deepest: Option<Vector3<f32>> = None;
    let mut max_depth = 0.0f32;

    while let Some(idx) = stack.pop() {
        let node = &bvh.nodes[idx as usize];

        if !aabb_overlaps_aabb(node.aabb_min, node.aabb_max, cap_min, cap_max) {
            continue;
        }

        if node.left == 0 {
            for i in node.tri_start..node.tri_start + node.tri_count {
                let tri = &bvh.triangles[i as usize];
                let (closest_seg, closest_tri) =
                    closest_points_segment_triangle(local_a, local_b, tri);
                let diff = closest_seg - closest_tri;
                let dist2 = diff.magnitude2();
                if dist2 < radius * radius {
                    let dist = dist2.sqrt();
                    let depth = radius - dist;
                    if depth > max_depth {
                        max_depth = depth;
                        let face_normal = triangle_normal(tri);
                        let mut local_normal = if dist > 1e-10 {
                            diff / dist
                        } else {
                            face_normal
                        };
                        if local_normal.dot(face_normal) < 0.0 {
                            local_normal = face_normal;
                        }
                        deepest = Some(mesh_rot * local_normal * depth);
                    }
                }
            }
        } else {
            stack.push(node.left);
            stack.push(node.right);
        }
    }

    deepest
}

fn sphere_vs_mesh(
    center: Vector3<f32>,
    radius: f32,
    bvh: &Bvh,
    mesh_pos: Vector3<f32>,
    mesh_rot: Quaternion<f32>,
) -> Option<Vector3<f32>> {
    if bvh.nodes.is_empty() {
        return None;
    }
    let inv_rot = mesh_rot.conjugate();
    let local_center = inv_rot * (center - mesh_pos);

    let mut stack = vec![0u32];
    let mut deepest: Option<Vector3<f32>> = None;
    let mut max_depth = 0.0f32;

    while let Some(idx) = stack.pop() {
        let node = &bvh.nodes[idx as usize];

        if !aabb_overlaps_sphere(node.aabb_min, node.aabb_max, local_center, radius) {
            continue;
        }

        if node.left == 0 {
            for i in node.tri_start..node.tri_start + node.tri_count {
                let tri = &bvh.triangles[i as usize];
                let closest = closest_point_on_triangle(local_center, tri);
                let diff = local_center - closest;
                let dist2 = diff.magnitude2();
                if dist2 < radius * radius {
                    let dist = dist2.sqrt();
                    let depth = radius - dist;
                    if depth > max_depth {
                        max_depth = depth;
                        let local_normal = if dist > 1e-10 {
                            diff / dist
                        } else {
                            triangle_normal(tri)
                        };
                        deepest = Some(mesh_rot * local_normal * depth);
                    }
                }
            }
        } else {
            stack.push(node.left);
            stack.push(node.right);
        }
    }

    deepest
}

fn aabb_overlaps_sphere(
    min: Vector3<f32>,
    max: Vector3<f32>,
    center: Vector3<f32>,
    radius: f32,
) -> bool {
    let closest = Vector3::new(
        center.x.clamp(min.x, max.x),
        center.y.clamp(min.y, max.y),
        center.z.clamp(min.z, max.z),
    );
    (center - closest).magnitude2() <= radius * radius
}

fn aabb_overlaps_aabb(
    min_a: Vector3<f32>,
    max_a: Vector3<f32>,
    min_b: Vector3<f32>,
    max_b: Vector3<f32>,
) -> bool {
    min_a.x <= max_b.x
        && max_a.x >= min_b.x
        && min_a.y <= max_b.y
        && max_a.y >= min_b.y
        && min_a.z <= max_b.z
        && max_a.z >= min_b.z
}

fn triangle_normal(tri: &[Vector3<f32>; 3]) -> Vector3<f32> {
    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
    if n.magnitude2() > 1e-10 {
        n.normalize()
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    }
}

// Christer Ericson's closest point on triangle algorithm
fn closest_point_on_triangle(p: Vector3<f32>, tri: &[Vector3<f32>; 3]) -> Vector3<f32> {
    let (a, b, c) = (tri[0], tri[1], tri[2]);
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }

    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }

    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

// Returns (closest point on segment, closest point on triangle)
fn closest_points_segment_triangle(
    seg_a: Vector3<f32>,
    seg_b: Vector3<f32>,
    tri: &[Vector3<f32>; 3],
) -> (Vector3<f32>, Vector3<f32>) {
    let mut best_dist2 = f32::MAX;
    let mut best = (seg_a, tri[0]);

    // capsule endpoint A vs triangle
    let q = closest_point_on_triangle(seg_a, tri);
    let d2 = (seg_a - q).magnitude2();
    if d2 < best_dist2 {
        best_dist2 = d2;
        best = (seg_a, q);
    }

    // capsule endpoint B vs triangle
    let q = closest_point_on_triangle(seg_b, tri);
    let d2 = (seg_b - q).magnitude2();
    if d2 < best_dist2 {
        best_dist2 = d2;
        best = (seg_b, q);
    }

    // capsule segment vs each triangle edge
    let edges = [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])];
    for (e0, e1) in edges {
        let (ps, pt) = closest_points_segment_segment(seg_a, seg_b, e0, e1);
        let d2 = (ps - pt).magnitude2();
        if d2 < best_dist2 {
            best_dist2 = d2;
            best = (ps, pt);
        }
    }

    best
}

#[allow(dead_code)]
fn closest_point_on_segment(a: Vector3<f32>, b: Vector3<f32>, p: Vector3<f32>) -> Vector3<f32> {
    let ab = b - a;
    let t = (p - a).dot(ab) / ab.magnitude2().max(1e-10);
    a + ab * t.clamp(0.0, 1.0)
}

fn closest_points_segment_segment(
    p1: Vector3<f32>,
    p2: Vector3<f32>,
    p3: Vector3<f32>,
    p4: Vector3<f32>,
) -> (Vector3<f32>, Vector3<f32>) {
    let d1 = p2 - p1;
    let d2 = p4 - p3;
    let r = p1 - p3;
    let a = d1.magnitude2();
    let e = d2.magnitude2();
    let f = d2.dot(r);

    let (s, t) = if a <= 1e-10 {
        (0.0f32, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-10 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b_dot = d1.dot(d2);
            let denom = a * e - b_dot * b_dot;
            let s = if denom > 1e-10 {
                ((b_dot * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let t = (b_dot * s + f) / e;
            if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b_dot - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };

    (p1 + d1 * s, p3 + d2 * t)
}
/// Calculated the AABB of a triangle.
pub fn triangle_aabb(tris: &[[Vector3<f32>; 3]]) -> (Vector3<f32>, Vector3<f32>) {
    let mut min = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut max = Vector3::new(f32::MIN, f32::MIN, f32::MIN);
    for tri in tris {
        for v in tri {
            min.x = min.x.min(v.x);
            min.y = min.y.min(v.y);
            min.z = min.z.min(v.z);
            max.x = max.x.max(v.x);
            max.y = max.y.max(v.y);
            max.z = max.z.max(v.z);
        }
    }
    (min, max)
}
