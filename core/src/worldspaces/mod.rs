use apostasy_macros::Resource;

pub mod cell;
pub mod cell_streaming;
pub mod worldspace;
pub mod worldspace_serializer;
pub mod worldspace_streaming;

/// Name of the currently loaded worldspace. Set by `load_worldspace`.
#[derive(Resource, Clone, Default)]
pub struct CurrentWorldspace(pub String);
