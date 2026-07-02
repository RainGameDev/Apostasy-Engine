use hail_macro::hail;

use crate::ecs::systems::{DeltaTime, EngineTimer};
use crate::rendering::WindowInfo;
use crate::scripting::hail::components::Vec2Value;
use crate::scripting::hail::runtime::WorldHandle;
use crate::states::ShouldExit;

/// Seconds since the last frame.
#[hail(property, "delta_time")]
pub fn delta_time(world: &WorldHandle) -> f64 {
    world
        .world()
        .get_resource::<DeltaTime>()
        .map_or(0.0, |dt| dt.0 as f64)
}

/// Total elapsed seconds since engine start.
#[hail(property, "time")]
pub fn time(world: &WorldHandle) -> f64 {
    world
        .world()
        .get_resource::<EngineTimer>()
        .map_or(0.0, |t| t.0 as f64)
}

/// Current window size in pixels.
#[hail(property, "window_size")]
pub fn window_size(world: &WorldHandle) -> Vec2Value {
    world
        .world()
        .get_resource::<WindowInfo>()
        .map_or(Vec2Value::default(), |w| Vec2Value {
            x: w.window_size.0 as f64,
            y: w.window_size.1 as f64,
        })
}

/// Asks the engine to exit at the end of the frame.
#[hail(method, "quit")]
pub fn quit(world: &WorldHandle) {
    world.world().insert_resource(ShouldExit);
}
