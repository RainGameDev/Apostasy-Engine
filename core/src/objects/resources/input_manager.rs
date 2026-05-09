use anyhow::Result;
use apostasy_macros::{Resource, late_update};
use cgmath::{Vector2, Vector3};
use hashbrown::{HashMap, HashSet};
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceEvent, MouseButton, MouseScrollDelta, WindowEvent},
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
pub struct KeyBind {
    pub key: PhysicalKey,
    pub action: KeyAction,
    pub name: String,
}
impl KeyBind {
    pub fn new(key: PhysicalKey, action: KeyAction, name: &str) -> Self {
        Self {
            key,
            action,
            name: name.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseBind {
    pub key: MouseButton,
    pub action: KeyAction,
    pub name: String,
}

impl MouseBind {
    pub fn new(key: MouseButton, action: KeyAction, name: &str) -> Self {
        Self {
            key,
            action,
            name: name.to_string(),
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct InputManager {
    pub keybinds: HashMap<String, KeyBind>,
    pub mouse_keybinds: HashMap<String, MouseBind>,
    pub keys_held: HashSet<PhysicalKey>,
    pub mouse_held: HashSet<MouseButton>,
    pub mouse_position: PhysicalPosition<f64>,
    pub mouse_delta: (f64, f64),
    pub scroll_delta: (f32, f32),

    // Resets each frame
    pub keys_pressed: HashSet<PhysicalKey>,
    pub keys_released: HashSet<PhysicalKey>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_released: HashSet<MouseButton>,
}

/// TODO: DOCUMENT THIS
impl InputManager {
    pub fn rebind_key(&mut self, key: KeyBind, name: &str) {
        self.keybinds.remove(name);
        self.keybinds.insert(name.to_string(), key);
        // self.serialize_input_manager().unwrap();
    }

    /// Registers a keybind to the input manager, usage:
    /// ```rust
    /// pub fn start(world: &mut World) -> Result<()> {
    ///     let inputs = world.get_resource_mut::<InputManager>()?;
    ///  
    ///     inputs.register_keybind(KeyBind::new(
    ///         PhysicalKey::Code(KeyCode::KeyW),
    ///         KeyAction::Hold,
    ///         "Forwards",
    ///     ));
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn register_keybind(&mut self, key: KeyBind) {
        log!("registering keybind: {}", key.name.clone());
        if self.keybinds.contains_key(&key.name) {
            log_warn!("Keybinding {} already exists", key.name);
            return;
        }
        self.keybinds.insert(key.name.clone(), key);
        // self.serialize_input_manager().unwrap();
    }

    pub fn register_mousebind(&mut self, key: MouseBind) {
        log!("registering mousebind: {}", key.name.clone());
        if self.mouse_keybinds.contains_key(&key.name) {
            log_warn!("Mouse binding {} already exists", key.name);
            return;
        }
        self.mouse_keybinds.insert(key.name.clone(), key);
        // self.serialize_input_manager().unwrap();
    }

    /// Detects if a keybind with the specified name is active
    pub fn is_keybind_active(&self, name: &str) -> bool {
        let key = self.keybinds.get(name);
        if key.is_none() {
            log_warn!("Key: {} does not exist", name.to_string());
            return false;
        }
        let key = key.unwrap();
        match key.action {
            KeyAction::Press => self.keys_pressed.contains(&key.key),
            KeyAction::Release => self.keys_released.contains(&key.key),
            KeyAction::Hold => self.keys_held.contains(&key.key),
        }
    }

    /// Detects if a mousebind with the specified name is active
    pub fn is_mousebind_active(&self, name: &str) -> bool {
        let key = self.mouse_keybinds.get(name);
        if key.is_none() {
            return false;
        }
        let key = key.unwrap();
        match key.action {
            KeyAction::Press => self.mouse_pressed.contains(&key.key),
            KeyAction::Release => self.mouse_released.contains(&key.key),
            KeyAction::Hold => self.mouse_held.contains(&key.key),
        }
    }

    pub fn input_vector_2d(&self, left: &str, right: &str, up: &str, down: &str) -> Vector2<f32> {
        let mut x = 0.0;
        let mut y = 0.0;
        if self.is_keybind_active(left) {
            x += 1.0;
        }
        if self.is_keybind_active(right) {
            x -= 1.0;
        }
        if self.is_keybind_active(up) {
            y += 1.0;
        }
        if self.is_keybind_active(down) {
            y -= 1.0;
        }
        Vector2::new(x, y)
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
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        if self.is_keybind_active(x_pos) {
            x += 1.0;
        }
        if self.is_keybind_active(x_neg) {
            x -= 1.0;
        }
        if self.is_keybind_active(y_pos) {
            y += 1.0;
        }
        if self.is_keybind_active(y_neg) {
            y -= 1.0;
        }
        if self.is_keybind_active(z_pos) {
            z += 1.0;
        }
        if self.is_keybind_active(z_neg) {
            z -= 1.0;
        }
        Vector3::new(x, y, z)
    }

    pub fn handle_device_event(&mut self, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            self.mouse_delta = delta;
        }
    }

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
            WindowEvent::CursorMoved { position, .. } => {
                let delta = (
                    position.x - self.mouse_position.x,
                    position.y - self.mouse_position.y,
                );
                self.mouse_delta = delta;
                self.mouse_position = position;
            }
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(x, y) => {
                    self.scroll_delta = (x, y);
                }
                _ => {}
            },
            _ => {}
        }
    }
    // pub fn serialize_input_manager(&self) -> Result<(), std::io::Error> {
    //     let keybinds = self.serialize_bindings().unwrap();
    //     let path = format!("{}/{}.yaml", "res/", "input_manager");
    //     if !Path::new(&path).exists() {
    //         std::fs::create_dir_all("res/")?;
    //     }
    //     std::fs::write(path, keybinds)
    // }
    //
    // pub fn deserialize_input_manager(&mut self) -> Result<(), std::io::Error> {
    //     let path = format!("{}/{}.yaml", "res/", "input_manager");
    //
    //     let contents = std::fs::read_to_string(path)?;
    //
    //     let (key_bindings, mouse_bindings) = self
    //         .deserialize_bindings(&contents)
    //         .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    //
    //     self.keybinds = key_bindings;
    //     self.mouse_keybinds = mouse_bindings;
    //
    //     Ok(())
    // }

    // pub fn serialize_bindings(&self) -> Result<String, serde_yaml::Error> {
    //     let key_binds: Vec<serde_yaml::Value> = self
    //         .keybinds
    //         .iter()
    //         .map(|(name, bind)| {
    //             serde_yaml::to_value(serde_yaml::Mapping::from_iter([
    //                 (
    //                     serde_yaml::Value::String("name".into()),
    //                     serde_yaml::to_value(name).unwrap(),
    //                 ),
    //                 (
    //                     serde_yaml::Value::String("bind".into()),
    //                     serde_yaml::to_value(bind).unwrap(),
    //                 ),
    //             ]))
    //             .unwrap()
    //         })
    //         .collect();
    //
    //     let mouse_binds: Vec<serde_yaml::Value> = self
    //         .mouse_keybinds
    //         .iter()
    //         .map(|(name, bind)| {
    //             serde_yaml::to_value(serde_yaml::Mapping::from_iter([
    //                 (
    //                     serde_yaml::Value::String("name".into()),
    //                     serde_yaml::to_value(name).unwrap(),
    //                 ),
    //                 (
    //                     serde_yaml::Value::String("bind".into()),
    //                     serde_yaml::to_value(bind).unwrap(),
    //                 ),
    //             ]))
    //             .unwrap()
    //         })
    //         .collect();
    //
    //     let mut output = serde_yaml::Mapping::new();
    //     output.insert(
    //         serde_yaml::Value::String("key_bindings".into()),
    //         serde_yaml::to_value(key_binds).unwrap(),
    //     );
    //     output.insert(
    //         serde_yaml::Value::String("mouse_bindings".into()),
    //         serde_yaml::to_value(mouse_binds).unwrap(),
    //     );
    //
    //     serde_yaml::to_string(&output)
    // }
    //
    // pub fn deserialize_bindings(
    //     &self,
    //     contents: &str,
    // ) -> Result<(HashMap<String, KeyBind>, HashMap<String, MouseBind>), serde_yaml::Error> {
    //     let raw: serde_yaml::Value = serde_yaml::from_str(contents)?;
    //
    //     let key_bindings = raw["key_bindings"]
    //         .as_sequence()
    //         .map(|seq| {
    //             seq.iter()
    //                 .filter_map(|entry| {
    //                     let name = entry["name"].as_str()?.to_string();
    //                     let bind = match serde_yaml::from_value::<KeyBind>(entry["bind"].clone()) {
    //                         Ok(b) => b,
    //                         Err(e) => {
    //                             eprintln!("failed to deserialize KeyBind: {e}");
    //                             return None;
    //                         }
    //                     };
    //                     Some((name, bind))
    //                 })
    //                 .collect()
    //         })
    //         .unwrap_or_default();
    //
    //     let mouse_bindings = raw["mouse_bindings"]
    //         .as_sequence()
    //         .map(|seq| {
    //             seq.iter()
    //                 .filter_map(|entry| {
    //                     let name = entry["name"].as_str()?.to_string();
    //                     let bind = match serde_yaml::from_value::<MouseBind>(entry["bind"].clone())
    //                     {
    //                         Ok(b) => b,
    //                         Err(e) => {
    //                             eprintln!("failed to deserialize MouseBind: {e}");
    //                             return None;
    //                         }
    //                     };
    //                     Some((name, bind))
    //                 })
    //                 .collect()
    //         })
    //         .unwrap_or_default();
    //
    //     Ok((key_bindings, mouse_bindings))
    // }
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
