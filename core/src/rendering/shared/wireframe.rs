use apostasy_macros::Resource;

/// When `true`, every model, terrain chunk, and voxel chunk renders in
/// wireframe regardless of its own settings. Toggled by the `twf` console
/// command.
#[derive(Resource, Clone, Default)]
pub struct GlobalWireframe(pub bool);
