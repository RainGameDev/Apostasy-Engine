use anyhow::Result;
use apostasy_macros::{Resource, late_update};
use cgmath::{Vector2, Vector3};
use hashbrown::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use std::collections::HashMap as StdHashMap;
use std::path::Path;
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{MouseButton, MouseScrollDelta, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

use crate::{log, log_warn, ecs::world::World};

/// Enum that describes how an input is activated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAction {
    Press,
    Release,
    Hold,
}

/// Which modifier keys must be held for a Press/Release bind to fire.
/// Hold-action binds ignore this (they check only whether the key is held).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModifierKeys {
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl ModifierKeys {
    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }
    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }
    pub fn alt() -> Self {
        Self {
            alt: true,
            ..Default::default()
        }
    }
}

/// Causes a Press-action keybind to fire repeatedly while the key is held.
/// The first repeat fires after `initial_delay_secs`, subsequent ones fire at `rate_hz`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepeatConfig {
    /// Seconds of held time before the first repeat fires.
    #[serde(default = "RepeatConfig::default_delay")]
    pub initial_delay_secs: f32,
    /// Repeats per second after the initial delay.
    #[serde(default = "RepeatConfig::default_rate")]
    pub rate_hz: f32,
}

impl RepeatConfig {
    pub fn new(initial_delay_secs: f32, rate_hz: f32) -> Self {
        Self {
            initial_delay_secs,
            rate_hz,
        }
    }
    fn default_delay() -> f32 {
        0.4
    }
    fn default_rate() -> f32 {
        15.0
    }
}

impl Default for RepeatConfig {
    fn default() -> Self {
        Self {
            initial_delay_secs: Self::default_delay(),
            rate_hz: Self::default_rate(),
        }
    }
}

/// A struct describing the state of a keybind.
/// Requires a key and action, takes an optional context and repeat config.
#[derive(Debug, Clone)]
pub struct Bind<K> {
    pub key: K,
    pub action: KeyAction,
    /// Required modifier state. Only checked for Press/Release actions.
    pub modifiers: ModifierKeys,
    /// If Some, only fires when this context is in `InputManager::active_contexts`.
    pub context: Option<String>,
    /// If Some, generates repeat virtual-press events while the key is held.
    pub repeat: Option<RepeatConfig>,
}

impl<K> Bind<K> {
    pub fn new(key: K, action: KeyAction) -> Self {
        Self {
            key,
            action,
            modifiers: ModifierKeys::default(),
            context: None,
            repeat: None,
        }
    }

    pub fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }
    pub fn with_shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }
    pub fn with_alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    pub fn with_context(mut self, ctx: impl Into<String>) -> Self {
        self.context = Some(ctx.into());
        self
    }

    pub fn with_repeat(mut self, config: RepeatConfig) -> Self {
        self.repeat = Some(config);
        self
    }
}

pub type KeyBind = Bind<PhysicalKey>;
pub type MouseBind = Bind<MouseButton>;

/// Definition of a Keybinding that has been serialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableKeyBind {
    pub key: String,
    pub action: String,
    #[serde(default)]
    pub modifiers: ModifierKeys,
    #[serde(default)]
    pub context: Option<String>,
}

/// Definition of a Mousebinding that has been serialized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMouseBind {
    pub key: String,
    pub action: String,
    #[serde(default)]
    pub modifiers: ModifierKeys,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeybindsFile {
    pub keybinds: StdHashMap<String, SerializableKeyBind>,
    pub mousebinds: StdHashMap<String, SerializableMouseBind>,
}

/// Implement on an enum to get typo-proof action names instead of raw `&str`.
pub trait Action: std::fmt::Debug + Clone + PartialEq + Eq + std::hash::Hash {}
impl Action for String {}

/// Describes an error returned by an input.
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

/// A resource that defines registered inputs and what's active.
#[derive(Resource, Clone, Default)]
pub struct InputManager {
    pub keybinds: HashMap<String, KeyBind>,
    pub default_keybinds: HashMap<String, KeyBind>,
    pub mouse_keybinds: HashMap<String, MouseBind>,

    pub keys_held: HashSet<PhysicalKey>,
    pub mouse_held: HashSet<MouseButton>,
    /// Cursor position in physical pixels relative to the top-left of the window.
    pub mouse_position: PhysicalPosition<f64>,

    /// Raw mouse delta accumulated from `DeviceEvent::MouseMotion` this frame.
    /// Not affected by cursor acceleration or OS pointer speed - use this for camera look.
    /// Reset to `(0.0, 0.0)` at the end of every frame.
    pub mouse_delta: (f64, f64),
    /// Scroll wheel delta accumulated this frame as `(horizontal, vertical)` in pixels.
    /// Reset to `(0.0, 0.0)` at the end of every frame.
    pub scroll_delta: (f32, f32),

    /// Keys that became pressed this frame (cleared at end of frame).
    pub keys_pressed: HashSet<PhysicalKey>,
    /// Keys that were released this frame (cleared at end of frame).
    pub keys_released: HashSet<PhysicalKey>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_released: HashSet<MouseButton>,

    /// Active input contexts.
    /// A bind with `context = Some(name)` only fires when `name` is present here.
    /// Set by systems each frame; cleared at end of frame.
    pub active_contexts: HashSet<String>,

    // Repeat tracking, when each key was first held, and when it last repeated.
    held_since: HashMap<PhysicalKey, Instant>,
    last_repeat: HashMap<PhysicalKey, Instant>,
    /// Virtual press events generated by hold-repeat.
    /// Only consumed by binds that have `repeat: Some(_)`.
    pub keys_repeat: HashSet<PhysicalKey>,

    keybinds_file_path: Option<String>,
}

/// Detects if an input is a modifier.
fn is_modifier_key(key: &PhysicalKey) -> bool {
    matches!(
        key,
        PhysicalKey::Code(
            KeyCode::ShiftLeft
                | KeyCode::ShiftRight
                | KeyCode::ControlLeft
                | KeyCode::ControlRight
                | KeyCode::AltLeft
                | KeyCode::AltRight
        )
    )
}

impl InputManager {
    // Returns if an input is a modifier.
    fn modifier_state(&self) -> ModifierKeys {
        ModifierKeys {
            ctrl: self
                .keys_held
                .contains(&PhysicalKey::Code(KeyCode::ControlLeft))
                || self
                    .keys_held
                    .contains(&PhysicalKey::Code(KeyCode::ControlRight)),
            shift: self
                .keys_held
                .contains(&PhysicalKey::Code(KeyCode::ShiftLeft))
                || self
                    .keys_held
                    .contains(&PhysicalKey::Code(KeyCode::ShiftRight)),
            alt: self
                .keys_held
                .contains(&PhysicalKey::Code(KeyCode::AltLeft))
                || self
                    .keys_held
                    .contains(&PhysicalKey::Code(KeyCode::AltRight)),
        }
    }

    /// Returns the current modifier key state.
    pub fn modifiers(&self) -> ModifierKeys {
        self.modifier_state()
    }

    /// Determins if a keybinds conditions are valid for the current frame.
    fn eval_key_bind(&self, bind: &KeyBind) -> bool {
        // context check
        if let Some(ref ctx) = bind.context
            && !self.active_contexts.contains(ctx.as_str())
        {
            return false;
        }

        // key state
        let key_active = match bind.action {
            KeyAction::Press => {
                self.keys_pressed.contains(&bind.key)
                    || (bind.repeat.is_some() && self.keys_repeat.contains(&bind.key))
            }
            KeyAction::Release => self.keys_released.contains(&bind.key),
            KeyAction::Hold => {
                if !self.keys_held.contains(&bind.key) {
                    return false;
                }
                if is_modifier_key(&bind.key) {
                    return true;
                }
                let requires_modifier = bind.modifiers.ctrl || bind.modifiers.shift || bind.modifiers.alt;
                return !requires_modifier || self.modifier_state() == bind.modifiers;
            }
        };
        if !key_active {
            return false;
        }

        // determins if a keybind is a modifier
        if is_modifier_key(&bind.key) {
            return true;
        }

        self.modifier_state() == bind.modifiers
    }

    /// Determins if a mousebind's conditions are valid for the current frame.
    fn eval_mouse_bind(&self, bind: &MouseBind) -> bool {
        if let Some(ref ctx) = bind.context
            && !self.active_contexts.contains(ctx.as_str())
        {
            return false;
        }

        // Button state
        match bind.action {
            KeyAction::Hold => return self.mouse_held.contains(&bind.key),
            KeyAction::Press => {
                if !self.mouse_pressed.contains(&bind.key) {
                    return false;
                }
            }
            KeyAction::Release => {
                if !self.mouse_released.contains(&bind.key) {
                    return false;
                }
            }
        }

        self.modifier_state() == bind.modifiers
    }

    /// Converts a physical key to a string.
    fn key_to_string(key: &PhysicalKey) -> String {
        match key {
            PhysicalKey::Code(code) => format!("{:?}", code),
            PhysicalKey::Unidentified(native) => format!("Unidentified({:?})", native),
        }
    }

    /// Converts a string to a physical key.
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

    /// Converts a mouse button to a string.
    fn mouse_button_to_string(button: &MouseButton) -> String {
        format!("{:?}", button)
    }

    /// Converts a string to a mouse button.
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

    /// Converts an action to a string
    fn action_to_string(action: &KeyAction) -> String {
        format!("{:?}", action)
    }

    /// Converts a string to an action
    fn string_to_action(s: &str) -> Result<KeyAction> {
        match s {
            "Press" => Ok(KeyAction::Press),
            "Release" => Ok(KeyAction::Release),
            "Hold" => Ok(KeyAction::Hold),
            _ => Err(anyhow::anyhow!("Unknown action: {}", s)),
        }
    }

    /// Sets the file path for keybindings.
    pub fn set_keybinds_file_path(&mut self, path: impl Into<String>) {
        self.keybinds_file_path = Some(path.into());
    }

    /// Loads keybinds from a file.
    pub fn load_keybinds_from_file(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(path)?;
        let file: KeybindsFile = serde_yaml::from_str(&content)?;

        for (name, s) in file.keybinds {
            let key = Self::string_to_key(&s.key)?;
            let action = Self::string_to_action(&s.action)?;
            let mut bind = KeyBind::new(key, action);
            bind.modifiers = s.modifiers;
            bind.context = s.context;
            self.keybinds.insert(name, bind);
        }
        for (name, s) in file.mousebinds {
            let button = Self::string_to_mouse_button(&s.key)?;
            let action = Self::string_to_action(&s.action)?;
            let mut bind = MouseBind::new(button, action);
            bind.modifiers = s.modifiers;
            bind.context = s.context;
            self.mouse_keybinds.insert(name, bind);
        }
        log!("loaded keybinds from {:?}", path);
        Ok(())
    }

    /// Saves the keybinds to a file.
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
                    modifiers: bind.modifiers.clone(),
                    context: bind.context.clone(),
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
                    modifiers: bind.modifiers.clone(),
                    context: bind.context.clone(),
                },
            );
        }
        let file = KeybindsFile {
            keybinds,
            mousebinds,
        };
        std::fs::write(path, serde_yaml::to_string(&file)?)?;
        Ok(())
    }

    /// Either lods the keybinds or sets defaults.
    pub fn load_or_init_keybinds(&mut self, path: impl Into<String>) {
        let path_str = path.into();
        self.keybinds_file_path = Some(path_str.clone());
        if let Err(e) = self.load_keybinds_from_file(Path::new(&path_str)) {
            log_warn!("Failed to load keybinds: {}", e);
        }
    }

    /// Attempts to save the keybinds
    fn try_save_keybinds(&self) {
        if let Some(ref path_str) = self.keybinds_file_path
            && let Err(e) = self.save_keybinds_to_file(Path::new(path_str))
        {
            log_warn!("Failed to save keybinds: {}", e);
        }
    }

    /// Registers a keybind to the input manager.
    /// ```
    /// #[start(mode = "editor")]
    /// pub fn init(world: &mut World) -> Result<()> {
    ///     let inputs = world.get_resource_mut::<InputManager>().unwrap();
    ///
    ///     // Plain key fires once when the key is pressed.
    ///     inputs.register_default_keybind(
    ///         "Jump",
    ///         KeyBind::new(PhysicalKey::Code(KeyCode::Space), KeyAction::Press),
    ///     );
    ///
    ///     // With a modifier only fires when Ctrl is held.
    ///     inputs.register_default_keybind(
    ///         "Save",
    ///         KeyBind::new(PhysicalKey::Code(KeyCode::KeyS), KeyAction::Press)
    ///         .with_ctrl(),
    ///     );
    ///
    ///     // Hold action true every frame the key is held down.
    ///     inputs.register_default_keybind(
    ///         "Sprint",
    ///         KeyBind::new(PhysicalKey::Code(KeyCode::ShiftLeft), KeyAction::Hold),
    ///     );
    ///
    ///     // Context scoped only active when "gameplay" context is set that frame.
    ///     inputs.register_default_keybind(
    ///         "Interact",
    ///         KeyBind::new(PhysicalKey::Code(KeyCode::KeyE), KeyAction::Press)
    ///         .with_context("gameplay"),
    ///     );
    ///
    ///     Ok(())
    /// }
    ///
    ///
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

    /// Registers a mouse bind to the input manager.
    /// ```
    /// #[start(mode = "editor")]
    /// pub fn init(world: &mut World) -> Result<()> {
    ///         let inputs = world.get_resource_mut::<InputManager>().unwrap();
    ///
    ///         // Plain click fires once on the frame the button is pressed.
    ///         inputs.register_default_mousebind(
    ///             "Select",
    ///             MouseBind::new(MouseButton::Left, KeyAction::Press),
    ///         );
    ///
    ///         // Right-click hold true every frame the button is held.
    ///         inputs.register_default_mousebind(
    ///             "OrbitCamera",
    ///             MouseBind::new(MouseButton::Right, KeyAction::Hold),
    ///         );
    ///
    ///         // With a modifier only fires when Alt is held.
    ///         inputs.register_default_mousebind(
    ///             "Pan",
    ///             MouseBind::new(MouseButton::Middle, KeyAction::Hold).with_alt(),
    ///         );
    ///
    ///         // Context-scoped only active when "viewport" context is set that frame.
    ///         inputs.register_default_mousebind(
    ///             "PlaceEntity",
    ///             MouseBind::new(MouseButton::Left, KeyAction::Press).with_context("viewport"),
    ///         );
    ///
    ///         Ok(())
    /// }
    /// ```
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

    /// Register a default keybind only if not already registered.
    pub fn register_default_keybind(&mut self, name: impl Into<String>, bind: KeyBind) {
        let name = name.into();
        self.default_keybinds.insert(name.clone(), bind.clone());
        if !self.keybinds.contains_key(&name) {
            log!("registering default keybind: {}", name);
            self.keybinds.insert(name, bind);
            self.try_save_keybinds();
        }
    }

    /// Register a default mousebind only if not already registered.
    pub fn register_default_mousebind(&mut self, name: impl Into<String>, bind: MouseBind) {
        let name = name.into();
        if !self.mouse_keybinds.contains_key(&name) {
            log!("registering default mousebind: {}", name);
            self.mouse_keybinds.insert(name, bind);
            self.try_save_keybinds();
        }
    }

    /// Overwrites the current binding for `name` with `bind` and persists the change.
    ///
    /// Unlike [`register_default_keybind`], this always applies regardless of whether a
    /// binding already exists use it when the user has explicitly chosen a new key, such
    /// as from the preferences panel.
    ///
    /// [`register_default_keybind`]: Self::register_default_keybind
    pub fn rebind_key(&mut self, name: impl Into<String>, bind: KeyBind) {
        let name = name.into();
        log!("rebinding keybind: {}", name);
        self.keybinds.insert(name, bind);
        self.try_save_keybinds();
    }

    /// Overwrites the current binding for `name` with `bind` and persists the change.
    ///
    /// Mouse equivalent of [`rebind_key`] same semantics, use when the user has explicitly
    /// chosen a new mouse button.
    ///
    /// [`rebind_key`]: Self::rebind_key
    pub fn rebind_mouse(&mut self, name: impl Into<String>, bind: MouseBind) {
        let name = name.into();
        log!("rebinding mousebind: {}", name);
        self.mouse_keybinds.insert(name, bind);
        self.try_save_keybinds();
    }

    /// Checks if a keybind by the name of [`name`] is active.
    pub fn keybind_active(&self, name: &str) -> Result<bool, InputError> {
        let bind = self
            .keybinds
            .get(name)
            .ok_or_else(|| InputError::UnknownKeybind(name.to_string()))?;
        Ok(self.eval_key_bind(bind))
    }

    /// Convenience wrapper.
    /// Panics in debug builds on an unknown bind name so typos are caught early.
    /// logs a warning and returns `false` in release.
    pub fn is_keybind_active(&self, name: &str) -> bool {
        match self.keybind_active(name) {
            Ok(v) => v,
            Err(e) => {
                debug_assert!(false, "{e}");
                log_warn!("{e}");
                false
            }
        }
    }

    /// Checks if a mousebind by the name of [`name`] is active.
    pub fn mousebind_active(&self, name: &str) -> Result<bool, InputError> {
        let bind = self
            .mouse_keybinds
            .get(name)
            .ok_or_else(|| InputError::UnknownMousebind(name.to_string()))?;
        Ok(self.eval_mouse_bind(bind))
    }

    /// Convenience wrapper.
    /// Panics in debug builds on an unknown bind name so typos are caught early.
    /// logs a warning and returns `false` in release.
    pub fn is_mousebind_active(&self, name: &str) -> bool {
        match self.mousebind_active(name) {
            Ok(v) => v,
            Err(e) => {
                debug_assert!(false, "{e}");
                log_warn!("{e}");
                false
            }
        }
    }

    /// Builds a 2D input vector from four keybind names.
    /// ```
    /// // Returns (1, 0) while D is held, (-1, 0) while A is held, (0, 1) while W, etc.
    /// let dir = inputs.input_vector_2d("MoveLeft", "MoveRight", "MoveUp", "MoveDown");
    /// ```
    pub fn input_vector_2d(&self, left: &str, right: &str, up: &str, down: &str) -> Vector2<f32> {
        let x = self.is_keybind_active(right) as i32 - self.is_keybind_active(left) as i32;
        let y = self.is_keybind_active(up) as i32 - self.is_keybind_active(down) as i32;
        Vector2::new(x as f32, y as f32)
    }

    /// Builds a 3D input vector from six keybind names.
    /// ```
    /// // Returns (1,0,0) while D held, (0,1,0) while Space held, (0,0,-1) while W held, etc.
    /// let dir = inputs.input_vector_3d("MoveRight", "MoveLeft", "MoveUp", "MoveDown", "MoveBack", "MoveForward");
    /// ```
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

    /// Detects the input movement of the mouse.
    pub fn handle_mouse_motion(&mut self, delta: (f64, f64)) {
        self.mouse_delta.0 += delta.0;
        self.mouse_delta.1 += delta.1;
    }

    /// Detects inputs of the keyboard.
    pub fn handle_input_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    if !event.repeat {
                        self.keys_pressed.insert(event.physical_key);
                        // Record when this key was first held (for repeat timing).
                        self.held_since
                            .entry(event.physical_key)
                            .or_insert_with(Instant::now);
                    }
                    self.keys_held.insert(event.physical_key);
                } else {
                    self.keys_released.insert(event.physical_key);
                    self.keys_held.remove(&event.physical_key);
                    self.held_since.remove(&event.physical_key);
                    self.last_repeat.remove(&event.physical_key);
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
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = position;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let MouseScrollDelta::LineDelta(x, y) = delta {
                    self.scroll_delta.0 += x;
                    self.scroll_delta.1 += y;
                }
            }
            _ => {}
        }
    }
}

#[late_update(mode = "all")]
pub fn clear_actions(world: &mut World) -> Result<()> {
    let im = world.get_resource_mut::<InputManager>()?;
    let now = Instant::now();

    // Generate virtual repeat-press events for held keys.
    im.keys_repeat.clear();
    let held: Vec<PhysicalKey> = im.keys_held.iter().copied().collect();
    for key in held {
        // Skip freshly-pressed keys they already appear in keys_pressed this frame.
        if im.keys_pressed.contains(&key) {
            continue;
        }

        let since = match im.held_since.get(&key) {
            Some(t) => *t,
            None => continue,
        };
        let elapsed = now.duration_since(since).as_secs_f32();

        // Use the first registered repeat config for this physical key.
        let cfg = im
            .keybinds
            .values()
            .filter_map(|b| {
                if b.key == key {
                    b.repeat.as_ref()
                } else {
                    None
                }
            })
            .next()
            .cloned();

        if let Some(cfg) = cfg {
            if elapsed >= cfg.initial_delay_secs {
                let last = im.last_repeat.get(&key).copied().unwrap_or(since);
                if now.duration_since(last).as_secs_f32() >= 1.0 / cfg.rate_hz.max(0.1) {
                    im.keys_repeat.insert(key);
                    im.last_repeat.insert(key, now);
                }
            }
        }
    }

    // Active contexts are renewed each frame by the systems that own them.
    im.active_contexts.clear();

    im.keys_pressed.clear();
    im.keys_released.clear();
    im.mouse_pressed.clear();
    im.mouse_released.clear();
    im.mouse_delta = (0.0, 0.0);
    im.scroll_delta = (0.0, 0.0);

    Ok(())
}
