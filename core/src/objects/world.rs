use anyhow::Result;
use hashbrown::HashMap;

use crate::{
    objects::{
        Object,
        component::Component,
        resource::{Resource, ResourceMap},
        scene::{ObjectId, Scene},
        systems::{
            DeltaTime, FixedUpdateSystem, FixedUpdateTimer, HasPriority, LateUpdateSystem,
            StartSystem, UpdateSystem,
        },
        tag::Tag,
    },
    utils::flatten::flatten,
    voxels::{VoxelTransform, chunk::Chunk, meshes::NeedsRemeshing, voxel::VoxelId},
};

#[derive(Default)]
pub struct World {
    pub(crate) scene: Scene,
    pub(crate) resources: ResourceMap,
    pub(crate) chunk_position_index: HashMap<(i32, i32, i32), ObjectId>,

    update_systems: Vec<&'static UpdateSystem>,
    fixed_update_systems: Vec<&'static FixedUpdateSystem>,
    late_update_systems: Vec<&'static LateUpdateSystem>,
}

#[allow(unused)]
impl World {
    // ========== ========== Systems ========== ==========

    /// Collects and caches all systems
    pub fn build_systems(&mut self) {
        self.update_systems = Self::collect_sorted(inventory::iter::<UpdateSystem>());
        self.fixed_update_systems = Self::collect_sorted(inventory::iter::<FixedUpdateSystem>());
        self.late_update_systems = Self::collect_sorted(inventory::iter::<LateUpdateSystem>());
        self.insert_resource(FixedUpdateTimer {
            accumulator: 0.0,
            fixed_timestep: 1.0 / 20.0,
            last_time: None,
        });
        self.insert_resource(DeltaTime(0.0));
    }

    /// Collects and sorts the Iterator
    fn collect_sorted<T: HasPriority>(iter: impl Iterator<Item = &'static T>) -> Vec<&'static T> {
        let mut systems: Vec<_> = iter.collect();
        systems.sort_by(|a, b| b.priority().cmp(&a.priority()));

        systems
    }
    /// Runs all start systems
    pub(crate) fn start(&mut self) {
        let mut systems = inventory::iter::<StartSystem>().collect::<Vec<_>>();
        systems.sort_by(|a, b| a.priority.cmp(&b.priority));
        systems.reverse();
        for system in systems.iter_mut() {
            (system.func)(self);
        }
    }

    pub(crate) fn update(&mut self) {
        // update delta time
        {
            let timer = self.get_resource_mut::<FixedUpdateTimer>().unwrap();
            let now = std::time::Instant::now();
            let delta = match timer.last_time {
                Some(last) => now.duration_since(last).as_secs_f32().min(0.25),
                None => 0.0,
            };
            timer.last_time = Some(now);
            timer.accumulator += delta;
            timer.accumulator = timer.accumulator.min(timer.fixed_timestep * 5.0);

            let dt = self.get_resource_mut::<DeltaTime>().unwrap();
            dt.0 = delta;
        }

        // run regular update systems immediately, no waiting
        let mut systems: Vec<&UpdateSystem> = inventory::iter::<UpdateSystem>().collect();
        systems.sort_by(|a, b| b.priority().cmp(&a.priority()));

        for system in systems {
            (system.func)(self).unwrap();
        }
    }

    pub(crate) fn fixed_update(&mut self) {
        loop {
            let (should_run, timestep) = {
                let timer = self.get_resource::<FixedUpdateTimer>().unwrap();
                (
                    timer.accumulator >= timer.fixed_timestep,
                    timer.fixed_timestep,
                )
            };

            if !should_run {
                break;
            }

            self.get_resource_mut::<FixedUpdateTimer>()
                .unwrap()
                .accumulator -= timestep;

            let mut systems: Vec<&FixedUpdateSystem> =
                inventory::iter::<FixedUpdateSystem>().collect();
            systems.sort_by(|a, b| b.priority().cmp(&a.priority()));

            for system in systems {
                (system.func)(self, timestep).unwrap();
            }
        }
    }

    /// Runs all late update systems
    pub(crate) fn late_update(&mut self) {
        let systems = std::mem::take(&mut self.late_update_systems);
        for system in &systems {
            (system.func)(self);
        }
        self.late_update_systems = systems;
    } // ========== ========== Objects ========== ==========

    /// Adds a new Object to the world
    pub fn add_new_object(&mut self) -> ObjectId {
        self.scene.add_new_object()
    }

    /// Adds an Object to the world
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        self.scene.add_object(object)
    }

    /// Removes an Object from the world
    pub fn remove_object(&mut self, id: ObjectId) {
        self.scene.remove_object(id);
    }

    pub fn debug_objects(&self) {
        self.scene.debug_objects();
    }

    pub fn object_count(&self) -> usize {
        self.scene.objects.len()
    }

    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        self.scene.get_object(id)
    }

    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.scene.get_object_mut(id)
    }

    pub fn get_objects_with_component_with_ids<T: Component + 'static>(
        &self,
    ) -> Vec<(ObjectId, &Object)> {
        self.scene.get_objects_with_component_with_ids::<T>()
    }

    pub fn get_objects_with_component<T: Component + 'static>(&self) -> Vec<&Object> {
        self.scene.get_objects_with_component::<T>()
    }

    pub fn get_objects_with_component_mut<T: Component + 'static>(&mut self) -> Vec<&mut Object> {
        self.scene.get_objects_with_component_mut::<T>()
    }

    pub fn get_object_with_tag<T: Tag + 'static>(&self) -> Result<&Object> {
        self.scene.get_object_with_tag::<T>()
    }

    pub fn get_object_with_tag_mut<T: Tag + 'static>(&mut self) -> Result<&mut Object> {
        self.scene.get_object_with_tag_mut::<T>()
    }

    pub fn get_objects_with_tag<T: Tag + 'static>(&self) -> Vec<&Object> {
        self.scene.get_objects_with_tag::<T>()
    }

    pub fn get_objects_with_tag_mut<T: Tag + 'static>(&mut self) -> Vec<&mut Object> {
        self.scene.get_objects_with_tag_mut::<T>()
    }
    pub fn get_objects_with_tag_with_ids<T: Tag + 'static>(&self) -> Vec<(ObjectId, &Object)> {
        self.scene.get_objects_with_tag_with_ids::<T>()
    }

    // ========== ========== Hierarchy ========== ==========

    /// Reparents an object. Pass `None` to make it a root object.
    pub fn set_parent(&mut self, child_id: ObjectId, parent_id: Option<ObjectId>) -> Result<()> {
        self.scene.set_parent(child_id, parent_id)
    }

    /// Detaches an object from its parent, making it a root object
    pub fn detach(&mut self, id: ObjectId) -> Result<()> {
        self.scene.detach_from_parent(id)
    }

    pub fn get_parent(&self, id: ObjectId) -> Option<&Object> {
        self.scene.get_parent(id)
    }

    pub fn get_parent_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.scene.get_parent_id(id)
    }

    pub fn get_children(&self, id: ObjectId) -> Vec<&Object> {
        self.scene.get_children(id)
    }

    pub fn get_children_ids(&self, id: ObjectId) -> &[ObjectId] {
        self.scene.get_children_ids(id)
    }

    pub fn get_ancestors(&self, id: ObjectId) -> Vec<ObjectId> {
        self.scene.get_ancestors(id)
    }

    pub fn get_descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        self.scene.get_descendants(id)
    }

    pub fn is_ancestor_of(&self, ancestor_id: ObjectId, descendant_id: ObjectId) -> bool {
        self.scene.is_ancestor_of(ancestor_id, descendant_id)
    }

    pub fn get_root_objects(&self) -> Vec<(ObjectId, &Object)> {
        self.scene.get_root_objects()
    }

    // ========== ========== Resources ========== ==========

    /// Insert a new resource into the map
    pub fn insert_resource<T: Resource + 'static>(&mut self, resource: T) -> &mut Self {
        self.resources.insert(resource);
        self
    }

    /// Get a resource from the map
    pub fn get_resource<T: Resource + 'static>(&self) -> Result<&T> {
        self.resources.get::<T>()
    }

    /// Get a resource mutably from the map
    pub fn get_resource_mut<T: Resource + 'static>(&mut self) -> Result<&mut T> {
        self.resources.get_mut::<T>()
    }

    /// Remove a resource from the map
    pub fn remove_resource<T: Resource + 'static>(&mut self) -> &mut Self {
        self.resources.remove::<T>();
        self
    }

    // ========== ========== Voxel Specific ========== ==========

    pub fn register_chunk(&mut self, id: ObjectId) {
        if let Some(obj) = self.scene.objects.get(id) {
            if let Ok(t) = obj.get_component::<VoxelTransform>() {
                self.chunk_position_index
                    .insert((t.position.x, t.position.y, t.position.z), id);
            }
        }
    }

    pub fn unregister_chunk(&mut self, id: ObjectId) {
        self.chunk_position_index.retain(|_, &mut oid| oid != id);
    }
    #[inline]
    pub fn get_voxel(&self, wx: i32, wy: i32, wz: i32) -> Option<VoxelId> {
        let key = (wx.div_euclid(32), wy.div_euclid(32), wz.div_euclid(32));
        let id = self.chunk_position_index.get(&key)?;
        let obj = self.scene.objects.get(*id)?;
        let chunk = obj.get_component::<Chunk>().ok()?;
        let lx = wx.rem_euclid(32) as u32;
        let ly = wy.rem_euclid(32) as u32;
        let lz = wz.rem_euclid(32) as u32;
        Some(chunk.voxels[flatten(lx, ly, lz, 32)])
    }
    #[inline]
    pub fn set_voxel(&mut self, wx: i32, wy: i32, wz: i32, id: VoxelId) -> bool {
        let key = (wx.div_euclid(32), wy.div_euclid(32), wz.div_euclid(32));
        let Some(&oid) = self.chunk_position_index.get(&key) else {
            return false;
        };
        let lx = wx.rem_euclid(32) as u32;
        let ly = wy.rem_euclid(32) as u32;
        let lz = wz.rem_euclid(32) as u32;
        let obj = self.scene.objects.get_mut(oid).unwrap();
        if let Ok(chunk) = obj.get_component_mut::<Chunk>() {
            chunk.voxels[flatten(lx, ly, lz, 32)] = id;
        }
        obj.add_tag(NeedsRemeshing);
        true
    }

    pub fn build_raw_chunk_lookup(
        &self,
    ) -> HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]> {
        self.chunk_position_index
            .iter()
            .filter_map(|(&pos, &id)| {
                let obj = self.scene.objects.get(id)?;
                let chunk = obj.get_component::<Chunk>().ok()?;
                Some((pos, chunk.voxels.as_ref() as *const _))
            })
            .collect()
    }

    /// SAFETY: chunk must still be alive (no add/remove between build and query)
    #[inline(always)]
    pub unsafe fn get_voxel_raw(
        map: &HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]>,
        wx: i32,
        wy: i32,
        wz: i32,
    ) -> VoxelId {
        let key = (wx >> 5, wy >> 5, wz >> 5);
        match map.get(&key) {
            Some(&ptr) => {
                *(*ptr).get_unchecked(((wx & 31) + (wy & 31) * 32 + (wz & 31) * 1024) as usize)
            }
            None => 0,
        }
    }
}
