use anyhow::{Result, anyhow};
use slotmap::{DefaultKey, SlotMap};

use crate::{
    log_error,
    objects::{Object, component::Component, tag::Tag},
};

pub type ObjectId = DefaultKey;

pub struct Scene {
    pub(crate) objects: SlotMap<ObjectId, Object>,
}

impl Default for Scene {
    fn default() -> Self {
        let mut scene = Scene {
            objects: SlotMap::new(),
        };
        scene.add_default_objects();
        scene
    }
}

impl Scene {
    pub(crate) fn add_default_objects(&mut self) {}

    // ========== ========== Object Management ========== ==========

    /// Adds a new default Object and returns its ID
    pub fn add_new_object(&mut self) -> ObjectId {
        self.objects.insert(Object::default())
    }

    /// Adds an Object as a root (no parent) and returns its ID
    pub fn add_object(&mut self, mut object: Object) -> ObjectId {
        object.parent = None;
        self.objects.insert(object)
    }

    /// Inserts `child` into the scene parented under `parent_id`.
    /// Returns the new child's ID, or an error if the parent doesn't exist.
    pub fn add_child_object(&mut self, parent_id: ObjectId, mut child: Object) -> Result<ObjectId> {
        if !self.objects.contains_key(parent_id) {
            return Err(anyhow!("Parent object does not exist"));
        }
        child.parent = Some(parent_id);
        let child_id = self.objects.insert(child);
        self.objects[parent_id].children.push(child_id);
        Ok(child_id)
    }

    /// Removes an Object and all of its descendants from the scene.
    /// Also detaches the object from its parent's children list.
    pub fn remove_object(&mut self, id: ObjectId) {
        if !self.objects.contains_key(id) {
            log_error!("Object does not exist!");
            return;
        }

        // Collect the full subtree (breadth-first)
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            let current = to_remove[i];
            if let Some(obj) = self.objects.get(current) {
                to_remove.extend_from_slice(&obj.children.clone());
            }
            i += 1;
        }

        // Detach root of subtree from its parent
        if let Some(parent_id) = self.objects[id].parent {
            if let Some(parent) = self.objects.get_mut(parent_id) {
                parent.children.retain(|&cid| cid != id);
            }
        }

        for rid in to_remove {
            self.objects.remove(rid);
        }
    }

    // ========== ========== Hierarchy ========== ==========

    /// Reparents an already-inserted object to a new parent, or to root if `None`.
    /// Returns an error if the operation would create a cycle.
    pub fn set_parent(
        &mut self,
        child_id: ObjectId,
        new_parent_id: Option<ObjectId>,
    ) -> Result<()> {
        if !self.objects.contains_key(child_id) {
            return Err(anyhow!("Child object does not exist"));
        }

        // Detach from old parent
        if let Some(old_parent_id) = self.objects[child_id].parent {
            if let Some(old_parent) = self.objects.get_mut(old_parent_id) {
                old_parent.children.retain(|&id| id != child_id);
            }
        }

        match new_parent_id {
            Some(parent_id) => {
                if !self.objects.contains_key(parent_id) {
                    return Err(anyhow!("New parent object does not exist"));
                }
                if self.is_ancestor_of(child_id, parent_id) {
                    return Err(anyhow!(
                        "Cannot parent an object to one of its own descendants"
                    ));
                }
                self.objects[child_id].parent = Some(parent_id);
                self.objects[parent_id].children.push(child_id);
            }
            None => {
                self.objects[child_id].parent = None;
            }
        }

        Ok(())
    }

    /// Detaches an object from its parent, making it a root object.
    pub fn detach_from_parent(&mut self, child_id: ObjectId) -> Result<()> {
        self.set_parent(child_id, None)
    }

    /// Returns true if `ancestor_id` is a strict ancestor of `descendant_id`.
    pub fn is_ancestor_of(&self, ancestor_id: ObjectId, descendant_id: ObjectId) -> bool {
        let mut current = descendant_id;
        while let Some(parent_id) = self.objects.get(current).and_then(|o| o.parent) {
            if parent_id == ancestor_id {
                return true;
            }
            current = parent_id;
        }
        false
    }

    /// Returns immediate children of an object.
    pub fn get_children(&self, id: ObjectId) -> Vec<&Object> {
        self.objects
            .get(id)
            .map(|obj| {
                obj.children
                    .iter()
                    .filter_map(|&cid| self.objects.get(cid))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the IDs of immediate children.
    pub fn get_children_ids(&self, id: ObjectId) -> &[ObjectId] {
        self.objects
            .get(id)
            .map(|obj| obj.children.as_slice())
            .unwrap_or(&[])
    }

    /// Returns the parent object, if any.
    pub fn get_parent(&self, id: ObjectId) -> Option<&Object> {
        self.objects
            .get(id)?
            .parent
            .and_then(|pid| self.objects.get(pid))
    }

    /// Returns the parent ID, if any.
    pub fn get_parent_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.objects.get(id)?.parent
    }

    /// Returns all ancestors from root down to the immediate parent (not including `id`).
    pub fn get_ancestors(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut chain = Vec::new();
        let mut current = id;
        while let Some(parent_id) = self.objects.get(current).and_then(|o| o.parent) {
            chain.push(parent_id);
            current = parent_id;
        }
        chain.reverse();
        chain
    }

    /// Returns all descendants breadth-first (not including `id` itself).
    pub fn get_descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        let mut result = Vec::new();
        let mut queue = vec![id];
        while !queue.is_empty() {
            let current = queue.remove(0);
            if let Some(obj) = self.objects.get(current) {
                for &child_id in &obj.children {
                    result.push(child_id);
                    queue.push(child_id);
                }
            }
        }
        result
    }

    /// Returns all root objects (objects with no parent).
    pub fn get_root_objects(&self) -> Vec<(ObjectId, &Object)> {
        self.objects
            .iter()
            .filter(|(_, obj)| obj.parent.is_none())
            .collect()
    }

    // ========== ========== Debug ========== ==========

    pub fn debug_objects(&self) {
        for (id, object) in self.objects.iter() {
            println!(
                "{}: {:?} | parent: {:?} | children: {:?}",
                object.name, id, object.parent, object.children
            );
        }
    }

    /// Pretty-prints the full scene hierarchy as a tree.
    pub fn debug_hierarchy(&self) {
        for (id, _) in self.get_root_objects() {
            self.debug_hierarchy_node(id, 0);
        }
    }

    fn debug_hierarchy_node(&self, id: ObjectId, depth: usize) {
        if let Some(obj) = self.objects.get(id) {
            println!("{}{} ({:?})", "  ".repeat(depth), obj.name, id);
            for &child_id in &obj.children {
                self.debug_hierarchy_node(child_id, depth + 1);
            }
        }
    }

    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        if let Some(object) = self.objects.get(id) {
            return Some(object);
        }
        log_error!("Object does not exist!");
        None
    }

    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(id)
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
            .filter(|(_id, object)| object.has_component::<T>())
            .map(|(id, object)| (id, object))
            .collect()
    }

    // ========== ========== Tags ========== ==========

    pub fn get_object_with_tag<T: Tag + 'static>(&self) -> Result<&Object> {
        self.objects
            .values()
            .find(|object| object.has_tag::<T>())
            .ok_or(anyhow!("No objects with the tag"))
    }

    pub fn get_object_with_tag_mut<T: Tag + 'static>(&mut self) -> Result<&mut Object> {
        self.objects
            .values_mut()
            .find(|object| object.has_tag::<T>())
            .ok_or(anyhow!("No objects with the tag"))
    }

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
            .filter(|(_id, object)| object.has_tag::<T>())
            .collect()
    }
}
