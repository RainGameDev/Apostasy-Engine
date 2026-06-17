use std::collections::HashMap;
use std::collections::VecDeque;

use apostasy_macros::Resource;

pub const HISTORY_SIZE: usize = 512;

#[derive(Clone, Default)]
pub struct SystemTiming {
    pub phase: &'static str,
    pub name: &'static str,
    pub elapsed_ns: u64,
}

#[derive(Clone, Default)]
pub struct FrameSample {
    pub frame_ns: u64,
    pub render_total_ns: u64,
    pub shadow_pass_ns: u64,
    pub viewport_render_ns: u64,
    pub world_prerender_ns: u64,
    pub world_update_ns: u64,
    pub world_fixed_update_ns: u64,
    pub world_late_update_ns: u64,
}

#[derive(Clone, Resource)]
pub struct Profiler {
    pub history: VecDeque<FrameSample>,

    // Smoothed values for stable display
    pub frame_time_smooth: f64,
    pub world_update_smooth: f64,
    pub world_fixed_update_smooth: f64,
    pub world_prerender_smooth: f64,
    pub world_late_update_smooth: f64,
    pub render_total_smooth: f64,
    pub shadow_pass_smooth: f64,
    pub viewport_render_smooth: f64,

    // Per-system timing (latest frame only)
    pub last_system_timings: Vec<SystemTiming>,
    // Per-system smoothed values keyed by "phase::name"
    pub system_timings_smooth: HashMap<String, f64>,
}

impl Default for Profiler {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(HISTORY_SIZE),
            frame_time_smooth: 0.0,
            world_update_smooth: 0.0,
            world_fixed_update_smooth: 0.0,
            world_prerender_smooth: 0.0,
            world_late_update_smooth: 0.0,
            render_total_smooth: 0.0,
            shadow_pass_smooth: 0.0,
            viewport_render_smooth: 0.0,
            last_system_timings: Vec::new(),
            system_timings_smooth: HashMap::new(),
        }
    }
}

const ALPHA: f64 = 0.1;

impl Profiler {
    fn smooth(prev: f64, new: u64) -> f64 {
        ALPHA * new as f64 + (1.0 - ALPHA) * prev
    }

    pub fn push_sample(&mut self, sample: FrameSample, system_timings: Vec<SystemTiming>) {
        self.frame_time_smooth = Self::smooth(self.frame_time_smooth, sample.frame_ns);
        self.render_total_smooth = Self::smooth(self.render_total_smooth, sample.render_total_ns);
        self.shadow_pass_smooth = Self::smooth(self.shadow_pass_smooth, sample.shadow_pass_ns);
        self.viewport_render_smooth =
            Self::smooth(self.viewport_render_smooth, sample.viewport_render_ns);
        self.world_prerender_smooth =
            Self::smooth(self.world_prerender_smooth, sample.world_prerender_ns);
        self.world_update_smooth = Self::smooth(self.world_update_smooth, sample.world_update_ns);
        self.world_fixed_update_smooth =
            Self::smooth(self.world_fixed_update_smooth, sample.world_fixed_update_ns);
        self.world_late_update_smooth =
            Self::smooth(self.world_late_update_smooth, sample.world_late_update_ns);

        // Update per-system smoothed values
        self.last_system_timings = system_timings;
        for timing in &self.last_system_timings {
            let key = format!("{}::{}", timing.phase, timing.name);
            let prev = self.system_timings_smooth.get(&key).copied().unwrap_or(0.0);
            self.system_timings_smooth
                .insert(key, Self::smooth(prev, timing.elapsed_ns));
        }

        if self.history.len() == HISTORY_SIZE {
            self.history.pop_front();
        }
        self.history.push_back(sample);
    }

    /// Current FPS from smoothed frame time.
    pub fn fps_current(&self) -> f64 {
        if self.frame_time_smooth > 0.0 {
            1_000_000_000.0 / self.frame_time_smooth
        } else {
            0.0
        }
    }

    /// Average FPS over the entire history window.
    pub fn fps_avg(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }
        let mean_ns: f64 =
            self.history.iter().map(|s| s.frame_ns as f64).sum::<f64>() / self.history.len() as f64;
        if mean_ns > 0.0 {
            1_000_000_000.0 / mean_ns
        } else {
            0.0
        }
    }

    /// 1% low FPS — average FPS of the slowest 1% of frames.
    pub fn fps_low_1(&self) -> f64 {
        Self::percentile_low_fps(&self.history, 0.01)
    }

    /// 0.1% low FPS — average FPS of the slowest 0.1% of frames.
    pub fn fps_low_01(&self) -> f64 {
        Self::percentile_low_fps(&self.history, 0.001)
    }

    fn percentile_low_fps(history: &VecDeque<FrameSample>, frac: f64) -> f64 {
        if history.is_empty() {
            return 0.0;
        }
        let mut times: Vec<u64> = history.iter().map(|s| s.frame_ns).collect();
        times.sort_unstable_by(|a, b| b.cmp(a)); // descending — slowest first
        let count = ((times.len() as f64 * frac).ceil() as usize).max(1);
        let worst = &times[..count.min(times.len())];
        let mean_ns: f64 = worst.iter().map(|&t| t as f64).sum::<f64>() / worst.len() as f64;
        if mean_ns > 0.0 {
            1_000_000_000.0 / mean_ns
        } else {
            0.0
        }
    }

    pub fn fmt_ns(ns: f64) -> String {
        if ns >= 1_000_000.0 {
            format!("{:.2} ms", ns / 1_000_000.0)
        } else if ns >= 1_000.0 {
            format!("{:.1} µs", ns / 1_000.0)
        } else {
            format!("{:.0} ns", ns)
        }
    }
}
