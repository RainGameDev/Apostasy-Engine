use anyhow::Result;
use cgmath::Vector3;
use wasmtime::{Caller, Linker};

use crate::ecs::{
    cell::EntityId,
    components::{get_component_registration, transform::Transform},
    tag::get_tag_registration,
    world::World,
};
use crate::physics::collider::{Collider, ColliderShape};
use crate::rendering::components::model_renderer::ModelRenderer;
use crate::ecs::systems::{DeltaTime, EngineTimer};

/// Host state threaded through each wasmtime Store for the duration of one script call.
pub struct HostState {
    /// Raw pointer to the engine World. Valid only while the script call is in progress.
    pub world: *mut World,
    /// Per-call handle table mapping guest i32 handles to engine EntityIds.
    pub handles: Vec<EntityId>,
}

unsafe impl Send for HostState {}
unsafe impl Sync for HostState {}

impl HostState {
    pub fn new(world: *mut World) -> Self {
        Self { world, handles: Vec::new() }
    }

    pub fn push_id(&mut self, id: EntityId) -> i32 {
        let idx = self.handles.len() as i32;
        self.handles.push(id);
        idx
    }

    pub fn get_id(&self, handle: i32) -> Option<EntityId> {
        self.handles.get(handle as usize).copied()
    }

    pub fn world(&self) -> &World {
        unsafe { &*self.world }
    }

    pub fn world_mut(&mut self) -> &mut World {
        unsafe { &mut *self.world }
    }
}

/// Reads a UTF-8 string from guest linear memory at `[ptr, ptr+len)`.
fn read_str(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> String {
    let Some(wasmtime::Extern::Memory(mem)) = caller.get_export("memory") else {
        return String::new();
    };
    let bytes = mem.data(&*caller)[ptr as usize..(ptr + len) as usize].to_vec();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Registers all engine host functions on `linker`.
pub fn register_host_fns(linker: &mut Linker<HostState>) -> Result<()> {
    // Spawns an entity with the given name and returns its guest handle.
    linker.func_wrap("env", "host_spawn", |mut caller: Caller<'_, HostState>, name_ptr: u32, name_len: u32| -> i32 {
        let name = read_str(&mut caller, name_ptr, name_len);
        let world = caller.data_mut().world_mut();
        let id = world.spawn().id();
        world.set_name(id, &name);
        caller.data_mut().push_id(id)
    })?;

    // Adds a named component to the entity at `handle`.
    linker.func_wrap("env", "host_add_component", |mut caller: Caller<'_, HostState>, handle: i32, name_ptr: u32, name_len: u32| {
        let name = read_str(&mut caller, name_ptr, name_len);
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(reg) = get_component_registration(&name) {
            let component = (reg.create)();
            (reg.add_to_world)(world, id, component);
        }
    })?;

    // Adds a named tag to the entity at `handle`.
    linker.func_wrap("env", "host_add_tag", |mut caller: Caller<'_, HostState>, handle: i32, name_ptr: u32, name_len: u32| {
        let name = read_str(&mut caller, name_ptr, name_len);
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(reg) = get_tag_registration(&name) {
            (reg.add_to_world)(world, id);
        }
    })?;

    // Sets Transform fields on the entity at `handle`.
    linker.func_wrap("env", "host_set_transform", |mut caller: Caller<'_, HostState>,
        handle: i32,
        px: f32, py: f32, pz: f32,
        ex: f32, ey: f32, ez: f32,
        sx: f32, sy: f32, sz: f32,
    | {
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(t) = world.get_component_mut::<Transform>(id) {
            t.local_position = Vector3::new(px, py, pz);
            t.local_euler_angles = Vector3::new(ex, ey, ez);
            t.local_scale = Vector3::new(sx, sy, sz);
        }
    })?;

    // Sets the model path on the ModelRenderer at `handle`.
    linker.func_wrap("env", "host_set_model_path", |mut caller: Caller<'_, HostState>, handle: i32, ptr: u32, len: u32| {
        let path = read_str(&mut caller, ptr, len);
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(mr) = world.get_component_mut::<ModelRenderer>(id) {
            mr.model_path = path;
            mr.model = None;
        }
    })?;

    // Sets the material override on the ModelRenderer at `handle`.
    linker.func_wrap("env", "host_set_material", |mut caller: Caller<'_, HostState>, handle: i32, ptr: u32, len: u32| {
        let name = read_str(&mut caller, ptr, len);
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(mr) = world.get_component_mut::<ModelRenderer>(id) {
            mr.material_override = if name.is_empty() { None } else { Some(name) };
        }
    })?;

    // Sets a cuboid collider shape on the Collider at `handle`.
    linker.func_wrap("env", "host_set_collider_cuboid", |mut caller: Caller<'_, HostState>, handle: i32, sx: f32, sy: f32, sz: f32| {
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(col) = world.get_component_mut::<Collider>(id) {
            col.shape = ColliderShape::Cuboid { size: Vector3::new(sx, sy, sz) };
        }
    })?;

    // Sets a sphere collider shape on the Collider at `handle`.
    linker.func_wrap("env", "host_set_collider_sphere", |mut caller: Caller<'_, HostState>, handle: i32, radius: f32| {
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(col) = world.get_component_mut::<Collider>(id) {
            col.shape = ColliderShape::Sphere { radius };
        }
    })?;

    // Moves the entity at `handle` by (dx, dy, dz) in world space.
    linker.func_wrap("env", "host_translate", |mut caller: Caller<'_, HostState>, handle: i32, dx: f32, dy: f32, dz: f32| {
        let Some(id) = caller.data().get_id(handle) else { return };
        let world = caller.data_mut().world_mut();
        if let Some(t) = world.get_component_mut::<Transform>(id) {
            t.local_position += Vector3::new(dx, dy, dz);
        }
    })?;

    // Forwards a log message from the script to the engine log.
    linker.func_wrap("env", "host_log", |mut caller: Caller<'_, HostState>, ptr: u32, len: u32| {
        let msg = read_str(&mut caller, ptr, len);
        tracing::info!("[script] {}", msg);
    })?;

    // Returns delta time in seconds as f64.
    linker.func_wrap("env", "host_delta", |caller: Caller<'_, HostState>| -> f64 {
        caller.data().world().get_resource::<DeltaTime>()
            .map(|d| d.0 as f64)
            .unwrap_or(0.0)
    })?;

    // Returns total elapsed engine time in seconds as f64.
    linker.func_wrap("env", "host_time", |caller: Caller<'_, HostState>| -> f64 {
        caller.data().world().get_resource::<EngineTimer>()
            .map(|t| t.0 as f64)
            .unwrap_or(0.0)
    })?;

    Ok(())
}
