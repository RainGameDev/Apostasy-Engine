mod fixed_update;
mod late_update;
mod prerender;
mod start;
mod update;

pub use fixed_update::{DeltaTime, EngineTimer, FixedUpdateSystem, FixedUpdateTimer};
pub use late_update::LateUpdateSystem;
pub use prerender::PreRenderSystem;
pub use start::StartSystem;
pub use update::UpdateSystem;

use crate::EngineMode;

pub trait HasPriority {
    fn priority(&self) -> u32;
}

pub trait HasMode {
    fn mode(&self) -> EngineMode;
}
