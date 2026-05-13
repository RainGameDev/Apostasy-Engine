use apostasy_macros::Tag;

pub mod cache;
pub mod chunk_loader;
pub mod consts;
pub mod generation;
pub mod heightmap;
pub mod helpers;
pub mod loading_screen;
pub mod loading_state;
pub mod noise;
pub mod raycast;
pub mod structure_selection;

#[derive(Tag, Clone)]
pub struct VoxelOutline;
