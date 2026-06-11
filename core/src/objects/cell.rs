use anyhow::{Error, Result, anyhow};
use cgmath::Vector3;
use slotmap::{DefaultKey, SlotMap};

use crate::{
    log_error,
    objects::{Object, component::Component, tag::Tag},
};

/// Side length of a cell along the X and Z axes, in world units.
/// Cells are infinite along Y
pub const CELL_SIZE: i32 = 128;

/// Grid coordinate of a cell. Only X and Z are meaningful, Y is always 0.
pub type CellCoord = Vector3<i32>;

/// A stable handle to an object
/// Objects are owned by the [`Cell`] named in `cell`
/// `key` indexes into that cell's slot map
/// Note: moving an object across a cell changes it's ID as it enters a new cell map
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ObjectId {
    pub cell: CellCoord,
    pub key: DefaultKey,
}

impl Default for ObjectId {
    fn default() -> Self {
        Self {
            cell: Vector3::new(0, 0, 0),
            key: DefaultKey::default(),
        }
    }
}

/// Returns the cell coordinate that contains the given world-space position
/// Cells tile the XZ plane in [`CELL_SIZE`]-unit squares and are infinite in the Y axis
#[inline]
pub fn world_to_cell(position: Vector3<f32>) -> CellCoord {
    Vector3::new(
        (position.x.floor() as i32).div_euclid(CELL_SIZE),
        0,
        (position.z.floor() as i32).div_euclid(CELL_SIZE),
    )
}

/// A single cell of a worldspace, stores objects
pub struct Cell {
    pub coord: CellCoord,
    /// Optional name, empty means unnamed (display falls back to coord)
    pub name: String,
    pub(crate) objects: SlotMap<DefaultKey, Object>,
}

impl Cell {
    pub fn new(coord: CellCoord) -> Self {
        Self {
            coord,
            name: String::new(),
            objects: SlotMap::new(),
        }
    }

    #[inline]
    fn oid(&self, key: DefaultKey) -> ObjectId {
        ObjectId {
            cell: self.coord,
            key,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    // ========== ========== Object Management ========== ==========

    /// Adds a new default Object and returns its ID
    pub fn add_new_object(&mut self) -> ObjectId {
        self.add_object(Object::default())
    }

    /// Adds an Object as a root (no parent) and returns its ID
    pub fn add_object(&mut self, mut object: Object) -> ObjectId {
        object.parent = None;
        let key = self.objects.insert(object);
        let id = self.oid(key);
        self.objects[key].id = id;
        id
    }

    /// Inserts `child` into the cell parented under `parent_id`
    pub fn add_child_object(&mut self, parent_id: ObjectId, mut child: Object) -> Result<ObjectId> {
        if !self.objects.contains_key(parent_id.key) {
            return Err(anyhow!("Parent object does not exist"));
        }
        child.parent = Some(parent_id);
        let key = self.objects.insert(child);
        let child_id = self.oid(key);
        self.objects[key].id = child_id;
        self.objects[parent_id.key].children.push(child_id);
        Ok(child_id)
    }

    /// Removes an Object and all of its descendants from the cell
    pub fn remove_object(&mut self, id: ObjectId) {
        if !self.objects.contains_key(id.key) {
            log_error!("Object does not exist!");
            return;
        }

        // Collect the subtree
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            let current = to_remove[i];
            if let Some(obj) = self.objects.get(current.key) {
                to_remove.extend_from_slice(&obj.children.clone());
            }
            i += 1;
        }

        // Detach root of subtree from its parent
        if let Some(parent_id) = self.objects[id.key].parent {
            if let Some(parent) = self.objects.get_mut(parent_id.key) {
                parent.children.retain(|&cid| cid != id);
            }
        }

        for rid in to_remove {
            self.objects.remove(rid.key);
        }
    }

    // ========== ========== Hierarchy ========== ==========

    /// Reparents an already inserted object to a new parent, or to root if `None`
    /// Both objects must live in this cell
    pub fn set_parent(
        &mut self,
        child_id: ObjectId,
        new_parent_id: Option<ObjectId>,
    ) -> Result<()> {
        if !self.objects.contains_key(child_id.key) {
            return Err(anyhow!("Child object does not exist"));
        }

        // Detach from old parent
        if let Some(old_parent_id) = self.objects[child_id.key].parent {
            if let Some(old_parent) = self.objects.get_mut(old_parent_id.key) {
                old_parent.children.retain(|&id| id != child_id);
            }
        }

        match new_parent_id {
            Some(parent_id) => {
                if !self.objects.contains_key(parent_id.key) {
                    return Err(anyhow!("New parent object does not exist"));
                }
                if self.is_ancestor_of(child_id, parent_id) {
                    return Err(anyhow!(
                        "Cannot parent an object to one of its own descendants"
                    ));
                }
                self.objects[child_id.key].parent = Some(parent_id);
                self.objects[parent_id.key].children.push(child_id);
            }
            None => {
                self.objects[child_id.key].parent = None;
            }
        }

        Ok(())
    }

    /// Detaches an object from its parent making it a root object
    pub fn detach_from_parent(&mut self, child_id: ObjectId) -> Result<()> {
        self.set_parent(child_id, None)
    }

    /// Returns true if `ancestor_id` is a strict ancestor of `descendant_id`
    pub fn is_ancestor_of(&self, ancestor_id: ObjectId, descendant_id: ObjectId) -> bool {
        let mut current = descendant_id;
        while let Some(parent_id) = self.objects.get(current.key).and_then(|o| o.parent) {
            if parent_id == ancestor_id {
                return true;
            }
            current = parent_id;
        }
        false
    }

    /// Returns immediate children of an object
    pub fn get_children(&self, id: ObjectId) -> Vec<&Object> {
        self.objects
            .get(id.key)
            .map(|obj| {
                obj.children
                    .iter()
                    .filter_map(|&cid| self.objects.get(cid.key))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the IDs of immediate children
    pub fn get_children_ids(&self, id: ObjectId) -> &[ObjectId] {
        self.objects
            .get(id.key)
            .map(|obj| obj.children.as_slice())
            .unwrap_or(&[])
    }

    /// Returns the parent object, if any
    pub fn get_parent(&self, id: ObjectId) -> Option<&Object> {
        self.objects
            .get(id.key)?
            .parent
            .and_then(|pid| self.objects.get(pid.key))
    }

    /// Returns the parent ID, if any
    pub fn get_parent_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.objects.get(id.key)?.parent
    }

    /// Returns all ancestors from root down to the immediate parent (not including `id`)
    pub fn get_ancestors(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut chain = Vec::new();
        let mut current = id;
        while let Some(parent_id) = self.objects.get(current.key).and_then(|o| o.parent) {
            chain.push(parent_id);
            current = parent_id;
        }
        chain.reverse();
        chain
    }

    /// Returns all descendants breadth-first (not including `id` itself)
    pub fn get_descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut result = Vec::new();
        let mut queue = vec![id];
        while !queue.is_empty() {
            let current = queue.remove(0);
            if let Some(obj) = self.objects.get(current.key) {
                for &child_id in &obj.children {
                    result.push(child_id);
                    queue.push(child_id);
                }
            }
        }
        result
    }

    /// Returns all root objects (objects with no parent)
    pub fn get_root_objects(&self) -> Vec<(ObjectId, &Object)> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.parent.is_none())
            .map(|(key, obj)| (self.oid(key), obj))
            .collect()
    }

    /// Returns all objects in this cell
    pub fn get_all_objects(&self) -> Vec<(ObjectId, &Object)> {
        self.objects
            .iter()
            .map(|(key, obj)| (self.oid(key), obj))
            .collect()
    }

    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(id.key)
    }

    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(id.key)
    }

    /// Removes an object from this cell without touching its descendants and returns it
    /// Used when migrating an object to another cell
    pub(crate) fn take_object(&mut self, id: ObjectId) -> Option<Object> {
        self.objects.remove(id.key)
    }

    // ========== ========== Components ========== ==========

    pub fn get_objects_with_component<T: Component + 'static>(&self) -> Vec<&Object> {
        self.objects
            .values()
            .filter(|object| object.has_component::<T>())
            .collect()
    }

    pub fn get_objects_with_component_mut<T: Component + 'static>(&mut self) -> Vec<&mut Object> {
        self.objects
            .values_mut()
            .filter(|object| object.has_component::<T>())
            .collect()
    }

    pub fn get_objects_with_component_with_ids<T: Component + 'static>(
        &self,
    ) -> Vec<(ObjectId, &Object)> {
        self.objects
            .iter()
            .filter(|(_key, object)| object.has_component::<T>())
            .map(|(key, object)| (self.oid(key), object))
            .collect()
    }

    // ========== ========== Tags ========== ==========

    pub fn get_objects_with_tag<T: Tag + 'static>(&self) -> Vec<&Object> {
        self.objects
            .values()
            .filter(|object| object.has_tag::<T>())
            .collect()
    }

    pub fn get_objects_with_tag_mut<T: Tag + 'static>(&mut self) -> Vec<&mut Object> {
        self.objects
            .values_mut()
            .filter(|object| object.has_tag::<T>())
            .collect()
    }

    pub fn get_objects_with_tag_with_ids<T: Tag + 'static>(&self) -> Vec<(ObjectId, &Object)> {
        self.objects
            .iter()
            .filter(|(_key, object)| object.has_tag::<T>())
            .map(|(key, object)| (self.oid(key), object))
            .collect()
    }

    pub fn debug_objects(&self) {
        for (key, object) in self.objects.iter() {
            println!(
                "{}: {:?} | parent: {:?} | children: {:?}",
                object.name,
                self.oid(key),
                object.parent,
                object.children
            );
        }
    }

    // Generic helper: find the first object across this cell matching a tag type
    pub fn find_object_with_tag<T: Tag + 'static>(&self) -> Option<&Object> {
        self.objects.values().find(|object| object.has_tag::<T>())
    }

    pub fn find_object_with_tag_mut<T: Tag + 'static>(&mut self) -> Option<&mut Object> {
        self.objects
            .values_mut()
            .find(|object| object.has_tag::<T>())
    }
}

/// Error helper mirroring the old Scene tag lookups
pub fn no_tag_error<T: Tag + 'static>() -> Error {
    Error::msg(format!("No objects of type: {}", T::name()))
}
