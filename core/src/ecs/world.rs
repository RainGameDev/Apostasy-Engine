use std::any::{Any, TypeId};
use std::cmp::Reverse;

use anyhow::Result;
use cgmath::Vector3;
use hashbrown::{HashMap, HashSet};

use crate::EngineMode;
use crate::ecs::{
    components::{BoxedComponent, Component, ComponentRegistration},
    entity_builder::EntityBuilder,
    query::{QueryBuilder, QueryFetch},
    resources::{Resource, ResourceMap},
    systems::{
        DeltaTime, EngineTimer, FixedUpdateSystem, FixedUpdateTimer, HasMode, HasPriority,
        LateUpdateSystem, PreRenderSystem, StartSystem, UpdateSystem,
    },
    tags::{Tag, TagRegistration},
};
use crate::utils::flatten::flatten;
use crate::voxels::{VoxelTransform, chunk::Chunk, meshes::NeedsRemeshing, voxel::VoxelId};
use crate::worldspaces::{
    cell::{CellCoord, EntityBlob, EntityId},
    worldspace::Worldspace,
};

#[derive(Default)]
pub struct World {
    pub(crate) worldspace: Worldspace,
    pub(crate) resources: ResourceMap,
    pub(crate) chunk_position_index: HashMap<(i32, i32, i32), EntityId>,
    tag_index: HashMap<TypeId, HashSet<EntityId>>,

    start_systems: Vec<StartSystem>,
    update_systems: Vec<UpdateSystem>,
    fixed_update_systems: Vec<FixedUpdateSystem>,
    late_update_systems: Vec<LateUpdateSystem>,
    prerender_systems: Vec<PreRenderSystem>,
}

#[allow(unused)]
impl World {
    // ========== Systems ==========

    /// Collects and sorts all registered systems by mode and priority.
    pub fn build_systems(&mut self) {
        let engine_mode = self
            .get_resource::<EngineMode>()
            .ok()
            .copied()
            .unwrap_or(EngineMode::Game);

        self.start_systems = Self::collect_sorted(inventory::iter::<StartSystem>(), engine_mode);
        self.update_systems = Self::collect_sorted(inventory::iter::<UpdateSystem>(), engine_mode);
        self.fixed_update_systems =
            Self::collect_sorted(inventory::iter::<FixedUpdateSystem>(), engine_mode);
        self.late_update_systems =
            Self::collect_sorted(inventory::iter::<LateUpdateSystem>(), engine_mode);
        self.prerender_systems =
            Self::collect_sorted(inventory::iter::<PreRenderSystem>(), engine_mode);
        self.insert_resource(FixedUpdateTimer {
            accumulator: 0.0,
            fixed_timestep: 1.0 / 20.0,
            last_time: None,
        });
        self.insert_resource(DeltaTime(0.0));
    }

    fn collect_sorted<T: HasPriority + HasMode + Copy + 'static>(
        iter: impl Iterator<Item = &'static T>,
        engine_mode: EngineMode,
    ) -> Vec<T> {
        let mut systems: Vec<T> = iter
            .filter(|s| s.mode().matches(engine_mode))
            .copied()
            .collect();
        systems.sort_by_key(|s| Reverse(s.priority()));
        systems
    }

    pub fn register_update_system(
        &mut self,
        func: fn(&mut World) -> Result<()>,
        priority: u32,
    ) -> &mut Self {
        self.update_systems.push(UpdateSystem {
            name: "",
            func,
            priority,
            mode: EngineMode::All,
        });
        self.update_systems.sort_by_key(|s| Reverse(s.priority));
        self
    }

    pub fn register_fixed_update_system(
        &mut self,
        func: fn(&mut World, f32) -> Result<()>,
        priority: u32,
    ) -> &mut Self {
        self.fixed_update_systems.push(FixedUpdateSystem {
            name: "",
            func,
            priority,
            mode: EngineMode::Game,
        });
        self.fixed_update_systems
            .sort_by_key(|s| Reverse(s.priority));
        self
    }

    pub fn register_late_update_system(
        &mut self,
        func: fn(&mut World) -> Result<()>,
        priority: u32,
    ) -> &mut Self {
        self.late_update_systems.push(LateUpdateSystem {
            name: "",
            func,
            priority,
            mode: EngineMode::Game,
        });
        self.late_update_systems
            .sort_by_key(|s| Reverse(s.priority));
        self
    }

    pub fn register_prerender_system(
        &mut self,
        func: fn(&mut World) -> Result<()>,
        priority: u32,
    ) -> &mut Self {
        self.prerender_systems.push(PreRenderSystem {
            name: "",
            func,
            priority,
            mode: EngineMode::Game,
        });
        self.prerender_systems.sort_by_key(|s| Reverse(s.priority));
        self
    }

    pub fn register_start_system(
        &mut self,
        func: fn(&mut World) -> Result<()>,
        priority: u32,
    ) -> &mut Self {
        self.start_systems.push(StartSystem {
            name: "",
            func,
            priority,
            mode: EngineMode::Game,
        });
        self.start_systems.sort_by_key(|s| Reverse(s.priority));
        self
    }

    pub(crate) fn start(&mut self) {
        let systems = std::mem::take(&mut self.start_systems);
        for system in &systems {
            (system.func)(self).unwrap();
        }
    }

    pub(crate) fn prerender(&mut self) -> Vec<(&'static str, u64)> {
        let systems = std::mem::take(&mut self.prerender_systems);
        let mut timings = Vec::with_capacity(systems.len());
        for system in &systems {
            let start = std::time::Instant::now();
            (system.func)(self).unwrap();
            timings.push((system.name, start.elapsed().as_nanos() as u64));
        }
        self.prerender_systems = systems;
        timings
    }

    pub(crate) fn update(&mut self) -> Vec<(&'static str, u64)> {
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
            let engine_timer = self.get_resource_mut::<EngineTimer>().unwrap();
            engine_timer.0 += delta;
            let dt = self.get_resource_mut::<DeltaTime>().unwrap();
            dt.0 = delta;
        }
        let systems = std::mem::take(&mut self.update_systems);
        let mut timings = Vec::with_capacity(systems.len());
        for system in &systems {
            let start = std::time::Instant::now();
            (system.func)(self).unwrap();
            timings.push((system.name, start.elapsed().as_nanos() as u64));
        }
        self.update_systems = systems;
        timings
    }

    pub(crate) fn fixed_update(&mut self) -> Vec<(&'static str, u64)> {
        let mut timings = Vec::new();
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
            let systems = std::mem::take(&mut self.fixed_update_systems);
            for system in &systems {
                let start = std::time::Instant::now();
                (system.func)(self, timestep).unwrap();
                let elapsed = start.elapsed().as_nanos() as u64;
                if let Some((_, existing)) = timings.iter_mut().find(|(n, _)| *n == system.name) {
                    *existing += elapsed;
                } else {
                    timings.push((system.name, elapsed));
                }
            }
            self.fixed_update_systems = systems;
        }
        timings
    }

    pub(crate) fn late_update(&mut self) -> Vec<(&'static str, u64)> {
        let systems = std::mem::take(&mut self.late_update_systems);
        let mut timings = Vec::with_capacity(systems.len());
        for system in &systems {
            let start = std::time::Instant::now();
            (system.func)(self).unwrap();
            timings.push((system.name, start.elapsed().as_nanos() as u64));
        }
        self.late_update_systems = systems;
        timings
    }

    // ========== Entity Lifecycle ==========

    /// Spawns a new entity in cell (0, 0, 0).
    ///
    /// Returns an [`EntityBuilder`] for fluent setup; call `.id()` to obtain the
    /// [`EntityId`]:
    ///
    /// ```ignore
    /// let id = world.spawn()
    ///     .add_component(Transform::default())
    ///     .add_tag::<Player>()
    ///     .with_name("Player")
    ///     .id();
    /// ```
    pub fn spawn(&mut self) -> EntityBuilder<'_> {
        let id = self.worldspace.spawn();
        EntityBuilder::new(self, id)
    }

    /// Spawns a new entity in the cell containing `position`.
    pub fn spawn_at_position(&mut self, position: Vector3<f32>) -> EntityBuilder<'_> {
        let id = self.worldspace.spawn_at_position(position);
        EntityBuilder::new(self, id)
    }

    /// Spawns a new entity in a specific cell.
    pub fn spawn_in_cell(&mut self, coord: CellCoord) -> EntityBuilder<'_> {
        let id = self.worldspace.spawn_in_cell(coord);
        EntityBuilder::new(self, id)
    }

    /// Despawns an entity and all its descendants.
    pub fn despawn(&mut self, id: EntityId) {
        let mut to_remove: HashSet<EntityId> = HashSet::new();
        to_remove.insert(id);
        for desc in self.worldspace.get_descendants(id) {
            to_remove.insert(desc);
        }
        for set in self.tag_index.values_mut() {
            set.retain(|oid| !to_remove.contains(oid));
        }
        self.worldspace.despawn(id);
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        self.worldspace.is_alive(id)
    }

    // ========== Names ==========

    pub fn set_name(&mut self, id: EntityId, name: &str) {
        self.worldspace.set_name(id, name);
    }

    pub fn get_name(&self, id: EntityId) -> Option<&str> {
        self.worldspace.get_name(id)
    }

    // ========== Components ==========

    pub fn add_component<T: Component + Clone + 'static>(&mut self, id: EntityId, component: T) {
        self.worldspace.add_component(id, component);
    }

    pub fn get_component<T: Component + 'static>(&self, id: EntityId) -> Option<&T> {
        self.worldspace.get_component(id)
    }

    pub fn get_component_mut<T: Component + 'static>(&mut self, id: EntityId) -> Option<&mut T> {
        self.worldspace.get_component_mut(id)
    }

    pub fn remove_component<T: Component + 'static>(&mut self, id: EntityId) {
        self.worldspace.remove_component::<T>(id);
    }

    pub fn has_component<T: Component + 'static>(&self, id: EntityId) -> bool {
        self.worldspace.has_component::<T>(id)
    }

    /// Returns all entity IDs across all cells that have component T.
    pub fn get_entities_with_component<T: Component + 'static>(&self) -> Vec<EntityId> {
        self.worldspace.get_entities_with_component::<T>()
    }

    // ========== Tags ==========

    pub fn add_tag<T: Tag + 'static>(&mut self, id: EntityId) {
        self.worldspace.add_tag::<T>(id);
        self.tag_index
            .entry(TypeId::of::<T>())
            .or_default()
            .insert(id);
    }

    pub fn remove_tag<T: Tag + 'static>(&mut self, id: EntityId) {
        self.worldspace.remove_tag::<T>(id);
        if let Some(set) = self.tag_index.get_mut(&TypeId::of::<T>()) {
            set.remove(&id);
        }
    }

    pub fn has_tag<T: Tag + 'static>(&self, id: EntityId) -> bool {
        self.worldspace.has_tag::<T>(id)
    }

    /// Returns the first entity across all cells with tag T. O(1).
    pub fn get_entity_with_tag<T: Tag + 'static>(&self) -> Result<EntityId> {
        self.tag_index
            .get(&TypeId::of::<T>())
            .and_then(|set| set.iter().next().copied())
            .ok_or_else(crate::worldspaces::cell::no_tag_error::<T>)
    }

    /// Returns all entities across all cells with tag T. O(n) where n = matching entities.
    pub fn get_entities_with_tag<T: Tag + 'static>(&self) -> Vec<EntityId> {
        self.tag_index
            .get(&TypeId::of::<T>())
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Rebuilds the tag index from scratch by scanning all loaded cells.
    /// Call this after bulk worldspace operations (scene load, cell unload) that bypass
    /// the typed `add_tag`/`remove_tag` methods.
    pub fn rebuild_tag_index(&mut self) {
        self.tag_index.clear();
        for cell in self.worldspace.cells.values() {
            for (type_id, id) in cell.iter_tagged_entities() {
                self.tag_index.entry(type_id).or_default().insert(id);
            }
        }
    }

    // ========== Hierarchy ==========

    pub fn set_parent(&mut self, child_id: EntityId, parent_id: Option<EntityId>) -> Result<()> {
        let cross_cell = parent_id.map_or(false, |p| p.cell != child_id.cell);
        self.worldspace.set_parent(child_id, parent_id)?;
        if cross_cell {
            self.rebuild_tag_index();
        }
        Ok(())
    }

    pub fn detach(&mut self, id: EntityId) -> Result<()> {
        self.worldspace.detach_from_parent(id)
    }

    pub fn get_parent_id(&self, id: EntityId) -> Option<EntityId> {
        self.worldspace.get_parent_id(id)
    }

    pub fn get_children_ids(&self, id: EntityId) -> Vec<EntityId> {
        self.worldspace.get_children_ids(id)
    }

    pub fn get_ancestors(&self, id: EntityId) -> Vec<EntityId> {
        self.worldspace.get_ancestors(id)
    }

    pub fn get_descendants(&self, id: EntityId) -> Vec<EntityId> {
        self.worldspace.get_descendants(id)
    }

    pub fn is_ancestor_of(&self, ancestor_id: EntityId, descendant_id: EntityId) -> bool {
        self.worldspace.is_ancestor_of(ancestor_id, descendant_id)
    }

    pub fn get_all_ids(&self) -> Vec<EntityId> {
        self.worldspace.get_all_ids()
    }

    pub fn get_root_ids(&self) -> Vec<EntityId> {
        self.worldspace.get_root_ids()
    }

    pub fn entity_count(&self) -> usize {
        self.worldspace.entity_count()
    }

    pub fn replace_worldspace(&mut self, ws: Worldspace) {
        self.worldspace = ws;
        self.rebuild_tag_index();
    }

    pub fn worldspace(&self) -> &Worldspace {
        &self.worldspace
    }

    pub fn worldspace_mut(&mut self) -> &mut Worldspace {
        &mut self.worldspace
    }

    pub fn debug_entities(&self) {
        self.worldspace.debug_entities();
    }

    // ========== Resources ==========

    // ========== Entity Blob / Capture / Restore ==========

    /// Non-destructively captures an entity's full state into an [`EntityBlob`].
    pub fn capture_entity(&self, id: EntityId) -> Option<EntityBlob> {
        self.worldspace.capture_entity(id)
    }

    /// Spawns a new entity from an [`EntityBlob`] (in the blob's original cell) and returns its ID.
    pub fn spawn_from_blob(&mut self, blob: &EntityBlob) -> EntityId {
        let id = self.worldspace.spawn_from_blob(blob);
        for &type_id in &blob.tags {
            self.tag_index.entry(type_id).or_default().insert(id);
        }
        id
    }

    /// Spawns a new entity from an [`EntityBlob`] into a specific cell and returns its ID.
    pub fn spawn_from_blob_in_cell(&mut self, blob: &EntityBlob, coord: CellCoord) -> EntityId {
        let mut blob2 = blob.clone();
        blob2.cell = coord;
        let id = self.worldspace.spawn_from_blob(&blob2);
        for &type_id in &blob2.tags {
            self.tag_index.entry(type_id).or_default().insert(id);
        }
        id
    }

    // ========== Inspector / Editor helpers ==========

    /// Returns the entity name as a mutable reference for inline editing.
    pub fn get_name_mut(&mut self, id: EntityId) -> Option<&mut String> {
        self.worldspace.get_name_mut(id)
    }

    /// Returns all component TypeIds present on the given entity.
    pub fn get_entity_component_type_ids(&self, id: EntityId) -> Vec<TypeId> {
        self.worldspace.get_entity_component_type_ids(id)
    }

    /// Returns all tag TypeIds present on the given entity.
    pub fn get_entity_tag_type_ids(&self, id: EntityId) -> Vec<TypeId> {
        self.worldspace.get_entity_tag_type_ids(id)
    }

    /// Removes a component identified by TypeId from the given entity.
    pub fn remove_component_by_type_id(&mut self, id: EntityId, type_id: TypeId) {
        self.worldspace.remove_component_by_type_id(id, type_id);
    }

    /// Returns the type name string for a component TypeId on the given entity.
    pub fn get_component_type_name(&self, id: EntityId, type_id: TypeId) -> Option<&'static str> {
        self.worldspace.get_component_type_name(id, type_id)
    }

    /// Calls `f` with mutable `dyn Any` access to a component identified by TypeId.
    pub fn with_component_any_mut(
        &mut self,
        id: EntityId,
        type_id: TypeId,
        f: impl FnOnce(&mut dyn Any),
    ) -> bool {
        self.worldspace.with_component_any_mut(id, type_id, f)
    }

    /// Adds a boxed component to the given entity using the registration's `add_to_world` fn.
    pub fn add_boxed_component(&mut self, id: EntityId, comp: BoxedComponent) {
        let type_name = comp.type_name();
        if let Some(reg) =
            inventory::iter::<ComponentRegistration>().find(|r| r.type_name == type_name)
        {
            (reg.add_to_world)(self, id, comp);
        }
    }

    /// Adds a component to the given entity by its registered type name.
    pub fn add_component_by_name(&mut self, id: EntityId, name: &str) -> Result<()> {
        let reg = inventory::iter::<ComponentRegistration>()
            .find(|r| r.type_name.to_lowercase() == name.to_lowercase())
            .ok_or_else(|| anyhow::anyhow!("No component registered as '{}'", name))?;
        let comp = (reg.create)();
        (reg.add_to_world)(self, id, comp);
        Ok(())
    }

    /// Adds a tag to the given entity by its registered type name.
    pub fn add_tag_by_name(&mut self, id: EntityId, name: &str) -> Result<()> {
        let reg = inventory::iter::<TagRegistration>()
            .find(|r| r.type_name.to_lowercase() == name.to_lowercase())
            .ok_or_else(|| anyhow::anyhow!("No tag registered as '{}'", name))?;
        (reg.add_to_world)(self, id);
        Ok(())
    }

    /// Removes a tag from the given entity by its registered type name.
    pub fn remove_tag_by_name(&mut self, id: EntityId, name: &str) {
        if let Some(reg) = inventory::iter::<TagRegistration>()
            .find(|r| r.type_name.to_lowercase() == name.to_lowercase())
        {
            let type_id = (reg.type_id)();
            if let Some(set) = self.tag_index.get_mut(&type_id) {
                set.remove(&id);
            }
        }
        self.worldspace.remove_tag_by_name(id, name);
    }

    /// Returns true if the entity has the tag identified by its registered type name.
    pub fn has_tag_by_name(&self, id: EntityId, name: &str) -> bool {
        if let Some(reg) = inventory::iter::<TagRegistration>()
            .find(|r| r.type_name.to_lowercase() == name.to_lowercase())
        {
            let type_id = (reg.type_id)();
            if let Some(set) = self.tag_index.get(&type_id) {
                return set.contains(&id);
            }
        }
        false
    }

    /// Returns all entities carrying the tag identified by its registered type name.
    pub fn get_entities_with_tag_by_name(&self, name: &str) -> Vec<EntityId> {
        if let Some(reg) = inventory::iter::<TagRegistration>()
            .find(|r| r.type_name.to_lowercase() == name.to_lowercase())
        {
            let type_id = (reg.type_id)();
            if let Some(set) = self.tag_index.get(&type_id) {
                return set.iter().copied().collect();
            }
        }
        Vec::new()
    }

    // ========== Queries ==========

    /// Build a typed query over components and tags. Fetch with `&T` / `&mut T`
    /// (tuples up to 8); candidates come from the first element, so put the
    /// rarest component first. Don't spawn/despawn inside `for_each`.
    ///
    /// ```rust
    /// world.query::<(&mut Transform, &Velocity)>()
    ///     .with_tag::<ActiveCamera>()
    ///     .without::<Frozen>()
    ///     .for_each(|id, (transform, vel)| {
    ///         transform.local_position += vel.linear * dt;
    ///     });
    /// ```
    pub fn query<F: QueryFetch>(&mut self) -> QueryBuilder<'_, F> {
        QueryBuilder::new(self)
    }

    pub fn insert_resource<T: Resource + 'static>(&mut self, resource: T) -> &mut Self {
        self.resources.insert(resource);
        self
    }

    pub fn get_resource<T: Resource + 'static>(&self) -> Result<&T> {
        self.resources.get::<T>()
    }

    pub fn has_resource<T: Resource + 'static>(&self) -> bool {
        self.resources.get::<T>().is_ok()
    }

    pub fn get_resource_mut<T: Resource + 'static>(&mut self) -> Result<&mut T> {
        self.resources.get_mut::<T>()
    }

    pub fn remove_resource<T: Resource + 'static>(&mut self) -> &mut Self {
        self.resources.remove::<T>();
        self
    }

    // ========== Voxel-specific ==========

    pub fn register_chunk(&mut self, id: EntityId) {
        if let Some(t) = self.worldspace.get_component::<VoxelTransform>(id) {
            self.chunk_position_index
                .insert((t.position.x, t.position.y, t.position.z), id);
        }
    }

    pub fn unregister_chunk(&mut self, id: EntityId) {
        self.chunk_position_index.retain(|_, &mut oid| oid != id);
    }

    #[inline]
    pub fn get_voxel(&self, wx: i32, wy: i32, wz: i32) -> Option<VoxelId> {
        let key = (wx.div_euclid(32), wy.div_euclid(32), wz.div_euclid(32));
        let id = self.chunk_position_index.get(&key)?;
        let chunk = self.worldspace.get_component::<Chunk>(*id)?;
        let lx = wx.rem_euclid(32) as u32;
        let ly = wy.rem_euclid(32) as u32;
        let lz = wz.rem_euclid(32) as u32;
        Some(chunk.voxels[flatten(lx, ly, lz, 32)])
    }

    #[inline]
    pub fn set_voxel(&mut self, wx: i32, wy: i32, wz: i32, voxel_id: VoxelId) -> bool {
        let key = (wx.div_euclid(32), wy.div_euclid(32), wz.div_euclid(32));
        let Some(&oid) = self.chunk_position_index.get(&key) else {
            return false;
        };
        let lx = wx.rem_euclid(32) as u32;
        let ly = wy.rem_euclid(32) as u32;
        let lz = wz.rem_euclid(32) as u32;
        if let Some(chunk) = self.worldspace.get_component_mut::<Chunk>(oid) {
            chunk.voxels[flatten(lx, ly, lz, 32)] = voxel_id;
        }
        self.worldspace.add_tag::<NeedsRemeshing>(oid);
        true
    }

    pub fn build_raw_chunk_lookup(
        &self,
    ) -> HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]> {
        self.chunk_position_index
            .iter()
            .filter_map(|(&pos, &id)| {
                let chunk = self.worldspace.get_component::<Chunk>(id)?;
                Some((pos, chunk.voxels.as_ref() as *const _))
            })
            .collect()
    }

    /// # Safety
    /// The chunk must still be alive - no add/remove between `build_raw_chunk_lookup` and this call.
    #[inline(always)]
    pub unsafe fn get_voxel_raw(
        map: &HashMap<(i32, i32, i32), *const [VoxelId; 32 * 32 * 32]>,
        wx: i32,
        wy: i32,
        wz: i32,
    ) -> VoxelId {
        unsafe {
            let key = (wx >> 5, wy >> 5, wz >> 5);
            match map.get(&key) {
                Some(&ptr) => {
                    *(*ptr).get_unchecked(((wx & 31) + (wy & 31) * 32 + (wz & 31) * 1024) as usize)
                }
                None => 0,
            }
        }
    }
}
