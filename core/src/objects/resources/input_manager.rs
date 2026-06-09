use anyhow::Result;
use apostasy_macros::{Resource, late_update};
use cgmath::{Vector2, Vector3};
use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use std::collections::HashMap as StdHashMap;
use std::path::Path;
use winit::{
    dpi::PhysicalPosition,
    event::{MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{PhysicalKey, KeyCode},
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableKeyBind {
    pub key: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMouseBind {
    pub key: String,
    pub action: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeybindsFile {
    pub keybinds: StdHashMap<String, SerializableKeyBind>,
    pub mousebinds: StdHashMap<String, SerializableMouseBind>,
}

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
    pub mouse_delta: (f64, f64),

    /// Scroll delta accumulated across all wheel events this frame.
    pub scroll_delta: (f32, f32),

    // Cleared at end of every frame
    pub keys_pressed: HashSet<PhysicalKey>,
    pub keys_released: HashSet<PhysicalKey>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_released: HashSet<MouseButton>,

    keybinds_file_path: Option<String>,
}

impl InputManager {
    fn key_to_string(key: &PhysicalKey) -> String {
        match key {
            PhysicalKey::Code(code) => format!("{:?}", code),
            PhysicalKey::Unidentified(native) => format!("Unidentified({:?})", native),
        }
    }

    fn string_to_key(s: &str) -> Result<PhysicalKey> {
        if s.starts_with("Unidentified") {
            return Err(anyhow::anyhow!("Unidentified keys cannot be deserialized"));
        }

        let key_code = match s {
            "KeyA" => KeyCode::KeyA,
            "KeyB" => KeyCode::KeyB,
            "KeyC" => KeyCode::KeyC,
            "KeyD" => KeyCode::KeyD,
            "KeyE" => KeyCode::KeyE,
            "KeyF" => KeyCode::KeyF,
            "KeyG" => KeyCode::KeyG,
            "KeyH" => KeyCode::KeyH,
            "KeyI" => KeyCode::KeyI,
            "KeyJ" => KeyCode::KeyJ,
            "KeyK" => KeyCode::KeyK,
            "KeyL" => KeyCode::KeyL,
            "KeyM" => KeyCode::KeyM,
            "KeyN" => KeyCode::KeyN,
            "KeyO" => KeyCode::KeyO,
            "KeyP" => KeyCode::KeyP,
            "KeyQ" => KeyCode::KeyQ,
            "KeyR" => KeyCode::KeyR,
            "KeyS" => KeyCode::KeyS,
            "KeyT" => KeyCode::KeyT,
            "KeyU" => KeyCode::KeyU,
            "KeyV" => KeyCode::KeyV,
            "KeyW" => KeyCode::KeyW,
            "KeyX" => KeyCode::KeyX,
            "KeyY" => KeyCode::KeyY,
            "KeyZ" => KeyCode::KeyZ,
            "Digit0" => KeyCode::Digit0,
            "Digit1" => KeyCode::Digit1,
            "Digit2" => KeyCode::Digit2,
            "Digit3" => KeyCode::Digit3,
            "Digit4" => KeyCode::Digit4,
            "Digit5" => KeyCode::Digit5,
            "Digit6" => KeyCode::Digit6,
            "Digit7" => KeyCode::Digit7,
            "Digit8" => KeyCode::Digit8,
            "Digit9" => KeyCode::Digit9,
            "F1" => KeyCode::F1,
            "F2" => KeyCode::F2,
            "F3" => KeyCode::F3,
            "F4" => KeyCode::F4,
            "F5" => KeyCode::F5,
            "F6" => KeyCode::F6,
            "F7" => KeyCode::F7,
            "F8" => KeyCode::F8,
            "F9" => KeyCode::F9,
            "F10" => KeyCode::F10,
            "F11" => KeyCode::F11,
            "F12" => KeyCode::F12,
            "Enter" => KeyCode::Enter,
            "Space" => KeyCode::Space,
            "Tab" => KeyCode::Tab,
            "Backspace" => KeyCode::Backspace,
            "Escape" => KeyCode::Escape,
            "ControlLeft" => KeyCode::ControlLeft,
            "ControlRight" => KeyCode::ControlRight,
            "ShiftLeft" => KeyCode::ShiftLeft,
            "ShiftRight" => KeyCode::ShiftRight,
            "AltLeft" => KeyCode::AltLeft,
            "AltRight" => KeyCode::AltRight,
            "ArrowLeft" => KeyCode::ArrowLeft,
            "ArrowRight" => KeyCode::ArrowRight,
            "ArrowUp" => KeyCode::ArrowUp,
            "ArrowDown" => KeyCode::ArrowDown,
            _ => return Err(anyhow::anyhow!("Unknown key: {}", s)),
        };

        Ok(PhysicalKey::Code(key_code))
    }

    fn mouse_button_to_string(button: &MouseButton) -> String {
        format!("{:?}", button)
    }

    fn string_to_mouse_button(s: &str) -> Result<MouseButton> {
        match s {
            "Left" => Ok(MouseButton::Left),
            "Right" => Ok(MouseButton::Right),
            "Middle" => Ok(MouseButton::Middle),
            "Back" => Ok(MouseButton::Back),
            "Forward" => Ok(MouseButton::Forward),
            _ => Err(anyhow::anyhow!("Unknown mouse button: {}", s)),
        }
    }

    fn action_to_string(action: &KeyAction) -> String {
        format!("{:?}", action)
    }

    fn string_to_action(s: &str) -> Result<KeyAction> {
        match s {
            "Press" => Ok(KeyAction::Press),
            "Release" => Ok(KeyAction::Release),
            "Hold" => Ok(KeyAction::Hold),
            _ => Err(anyhow::anyhow!("Unknown action: {}", s)),
        }
    }

    pub fn set_keybinds_file_path(&mut self, path: impl Into<String>) {
        self.keybinds_file_path = Some(path.into());
    }

    pub fn load_keybinds_from_file(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        let file: KeybindsFile = serde_yaml::from_str(&content)?;

        for (name, serialized) in file.keybinds {
            let key = Self::string_to_key(&serialized.key)?;
            let action = Self::string_to_action(&serialized.action)?;
            self.keybinds.insert(name, KeyBind::new(key, action));
        }

        for (name, serialized) in file.mousebinds {
            let button = Self::string_to_mouse_button(&serialized.key)?;
            let action = Self::string_to_action(&serialized.action)?;
            self.mouse_keybinds.insert(name, MouseBind::new(button, action));
        }

        log!("loaded keybinds from {:?}", path);
        Ok(())
    }

    pub fn save_keybinds_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut keybinds = StdHashMap::new();
        for (name, bind) in &self.keybinds {
            keybinds.insert(
                name.clone(),
                SerializableKeyBind {
                    key: Self::key_to_string(&bind.key),
                    action: Self::action_to_string(&bind.action),
                },
            );
        }

        let mut mousebinds = StdHashMap::new();
        for (name, bind) in &self.mouse_keybinds {
            mousebinds.insert(
                name.clone(),
                SerializableMouseBind {
                    key: Self::mouse_button_to_string(&bind.key),
                    action: Self::action_to_string(&bind.action),
                },
            );
        }

        let file = KeybindsFile { keybinds, mousebinds };
        let content = serde_yaml::to_string(&file)?;
        std::fs::write(path, content)?;

        Ok(())
    }

    pub fn load_or_init_keybinds(&mut self, path: impl Into<String>) {
        let path_str = path.into();
        self.keybinds_file_path = Some(path_str.clone());

        let path_obj = Path::new(&path_str);
        if let Err(e) = self.load_keybinds_from_file(path_obj) {
            log_warn!("Failed to load keybinds: {}", e);
        }
    }

    fn try_save_keybinds(&self) {
        if let Some(ref path_str) = self.keybinds_file_path {
            let path = Path::new(path_str);
            if let Err(e) = self.save_keybinds_to_file(path) {
                log_warn!("Failed to save keybinds: {}", e);
            }
        }
    }
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
        self.try_save_keybinds();
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
        self.try_save_keybinds();
        Ok(())
    }

    /// Register a default keybind only if not already registered. For use in startup systems.
    pub fn register_default_keybind(&mut self, name: impl Into<String>, bind: KeyBind) {
        let name = name.into();
        if !self.keybinds.contains_key(&name) {
            log!("registering default keybind: {}", name);
            self.keybinds.insert(name, bind);
            self.try_save_keybinds();
        }
    }

    /// Register a default mousebind only if not already registered. For use in startup systems.
    pub fn register_default_mousebind(&mut self, name: impl Into<String>, bind: MouseBind) {
        let name = name.into();
        if !self.mouse_keybinds.contains_key(&name) {
            log!("registering default mousebind: {}", name);
            self.mouse_keybinds.insert(name, bind);
            self.try_save_keybinds();
        }
    }

    /// Overwrite an existing keybind (or insert if absent).
    pub fn rebind_key(&mut self, name: impl Into<String>, bind: KeyBind) {
        let name = name.into();
        log!("rebinding keybind: {}", name);
        self.keybinds.insert(name, bind);
        self.try_save_keybinds();
    }

    /// Overwrite an existing mousebind (or insert if absent).
    pub fn rebind_mouse(&mut self, name: impl Into<String>, bind: MouseBind) {
        let name = name.into();
        log!("rebinding mousebind: {}", name);
        self.mouse_keybinds.insert(name, bind);
        self.try_save_keybinds();
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

#[late_update(mode = "all")]
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
