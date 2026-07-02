use hail_macro::hail;

use crate::scripting::hail::runtime::WorldHandle;

#[hail(method, "log")]
pub fn log(world: &WorldHandle, message: String) {
    let _ = world;
    crate::log!("[hail] {message}");
}
