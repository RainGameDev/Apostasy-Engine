use crate::ecs::Entity;

pub struct SparseSet<T> {
    /// A map of indices to slots.
    pub sparse: Vec<Option<u32>>,
    /// Packed content data.
    pub dense: Vec<T>,
    /// A vec of all entities in the set.
    pub dense_entities: Vec<Entity>,
}

impl<T: Clone> Clone for SparseSet<T> {
    fn clone(&self) -> Self {
        Self {
            sparse: self.sparse.clone(),
            dense: self.dense.clone(),
            dense_entities: self.dense_entities.clone(),
        }
    }
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self {
            sparse: Vec::new(),
            dense: Vec::new(),
            dense_entities: Vec::new(),
        }
    }
}

impl<T> SparseSet<T> {
    /// Creates a new sparse set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an entity into the sparse set.
    pub fn insert(&mut self, entity: Entity, value: T) {
        let idx = entity.index as usize;
        if idx >= self.sparse.len() {
            self.sparse.resize(idx + 1, None);
        }
        if let Some(dense_idx) = self.sparse[idx] {
            self.dense[dense_idx as usize] = value;
        } else {
            let dense_idx = self.dense.len() as u32;
            self.sparse[idx] = Some(dense_idx);
            self.dense.push(value);
            self.dense_entities.push(entity);
        }
    }

    /// Returns the component for the given entity, or `None` if it isn't in this set.
    pub fn get(&self, entity: Entity) -> Option<&T> {
        let dense_idx = self.sparse.get(entity.index as usize)?.as_ref().copied()? as usize;
        if self.dense_entities[dense_idx] != entity {
            return None;
        }
        Some(&self.dense[dense_idx])
    }

    /// Returns the component for the given entity mutably, or `None` if it isn't in this set.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let dense_idx = self.sparse.get(entity.index as usize)?.as_ref().copied()? as usize;
        if self.dense_entities[dense_idx] != entity {
            return None;
        }
        Some(&mut self.dense[dense_idx])
    }

    /// Removes an entity from the sparse set if it exists.
    pub fn remove(&mut self, entity: Entity) -> Option<T> {
        let idx = entity.index as usize;
        let dense_idx = self.sparse.get(idx)?.as_ref().copied()? as usize;
        if self.dense_entities[dense_idx] != entity {
            return None;
        }
        self.sparse[idx] = None;
        let last = self.dense.len() - 1;
        if dense_idx != last {
            self.dense.swap(dense_idx, last);
            self.dense_entities.swap(dense_idx, last);
            let moved = self.dense_entities[dense_idx];
            self.sparse[moved.index as usize] = Some(dense_idx as u32);
        }
        self.dense_entities.pop();
        Some(self.dense.swap_remove(dense_idx))
    }

    /// Checks if the sparse set contains the entity.
    pub fn contains(&self, entity: Entity) -> bool {
        self.sparse
            .get(entity.index as usize)
            .and_then(|s| s.as_ref())
            .map(|&dense_idx| self.dense_entities[dense_idx as usize] == entity)
            .unwrap_or(false)
    }

    /// Generates an iterator of the sparse set.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.dense_entities.iter().copied().zip(self.dense.iter())
    }

    /// Generates a mutable iterator of the sparse set.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        self.dense_entities
            .iter()
            .copied()
            .zip(self.dense.iter_mut())
    }

    /// Returns the number of components stored in this set.
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    /// Checks if set is empty.
    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }
}
