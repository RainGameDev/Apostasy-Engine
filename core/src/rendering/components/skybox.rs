use std::{f32::consts::PI, sync::Arc};

use crate::ecs::Query;
use anyhow::Result;
use apostasy_macros::{Component, update};
use ash::vk;
use serde::{Deserialize, Serialize};

use crate::{
    assets::gltf::upload_texture_from_pixels,
    ecs::{
        components::{Inspect, transform::Transform},
        systems::DeltaTime,
        world::World,
    },
    egui::{self, DragAndDrop, StrokeKind},
    rendering::{
        shared::{model::Mesh, texture::GpuTexture, vertex::Vertex},
        vulkan::rendering_context::VulkanRenderingContext,
    },
    terrain::texture_atlas::load_terrain_texture,
    ui::{DRAG_SIZE, LABEL_WIDTH},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SkyProjection {
    Spherical,
    #[default]
    Planar,
    Celestial,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkyLayer {
    pub path: Option<String>,
    pub projection: SkyProjection,
    /// UV tiling scale for planar projection.
    pub scale: f32,
}

impl Default for SkyLayer {
    fn default() -> Self {
        Self {
            path: None,
            projection: SkyProjection::Planar,
            scale: 0.3,
        }
    }
}

#[derive(Clone, Debug, Component, Serialize, Deserialize)]
#[component(serde)]
#[serde(default)]
pub struct Skybox {
    /// The paths to the day textures,
    /// Textures are loaded as layers (0 then 1 then 2 ect)
    pub day_textures: Vec<SkyLayer>,
    /// The paths to the night textures,
    /// Textures are loaded as layers (0 then 1 then 2 ect)
    pub night_textures: Vec<SkyLayer>,
    // and the snapshots:
    #[serde(skip)]
    pub(crate) loaded_day_paths: Vec<SkyLayer>,
    #[serde(skip)]
    pub(crate) loaded_night_paths: Vec<SkyLayer>,

    /// Time in a 24 hour setting
    pub time: f32,
    /// Should the time progress
    pub progress_time: bool,
    /// Real seconds for a full 24 hour cycle.
    pub day_length: f32,
    /// The blend between day and night textures
    pub blend: f32,

    /// Gpu textures for the day.
    #[serde(skip)]
    pub(crate) day_texture_resources: Vec<Option<GpuTexture>>,
    /// Gpu textures for the night.
    #[serde(skip)]
    pub(crate) night_texture_resources: Vec<Option<GpuTexture>>,
    /// Mesh used to render the skybox.
    #[serde(skip)]
    pub(crate) skybox_mesh: Option<Mesh>,
}

impl Default for Skybox {
    fn default() -> Self {
        Self {
            day_textures: Vec::new(),
            night_textures: Vec::new(),
            time: 12.0,
            progress_time: false,
            day_length: 600.0,
            blend: 0.0,
            day_texture_resources: Vec::new(),
            night_texture_resources: Vec::new(),
            skybox_mesh: None,
            loaded_day_paths: Vec::new(),
            loaded_night_paths: Vec::new(),
        }
    }
}

/// Editable list of layer texture paths with add/remove and texture drag-drop.
/// Typed edits commit on Enter/focus-loss so the render loop doesn't re-upload
/// textures on every keystroke.
fn texture_list_ui(ui: &mut egui::Ui, label: &str, layers: &mut Vec<SkyLayer>) {
    let row_h = 20.0;
    let has_texture_drag = DragAndDrop::has_payload_of_type::<String>(ui.ctx());

    ui.label(label);
    let mut remove: Option<usize> = None;
    for i in 0..layers.len() {
        let draft_id = ui.make_persistent_id(("skybox_layer", label, i));
        let committed = layers[i].path.clone().unwrap_or_default();
        let mut draft: String = ui
            .data_mut(|d| d.get_temp(draft_id))
            .unwrap_or_else(|| committed.clone());

        let inner = ui.horizontal(|ui| {
            ui.add_sized(
                [LABEL_WIDTH, row_h],
                egui::Label::new(format!("Layer {}", i)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized([20.0, row_h], egui::Button::new("✕"))
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.add_sized(
                    [ui.available_width(), row_h],
                    egui::TextEdit::singleline(&mut draft).hint_text("drag a texture here…"),
                )
            })
            .inner
        });
        let resp = inner.inner;

        if resp.changed() {
            ui.data_mut(|d| d.insert_temp(draft_id, draft.clone()));
        }
        if resp.lost_focus() {
            ui.data_mut(|d| d.remove_temp::<String>(draft_id));
            if draft != committed {
                let trimmed = draft.trim();
                layers[i].path = (!trimmed.is_empty()).then(|| trimmed.to_string());
            }
        }

        if has_texture_drag && ui.rect_contains_pointer(resp.rect) {
            ui.painter().rect_stroke(
                resp.rect.expand(2.0),
                3.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 160, 220)),
                StrokeKind::Outside,
            );
        }
        if let Some(payload) = resp.dnd_release_payload::<String>()
            && let Some(name) = payload.strip_prefix("texture:")
        {
            layers[i].path = Some(name.to_string());
            ui.data_mut(|d| d.remove_temp::<String>(draft_id));
        }

        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("  Projection"));
            egui::ComboBox::from_id_salt(("sky_projection", label, i))
                .selected_text(match layers[i].projection {
                    SkyProjection::Spherical => "Spherical",
                    SkyProjection::Planar => "Planar",
                    SkyProjection::Celestial => "Celestial",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut layers[i].projection,
                        SkyProjection::Spherical,
                        "Spherical",
                    );
                    ui.selectable_value(&mut layers[i].projection, SkyProjection::Planar, "Planar");
                    ui.selectable_value(
                        &mut layers[i].projection,
                        SkyProjection::Celestial,
                        "Celestial",
                    );
                });
            if layers[i].projection == SkyProjection::Planar {
                ui.add_sized(
                    DRAG_SIZE,
                    egui::DragValue::new(&mut layers[i].scale)
                        .speed(0.05)
                        .range(0.05..=32.0)
                        .prefix("scale "),
                );
            }
        });
    }
    if let Some(i) = remove {
        layers.remove(i);
    }
    if ui.button("Add layer").clicked() {
        layers.push(SkyLayer::default());
    }
}

impl Inspect for Skybox {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        let row_h = 20.0;

        texture_list_ui(ui, "Day textures", &mut self.day_textures);
        texture_list_ui(ui, "Night textures", &mut self.night_textures);

        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Time"));
            ui.add(egui::Slider::new(&mut self.time, 0.0..=24.0));
        });
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Progress time"));
            ui.checkbox(&mut self.progress_time, "");
        });
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Day length"));
            ui.add_sized(
                DRAG_SIZE,
                egui::DragValue::new(&mut self.day_length)
                    .speed(1.0)
                    .range(1.0..=86400.0)
                    .suffix(" s"),
            );
        });
        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Blend"));
            ui.add(egui::Slider::new(&mut self.blend, 0.0..=1.0));
        });
    }
}

/// Loads one sky layer texture (project/res path resolution) and uploads it
/// with its own combined-image-sampler descriptor set.
pub(crate) fn upload_skybox_layer(
    context: &Arc<VulkanRenderingContext>,
    command_pool: vk::CommandPool,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set_layout: vk::DescriptorSetLayout,
    path: &str,
) -> Result<GpuTexture> {
    let rgba = load_terrain_texture(path).to_rgba8();
    upload_texture_from_pixels(
        rgba.as_raw(),
        rgba.width().max(1),
        rgba.height().max(1),
        path,
        ash::vk::Format::R8G8B8A8_SRGB,
        context,
        command_pool,
        descriptor_pool,
        descriptor_set_layout,
    )
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Advances `time` and derives `blend` from the sun's elevation. Both stay
/// manual while `progress_time` is off.
#[update(mode = "all", priority = 2)]
pub fn skybox_time_update(world: &mut World) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    for id in world.get_entities_with_component::<Skybox>() {
        let Some(sky) = world.get_component_mut::<Skybox>(id) else {
            continue;
        };
        if !sky.progress_time {
            continue;
        }
        if sky.day_length > 0.0 {
            sky.time = (sky.time + delta * 24.0 / sky.day_length).rem_euclid(24.0);
        }
        let sun_elevation = ((sky.time - 6.0) / 24.0 * std::f32::consts::TAU).sin();
        sky.blend = 1.0 - smoothstep(-0.15, 0.1, sun_elevation);
    }

    Ok(())
}

pub fn build_skybox_sphere_mesh(
    context: &Arc<VulkanRenderingContext>,
    command_pool: vk::CommandPool,
) -> Result<Mesh> {
    const RINGS: u32 = 16;
    const SEGMENTS: u32 = 32;

    let mut vertices: Vec<Vertex> = Vec::with_capacity(((RINGS + 1) * (SEGMENTS + 1)) as usize);
    for ring in 0..=RINGS {
        let v = ring as f32 / RINGS as f32;
        let phi = v * PI;
        for segment in 0..=SEGMENTS {
            let u = segment as f32 / SEGMENTS as f32;
            let theta = u * std::f32::consts::TAU;
            let position = [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()];
            vertices.push(Vertex {
                position,
                normal: position,
                tex_coord: [u, v],
                weights: [0.0; 32],
                color: [1.0, 1.0, 1.0],
                tangent: [1.0, 0.0, 0.0, 1.0],
            });
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity((RINGS * SEGMENTS * 6) as usize);
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let a = ring * (SEGMENTS + 1) + segment;
            let b = a + SEGMENTS + 1;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    let (vertex_buffer, vertex_buffer_memory) =
        context.create_vertex_buffer(&vertices, command_pool)?;
    let (index_buffer, index_buffer_memory) =
        context.create_index_buffer(&indices, command_pool)?;

    Ok(Mesh {
        vertex_buffer,
        vertex_buffer_memory,
        index_buffer,
        index_buffer_memory,
        index_count: indices.len() as u32,
        material_name: String::new(),
        material: None,
    })
}

#[update(mode = "all")]
pub fn update_skybox_time(
    world: &mut World,
    q: Query<(&mut Transform, &mut Skybox)>,
) -> Result<()> {
    let delta = world.get_resource::<DeltaTime>()?.0;

    q.for_each(|_id, (transform, sky)| {
        if sky.progress_time {
            if sky.day_length > 0.0 {
                sky.time = (sky.time + delta * 24.0 / sky.day_length).rem_euclid(24.0);
            }
            let time_of_day = sky.time;
            let pitch = -(time_of_day - 6.0) / 24.0 * 360.0;
            let sun_elevation = -pitch.to_radians().sin();

            let blend = 1.0 - smoothstep(-0.15, 0.1, sun_elevation);

            transform.local_euler_angles.x = pitch;
            sky.blend = blend;
        }
    });

    Ok(())
}
