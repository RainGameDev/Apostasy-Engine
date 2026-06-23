use std::ops::Deref;

use crate::ecs::components::Component;
use crate::ecs::tags::Tag;
use crate::ecs::world::World;
use crate::worldspaces::cell::EntityId;

/// Fluent builder returned by [`World::spawn`] and friends.
///
/// Holds a mutable borrow of the [`World`] together with the freshly spawned
/// entity id, so components, tags and a name can be attached in a single chain:
///
/// ```ignore
/// let player = world
///     .spawn()
///     .add_component(Transform::default())
///     .add_component(Velocity::default())
///     .add_tag::<Player>()
///     .with_name("Player")
///     .id();
/// ```
///
/// Each method is eagerly side-effecting — the entity and its components exist
/// in the world as soon as the call returns — so a fire-and-forget chain
/// without a trailing `.id()` is valid. Call [`EntityBuilder::id`] only when you
/// need the [`EntityId`]. The builder also derefs to [`EntityId`], so
/// `*world.spawn()` works too.
pub struct EntityBuilder<'w> {
    world: &'w mut World,
    id: EntityId,
}

impl<'w> EntityBuilder<'w> {
    pub(crate) fn new(world: &'w mut World, id: EntityId) -> Self {
        Self { world, id }
    }

    /// The id of the entity being built. Use this to finish the chain.
    pub fn id(&self) -> EntityId {
        self.id
    }

    /// Attaches a component to the entity.
    pub fn add_component<T: Component + Clone + 'static>(self, component: T) -> Self {
        self.world.add_component(self.id, component);
        self
    }

    /// Alias for [`EntityBuilder::add_component`].
    pub fn with_component<T: Component + Clone + 'static>(self, component: T) -> Self {
        self.add_component(component)
    }

    /// Attaches a zero-size tag to the entity.
    pub fn add_tag<T: Tag + 'static>(self) -> Self {
        self.world.add_tag::<T>(self.id);
        self
    }

    /// Alias for [`EntityBuilder::add_tag`].
    pub fn with_tag<T: Tag + 'static>(self) -> Self {
        self.add_tag::<T>()
    }

    /// Sets the entity's display name.
    pub fn with_name(self, name: &str) -> Self {
        self.world.set_name(self.id, name);
        self
    }

    /// Parents this entity under `parent`.
    pub fn child_of(self, parent: EntityId) -> Self {
        let _ = self.world.set_parent(self.id, Some(parent));
        self
    }

    /// Escape hatch: run arbitrary setup against the world for this entity.
    pub fn with(self, mut f: impl FnMut(&mut World, EntityId)) -> Self {
        f(self.world, self.id);
        self
    }
}

impl Deref for EntityBuilder<'_> {
    type Target = EntityId;

    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

impl From<EntityBuilder<'_>> for EntityId {
    fn from(builder: EntityBuilder<'_>) -> Self {
        builder.id
    }
}
