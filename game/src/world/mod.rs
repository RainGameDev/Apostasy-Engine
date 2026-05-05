use apostasy_macros::Tag;

pub mod chunk_loader;
pub mod generation;
pub mod raycast;
pub mod loading_state;
pub mod loading_screen;

#[derive(Tag, Clone)]
pub struct VoxelOutline;
