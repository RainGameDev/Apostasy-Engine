use std::any::TypeId;

use anyhow::{Result, anyhow};
use cgmath::Vector3;
use hashbrown::HashMap;

use crate::ecs::{
    Entity,
    components::Component,
    tags::{Tag, TagRegistration},
};
use crate::worldspaces::cell::{
    Cell, CellCoord, EntityBlob, EntityId, EntitySnapshot, no_tag_error, world_to_cell,
};

/// An effectively infinite grid of [`Cell`]s.
/// Cells are created lazily when first needed and dropped when empty and unnamed.
#[derive(Default)]
pub struct Worldspace {
    pub name: String,
    pub is_interior: bool,
    pub(crate) cells: HashMap<CellCoord, Cell>,
}

impl Worldspace {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_interior: false,
            cells: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    // ========== Cell Access ==========

    pub fn get_cell(&self, coord: CellCoord) -> Option<&Cell> {
        self.cells.get(&coord)
    }

    pub fn get_cell_mut(&mut self, coord: CellCoord) -> Option<&mut Cell> {
        self.cells.get_mut(&coord)
    }

    /// Returns the cell at `coord`, creating an empty one if it doesn't exist.
    pub fn get_or_create_cell(&mut self, coord: CellCoord) -> &mut Cell {
        self.cells.entry(coord).or_insert_with(|| Cell::new(coord))
    }

    pub fn loaded_cells(&self) -> impl Iterator<Item = &Cell> {
        self.cells.values()
    }

    pub fn loaded_cell_coords(&self) -> Vec<CellCoord> {
        self.cells.keys().copied().collect()
    }

    /// Drops a cell and all entities within it.
    pub fn unload_cell(&mut self, coord: CellCoord) -> Option<Cell> {
        self.cells.remove(&coord)
    }

    pub fn entity_count(&self) -> usize {
        self.cells.values().map(|c| c.len()).sum()
    }

    /// Sets (or clears) the display name of a cell, creating it if needed.
    pub fn set_cell_name(&mut self, coord: CellCoord, name: impl Into<String>) {
        let name = name.into();
        if name.is_empty() && !self.cells.contains_key(&coord) {
            return;
        }
        self.get_or_create_cell(coord).name = name;
        self.drop_if_empty(coord);
    }

    fn drop_if_empty(&mut self, coord: CellCoord) {
        let droppable = self
            .cells
            .get(&coord)
            .map(|c| c.is_empty() && c.name.is_empty())
            .unwrap_or(false);
        if droppable {
            self.cells.remove(&coord);
        }
    }

    // ========== Entity Management ==========

    /// Spawns a new entity in cell (0, 0, 0).
    pub fn spawn(&mut self) -> EntityId {
        self.get_or_create_cell(Vector3::new(0, 0, 0)).spawn()
    }

    /// Spawns a new entity in the cell that contains `position`.
    pub fn spawn_at_position(&mut self, position: Vector3<f32>) -> EntityId {
        self.get_or_create_cell(world_to_cell(position)).spawn()
    }

    /// Spawns a new entity in a specific cell.
    pub fn spawn_in_cell(&mut self, coord: CellCoord) -> EntityId {
        self.get_or_create_cell(coord).spawn()
    }

    /// Despawns an entity and all its descendants.
    pub fn despawn(&mut self, id: EntityId) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.despawn(id);
        }
        self.drop_if_empty(id.cell);
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        self.cells
            .get(&id.cell)
            .map(|c| c.is_alive(id))
            .unwrap_or(false)
    }

    // ========== Names ==========

    pub fn set_name(&mut self, id: EntityId, name: &str) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.set_name(id, name);
        }
    }

    pub fn get_name(&self, id: EntityId) -> Option<&str> {
        self.cells.get(&id.cell)?.get_name(id)
    }

    pub fn get_name_mut(&mut self, id: EntityId) -> Option<&mut String> {
        self.cells.get_mut(&id.cell)?.get_name_mut(id)
    }

    // ========== Components ==========

    pub fn add_component<T: Component + Clone + 'static>(&mut self, id: EntityId, component: T) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.add_component(id, component);
        }
    }

    pub fn get_component<T: Component + 'static>(&self, id: EntityId) -> Option<&T> {
        self.cells.get(&id.cell)?.get_component(id)
    }

    pub fn get_component_mut<T: Component + 'static>(&mut self, id: EntityId) -> Option<&mut T> {
        self.cells.get_mut(&id.cell)?.get_component_mut(id)
    }

    pub fn remove_component<T: Component + 'static>(&mut self, id: EntityId) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.remove_component::<T>(id);
        }
    }

    pub fn has_component<T: Component + 'static>(&self, id: EntityId) -> bool {
        self.cells
            .get(&id.cell)
            .map(|c| c.has_component::<T>(id))
            .unwrap_or(false)
    }

    /// Returns all entity IDs across all cells that have component T.
    pub fn get_entities_with_component<T: Component + 'static>(&self) -> Vec<EntityId> {
        self.cells
            .values()
            .flat_map(|c| c.get_entities_with_component::<T>())
            .collect()
    }

    // ========== Tags ==========

    pub fn add_tag<T: Tag + 'static>(&mut self, id: EntityId) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.add_tag::<T>(id);
        }
    }

    pub fn remove_tag<T: Tag + 'static>(&mut self, id: EntityId) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.remove_tag::<T>(id);
        }
    }

    pub fn has_tag<T: Tag + 'static>(&self, id: EntityId) -> bool {
        self.cells
            .get(&id.cell)
            .map(|c| c.has_tag::<T>(id))
            .unwrap_or(false)
    }

    /// Returns the first entity across all cells with tag T.
    pub fn get_entity_with_tag<T: Tag + 'static>(&self) -> Result<EntityId> {
        self.cells
            .values()
            .find_map(|c| c.get_first_entity_with_tag::<T>())
            .ok_or_else(no_tag_error::<T>)
    }

    /// Returns all entities across all cells with tag T.
    pub fn get_entities_with_tag<T: Tag + 'static>(&self) -> Vec<EntityId> {
        self.cells
            .values()
            .flat_map(|c| c.get_entities_with_tag::<T>())
            .collect()
    }

    // ========== Entity Blob / Capture / Restore ==========

    /// Non-destructively captures an entity's state into an [`EntityBlob`].
    pub fn capture_entity(&self, id: EntityId) -> Option<EntityBlob> {
        self.cells.get(&id.cell)?.capture_entity(id)
    }

    /// Spawns a new entity using data from an [`EntityBlob`] and returns its ID.
    pub fn spawn_from_blob(&mut self, blob: &EntityBlob) -> EntityId {
        self.get_or_create_cell(blob.cell).spawn_from_blob(blob)
    }

    // ========== Inspector / Editor helpers ==========

    pub fn get_entity_component_type_ids(&self, id: EntityId) -> Vec<TypeId> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_entity_component_type_ids(id))
            .unwrap_or_default()
    }

    pub fn get_entity_tag_type_ids(&self, id: EntityId) -> Vec<TypeId> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_entity_tag_type_ids(id))
            .unwrap_or_default()
    }

    pub fn remove_component_by_type_id(&mut self, id: EntityId, type_id: TypeId) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.remove_component_by_type_id(id, type_id);
        }
    }

    pub fn get_component_type_name(&self, id: EntityId, type_id: TypeId) -> Option<&'static str> {
        self.cells
            .get(&id.cell)?
            .get_component_type_name(id, type_id)
    }

    pub fn with_component_any_mut(
        &mut self,
        id: EntityId,
        type_id: TypeId,
        f: impl FnOnce(&mut dyn std::any::Any),
    ) -> bool {
        self.cells
            .get_mut(&id.cell)
            .map(|c| c.with_component_any_mut(id, type_id, f))
            .unwrap_or(false)
    }

    pub fn remove_tag_by_name(&mut self, id: EntityId, name: &str) {
        if let Some(reg) = inventory::iter::<TagRegistration>()
            .find(|r| r.type_name.to_lowercase() == name.to_lowercase())
        {
            let type_id = (reg.type_id)();
            if let Some(cell) = self.cells.get_mut(&id.cell) {
                cell.remove_tag_by_type_id(id, type_id);
            }
        }
    }

    // ========== Hierarchy ==========

    /// Parents `child_id` under `parent_id`.
    /// If they live in different cells, the child's subtree is migrated into the parent's cell first.
    pub fn set_parent(
        &mut self,
        child_id: EntityId,
        new_parent_id: Option<EntityId>,
    ) -> Result<()> {
        match new_parent_id {
            None => {
                self.cells
                    .get_mut(&child_id.cell)
                    .ok_or_else(|| anyhow!("Child's cell is not loaded"))?
                    .detach_from_parent(child_id);
                Ok(())
            }
            Some(parent_id) => {
                let child_id = if child_id.cell != parent_id.cell {
                    self.move_subtree_to_cell(child_id, parent_id.cell)
                        .ok_or_else(|| anyhow!("Failed to migrate child into parent's cell"))?
                } else {
                    child_id
                };
                self.cells
                    .get_mut(&parent_id.cell)
                    .ok_or_else(|| anyhow!("Parent's cell is not loaded"))?
                    .set_parent(child_id, parent_id)
            }
        }
    }

    pub fn detach_from_parent(&mut self, id: EntityId) -> Result<()> {
        self.set_parent(id, None)
    }

    pub fn is_ancestor_of(&self, ancestor_id: EntityId, descendant_id: EntityId) -> bool {
        if ancestor_id.cell != descendant_id.cell {
            return false; // hierarchies never span cells
        }
        self.cells
            .get(&ancestor_id.cell)
            .map(|c| c.is_ancestor_of(ancestor_id, descendant_id))
            .unwrap_or(false)
    }

    pub fn get_parent_id(&self, id: EntityId) -> Option<EntityId> {
        self.cells.get(&id.cell)?.get_parent_id(id)
    }

    pub fn get_children_ids(&self, id: EntityId) -> Vec<EntityId> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_children_ids(id))
            .unwrap_or_default()
    }

    pub fn get_ancestors(&self, id: EntityId) -> Vec<EntityId> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_ancestors(id))
            .unwrap_or_default()
    }

    pub fn get_descendants(&self, id: EntityId) -> Vec<EntityId> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_descendants(id))
            .unwrap_or_default()
    }

    pub fn get_all_ids(&self) -> Vec<EntityId> {
        self.cells.values().flat_map(|c| c.get_all_ids()).collect()
    }

    pub fn get_root_ids(&self) -> Vec<EntityId> {
        self.cells.values().flat_map(|c| c.get_root_ids()).collect()
    }

    pub fn debug_entities(&self) {
        for cell in self.cells.values() {
            cell.debug_entities();
        }
    }

    // ========== Cell Migration ==========

    /// Moves an entity's subtree into the cell that contains `position`.
    /// Only meaningful for root entities — children always follow their parent's cell.
    /// Returns the root's new ID (changes when the cell changes).
    pub fn rehome_by_position(&mut self, id: EntityId, position: Vector3<f32>) -> Option<EntityId> {
        let target = world_to_cell(position);
        if target == id.cell {
            return Some(id);
        }
        self.move_subtree_to_cell(id, target)
    }

    /// Moves the subtree rooted at `root_id` into `target`, remapping all IDs.
    /// Returns the root's new ID.
    pub fn move_subtree_to_cell(
        &mut self,
        root_id: EntityId,
        target: CellCoord,
    ) -> Option<EntityId> {
        if root_id.cell == target {
            return Some(root_id);
        }

        // Detach from any parent before moving.
        let _ = self.detach_from_parent(root_id);

        let source = root_id.cell;
        let descendants = self.cells.get(&source)?.get_descendants(root_id);
        // BFS order: root first, then children — so parents always get a new ID before their children.
        let all_ids: Vec<EntityId> = std::iter::once(root_id).chain(descendants).collect();

        // Extract all entity data from the source cell.
        let mut snapshots: Vec<EntitySnapshot> = Vec::new();
        {
            let src = self.cells.get_mut(&source)?;
            for &id in &all_ids {
                if let Some(snap) = src.extract_entity(id) {
                    snapshots.push(snap);
                }
            }
        }
        self.drop_if_empty(source);

        // Allocate new IDs in the target cell and build old→new entity remap.
        let mut remap: HashMap<Entity, Entity> = HashMap::new();
        let mut new_ids: Vec<EntityId> = Vec::new();
        {
            let dst = self.get_or_create_cell(target);
            for snap in &snapshots {
                let new_id = dst.spawn_raw();
                remap.insert(snap.old_entity, new_id.entity);
                new_ids.push(new_id);
            }
        }

        // Restore all entity data with remapped references.
        let dst = self.cells.get_mut(&target)?;
        for (snap, new_id) in snapshots.into_iter().zip(new_ids.iter().copied()) {
            dst.restore_entity(new_id, snap, &remap);
        }

        // The root is always the first entry.
        new_ids.into_iter().next()
    }
}
