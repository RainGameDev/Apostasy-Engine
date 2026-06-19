// Guest-side scripting API for Apostasy Engine scripts compiled to wasm32-unknown-unknown.
// Import everything with `use apostasy_script_api::*;` in your script crate.

// --- Host imports (satisfied by the engine at runtime via wasmtime) ---

unsafe extern "C" {
    fn host_spawn(name_ptr: u32, name_len: u32) -> i32;
    fn host_add_component(handle: i32, name_ptr: u32, name_len: u32);
    fn host_add_tag(handle: i32, name_ptr: u32, name_len: u32);
    fn host_set_transform(
        handle: i32,
        px: f32, py: f32, pz: f32,
        ex: f32, ey: f32, ez: f32,
        sx: f32, sy: f32, sz: f32,
    );
    fn host_set_model_path(handle: i32, ptr: u32, len: u32);
    fn host_set_material(handle: i32, ptr: u32, len: u32);
    fn host_set_collider_cuboid(handle: i32, sx: f32, sy: f32, sz: f32);
    fn host_set_collider_sphere(handle: i32, radius: f32);
    fn host_translate(handle: i32, dx: f32, dy: f32, dz: f32);
    fn host_log(ptr: u32, len: u32);
    fn host_delta() -> f64;
    fn host_time() -> f64;
}

// --- Free functions ---

/// Spawns a named object in the world and returns its handle.
pub fn spawn(name: &str) -> ObjectHandle {
    let handle = unsafe { host_spawn(name.as_ptr() as u32, name.len() as u32) };
    ObjectHandle(handle)
}

/// Prints a message to the engine log.
pub fn log(msg: &str) {
    unsafe { host_log(msg.as_ptr() as u32, msg.len() as u32) };
}

/// Returns seconds elapsed since the last frame.
pub fn delta() -> f32 {
    unsafe { host_delta() as f32 }
}

/// Returns total elapsed engine time in seconds.
pub fn time() -> f32 {
    unsafe { host_time() as f32 }
}

// --- Object handle ---

/// A lightweight handle to a world object returned by `spawn`.
#[derive(Clone, Copy)]
pub struct ObjectHandle(i32);

impl ObjectHandle {
    /// Adds a component and initializes it from `value`, then returns self for chaining.
    pub fn with<T: Component>(self, value: T) -> Self {
        let name = T::NAME;
        unsafe { host_add_component(self.0, name.as_ptr() as u32, name.len() as u32) };
        value.apply(self.0);
        self
    }

    /// Adds a component with its default values.
    pub fn add_component<T: Component + Default>(self) -> Self {
        self.with(T::default())
    }

    /// Adds a tag to this object, then returns self for chaining.
    pub fn add_tag<T: Tag>(self) -> Self {
        let name = T::NAME;
        unsafe { host_add_tag(self.0, name.as_ptr() as u32, name.len() as u32) };
        self
    }

    /// Moves the object by the given world-space offset.
    pub fn translate(&self, dx: f32, dy: f32, dz: f32) {
        unsafe { host_translate(self.0, dx, dy, dz) };
    }
}

// --- Traits ---

/// Implemented by each mirror component type so `ObjectHandle::with` can apply it.
pub trait Component {
    const NAME: &'static str;

    /// Sends the component's field values to the host via typed host functions.
    fn apply(&self, handle: i32);
}

/// Implemented by each mirror tag type so `ObjectHandle::add_tag` can look up the name.
pub trait Tag {
    const NAME: &'static str;
}

// --- Math ---

/// A 3D floating-point vector.
#[derive(Clone, Copy, Default)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::default()
    }

    pub fn one() -> Self {
        Self::new(1.0, 1.0, 1.0)
    }
}

// --- Mirror component types ---

/// Mirror of the engine Transform component.
#[derive(Clone, Default)]
pub struct Transform {
    pub local_position: Vector3,
    pub local_euler_angles: Vector3,
    pub local_scale: Vector3,
}

impl Transform {
    /// Creates a Transform at the given position with default rotation and unit scale.
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        Self {
            local_position: Vector3::new(x, y, z),
            local_scale: Vector3::one(),
            ..Default::default()
        }
    }
}

impl Component for Transform {
    const NAME: &'static str = "Transform";

    fn apply(&self, handle: i32) {
        unsafe {
            host_set_transform(
                handle,
                self.local_position.x, self.local_position.y, self.local_position.z,
                self.local_euler_angles.x, self.local_euler_angles.y, self.local_euler_angles.z,
                self.local_scale.x, self.local_scale.y, self.local_scale.z,
            );
        }
    }
}

/// Mirror of the engine Camera component.
#[derive(Clone)]
pub struct Camera {
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
    pub is_main: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self { fov_y: 90.0, near: 0.001, far: 10000.0, is_main: false }
    }
}

impl Component for Camera {
    const NAME: &'static str = "Camera";

    fn apply(&self, _handle: i32) {
        // Camera is default-initialized by host_add_component; no fast-path host fn needed.
    }
}

/// Mirror of the engine ModelRenderer component.
#[derive(Clone)]
pub struct ModelRenderer {
    pub model_path: String,
    pub material_override: Option<String>,
}

impl Default for ModelRenderer {
    fn default() -> Self {
        Self { model_path: "cube".to_string(), material_override: None }
    }
}

impl ModelRenderer {
    /// Constructs a ModelRenderer pointing at the given model path.
    pub fn from_path(path: &str) -> Self {
        Self { model_path: path.to_string(), material_override: None }
    }
}

impl Component for ModelRenderer {
    const NAME: &'static str = "ModelRenderer";

    fn apply(&self, handle: i32) {
        unsafe {
            host_set_model_path(
                handle,
                self.model_path.as_ptr() as u32,
                self.model_path.len() as u32,
            );
        }
        if let Some(mat) = &self.material_override {
            unsafe { host_set_material(handle, mat.as_ptr() as u32, mat.len() as u32) };
        }
    }
}

/// Collision shape for a `Collider`.
#[derive(Clone)]
pub enum ColliderShape {
    /// Axis-aligned box. `size` is the full extent on each axis.
    Cuboid { size: Vector3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, height: f32 },
}

/// Mirror of the engine Collider component.
#[derive(Clone)]
pub struct Collider {
    pub shape: ColliderShape,
    pub is_static: bool,
}

impl Default for Collider {
    fn default() -> Self {
        Self {
            shape: ColliderShape::Cuboid { size: Vector3::one() },
            is_static: false,
        }
    }
}

impl Collider {
    /// Creates a box collider with the given full extents.
    pub fn cuboid(x: f32, y: f32, z: f32) -> Self {
        Self { shape: ColliderShape::Cuboid { size: Vector3::new(x, y, z) }, is_static: false }
    }

    /// Creates a sphere collider with the given radius.
    pub fn sphere(radius: f32) -> Self {
        Self { shape: ColliderShape::Sphere { radius }, is_static: false }
    }

    /// Marks this collider as static (immovable).
    pub fn as_static(mut self) -> Self {
        self.is_static = true;
        self
    }
}

impl Component for Collider {
    const NAME: &'static str = "Collider";

    fn apply(&self, handle: i32) {
        unsafe {
            match &self.shape {
                ColliderShape::Cuboid { size } => {
                    host_set_collider_cuboid(handle, size.x, size.y, size.z);
                }
                ColliderShape::Sphere { radius } => {
                    host_set_collider_sphere(handle, *radius);
                }
                ColliderShape::Capsule { radius, height } => {
                    host_set_collider_cuboid(handle, *radius, *height * 0.5 + *radius, *radius);
                }
            }
        }
    }
}

// --- Mirror tag types ---

/// Marks the camera object used for rendering.
pub struct ActiveCamera;
impl Tag for ActiveCamera { const NAME: &'static str = "ActiveCamera"; }

/// Marks the in-game camera (as opposed to the editor fly-cam).
pub struct GameCamera;
impl Tag for GameCamera { const NAME: &'static str = "GameCamera"; }

/// Marks the editor fly camera.
pub struct EditorCamera;
impl Tag for EditorCamera { const NAME: &'static str = "EditorCamera"; }

/// Marks a player-controlled object.
pub struct Player;
impl Tag for Player { const NAME: &'static str = "Player"; }
