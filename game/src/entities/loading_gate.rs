use apostasy_macros::Tag;

/// Tag that prevents the player from moving and spawning until loading is complete
#[derive(Tag, Clone)]
pub struct LoadingGate;
