use apostasy_macros::Tag;

pub mod chunk_loader;
pub mod generation;
pub mod loading_screen;
pub mod loading_state;
pub mod raycast;
pub mod structure_selection;

#[derive(Tag, Clone)]
pub struct VoxelOutline;
