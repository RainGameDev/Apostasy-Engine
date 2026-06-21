use apostasy_core::{
    egui,
    ecs::resources::input_manager::{InputManager, KeyAction, KeyBind, ModifierKeys},
    winit::keyboard::{KeyCode, PhysicalKey},
};

use super::EditorStyle;

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub fn is_modifier_key(key: &PhysicalKey) -> bool {
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

pub fn pretty_key(key: &PhysicalKey) -> String {
    match key {
        PhysicalKey::Code(code) => match code {
            KeyCode::KeyA => "A",  KeyCode::KeyB => "B",  KeyCode::KeyC => "C",
            KeyCode::KeyD => "D",  KeyCode::KeyE => "E",  KeyCode::KeyF => "F",
            KeyCode::KeyG => "G",  KeyCode::KeyH => "H",  KeyCode::KeyI => "I",
            KeyCode::KeyJ => "J",  KeyCode::KeyK => "K",  KeyCode::KeyL => "L",
            KeyCode::KeyM => "M",  KeyCode::KeyN => "N",  KeyCode::KeyO => "O",
            KeyCode::KeyP => "P",  KeyCode::KeyQ => "Q",  KeyCode::KeyR => "R",
            KeyCode::KeyS => "S",  KeyCode::KeyT => "T",  KeyCode::KeyU => "U",
            KeyCode::KeyV => "V",  KeyCode::KeyW => "W",  KeyCode::KeyX => "X",
            KeyCode::KeyY => "Y",  KeyCode::KeyZ => "Z",
            KeyCode::Digit0 => "0", KeyCode::Digit1 => "1", KeyCode::Digit2 => "2",
            KeyCode::Digit3 => "3", KeyCode::Digit4 => "4", KeyCode::Digit5 => "5",
            KeyCode::Digit6 => "6", KeyCode::Digit7 => "7", KeyCode::Digit8 => "8",
            KeyCode::Digit9 => "9",
            KeyCode::F1  => "F1",  KeyCode::F2  => "F2",  KeyCode::F3  => "F3",
            KeyCode::F4  => "F4",  KeyCode::F5  => "F5",  KeyCode::F6  => "F6",
            KeyCode::F7  => "F7",  KeyCode::F8  => "F8",  KeyCode::F9  => "F9",
            KeyCode::F10 => "F10", KeyCode::F11 => "F11", KeyCode::F12 => "F12",
            KeyCode::ShiftLeft   | KeyCode::ShiftRight   => "Shift",
            KeyCode::ControlLeft | KeyCode::ControlRight => "Ctrl",
            KeyCode::AltLeft     | KeyCode::AltRight     => "Alt",
            KeyCode::Space     => "Space",
            KeyCode::Enter     => "Enter",
            KeyCode::Escape    => "Esc",
            KeyCode::Tab       => "Tab",
            KeyCode::Backspace => "Backspace",
            KeyCode::Delete    => "Delete",
            KeyCode::ArrowLeft  => "Left",
            KeyCode::ArrowRight => "Right",
            KeyCode::ArrowUp    => "Up",
            KeyCode::ArrowDown  => "Down",
            other => return format!("{other:?}"),
        }
        .to_string(),
        other => format!("{other:?}"),
    }
}

/// Human-readable representation of a full keybind combo, e.g. "Ctrl+C".
pub fn pretty_bind(bind: &KeyBind) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if bind.modifiers.ctrl  { parts.push("Ctrl"); }
    if bind.modifiers.alt   { parts.push("Alt"); }
    if bind.modifiers.shift { parts.push("Shift"); }
    let key = pretty_key(&bind.key);
    // Avoid doubling "Shift/Ctrl/Alt" for modifier-key binds themselves.
    if parts.iter().any(|p| key.eq_ignore_ascii_case(p)) {
        return key;
    }
    if parts.is_empty() { key } else { format!("{}+{}", parts.join("+"), key) }
}

// ─── KeybindCapture ──────────────────────────────────────────────────────────

/// Snapshot of InputManager state needed by keybind widgets.
/// Extract this once before your egui closure and pass it to each widget call.
pub struct KeybindCapture {
    /// Non-modifier, non-Escape keys that became pressed this frame.
    pub pressed: Vec<PhysicalKey>,
    /// Modifier keys held this frame.
    pub modifiers: ModifierKeys,
    /// Whether Escape was pressed this frame.
    pub escape: bool,
}

impl KeybindCapture {
    pub fn from_inputs(inputs: &InputManager) -> Self {
        let escape = inputs.keys_pressed.contains(&PhysicalKey::Code(KeyCode::Escape));
        let pressed = inputs
            .keys_pressed
            .iter()
            .filter(|k| {
                !is_modifier_key(k) && !matches!(*k, PhysicalKey::Code(KeyCode::Escape))
            })
            .copied()
            .collect();
        Self { pressed, modifiers: inputs.modifiers(), escape }
    }

    pub fn none() -> Self {
        Self { pressed: Vec::new(), modifiers: ModifierKeys::default(), escape: false }
    }
}

// ─── Widget ──────────────────────────────────────────────────────────────────

/// Clickable chip that captures a new keybinding.
///
/// Left-click to enter listening mode; the next non-modifier key press becomes the
/// proposed new binding. Right-click for a context menu with "Reset to default".
/// Press Escape to cancel listening.
///
/// Returns `Some(new_bind)` when the user captures a new binding or resets to default.
/// The caller is responsible for validating and applying the proposed bind.
pub fn keybind_editor(
    ui: &mut egui::Ui,
    id: egui::Id,
    current: Option<&KeyBind>,
    default: Option<&KeyBind>,
    capture: &KeybindCapture,
    style: &EditorStyle,
) -> Option<KeyBind> {
    let listening: bool = ui.memory(|m| m.data.get_temp::<bool>(id).unwrap_or(false));
    let mut result = None;

    if listening {
        if capture.escape {
            ui.memory_mut(|m| m.data.insert_temp(id, false));
        } else if let Some(&key) = capture.pressed.first() {
            let action = current.map(|b| b.action.clone()).unwrap_or(KeyAction::Press);
            let context = current.and_then(|b| b.context.clone());
            let mut new_bind = KeyBind::new(key, action);
            new_bind.modifiers = capture.modifiers.clone();
            new_bind.context = context;
            result = Some(new_bind);
            ui.memory_mut(|m| m.data.insert_temp(id, false));
        }
    }

    let label = if listening {
        let mut s = String::new();
        if capture.modifiers.ctrl  { s.push_str("Ctrl+"); }
        if capture.modifiers.alt   { s.push_str("Alt+"); }
        if capture.modifiers.shift { s.push_str("Shift+"); }
        s.push('_');
        s
    } else {
        current.map(pretty_bind).unwrap_or_else(|| "-".to_string())
    };

    let row_h = style.row_height();
    let chip_w = 120.0_f32;

    let (chip_rect, resp) =
        ui.allocate_exact_size(egui::Vec2::new(chip_w, row_h), egui::Sense::click());

    let was_clicked = resp.clicked();
    let is_hovered  = resp.hovered();

    let bg = if listening { style.sel_bg } else { style.hover_bg };
    ui.painter().rect_filled(chip_rect, 3.0, bg);

    if listening {
        ui.painter().rect_stroke(
            chip_rect,
            3.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 150, 230)),
            egui::StrokeKind::Inside,
        );
    } else if is_hovered {
        ui.painter().rect_stroke(
            chip_rect,
            3.0,
            egui::Stroke::new(1.0, style.div_col),
            egui::StrokeKind::Inside,
        );
    }

    ui.painter().text(
        chip_rect.center(),
        egui::Align2::CENTER_CENTER,
        &label,
        style.font_ui(),
        style.text_col,
    );

    if was_clicked && !listening {
        ui.memory_mut(|m| m.data.insert_temp(id, true));
    }

    if listening {
        resp.on_hover_text("Press Esc to cancel");
    } else {
        resp.context_menu(|ui| {
            let at_default = default
                .zip(current)
                .map(|(d, c)| d.key == c.key && d.modifiers == c.modifiers)
                .unwrap_or(default.is_none());

            if ui
                .add_enabled(!at_default, egui::Button::new("Reset to default"))
                .clicked()
            {
                if let Some(d) = default {
                    result = Some(d.clone());
                }
                ui.close();
            }
        });
    }

    result
}
