use hail::memory_usage::MemoryUsage;
use hail_macro::hail;

use crate::ecs::cell::EntityId;
use crate::ecs::resources::input_manager::InputManager;
use crate::scripting::hail::components::Vec2Value;
use crate::scripting::hail::runtime::WorldHandle;

/// An entity reference for hail scripts, registered as the script type `Entity`.
#[derive(Clone, Copy, PartialEq)]
pub struct EntityHandle(pub EntityId);

impl MemoryUsage for EntityHandle {}

#[hail(method, "==")]
pub fn entity_equal(a: &EntityHandle, b: EntityHandle) -> bool {
    *a == b
}

#[hail(method, "!=")]
pub fn entity_not_equal(a: &EntityHandle, b: EntityHandle) -> bool {
    *a != b
}

#[hail(method, "spawn")]
pub fn spawn(world: &WorldHandle) -> EntityHandle {
    EntityHandle(world.world().spawn().id())
}

#[hail(method, "despawn")]
pub fn despawn(world: &WorldHandle, entity: EntityHandle) {
    world.world().despawn(entity.0);
}

#[hail(method, "set_name")]
pub fn set_name(world: &WorldHandle, entity: EntityHandle, name: String) {
    world.world().set_name(entity.0, &name);
}

#[hail(method, "add_tag")]
pub fn add_tag(world: &WorldHandle, entity: EntityHandle, tag: String) {
    if let Err(e) = world.world().add_tag_by_name(entity.0, &tag) {
        crate::log_error!("[hail] add_tag '{tag}': {e}");
    }
}

#[hail(method, "remove_tag")]
pub fn remove_tag(world: &WorldHandle, entity: EntityHandle, tag: String) {
    world.world().remove_tag_by_name(entity.0, &tag);
}

#[hail(method, "has_tag")]
pub fn has_tag(world: &WorldHandle, entity: EntityHandle, tag: String) -> bool {
    world.world().has_tag_by_name(entity.0, &tag)
}

#[hail(method, "is_keybind_active")]
pub fn is_keybind_active(world: &WorldHandle, name: String) -> bool {
    world
        .world()
        .get_resource::<InputManager>()
        .map(|im| im.is_keybind_active(&name))
        .unwrap_or(false)
}

#[hail(method, "input_vector_2d")]
pub fn input_vector_2d(
    world: &WorldHandle,
    left: String,
    right: String,
    up: String,
    down: String,
) -> Vec2Value {
    let v = world
        .world()
        .get_resource::<InputManager>()
        .map(|im| im.input_vector_2d(&left, &right, &up, &down))
        .unwrap_or_else(|_| cgmath::Vector2::new(0.0, 0.0));
    Vec2Value {
        x: v.x as f64,
        y: v.y as f64,
    }
}

#[hail(method, "mouse_delta")]
pub fn mouse_delta(world: &WorldHandle) -> Vec2Value {
    let delta = world
        .world()
        .get_resource::<InputManager>()
        .map(|im| im.mouse_delta)
        .unwrap_or((0.0, 0.0));
    Vec2Value {
        x: delta.0 as f64,
        y: delta.1 as f64,
    }
}

#[hail(method, "entities_with_tag")]
pub fn entities_with_tag(world: &WorldHandle, tag: String) -> Vec<EntityHandle> {
    world
        .world()
        .get_entities_with_tag_by_name(&tag)
        .into_iter()
        .map(EntityHandle)
        .collect()
}
