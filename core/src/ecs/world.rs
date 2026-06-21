use std::cmp::Reverse;

use anyhow::Result;
use cgmath::Vector3;
use hashbrown::HashMap;

use crate::EngineMode;
use crate::ecs::{
    components::Component,
    resources::{Resource, ResourceMap},
    systems::{
        DeltaTime, EngineTimer, FixedUpdateSystem, FixedUpdateTimer, HasMode, HasPriority,
        LateUpdateSystem, PreRenderSystem, StartSystem, UpdateSystem,
    },
    tags::Tag,
};
use crate::utils::flatten::flatten;
use crate::voxels::{VoxelTransform, chunk::Chunk, meshes::NeedsRemeshing, voxel::VoxelId};
use crate::worldspaces::{
    cell::{CellCoord, ObjectId},
    worldspace::Worldspace,
};

#[derive(Default)]
pub struct World {
    pub(crate) worldspace: Worldspace,
    pub(crate) resources: ResourceMap,
    pub(crate) chunk_position_index: HashMap<(i32, i32, i32), ObjectId>,

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
    pub fn spawn(&mut self) -> ObjectId {
        self.worldspace.spawn()
    }

    /// Spawns a new entity in the cell containing `position`.
    pub fn spawn_at_position(&mut self, position: Vector3<f32>) -> ObjectId {
        self.worldspace.spawn_at_position(position)
    }

    /// Spawns a new entity in a specific cell.
    pub fn spawn_in_cell(&mut self, coord: CellCoord) -> ObjectId {
        self.worldspace.spawn_in_cell(coord)
    }

    /// Despawns an entity and all its descendants.
    pub fn despawn(&mut self, id: ObjectId) {
        self.worldspace.despawn(id);
    }

    pub fn is_alive(&self, id: ObjectId) -> bool {
        self.worldspace.is_alive(id)
    }

    // ========== Names ==========

    pub fn set_name(&mut self, id: ObjectId, name: &str) {
        self.worldspace.set_name(id, name);
    }

    pub fn get_name(&self, id: ObjectId) -> Option<&str> {
        self.worldspace.get_name(id)
    }

    // ========== Components ==========

    pub fn add_component<T: Component + Clone + 'static>(&mut self, id: ObjectId, component: T) {
        self.worldspace.add_component(id, component);
    }

    pub fn get_component<T: Component + 'static>(&self, id: ObjectId) -> Option<&T> {
        self.worldspace.get_component(id)
    }

    pub fn get_component_mut<T: Component + 'static>(&mut self, id: ObjectId) -> Option<&mut T> {
        self.worldspace.get_component_mut(id)
    }

    pub fn remove_component<T: Component + 'static>(&mut self, id: ObjectId) {
        self.worldspace.remove_component::<T>(id);
    }

    pub fn has_component<T: Component + 'static>(&self, id: ObjectId) -> bool {
        self.worldspace.has_component::<T>(id)
    }

    /// Returns all entity IDs across all cells that have component T.
    pub fn get_entities_with_component<T: Component + 'static>(&self) -> Vec<ObjectId> {
        self.worldspace.get_entities_with_component::<T>()
    }

    // ========== Tags ==========

    pub fn add_tag<T: Tag + 'static>(&mut self, id: ObjectId) {
        self.worldspace.add_tag::<T>(id);
    }

    pub fn remove_tag<T: Tag + 'static>(&mut self, id: ObjectId) {
        self.worldspace.remove_tag::<T>(id);
    }

    pub fn has_tag<T: Tag + 'static>(&self, id: ObjectId) -> bool {
        self.worldspace.has_tag::<T>(id)
    }

    /// Returns the first entity across all cells with tag T.
    pub fn get_entity_with_tag<T: Tag + 'static>(&self) -> Result<ObjectId> {
        self.worldspace.get_entity_with_tag::<T>()
    }

    /// Returns all entities across all cells with tag T.
    pub fn get_entities_with_tag<T: Tag + 'static>(&self) -> Vec<ObjectId> {
        self.worldspace.get_entities_with_tag::<T>()
    }

    // ========== Hierarchy ==========

    pub fn set_parent(&mut self, child_id: ObjectId, parent_id: Option<ObjectId>) -> Result<()> {
        self.worldspace.set_parent(child_id, parent_id)
    }

    pub fn detach(&mut self, id: ObjectId) -> Result<()> {
        self.worldspace.detach_from_parent(id)
    }

    pub fn get_parent_id(&self, id: ObjectId) -> Option<ObjectId> {
        self.worldspace.get_parent_id(id)
    }

    pub fn get_children_ids(&self, id: ObjectId) -> Vec<ObjectId> {
        self.worldspace.get_children_ids(id)
    }

    pub fn get_ancestors(&self, id: ObjectId) -> Vec<ObjectId> {
        self.worldspace.get_ancestors(id)
    }

    pub fn get_descendants(&self, id: ObjectId) -> Vec<ObjectId> {
        self.worldspace.get_descendants(id)
    }

    pub fn is_ancestor_of(&self, ancestor_id: ObjectId, descendant_id: ObjectId) -> bool {
        self.worldspace.is_ancestor_of(ancestor_id, descendant_id)
    }

    pub fn get_all_ids(&self) -> Vec<ObjectId> {
        self.worldspace.get_all_ids()
    }

    pub fn get_root_ids(&self) -> Vec<ObjectId> {
        self.worldspace.get_root_ids()
    }

    pub fn object_count(&self) -> usize {
        self.worldspace.object_count()
    }

    pub fn replace_worldspace(&mut self, ws: Worldspace) {
        self.worldspace = ws;
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

    pub fn register_chunk(&mut self, id: ObjectId) {
        if let Some(t) = self.worldspace.get_component::<VoxelTransform>(id) {
            self.chunk_position_index
                .insert((t.position.x, t.position.y, t.position.z), id);
        }
    }

    pub fn unregister_chunk(&mut self, id: ObjectId) {
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
