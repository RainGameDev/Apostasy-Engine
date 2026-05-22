use anyhow::Result;
use apostasy_macros::{Resource, late_update};
use cgmath::{Vector2, Vector3};
use hashbrown::{HashMap, HashSet};
use winit::{
    dpi::PhysicalPosition,
    event::{MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::PhysicalKey,
};

use crate::{log, log_warn, objects::world::World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    Press,
    Release,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bind<K> {
    pub key: K,
    pub action: KeyAction,
}

impl<K> Bind<K> {
    pub fn new(key: K, action: KeyAction) -> Self {
        Self { key, action }
    }
}

pub type KeyBind = Bind<PhysicalKey>;
pub type MouseBind = Bind<MouseButton>;

/// Implement this trait on your own enum to get zero-cost, typo-proof action
/// names instead of raw `&str` lookups.
///
/// ```rust
/// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// pub enum GameAction {
///     MoveForward,
///     MoveBack,
///     Jump,
/// }
/// impl Action for GameAction {}
/// ```
pub trait Action: std::fmt::Debug + Clone + PartialEq + Eq + std::hash::Hash {}

// Blanket impl so plain `&str` still works during prototyping.
impl Action for String {}

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("keybind '{0}' is not registered")]
    UnknownKeybind(String),
    #[error("mousebind '{0}' is not registered")]
    UnknownMousebind(String),
    #[error("keybind '{0}' is already registered")]
    DuplicateKeybind(String),
    #[error("mousebind '{0}' is already registered")]
    DuplicateMousebind(String),
}

#[derive(Resource, Clone, Default)]
pub struct InputManager {
    pub keybinds: HashMap<String, KeyBind>,
    pub mouse_keybinds: HashMap<String, MouseBind>,

    pub keys_held: HashSet<PhysicalKey>,
    pub mouse_held: HashSet<MouseButton>,
    pub mouse_position: PhysicalPosition<f64>,

    /// Raw mouse delta accumulated from `DeviceEvent::MouseMotion`.
    /// This is the single source of truth for camera / look control;
    /// `CursorMoved` is used only to track `mouse_position`.
    pub mouse_delta: (f64, f64),

    /// Scroll delta accumulated across all wheel events this frame.
    pub scroll_delta: (f32, f32),

    // Cleared at end of every frame
    pub keys_pressed: HashSet<PhysicalKey>,
    pub keys_released: HashSet<PhysicalKey>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_released: HashSet<MouseButton>,
}

impl InputManager {
    /// Register a named keybind. Returns `Err` if the name is already taken.
    ///
    /// ```rust
    /// inputs.register_keybind("Forwards", KeyBind::new(
    ///     PhysicalKey::Code(KeyCode::KeyW),
    ///     KeyAction::Hold,
    /// ))?;
    /// ```
    pub fn register_keybind(
        &mut self,
        name: impl Into<String>,
        bind: KeyBind,
    ) -> Result<(), InputError> {
        let name = name.into();
        if self.keybinds.contains_key(&name) {
            return Err(InputError::DuplicateKeybind(name));
        }
        log!("registering keybind: {}", name);
        self.keybinds.insert(name, bind);
        Ok(())
    }

    /// Register a named mousebind. Returns `Err` if the name is already taken.
    pub fn register_mousebind(
        &mut self,
        name: impl Into<String>,
        bind: MouseBind,
    ) -> Result<(), InputError> {
        let name = name.into();
        if self.mouse_keybinds.contains_key(&name) {
            return Err(InputError::DuplicateMousebind(name));
        }
        log!("registering mousebind: {}", name);
        self.mouse_keybinds.insert(name, bind);
        Ok(())
    }

    /// Overwrite an existing keybind (or insert if absent).
    pub fn rebind_key(&mut self, name: impl Into<String>, bind: KeyBind) {
        let name = name.into();
        log!("rebinding keybind: {}", name);
        self.keybinds.insert(name, bind);
    }

    /// Overwrite an existing mousebind (or insert if absent).
    pub fn rebind_mouse(&mut self, name: impl Into<String>, bind: MouseBind) {
        let name = name.into();
        log!("rebinding mousebind: {}", name);
        self.mouse_keybinds.insert(name, bind);
    }

    /// Returns whether the named keybind is active, or `Err` if not registered.
    /// Prefer this over `is_keybind_active` when you want to catch typos at
    /// the call site rather than silently get `false`.
    pub fn keybind_active(&self, name: &str) -> Result<bool, InputError> {
        let bind = self
            .keybinds
            .get(name)
            .ok_or_else(|| InputError::UnknownKeybind(name.to_string()))?;
        Ok(self.eval_key_action(&bind.action, &bind.key))
    }

    /// Convenience wrapper: logs a warning and returns `false` on unknown binds.
    /// Useful in hot paths where propagating errors is inconvenient.
    pub fn is_keybind_active(&self, name: &str) -> bool {
        match self.keybind_active(name) {
            Ok(v) => v,
            Err(e) => {
                log_warn!("{e}");
                false
            }
        }
    }

    /// Returns whether the named mousebind is active, or `Err` if not registered.
    pub fn mousebind_active(&self, name: &str) -> Result<bool, InputError> {
        let bind = self
            .mouse_keybinds
            .get(name)
            .ok_or_else(|| InputError::UnknownMousebind(name.to_string()))?;
        Ok(self.eval_mouse_action(&bind.action, &bind.key))
    }

    /// Convenience wrapper: returns `false` on unknown binds.
    pub fn is_mousebind_active(&self, name: &str) -> bool {
        match self.mousebind_active(name) {
            Ok(v) => v,
            Err(e) => {
                log_warn!("{e}");
                false
            }
        }
    }

    pub fn input_vector_2d(&self, left: &str, right: &str, up: &str, down: &str) -> Vector2<f32> {
        let x = self.is_keybind_active(right) as i32 - self.is_keybind_active(left) as i32;
        let y = self.is_keybind_active(up) as i32 - self.is_keybind_active(down) as i32;
        Vector2::new(x as f32, y as f32)
    }

    pub fn input_vector_3d(
        &self,
        x_pos: &str,
        x_neg: &str,
        y_pos: &str,
        y_neg: &str,
        z_pos: &str,
        z_neg: &str,
    ) -> Vector3<f32> {
        let x = self.is_keybind_active(x_pos) as i32 - self.is_keybind_active(x_neg) as i32;
        let y = self.is_keybind_active(y_pos) as i32 - self.is_keybind_active(y_neg) as i32;
        let z = self.is_keybind_active(z_pos) as i32 - self.is_keybind_active(z_neg) as i32;
        Vector3::new(x as f32, y as f32, z as f32)
    }

    /// Feed raw device events here. Used exclusively for mouse-delta
    pub fn handle_mouse_motion(&mut self, delta: (f64, f64)) {
        // Accumulate so multiple motion events in one frame aren't lost.
        self.mouse_delta.0 += delta.0;
        self.mouse_delta.1 += delta.1;
    }

    /// Feed window events here (keyboard, mouse buttons, scroll, cursor pos).
    pub fn handle_input_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    self.keys_pressed.insert(event.physical_key);
                    self.keys_held.insert(event.physical_key);
                } else {
                    self.keys_released.insert(event.physical_key);
                    self.keys_held.remove(&event.physical_key);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state.is_pressed() {
                    self.mouse_pressed.insert(button);
                    self.mouse_held.insert(button);
                } else {
                    self.mouse_released.insert(button);
                    self.mouse_held.remove(&button);
                }
            }
            // CursorMoved only updates the screen-space cursor position.
            // Delta is NOT derived here to avoid conflicting with DeviceEvent.
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = position;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let MouseScrollDelta::LineDelta(x, y) = delta {
                    // Accumulate scroll so fast scrolling isn't dropped.
                    self.scroll_delta.0 += x;
                    self.scroll_delta.1 += y;
                }
            }
            _ => {}
        }
    }

    #[inline]
    fn eval_key_action(&self, action: &KeyAction, key: &PhysicalKey) -> bool {
        match action {
            KeyAction::Press => self.keys_pressed.contains(key),
            KeyAction::Release => self.keys_released.contains(key),
            KeyAction::Hold => self.keys_held.contains(key),
        }
    }

    #[inline]
    fn eval_mouse_action(&self, action: &KeyAction, key: &MouseButton) -> bool {
        match action {
            KeyAction::Press => self.mouse_pressed.contains(key),
            KeyAction::Release => self.mouse_released.contains(key),
            KeyAction::Hold => self.mouse_held.contains(key),
        }
    }
}

#[late_update]
pub fn clear_actions(world: &mut World) -> Result<()> {
    let input_manager = world.get_resource_mut::<InputManager>()?;

    input_manager.keys_pressed.clear();
    input_manager.keys_released.clear();
    input_manager.mouse_pressed.clear();
    input_manager.mouse_released.clear();
    input_manager.mouse_delta = (0.0, 0.0);
    input_manager.scroll_delta = (0.0, 0.0);

    Ok(())
}
