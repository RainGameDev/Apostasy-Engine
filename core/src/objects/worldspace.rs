use anyhow::{Result, anyhow};
use cgmath::Vector3;
use hashbrown::HashMap;
use slotmap::DefaultKey;

use crate::objects::{
    Object,
    cell::{Cell, CellCoord, ObjectId, no_tag_error, world_to_cell},
    component::Component,
    components::transform::Transform,
    tag::Tag,
};

/// An effectively infinite grid of [`Cell`]s, cells are created lazily the first
/// time an object is placed in them, and may be unloaded when nothing occupies them
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

    // ========== ========== Cell access ========== ==========

    pub fn get_cell(&self, coord: CellCoord) -> Option<&Cell> {
        self.cells.get(&coord)
    }

    pub fn get_cell_mut(&mut self, coord: CellCoord) -> Option<&mut Cell> {
        self.cells.get_mut(&coord)
    }

    /// Returns the cell at `coord`, creating an empty one if it does not exist.
    pub fn get_or_create_cell(&mut self, coord: CellCoord) -> &mut Cell {
        self.cells.entry(coord).or_insert_with(|| Cell::new(coord))
    }

    pub fn loaded_cells(&self) -> impl Iterator<Item = &Cell> {
        self.cells.values()
    }

    pub fn loaded_cell_coords(&self) -> Vec<CellCoord> {
        self.cells.keys().copied().collect()
    }

    /// Drops a cell and all objects within it.
    pub fn unload_cell(&mut self, coord: CellCoord) -> Option<Cell> {
        self.cells.remove(&coord)
    }

    pub fn object_count(&self) -> usize {
        self.cells.values().map(|c| c.len()).sum()
    }

    fn drop_if_empty(&mut self, coord: CellCoord) {
        // Keep named cells around even when empty so user-assigned names survive.
        let droppable = self
            .cells
            .get(&coord)
            .map(|c| c.is_empty() && c.name.is_empty())
            .unwrap_or(false);
        if droppable {
            self.cells.remove(&coord);
        }
    }

    /// Sets (or clears, if empty) the name of a cell, creating it if needed.
    pub fn set_cell_name(&mut self, coord: CellCoord, name: impl Into<String>) {
        let name = name.into();
        // Avoid creating an empty unnamed cell when clearing a non-existent one.
        if name.is_empty() && !self.cells.contains_key(&coord) {
            return;
        }
        self.get_or_create_cell(coord).name = name;
        // Clearing the name of an empty cell makes it eligible for cleanup.
        self.drop_if_empty(coord);
    }

    // ========== ========== Object Management ========== ==========

    /// Adds a new default object to cell (0, 0, 0).
    pub fn add_new_object(&mut self) -> ObjectId {
        self.get_or_create_cell(Vector3::new(0, 0, 0))
            .add_new_object()
    }

    /// Adds a root object, placing it in the cell that contains its transform's
    /// local position (or cell (0,0,0) if it has no transform).
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        let coord = object
            .get_component::<Transform>()
            .ok()
            .map(|t| world_to_cell(t.local_position))
            .unwrap_or_else(|| Vector3::new(0, 0, 0));
        self.get_or_create_cell(coord).add_object(object)
    }

    /// Adds a child under `parent_id`, in the parent's cell.
    pub fn add_child_object(&mut self, parent_id: ObjectId, child: Object) -> Result<ObjectId> {
        let cell = self
            .cells
            .get_mut(&parent_id.cell)
            .ok_or_else(|| anyhow!("Parent's cell is not loaded"))?;
        cell.add_child_object(parent_id, child)
    }

    pub fn remove_object(&mut self, id: ObjectId) {
        if let Some(cell) = self.cells.get_mut(&id.cell) {
            cell.remove_object(id);
        }
        self.drop_if_empty(id.cell);
    }

    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        self.cells.get(&id.cell)?.get_object(id)
    }

    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.cells.get_mut(&id.cell)?.get_object_mut(id)
    }

    // ========== ========== Hierarchy ========== ==========

    /// Reparents an object, Pass `None` to make it a root. If the new parent
    /// lives in another cell, the child subtree is migrated into that cell first changeing its `ObjectId`
    pub fn set_parent(
        &mut self,
        child_id: ObjectId,
        new_parent_id: Option<ObjectId>,
    ) -> Result<()> {
        match new_parent_id {
            None => {
                let cell = self
                    .cells
                    .get_mut(&child_id.cell)
                    .ok_or_else(|| anyhow!("Child's cell is not loaded"))?;
                cell.detach_from_parent(child_id)
            }
            Some(parent_id) => {
                let child_id = if child_id.cell != parent_id.cell {
                    self.move_subtree_to_cell(child_id, parent_id.cell)
                        .ok_or_else(|| anyhow!("Failed to migrate child into parent's cell"))?
                } else {
                    child_id
                };
                let cell = self
                    .cells
                    .get_mut(&parent_id.cell)
                    .ok_or_else(|| anyhow!("Parent's cell is not loaded"))?;
                cell.set_parent(child_id, Some(parent_id))
            }
        }
    }

    pub fn detach_from_parent(&mut self, child_id: ObjectId) -> Result<()> {
        self.set_parent(child_id, None)
    }

    pub fn is_ancestor_of(&self, ancestor_id: ObjectId, descendant_id: ObjectId) -> bool {
        // Hierarchies never span cells, so a different cell means no relation.
        if ancestor_id.cell != descendant_id.cell {
            return false;
        }
        self.cells
            .get(&ancestor_id.cell)
            .map(|c| c.is_ancestor_of(ancestor_id, descendant_id))
            .unwrap_or(false)
    }

    pub fn get_children(&self, id: ObjectId) -> Vec<&Object> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_children(id))
            .unwrap_or_default()
    }

    pub fn get_children_ids(&self, id: ObjectId) -> &[ObjectId] {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_children_ids(id))
            .unwrap_or(&[])
    }

    pub fn get_parent(&self, id: ObjectId) -> Option<&Object> {
        self.cells.get(&id.cell)?.get_parent(id)
    }

    pub fn get_parent_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.cells.get(&id.cell)?.get_parent_id(id)
    }

    pub fn get_ancestors(&self, id: ObjectId) -> Vec<ObjectId> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_ancestors(id))
            .unwrap_or_default()
    }

    pub fn get_descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        self.cells
            .get(&id.cell)
            .map(|c| c.get_descendants(id))
            .unwrap_or_default()
    }

    pub fn get_root_objects(&self) -> Vec<(ObjectId, &Object)> {
        self.cells
            .values()
            .flat_map(|c| c.get_root_objects())
            .collect()
    }

    pub fn get_all_objects(&self) -> Vec<(ObjectId, &Object)> {
        self.cells
            .values()
            .flat_map(|c| c.get_all_objects())
            .collect()
    }

    // ========== ========== Migration ========== ==========

    /// Moves an object's transform into a cell coordinate from its
    /// world position, migrating it if it crossed a cell boundary. Returns the, possibly new, id.
    /// Only meaningful for root objects, children follow their parent's cell.
    pub fn rehome_by_position(&mut self, id: ObjectId, position: Vector3<f32>) -> Option<ObjectId> {
        let target = world_to_cell(position);
        if target == id.cell {
            return Some(id);
        }
        self.move_subtree_to_cell(id, target)
    }

    /// Removes the subtree rooted at `root_id` from its current cell and reinserts it into `target`
    /// remapping all internal parent/child references.
    /// The roots detached from any old parent. Returns the root's new id.
    pub fn move_subtree_to_cell(
        &mut self,
        root_id: ObjectId,
        target: CellCoord,
    ) -> Option<ObjectId> {
        if root_id.cell == target {
            return Some(root_id);
        }

        let _ = self.detach_from_parent(root_id);

        let source = root_id.cell;
        let ids = {
            let cell = self.cells.get(&source)?;
            let mut ids = vec![root_id];
            ids.extend(cell.get_descendants(root_id));
            ids
        };

        // Take the objects out of the source cell.
        let mut taken: Vec<Object> = Vec::with_capacity(ids.len());
        {
            let cell = self.cells.get_mut(&source)?;
            for id in &ids {
                if let Some(obj) = cell.take_object(*id) {
                    taken.push(obj);
                }
            }
        }
        self.drop_if_empty(source);

        // Reinsert into the target cell, building an old->new id map.
        let mut remap: HashMap<ObjectId, ObjectId> = HashMap::new();
        let mut new_keys: Vec<DefaultKey> = Vec::with_capacity(taken.len());
        {
            let dst = self.get_or_create_cell(target);
            for obj in taken {
                let old_id = obj.id;
                let key = dst.objects.insert(obj);
                let new_id = ObjectId { cell: target, key };
                dst.objects[key].id = new_id;
                remap.insert(old_id, new_id);
                new_keys.push(key);
            }
            // Fix up parent/children references to point at the new ids.
            for key in &new_keys {
                let obj = &mut dst.objects[*key];
                obj.parent = obj.parent.and_then(|p| remap.get(&p).copied());
                obj.children = obj
                    .children
                    .iter()
                    .filter_map(|c| remap.get(c).copied())
                    .collect();
            }
        }

        remap.get(&root_id).copied()
    }

    // ========== ========== Components ========== ==========

    pub fn get_objects_with_component<T: Component + 'static>(&self) -> Vec<&Object> {
        self.cells
            .values()
            .flat_map(|c| c.get_objects_with_component::<T>())
            .collect()
    }

    pub fn get_objects_with_component_mut<T: Component + 'static>(&mut self) -> Vec<&mut Object> {
        self.cells
            .values_mut()
            .flat_map(|c| c.get_objects_with_component_mut::<T>())
            .collect()
    }

    pub fn get_objects_with_component_with_ids<T: Component + 'static>(
        &self,
    ) -> Vec<(ObjectId, &Object)> {
        self.cells
            .values()
            .flat_map(|c| c.get_objects_with_component_with_ids::<T>())
            .collect()
    }

    // ========== ========== Tags ========== ==========

    pub fn get_object_with_tag<T: Tag + 'static>(&self) -> Result<&Object> {
        self.cells
            .values()
            .find_map(|c| c.find_object_with_tag::<T>())
            .ok_or_else(no_tag_error::<T>)
    }

    pub fn get_object_with_tag_mut<T: Tag + 'static>(&mut self) -> Result<&mut Object> {
        self.cells
            .values_mut()
            .find_map(|c| c.find_object_with_tag_mut::<T>())
            .ok_or_else(no_tag_error::<T>)
    }

    pub fn get_objects_with_tag<T: Tag + 'static>(&self) -> Vec<&Object> {
        self.cells
            .values()
            .flat_map(|c| c.get_objects_with_tag::<T>())
            .collect()
    }

    pub fn get_objects_with_tag_mut<T: Tag + 'static>(&mut self) -> Vec<&mut Object> {
        self.cells
            .values_mut()
            .flat_map(|c| c.get_objects_with_tag_mut::<T>())
            .collect()
    }

    pub fn get_objects_with_tag_with_ids<T: Tag + 'static>(&self) -> Vec<(ObjectId, &Object)> {
        self.cells
            .values()
            .flat_map(|c| c.get_objects_with_tag_with_ids::<T>())
            .collect()
    }

    pub fn debug_objects(&self) {
        for cell in self.cells.values() {
            cell.debug_objects();
        }
    }
}
