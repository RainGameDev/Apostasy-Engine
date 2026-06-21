use std::any::TypeId;
use std::collections::HashSet;

use anyhow::{Result, anyhow};
use cgmath::Vector3;
use hashbrown::HashMap;

use crate::ecs::{
    Entity, EntityAllocator,
    components::{Component, ComponentStorage},
    sets::SparseSet,
    tags::Tag,
};

/// Side length of a cell along the X and Z axes, in world units.
/// Cells are infinite along Y.
pub const CELL_SIZE: i32 = 128;

/// Grid coordinate of a cell. Only X and Z are meaningful, Y is always 0.
pub type CellCoord = Vector3<i32>;

/// A stable handle to an entity within a specific cell.
/// Moving an entity across a cell boundary changes its ID.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ObjectId {
    pub cell: CellCoord,
    pub entity: Entity,
}

impl Default for ObjectId {
    fn default() -> Self {
        Self {
            cell: Vector3::new(0, 0, 0),
            entity: Entity { index: 0, generation: 0 },
        }
    }
}

/// Returns the cell coordinate containing the given world-space position.
/// Cells tile the XZ plane in [`CELL_SIZE`]-unit squares and are infinite in Y.
#[inline]
pub fn world_to_cell(position: Vector3<f32>) -> CellCoord {
    Vector3::new(
        (position.x.floor() as i32).div_euclid(CELL_SIZE),
        0,
        (position.z.floor() as i32).div_euclid(CELL_SIZE),
    )
}

/// A single cell of a worldspace.
/// Each cell owns its own ECS storage: component SparseSets, tag index sets, and hierarchy maps.
pub struct Cell {
    pub coord: CellCoord,
    /// Optional display name; empty means unnamed (falls back to coord).
    pub name: String,
    entities: EntityAllocator,
    /// Tracks all alive entity indices for iteration.
    alive: HashSet<u32>,
    /// Per-component-type storage.
    components: HashMap<TypeId, Box<dyn ComponentStorage>>,
    /// Per-tag-type set of entity indices.
    tags: HashMap<TypeId, HashSet<u32>>,
    /// entity.index → parent entity
    parents: HashMap<u32, Entity>,
    /// entity.index → child entities
    children: HashMap<u32, Vec<Entity>>,
    /// entity.index → display name
    names: HashMap<u32, String>,
}

impl Cell {
    pub fn new(coord: CellCoord) -> Self {
        Self {
            coord,
            name: String::new(),
            entities: EntityAllocator::default(),
            alive: HashSet::new(),
            components: HashMap::new(),
            tags: HashMap::new(),
            parents: HashMap::new(),
            children: HashMap::new(),
            names: HashMap::new(),
        }
    }

    #[inline]
    fn oid(&self, entity: Entity) -> ObjectId {
        ObjectId { cell: self.coord, entity }
    }

    pub fn is_empty(&self) -> bool {
        self.alive.is_empty()
    }

    pub fn len(&self) -> usize {
        self.alive.len()
    }

    // ========== Entity Management ==========

    /// Spawns a new entity in this cell and returns its ID.
    pub fn spawn(&mut self) -> ObjectId {
        let entity = self.entities.spawn();
        self.alive.insert(entity.index);
        self.names.insert(entity.index, "Entity".to_string());
        self.oid(entity)
    }

    /// Spawns a raw entity with no default name. Used internally for migration.
    pub(crate) fn spawn_raw(&mut self) -> ObjectId {
        let entity = self.entities.spawn();
        self.alive.insert(entity.index);
        self.oid(entity)
    }

    /// Despawns an entity and all of its descendants.
    /// Returns false if the entity is already dead.
    pub fn despawn(&mut self, id: ObjectId) -> bool {
        if !self.entities.is_alive(id.entity) {
            return false;
        }
        // Recursively despawn children first to maintain clean hierarchy state.
        let children: Vec<Entity> = self.children
            .get(&id.entity.index)
            .cloned()
            .unwrap_or_default();
        for child in children {
            self.despawn(self.oid(child));
        }
        for storage in self.components.values_mut() {
            storage.remove(id.entity);
        }
        for tag_set in self.tags.values_mut() {
            tag_set.remove(&id.entity.index);
        }
        if let Some(parent) = self.parents.remove(&id.entity.index) {
            if let Some(siblings) = self.children.get_mut(&parent.index) {
                siblings.retain(|&e| e != id.entity);
            }
        }
        self.children.remove(&id.entity.index);
        self.names.remove(&id.entity.index);
        self.alive.remove(&id.entity.index);
        self.entities.despawn(id.entity);
        true
    }

    pub fn is_alive(&self, id: ObjectId) -> bool {
        self.entities.is_alive(id.entity)
    }

    // ========== Names ==========

    pub fn set_name(&mut self, id: ObjectId, name: &str) {
        self.names.insert(id.entity.index, name.to_string());
    }

    pub fn get_name(&self, id: ObjectId) -> Option<&str> {
        self.names.get(&id.entity.index).map(|s| s.as_str())
    }

    pub fn get_name_mut(&mut self, id: ObjectId) -> Option<&mut String> {
        self.names.get_mut(&id.entity.index)
    }

    // ========== Components ==========

    pub fn add_component<T: Component + Clone + 'static>(&mut self, id: ObjectId, component: T) {
        let storage = self.components
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(SparseSet::<T>::new()));
        storage.as_any_mut()
            .downcast_mut::<SparseSet<T>>()
            .unwrap()
            .insert(id.entity, component);
    }

    pub fn get_component<T: Component + 'static>(&self, id: ObjectId) -> Option<&T> {
        self.components.get(&TypeId::of::<T>())?
            .as_any()
            .downcast_ref::<SparseSet<T>>()?
            .get(id.entity)
    }

    pub fn get_component_mut<T: Component + 'static>(&mut self, id: ObjectId) -> Option<&mut T> {
        self.components.get_mut(&TypeId::of::<T>())?
            .as_any_mut()
            .downcast_mut::<SparseSet<T>>()?
            .get_mut(id.entity)
    }

    pub fn remove_component<T: Component + 'static>(&mut self, id: ObjectId) {
        if let Some(storage) = self.components.get_mut(&TypeId::of::<T>()) {
            storage.remove(id.entity);
        }
    }

    pub fn has_component<T: Component + 'static>(&self, id: ObjectId) -> bool {
        self.components.get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref::<SparseSet<T>>())
            .map(|s| s.contains(id.entity))
            .unwrap_or(false)
    }

    /// Returns all entity IDs in this cell that have component T.
    pub fn get_entities_with_component<T: Component + 'static>(&self) -> Vec<ObjectId> {
        self.components.get(&TypeId::of::<T>())
            .and_then(|s| s.as_any().downcast_ref::<SparseSet<T>>())
            .map(|s| s.iter().map(|(e, _)| self.oid(e)).collect())
            .unwrap_or_default()
    }

    // ========== Entity Blob / Capture / Restore ==========

    /// Captures a snapshot of this entity's data into an [`EntityBlob`] (non-destructive).
    pub fn capture_entity(&self, id: ObjectId) -> Option<EntityBlob> {
        if !self.entities.is_alive(id.entity) {
            return None;
        }
        let name = self.names.get(&id.entity.index).cloned().unwrap_or_default();
        let tags: Vec<TypeId> = self.tags.iter()
            .filter(|(_, set)| set.contains(&id.entity.index))
            .map(|(&type_id, _)| type_id)
            .collect();
        let components: Vec<(TypeId, Box<dyn ComponentStorage>)> = self.components.iter()
            .filter(|(_, s)| s.contains_entity(id.entity))
            .map(|(&type_id, storage)| {
                let mut single = storage.make_empty();
                storage.clone_entity_into(id.entity, id.entity, &mut *single);
                (type_id, single)
            })
            .collect();
        Some(EntityBlob { name, cell: self.coord, tags, components, source_entity: id.entity })
    }

    /// Spawns a new entity from an [`EntityBlob`] and returns its ID.
    pub fn spawn_from_blob(&mut self, blob: &EntityBlob) -> ObjectId {
        let id = self.spawn();
        self.names.insert(id.entity.index, blob.name.clone());
        for &type_id in &blob.tags {
            self.tags.entry(type_id).or_default().insert(id.entity.index);
        }
        for (type_id, single) in &blob.components {
            let dst = self.components
                .entry(*type_id)
                .or_insert_with(|| single.make_empty());
            single.clone_entity_into(blob.source_entity, id.entity, &mut **dst);
        }
        id
    }

    // ========== Inspector / Editor helpers ==========

    /// Returns all component TypeIds present on the given entity.
    pub fn get_entity_component_type_ids(&self, id: ObjectId) -> Vec<TypeId> {
        self.components.iter()
            .filter(|(_, s)| s.contains_entity(id.entity))
            .map(|(&type_id, _)| type_id)
            .collect()
    }

    /// Returns the type name of a component on this entity by its TypeId.
    pub fn get_component_type_name(&self, id: ObjectId, type_id: TypeId) -> Option<&'static str> {
        let storage = self.components.get(&type_id)?;
        if storage.contains_entity(id.entity) {
            Some(storage.component_type_name())
        } else {
            None
        }
    }

    /// Calls `f` with a mutable reference to the component identified by `type_id` on this entity.
    pub fn with_component_any_mut(
        &mut self,
        id: ObjectId,
        type_id: TypeId,
        f: impl FnOnce(&mut dyn std::any::Any),
    ) -> bool {
        if let Some(storage) = self.components.get_mut(&type_id) {
            if let Some(any) = storage.get_entity_any_mut(id.entity) {
                f(any);
                return true;
            }
        }
        false
    }

    /// Calls `f` with an immutable reference to the component identified by `type_id` on this entity.
    pub fn with_component_any(
        &self,
        id: ObjectId,
        type_id: TypeId,
        f: impl FnOnce(&dyn std::any::Any),
    ) -> bool {
        if let Some(storage) = self.components.get(&type_id) {
            if let Some(any) = storage.get_entity_any(id.entity) {
                f(any);
                return true;
            }
        }
        false
    }

    /// Returns all tag TypeIds present on the given entity.
    pub fn get_entity_tag_type_ids(&self, id: ObjectId) -> Vec<TypeId> {
        self.tags.iter()
            .filter(|(_, set)| set.contains(&id.entity.index))
            .map(|(&type_id, _)| type_id)
            .collect()
    }

    /// Removes a component identified by its TypeId.
    pub fn remove_component_by_type_id(&mut self, id: ObjectId, type_id: TypeId) {
        if let Some(storage) = self.components.get_mut(&type_id) {
            storage.remove(id.entity);
        }
    }

    /// Removes a tag by TypeId.
    pub fn remove_tag_by_type_id(&mut self, id: ObjectId, type_id: TypeId) {
        if let Some(set) = self.tags.get_mut(&type_id) {
            set.remove(&id.entity.index);
        }
    }

    // ========== Tags ==========

    pub fn add_tag<T: Tag + 'static>(&mut self, id: ObjectId) {
        self.tags.entry(TypeId::of::<T>()).or_default().insert(id.entity.index);
    }

    /// Adds a tag by TypeId. Used during cell migration where the concrete type is erased.
    pub(crate) fn add_tag_by_type_id(&mut self, id: ObjectId, type_id: TypeId) {
        self.tags.entry(type_id).or_default().insert(id.entity.index);
    }

    pub fn remove_tag<T: Tag + 'static>(&mut self, id: ObjectId) {
        if let Some(set) = self.tags.get_mut(&TypeId::of::<T>()) {
            set.remove(&id.entity.index);
        }
    }

    pub fn has_tag<T: Tag + 'static>(&self, id: ObjectId) -> bool {
        self.tags.get(&TypeId::of::<T>())
            .map(|s| s.contains(&id.entity.index))
            .unwrap_or(false)
    }

    /// Returns all entity IDs in this cell that have tag T.
    pub fn get_entities_with_tag<T: Tag + 'static>(&self) -> Vec<ObjectId> {
        self.tags.get(&TypeId::of::<T>())
            .map(|set| {
                set.iter()
                    .filter_map(|&idx| {
                        let generation = self.entities.current_generation(idx)?;
                        Some(self.oid(Entity { index: idx, generation }))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the first entity in this cell with tag T, if any.
    pub fn get_first_entity_with_tag<T: Tag + 'static>(&self) -> Option<ObjectId> {
        self.tags.get(&TypeId::of::<T>())?.iter().find_map(|&idx| {
            let generation = self.entities.current_generation(idx)?;
            Some(self.oid(Entity { index: idx, generation }))
        })
    }

    // ========== Hierarchy ==========

    /// Parents `child_id` under `parent_id`. Both must live in this cell.
    pub fn set_parent(&mut self, child_id: ObjectId, parent_id: ObjectId) -> Result<()> {
        if !self.entities.is_alive(child_id.entity) {
            return Err(anyhow!("Child entity does not exist"));
        }
        if !self.entities.is_alive(parent_id.entity) {
            return Err(anyhow!("Parent entity does not exist"));
        }
        if self.is_ancestor_of(child_id, parent_id) {
            return Err(anyhow!("Cannot parent an entity to one of its own descendants"));
        }
        // Detach from old parent.
        if let Some(old_parent) = self.parents.get(&child_id.entity.index).copied() {
            if let Some(siblings) = self.children.get_mut(&old_parent.index) {
                siblings.retain(|&e| e != child_id.entity);
            }
        }
        self.parents.insert(child_id.entity.index, parent_id.entity);
        self.children.entry(parent_id.entity.index).or_default().push(child_id.entity);
        Ok(())
    }

    /// Removes `id` from its parent, making it a root entity.
    pub fn detach_from_parent(&mut self, id: ObjectId) {
        if let Some(parent) = self.parents.remove(&id.entity.index) {
            if let Some(siblings) = self.children.get_mut(&parent.index) {
                siblings.retain(|&e| e != id.entity);
            }
        }
    }

    pub fn is_ancestor_of(&self, ancestor_id: ObjectId, descendant_id: ObjectId) -> bool {
        let mut current = descendant_id.entity.index;
        while let Some(&parent) = self.parents.get(&current) {
            if parent == ancestor_id.entity {
                return true;
            }
            current = parent.index;
        }
        false
    }

    pub fn get_parent_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.parents.get(&id.entity.index).map(|&e| self.oid(e))
    }

    pub fn get_children_ids(&self, id: ObjectId) -> Vec<ObjectId> {
        self.children.get(&id.entity.index)
            .map(|v| v.iter().map(|&e| self.oid(e)).collect())
            .unwrap_or_default()
    }

    pub fn get_ancestors(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut chain = Vec::new();
        let mut current = id.entity.index;
        while let Some(&parent) = self.parents.get(&current) {
            chain.push(self.oid(parent));
            current = parent.index;
        }
        chain.reverse();
        chain
    }

    /// Returns all descendants in BFS order (not including `id` itself).
    pub fn get_descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut result = Vec::new();
        let mut queue = vec![id.entity.index];
        let mut head = 0;
        while head < queue.len() {
            let idx = queue[head];
            head += 1;
            if let Some(children) = self.children.get(&idx) {
                for &child in children {
                    result.push(self.oid(child));
                    queue.push(child.index);
                }
            }
        }
        result
    }

    /// Returns all root entities (entities with no parent) in this cell.
    pub fn get_root_ids(&self) -> Vec<ObjectId> {
        self.alive.iter()
            .filter(|&&idx| !self.parents.contains_key(&idx))
            .filter_map(|&idx| {
                let generation = self.entities.current_generation(idx)?;
                Some(self.oid(Entity { index: idx, generation }))
            })
            .collect()
    }

    /// Returns all entity IDs in this cell.
    pub fn get_all_ids(&self) -> Vec<ObjectId> {
        self.alive.iter()
            .filter_map(|&idx| {
                let generation = self.entities.current_generation(idx)?;
                Some(self.oid(Entity { index: idx, generation }))
            })
            .collect()
    }

    pub fn debug_entities(&self) {
        for &idx in &self.alive {
            if let Some(generation) = self.entities.current_generation(idx) {
                let id = self.oid(Entity { index: idx, generation });
                println!(
                    "{}: {:?} | parent: {:?} | children: {:?}",
                    self.get_name(id).unwrap_or("unnamed"),
                    id,
                    self.get_parent_id(id),
                    self.get_children_ids(id),
                );
            }
        }
    }

    // ========== Migration ==========

    /// Extracts an entity's data out of this cell without touching its descendants.
    /// Used by [`crate::worldspaces::worldspace::Worldspace::move_subtree_to_cell`].
    pub(crate) fn extract_entity(&mut self, id: ObjectId) -> Option<EntitySnapshot> {
        if !self.entities.is_alive(id.entity) {
            return None;
        }
        let name = self.names.remove(&id.entity.index).unwrap_or_default();
        let old_parent = self.parents.remove(&id.entity.index);
        // Remove this entity from its parent's child list.
        if let Some(parent) = old_parent {
            if let Some(siblings) = self.children.get_mut(&parent.index) {
                siblings.retain(|&e| e != id.entity);
            }
        }
        let old_children = self.children.remove(&id.entity.index).unwrap_or_default();

        let mut tags: Vec<TypeId> = Vec::new();
        for (type_id, set) in &mut self.tags {
            if set.remove(&id.entity.index) {
                tags.push(*type_id);
            }
        }

        // Copy component data into single-entity storages for transport.
        let mut components: Vec<(TypeId, Box<dyn ComponentStorage>)> = Vec::new();
        for (type_id, storage) in &mut self.components {
            if storage.contains_entity(id.entity) {
                let mut single = storage.make_empty();
                storage.clone_entity_into(id.entity, id.entity, &mut *single);
                storage.remove(id.entity);
                components.push((*type_id, single));
            }
        }

        self.alive.remove(&id.entity.index);
        self.entities.despawn(id.entity);

        Some(EntitySnapshot {
            old_entity: id.entity,
            name,
            old_parent,
            old_children,
            tags,
            components,
        })
    }

    /// Inserts a snapshot into this cell at the given pre-spawned `id`,
    /// remapping parent/child references using `remap`.
    pub(crate) fn restore_entity(
        &mut self,
        id: ObjectId,
        snapshot: EntitySnapshot,
        remap: &HashMap<Entity, Entity>,
    ) {
        self.names.insert(id.entity.index, snapshot.name);

        for type_id in snapshot.tags {
            self.tags.entry(type_id).or_default().insert(id.entity.index);
        }

        for (type_id, single) in snapshot.components {
            let dst = self.components
                .entry(type_id)
                .or_insert_with(|| single.make_empty());
            single.clone_entity_into(snapshot.old_entity, id.entity, &mut **dst);
        }

        // Re-attach to parent if it was migrated too.
        if let Some(old_parent) = snapshot.old_parent {
            if let Some(&new_parent) = remap.get(&old_parent) {
                self.parents.insert(id.entity.index, new_parent);
                self.children.entry(new_parent.index).or_default().push(id.entity);
            }
            // If the parent wasn't in the migrated set it means this was the root,
            // which the caller detaches before migration.
        }
    }
}

/// All data belonging to a single entity, extracted for cross-cell migration.
pub(crate) struct EntitySnapshot {
    pub old_entity: Entity,
    pub name: String,
    pub old_parent: Option<Entity>,
    pub old_children: Vec<Entity>,
    pub tags: Vec<TypeId>,
    pub components: Vec<(TypeId, Box<dyn ComponentStorage>)>,
}

/// A public, cloneable snapshot of an entity's data. Used by editor undo/redo and copy-paste.
#[derive(Clone)]
pub struct EntityBlob {
    pub name: String,
    pub cell: CellCoord,
    pub tags: Vec<TypeId>,
    /// Each element: (TypeId, single-entity storage containing the component under `source_entity`)
    pub(crate) components: Vec<(TypeId, Box<dyn ComponentStorage>)>,
    /// The entity index that owns each single-entity storage inside `components`.
    pub(crate) source_entity: Entity,
}

impl EntityBlob {
    pub fn cell(&self) -> CellCoord {
        self.cell
    }
}

impl std::fmt::Debug for EntityBlob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityBlob")
            .field("name", &self.name)
            .field("cell", &self.cell)
            .finish()
    }
}

/// Error helper for tag lookups that found nothing.
pub fn no_tag_error<T: Tag + 'static>() -> anyhow::Error {
    anyhow::Error::msg(format!("No entity with tag: {}", T::name()))
}
