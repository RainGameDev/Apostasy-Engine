pub mod components;
pub mod resources;
pub mod sets;
pub mod systems;
pub mod tags;
pub mod world;

pub use world::World;

// Reexport worldspace modules through ecs so existing import paths still resolve.
pub use crate::worldspaces::cell;
pub use crate::worldspaces::worldspace_serializer;
pub use crate::worldspaces::worldspace_streaming;

// Backward compat aliases: files importing `ecs::component` or `ecs::tag` still compile.
pub use components as component;
pub use tags as tag;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Entity {
    pub index: u32,
    /// Incremented on despawn to invalidate stale handles.
    pub generation: u32,
}

#[derive(Default)]
pub struct EntityAllocator {
    /// One entry per ever created index.
    generations: Vec<u32>,
    /// Indices of despawned entities, ready to reuse.
    free: Vec<u32>,
}

impl EntityAllocator {
    /// Spawns a new entity. Reuses a freed index if one is available.
    pub fn spawn(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
            Entity {
                index,
                generation: self.generations[index as usize],
            }
        } else {
            let index = self.generations.len() as u32;
            self.generations.push(0);
            Entity {
                index,
                generation: 0,
            }
        }
    }

    /// Despawns an entity, bumping its generation to invalidate all existing handles.
    /// Returns false if the entity is already dead or invalid.
    pub fn despawn(&mut self, entity: Entity) -> bool {
        let idx = entity.index as usize;
        if idx >= self.generations.len() || self.generations[idx] != entity.generation {
            return false;
        }
        self.generations[idx] += 1;
        self.free.push(entity.index);
        true
    }

    /// Returns true if the entity is still alive (not yet despawned).
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.generations
            .get(entity.index as usize)
            .map(|&g| g == entity.generation)
            .unwrap_or(false)
    }

    /// Returns the current generation at an index, or None if the index is out of range.
    pub fn current_generation(&self, index: u32) -> Option<u32> {
        self.generations.get(index as usize).copied()
    }
}
