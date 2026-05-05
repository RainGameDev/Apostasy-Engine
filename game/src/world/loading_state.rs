use apostasy_core::cgmath::Vector3;
use apostasy_macros::Resource;

/// Tracks the state of initial chunk loading
#[derive(Resource, Clone)]
pub struct LoadingState {
    pub is_complete: bool,
    pub initial_player_chunk_pos: Vector3<i32>,
    pub load_radius: i32,
    pub chunks_loaded: usize,
    pub total_chunks_expected: usize,
}

impl LoadingState {
    pub fn new(player_chunk_pos: Vector3<i32>, load_radius: i32) -> Self {
        // Calculate total expected chunks: (2*radius + 1)^3
        let diameter = 2 * load_radius + 1;
        let total_chunks_expected = (diameter * diameter * diameter) as usize;

        Self {
            is_complete: false,
            initial_player_chunk_pos: player_chunk_pos,
            load_radius,
            chunks_loaded: 0,
            total_chunks_expected,
        }
    }

    /// Get the loading progress as a percentage (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        if self.total_chunks_expected == 0 {
            1.0
        } else {
            (self.chunks_loaded as f32 / self.total_chunks_expected as f32).min(1.0)
        }
    }

    /// Check if loading is sufficiently complete (at least 90% loaded)
    pub fn is_progress_sufficient(&self) -> bool {
        self.progress() >= 0.9
    }
}

impl Default for LoadingState {
    fn default() -> Self {
        Self {
            is_complete: true,
            initial_player_chunk_pos: Vector3::new(0, 0, 0),
            load_radius: 8,
            chunks_loaded: 0,
            total_chunks_expected: 0,
        }
    }
}
