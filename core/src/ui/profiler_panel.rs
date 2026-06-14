use anyhow::Result;
use apostasy_macros::{Resource, start, update};
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::objects::resources::input_manager::{InputManager, KeyAction, KeyBind};
use crate::objects::world::World;
use crate::ui::ui_context::EguiContext;
use crate::utils::profiler::{FrameSample, Profiler};

#[derive(Resource, Clone, Default)]
pub struct ProfilerPanelState {
    pub open: bool,
}

// colours per category

const COL_SHADOW: Color32 = Color32::from_rgb(80, 120, 200);
const COL_VIEWPORT: Color32 = Color32::from_rgb(60, 180, 160);
const COL_RENDER_OTHER: Color32 = Color32::from_rgb(140, 80, 200);
const COL_PRERENDER: Color32 = Color32::from_rgb(200, 130, 60);
const COL_UPDATE: Color32 = Color32::from_rgb(200, 200, 60);
const COL_FIXED: Color32 = Color32::from_rgb(160, 200, 60);
const COL_LATE: Color32 = Color32::from_rgb(200, 80, 180);
const COL_OTHER: Color32 = Color32::from_rgb(100, 100, 100);
const COL_GRAPH_LINE: Color32 = Color32::from_rgb(200, 200, 200);
const COL_GRAPH_BG: Color32 = Color32::from_rgb(25, 25, 25);
const COL_GRAPH_GRID: Color32 = Color32::from_rgb(55, 55, 55);
const COL_GRAPH_TARGET: Color32 = Color32::from_rgb(80, 180, 80);

const GRAPH_HEIGHT: f32 = 80.0;
const BARS_HEIGHT: f32 = 60.0;

// category breakdown for one sample

struct Slices {
    shadow: u64,
    viewport: u64,
    render_other: u64,
    prerender: u64,
    update: u64,
    fixed: u64,
    late: u64,
    other: u64,
}

impl Slices {
    fn from(s: &FrameSample) -> Self {
        let render_accounted = s.shadow_pass_ns + s.viewport_render_ns;
        let render_other = s.render_total_ns.saturating_sub(render_accounted);
        let world_total = s.world_prerender_ns
            + s.world_update_ns
            + s.world_fixed_update_ns
            + s.world_late_update_ns;
        let other = s
            .frame_ns
            .saturating_sub(s.render_total_ns.saturating_add(world_total));
        Slices {
            shadow: s.shadow_pass_ns,
            viewport: s.viewport_render_ns,
            render_other,
            prerender: s.world_prerender_ns,
            update: s.world_update_ns,
            fixed: s.world_fixed_update_ns,
            late: s.world_late_update_ns,
            other,
        }
    }

    fn total(&self) -> u64 {
        self.shadow
            + self.viewport
            + self.render_other
            + self.prerender
            + self.update
            + self.fixed
            + self.late
            + self.other
    }

    fn as_pairs(&self) -> [(&'static str, u64, Color32); 8] {
        [
            ("Shadow Pass", self.shadow, COL_SHADOW),
            ("Viewport Render", self.viewport, COL_VIEWPORT),
            ("Render Other", self.render_other, COL_RENDER_OTHER),
            ("Prerender", self.prerender, COL_PRERENDER),
            ("World Update", self.update, COL_UPDATE),
            ("World Fixed", self.fixed, COL_FIXED),
            ("World Late", self.late, COL_LATE),
            ("Other", self.other, COL_OTHER),
        ]
    }
}

// stat block

fn stat_block(ui: &mut egui::Ui, label: &str, fps: f64, sub: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).small().weak());
        ui.label(
            RichText::new(format!("{:.1}", fps))
                .strong()
                .monospace()
                .size(18.0),
        );
        ui.label(RichText::new(sub).small().monospace().weak());
    });
}

// frame time line graph

fn draw_frametime_graph(ui: &mut egui::Ui, history: &std::collections::VecDeque<FrameSample>) {
    let (response, painter) = ui.allocate_painter(
        Vec2::new(ui.available_width(), GRAPH_HEIGHT),
        Sense::hover(),
    );
    let rect = response.rect;
    painter.rect_filled(rect, 2.0, COL_GRAPH_BG);

    if history.is_empty() {
        return;
    }

    let target_ns = 16_666_666.0_f64;
    let max_ns = history
        .iter()
        .map(|s| s.frame_ns)
        .max()
        .unwrap_or(1)
        .max(target_ns as u64 * 2) as f64;

    for &ms in &[8u64, 16, 33] {
        let ns = ms as f64 * 1_000_000.0;
        if ns <= max_ns {
            let y = rect.max.y - (ns / max_ns) as f32 * rect.height();
            let color = if ms == 16 {
                COL_GRAPH_TARGET
            } else {
                COL_GRAPH_GRID
            };
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                Stroke::new(1.0, color),
            );
            painter.text(
                Pos2::new(rect.min.x + 2.0, y - 1.0),
                Align2::LEFT_BOTTOM,
                format!("{}ms", ms),
                FontId::monospace(8.0),
                color,
            );
        }
    }

    let n = history.len();
    let points: Vec<Pos2> = history
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let x = rect.min.x + (i as f32 / (n - 1).max(1) as f32) * rect.width();
            let y = rect.max.y - (s.frame_ns as f64 / max_ns) as f32 * rect.height();
            Pos2::new(x, y.clamp(rect.min.y, rect.max.y))
        })
        .collect();

    for w in points.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(1.0, COL_GRAPH_LINE));
    }
}

// stacked bar char

fn draw_stacked_bars(ui: &mut egui::Ui, history: &std::collections::VecDeque<FrameSample>) {
    let target_ns = 16_666_666.0_f64;
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), BARS_HEIGHT), Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 2.0, COL_GRAPH_BG);

    if history.is_empty() {
        return;
    }

    let max_ns = history
        .iter()
        .map(|s| s.frame_ns)
        .max()
        .unwrap_or(1)
        .max(target_ns as u64 * 2) as f64;

    let n = history.len();
    let bar_w = (rect.width() / n as f32).max(1.0);

    for (i, sample) in history.iter().enumerate() {
        let x = rect.min.x + i as f32 * bar_w;
        let slices = Slices::from(sample);
        let total = slices.total().max(1) as f64;
        let bar_h = (sample.frame_ns as f64 / max_ns) as f32 * rect.height();

        let mut y_offset = 0.0_f32;
        for (_, ns, color) in slices.as_pairs() {
            if ns == 0 {
                continue;
            }
            let seg_h = (ns as f64 / total) as f32 * bar_h;
            let seg_rect = Rect::from_min_size(
                Pos2::new(x, rect.max.y - y_offset - seg_h),
                Vec2::new(bar_w.max(1.0), seg_h),
            );
            painter.rect_filled(seg_rect.shrink(0.3), 0.0, color);
            y_offset += seg_h;
        }
    }

    let target_y = rect.max.y - (target_ns / max_ns) as f32 * rect.height();
    if target_y >= rect.min.y {
        painter.line_segment(
            [
                Pos2::new(rect.min.x, target_y),
                Pos2::new(rect.max.x, target_y),
            ],
            Stroke::new(1.0, COL_GRAPH_TARGET),
        );
    }
}

// legend + timing row

fn legend_row(ui: &mut egui::Ui, color: Color32, label: &str, value_ns: f64) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
        ui.painter().rect_filled(r, 2.0, color);
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = Profiler::fmt_ns(value_ns);
            let col = if value_ns >= 8_000_000.0 {
                Color32::from_rgb(220, 80, 80)
            } else if value_ns >= 4_000_000.0 {
                Color32::from_rgb(220, 160, 60)
            } else {
                Color32::from_rgb(100, 200, 100)
            };
            ui.label(RichText::new(text).color(col).monospace());
        });
    });
}

// systems

#[start(mode = "all")]
pub fn profiler_start(world: &mut World) -> Result<()> {
    world.insert_resource(ProfilerPanelState::default());
    if let Ok(inputs) = world.get_resource_mut::<InputManager>() {
        inputs.register_default_keybind(
            "Toggle Profiler",
            KeyBind::new(PhysicalKey::Code(KeyCode::F3), KeyAction::Press),
        );
    }
    Ok(())
}

#[update(mode = "all")]
pub fn profiler_keybind(world: &mut World) -> Result<()> {
    let pressed = world
        .get_resource::<InputManager>()
        .map(|i| i.is_keybind_active("Toggle Profiler"))
        .unwrap_or(false);

    if pressed {
        if let Ok(s) = world.get_resource_mut::<ProfilerPanelState>() {
            s.open = !s.open;
        }
    }
    Ok(())
}

#[update(mode = "all")]
pub fn profiler_panel(world: &mut World) -> Result<()> {
    let open = world
        .get_resource::<ProfilerPanelState>()
        .map(|s| s.open)
        .unwrap_or(false);

    if !open {
        return Ok(());
    }

    let ctx = world.get_resource::<EguiContext>()?.0.clone();
    let profiler = world
        .get_resource::<Profiler>()
        .cloned()
        .unwrap_or_default();

    let mut still_open = open;

    egui::Window::new("Profiler")
        .open(&mut still_open)
        .resizable(true)
        .min_width(360.0)
        .default_width(420.0)
        .show(&ctx, |ui| {
            // FPS stats
            ui.horizontal(|ui| {
                let col_w = ui.available_width() / 4.0 - 8.0;

                stat_block(
                    ui,
                    "FPS",
                    profiler.fps_current(),
                    &Profiler::fmt_ns(profiler.frame_time_smooth),
                );
                ui.add_space((col_w - 60.0).max(0.0));
                stat_block(
                    ui,
                    "AVG",
                    profiler.fps_avg(),
                    &format!("{} samples", profiler.history.len()),
                );
                ui.add_space((col_w - 60.0).max(0.0));
                stat_block(ui, "1% LOW", profiler.fps_low_1(), "worst 1%");
                ui.add_space((col_w - 60.0).max(0.0));
                stat_block(ui, "0.1% LOW", profiler.fps_low_01(), "worst 0.1%");
            });

            ui.separator();

            // frame time graph
            ui.label(RichText::new("Frame Time").small().weak());
            draw_frametime_graph(ui, &profiler.history);
            ui.add_space(4.0);

            // stacked bars
            ui.label(RichText::new("Breakdown Per Frame").small().weak());
            draw_stacked_bars(ui, &profiler.history);
            ui.add_space(4.0);
            ui.separator();

            // legend + per-category times
            let last = profiler.history.back().cloned().unwrap_or_default();
            let slices = Slices::from(&last);

            legend_row(ui, COL_SHADOW, "  Shadow Pass", slices.shadow as f64);
            legend_row(
                ui,
                COL_VIEWPORT,
                "  Viewport Render",
                slices.viewport as f64,
            );
            legend_row(
                ui,
                COL_RENDER_OTHER,
                "  Render Other",
                slices.render_other as f64,
            );
            legend_row(
                ui,
                COL_PRERENDER,
                "World Prerender",
                slices.prerender as f64,
            );
            legend_row(ui, COL_UPDATE, "World Update", slices.update as f64);
            legend_row(ui, COL_FIXED, "World Fixed Update", slices.fixed as f64);
            legend_row(ui, COL_LATE, "World Late Update", slices.late as f64);
            legend_row(ui, COL_OTHER, "Other", slices.other as f64);

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Render Total");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(Profiler::fmt_ns(profiler.render_total_smooth))
                            .monospace()
                            .weak(),
                    );
                });
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Frame Total").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(Profiler::fmt_ns(profiler.frame_time_smooth))
                            .monospace()
                            .strong(),
                    );
                });
            });
        });

    if !still_open {
        if let Ok(s) = world.get_resource_mut::<ProfilerPanelState>() {
            s.open = false;
        }
    }

    Ok(())
}
