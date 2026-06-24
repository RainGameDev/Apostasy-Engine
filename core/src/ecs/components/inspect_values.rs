use cgmath::{Quaternion, Vector2, Vector3, Vector4};
use egui::emath::Numeric;

use crate::ui::{DRAG_SIZE, LABEL_WIDTH};

/// Lays out a fixed-width label, then the caller's widgets, on one row.
/// Returns whatever the widget closure reports (typically "was edited").
fn labelled(ui: &mut egui::Ui, text: &str, add: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized([LABEL_WIDTH, 20.0], egui::Label::new(text));
        changed = add(ui);
    });
    changed
}

/// Sensible default drag speed: whole steps for integers, fine steps for floats.
fn default_speed<N: Numeric>() -> f64 {
    if N::INTEGRAL { 1.0 } else { 0.1 }
}

/// A bare (unlabelled) drag field — the building block for the row helpers.
fn drag_field<N: Numeric>(ui: &mut egui::Ui, value: &mut N, speed: f64) -> bool {
    ui.add_sized(DRAG_SIZE, egui::DragValue::new(value).speed(speed))
        .changed()
}

// ========== generic numeric ==========

/// Labelled drag field for any numeric type, with an explicit drag speed.
pub fn drag<N: Numeric>(ui: &mut egui::Ui, text: &str, value: &mut N, speed: f64) -> bool {
    labelled(ui, text, |ui| drag_field(ui, value, speed))
}

/// Labelled drag field for any numeric type, using the [`default_speed`] for its kind.
pub fn number<N: Numeric>(ui: &mut egui::Ui, text: &str, value: &mut N) -> bool {
    drag(ui, text, value, default_speed::<N>())
}

// ========== concrete scalar helpers (all int/float variants) ==========

macro_rules! scalar_fns {
    ($($name:ident: $ty:ty),* $(,)?) => {
        $(
            #[doc = concat!("Labelled drag field for `", stringify!($ty), "`.")]
            pub fn $name(ui: &mut egui::Ui, text: &str, value: &mut $ty) -> bool {
                number(ui, text, value)
            }
        )*
    };
}

scalar_fns! {
    f32: f32,
    f64: f64,
    i8: i8,
    i16: i16,
    i32: i32,
    i64: i64,
    isize: isize,
    u8: u8,
    u16: u16,
    u32: u32,
    u64: u64,
    usize: usize,
}

// ========== non-numeric scalars ==========

/// Checkbox for a `bool`.
pub fn boolean(ui: &mut egui::Ui, text: &str, value: &mut bool) -> bool {
    labelled(ui, text, |ui| ui.checkbox(value, "").changed())
}

/// Single-line text field for a `String`.
pub fn text(ui: &mut egui::Ui, text_label: &str, value: &mut String) -> bool {
    labelled(ui, text_label, |ui| {
        ui.add_sized(DRAG_SIZE, egui::TextEdit::singleline(value))
            .changed()
    })
}

// ========== cgmath vectors / quaternion (generic over component type) ==========

/// A row of drag fields for the given numeric components, sharing one speed.
fn drag_components<N: Numeric>(ui: &mut egui::Ui, comps: &mut [&mut N]) -> bool {
    let speed = default_speed::<N>();
    let mut changed = false;
    for c in comps {
        changed |= drag_field(ui, *c, speed);
    }
    changed
}

/// `Vector2<N>` as two drag fields (x, y).
pub fn vec2<N: Numeric>(ui: &mut egui::Ui, text: &str, v: &mut Vector2<N>) -> bool {
    labelled(ui, text, |ui| {
        drag_components(ui, &mut [&mut v.x, &mut v.y])
    })
}

/// `Vector3<N>` as three drag fields (x, y, z).
pub fn vec3<N: Numeric>(ui: &mut egui::Ui, text: &str, v: &mut Vector3<N>) -> bool {
    labelled(ui, text, |ui| {
        drag_components(ui, &mut [&mut v.x, &mut v.y, &mut v.z])
    })
}

/// `Vector4<N>` as four drag fields (x, y, z, w).
pub fn vec4<N: Numeric>(ui: &mut egui::Ui, text: &str, v: &mut Vector4<N>) -> bool {
    labelled(ui, text, |ui| {
        drag_components(ui, &mut [&mut v.x, &mut v.y, &mut v.z, &mut v.w])
    })
}

/// `Quaternion<N>` as four drag fields laid out (x, y, z, w).
/// Editing a raw quaternion is rarely intuitive — prefer euler angles for
/// rotations the user authors directly.
pub fn quat<N: Numeric>(ui: &mut egui::Ui, text: &str, q: &mut Quaternion<N>) -> bool {
    labelled(ui, text, |ui| {
        drag_components(ui, &mut [&mut q.v.x, &mut q.v.y, &mut q.v.z, &mut q.s])
    })
}

// ========== colours ==========

/// RGB colour stored as `Vector3<f32>` (0..=1), edited with a colour picker.
pub fn color_rgb(ui: &mut egui::Ui, text: &str, v: &mut Vector3<f32>) -> bool {
    labelled(ui, text, |ui| {
        let mut rgb = [v.x, v.y, v.z];
        if ui.color_edit_button_rgb(&mut rgb).changed() {
            *v = Vector3::new(rgb[0], rgb[1], rgb[2]);
            true
        } else {
            false
        }
    })
}

/// RGBA colour stored as `Vector4<f32>` (0..=1, unmultiplied alpha).
pub fn color_rgba(ui: &mut egui::Ui, text: &str, v: &mut Vector4<f32>) -> bool {
    labelled(ui, text, |ui| {
        let mut rgba = [v.x, v.y, v.z, v.w];
        if ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed() {
            *v = Vector4::new(rgba[0], rgba[1], rgba[2], rgba[3]);
            true
        } else {
            false
        }
    })
}

/// An egui `Color32` (sRGBA, 0..=255).
pub fn color32(ui: &mut egui::Ui, text: &str, c: &mut egui::Color32) -> bool {
    labelled(ui, text, |ui| ui.color_edit_button_srgba(c).changed())
}
