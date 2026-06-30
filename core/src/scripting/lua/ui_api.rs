use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use egui::Color32;
use mlua::{UserData, UserDataMethods};

/// Wraps a raw `egui::Ui` pointer for the duration of a Lua UI callback.
/// `valid` is set to `false` once the enclosing window/layout closure returns,
/// so stale handles silently no-op rather than causing UB.
pub struct UiHandle {
    pub(crate) ui: *mut egui::Ui,
    pub(crate) valid: Arc<AtomicBool>,
}

impl UiHandle {
    fn ui(&self) -> Option<&mut egui::Ui> {
        if self.valid.load(Ordering::Relaxed) {
            Some(unsafe { &mut *self.ui })
        } else {
            None
        }
    }
}

/// Creates a child `UiHandle`, calls `func` with it, then invalidates the
/// handle. Used for layout containers (horizontal, vertical, collapsing).
pub(crate) fn invoke_with_ui(lua: &mlua::Lua, ui: &mut egui::Ui, func: &mlua::Function) {
    let valid = Arc::new(AtomicBool::new(true));
    let ui_ptr: *mut egui::Ui = ui;
    match lua.create_userdata(UiHandle {
        ui: ui_ptr,
        valid: valid.clone(),
    }) {
        Ok(h) => {
            if let Err(e) = func.call::<()>(h) {
                crate::log_error!("[lua] ui callback: {e}");
            }
        }
        Err(e) => {
            crate::log_error!("[lua] ui: create handle failed: {e}");
        }
    }
    valid.store(false, Ordering::Relaxed);
}

impl UserData for UiHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // -- text -------------------------------------------------------------

        methods.add_method("label", |_, this, text: String| {
            if let Some(ui) = this.ui() {
                ui.label(text);
            }
            Ok(())
        });

        methods.add_method("heading", |_, this, text: String| {
            if let Some(ui) = this.ui() {
                ui.heading(text);
            }
            Ok(())
        });

        methods.add_method("small", |_, this, text: String| {
            if let Some(ui) = this.ui() {
                ui.small(text);
            }
            Ok(())
        });

        // colored_label({r,g,b,a}, "text") — components are 0..255 integers.
        methods.add_method(
            "colored_label",
            |_, this, (color, text): (mlua::Table, String)| {
                if let Some(ui) = this.ui() {
                    let ch = |k: &str, i: i64| {
                        color
                            .get::<u8>(k)
                            .or_else(|_| color.get::<u8>(i))
                            .unwrap_or(255)
                    };
                    ui.colored_label(
                        Color32::from_rgba_premultiplied(
                            ch("r", 1),
                            ch("g", 2),
                            ch("b", 3),
                            ch("a", 4),
                        ),
                        text,
                    );
                }
                Ok(())
            },
        );

        // -- interactive widgets -----------------------------------------------

        // Returns true on the frame it is clicked.
        methods.add_method("button", |_, this, text: String| {
            Ok(this.ui().is_some_and(|ui| ui.button(text).clicked()))
        });

        // checkbox("label", value) → new_value
        methods.add_method("checkbox", |_, this, (text, value): (String, bool)| {
            let Some(ui) = this.ui() else {
                return Ok(value);
            };
            let mut v = value;
            ui.checkbox(&mut v, text);
            Ok(v)
        });

        // slider("label", value, min, max) → new_value (f64)
        methods.add_method(
            "slider",
            |_, this, (text, value, min, max): (String, f64, f64, f64)| {
                let Some(ui) = this.ui() else {
                    return Ok(value);
                };
                let mut v = value;
                ui.add(egui::Slider::new(&mut v, min..=max).text(text));
                Ok(v)
            },
        );

        // drag("label", value, speed?) → new_value (f64). speed defaults to 1.0.
        methods.add_method(
            "drag",
            |_, this, (text, value, speed): (String, f64, Option<f64>)| {
                let Some(ui) = this.ui() else {
                    return Ok(value);
                };
                let mut v = value;
                ui.add(
                    egui::DragValue::new(&mut v)
                        .speed(speed.unwrap_or(1.0))
                        .prefix(format!("{text}: ")),
                );
                Ok(v)
            },
        );

        // text_input("label", current_text) → new_text
        // Label and text field are placed on the same row.
        methods.add_method("text_input", |_, this, (label, text): (String, String)| {
            let Some(ui) = this.ui() else {
                return Ok(text);
            };
            let mut s = text;
            ui.horizontal(|ui| {
                if !label.is_empty() {
                    ui.label(&label);
                }
                ui.text_edit_singleline(&mut s);
            });
            Ok(s)
        });

        // combo_box("label", {"A","B","C"}, selected) → new_selected (1-based)
        methods.add_method(
            "combo_box",
            |_, this, (label, options, selected): (String, mlua::Table, usize)| {
                let Some(ui) = this.ui() else {
                    return Ok(selected);
                };
                let count = options.raw_len() as usize;
                let items: Vec<String> = (1..=count)
                    .map(|i| options.get::<String>(i as i64).unwrap_or_default())
                    .collect();
                if items.is_empty() {
                    return Ok(selected);
                }
                let mut idx = selected.saturating_sub(1).min(items.len() - 1);
                egui::ComboBox::from_label(&label)
                    .show_index(ui, &mut idx, items.len(), |i| items[i].clone());
                Ok(idx + 1)
            },
        );

        // progress_bar(fraction) — fraction in 0.0..=1.0.
        methods.add_method("progress_bar", |_, this, fraction: f32| {
            if let Some(ui) = this.ui() {
                ui.add(egui::ProgressBar::new(fraction.clamp(0.0, 1.0)));
            }
            Ok(())
        });

        // -- layout containers -------------------------------------------------

        // horizontal(callback) — lay out widgets in a row.
        methods.add_method("horizontal", |lua, this, func: mlua::Function| {
            let Some(ui) = this.ui() else {
                return Ok(());
            };
            ui.horizontal(|ui| invoke_with_ui(lua, ui, &func));
            Ok(())
        });

        // vertical(callback) — explicit vertical stack (same as default but
        // useful inside a horizontal layout to re-enter vertical flow).
        methods.add_method("vertical", |lua, this, func: mlua::Function| {
            let Some(ui) = this.ui() else {
                return Ok(());
            };
            ui.vertical(|ui| invoke_with_ui(lua, ui, &func));
            Ok(())
        });

        // columns(n, callback) — split into n equal columns.
        // callback receives a 1-indexed table of UiHandles: callback(cols).
        methods.add_method(
            "columns",
            |lua, this, (n, func): (usize, mlua::Function)| {
                let Some(ui) = this.ui() else {
                    return Ok(());
                };
                let n = n.max(1);
                ui.columns(n, |cols| {
                    let valid = Arc::new(AtomicBool::new(true));
                    let Ok(table) = lua.create_table() else {
                        return;
                    };
                    for (i, col_ui) in cols.iter_mut().enumerate() {
                        let ui_ptr: *mut egui::Ui = col_ui;
                        if let Ok(h) = lua.create_userdata(UiHandle {
                            ui: ui_ptr,
                            valid: valid.clone(),
                        }) {
                            let _ = table.set(i + 1, h);
                        }
                    }
                    if let Err(e) = func.call::<()>(table) {
                        crate::log_error!("[lua] ui:columns callback: {e}");
                    }
                    valid.store(false, Ordering::Relaxed);
                });
                Ok(())
            },
        );

        // collapsing("heading", callback) — collapsible section.
        methods.add_method(
            "collapsing",
            |lua, this, (label, func): (String, mlua::Function)| {
                let Some(ui) = this.ui() else {
                    return Ok(());
                };
                ui.collapsing(label, |ui| invoke_with_ui(lua, ui, &func));
                Ok(())
            },
        );

        // -- misc --------------------------------------------------------------

        methods.add_method("separator", |_, this, ()| {
            if let Some(ui) = this.ui() {
                ui.separator();
            }
            Ok(())
        });

        methods.add_method("space", |_, this, amount: f32| {
            if let Some(ui) = this.ui() {
                ui.add_space(amount);
            }
            Ok(())
        });
    }
}
