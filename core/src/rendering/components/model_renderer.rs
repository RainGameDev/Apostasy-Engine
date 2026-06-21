use apostasy_macros::Component;

use crate::{
    egui::{self, DragAndDrop, StrokeKind},
    ecs::component::Inspect,
    rendering::shared::model::GpuModel,
    ui::LABEL_WIDTH,
};

#[derive(Component, Clone, Debug)]
pub struct ModelRenderer {
    pub model: Option<Box<GpuModel>>,
    pub model_path: String,
    pub material_override: Option<String>,
    pub is_wireframe: bool,
}

impl Default for ModelRenderer {
    fn default() -> Self {
        Self {
            model: None,
            model_path: "cube".to_string(),
            material_override: None,
            is_wireframe: false,
        }
    }
}

impl Inspect for ModelRenderer {
    fn inspect(&mut self, ui: &mut egui::Ui) {
        let row_h = 20.0;

        let has_model_drag = DragAndDrop::has_payload_of_type::<String>(ui.ctx());
        let inner = ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Model"));
            ui.add_sized(
                [ui.available_width(), row_h],
                egui::TextEdit::singleline(&mut self.model_path).hint_text("drag a model here…"),
            )
        });
        let te_resp = inner.inner;
        let is_hovering = has_model_drag && ui.rect_contains_pointer(te_resp.rect);
        if is_hovering {
            ui.painter().rect_stroke(
                te_resp.rect.expand(2.0),
                3.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 200, 140)),
                StrokeKind::Outside,
            );
        }
        if let Some(payload) = te_resp.dnd_release_payload::<String>() {
            if let Some(name) = payload.strip_prefix("model:") {
                self.model_path = name.to_string();
                self.model = None;
            }
        }

        let mut override_buf = self.material_override.clone().unwrap_or_default();
        let has_mat_drag = DragAndDrop::has_payload_of_type::<String>(ui.ctx());
        let mat_inner = ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Material"));
            ui.add(
                egui::TextEdit::singleline(&mut override_buf)
                    .desired_width(ui.available_width())
                    .hint_text("drag a material or type name…"),
            )
        });
        let mat_resp = mat_inner.inner;
        let mat_hovering = has_mat_drag && ui.rect_contains_pointer(mat_resp.rect);
        if mat_hovering {
            ui.painter().rect_stroke(
                mat_resp.rect.expand(2.0),
                3.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(200, 140, 80)),
                StrokeKind::Outside,
            );
        }
        if let Some(payload) = mat_resp.dnd_release_payload::<String>() {
            // Payload is "namespace:Material:name" — extract just the name.
            let name = (*payload)
                .splitn(3, ':')
                .nth(2)
                .unwrap_or(&*payload)
                .to_string();
            self.material_override = if name.is_empty() { None } else { Some(name) };
        } else if mat_resp.middle_clicked() {
            self.material_override = None;
        } else if mat_resp.changed() {
            self.material_override = if override_buf.trim().is_empty() {
                None
            } else {
                Some(override_buf.trim().to_string())
            };
        }

        ui.horizontal(|ui| {
            ui.add_sized([LABEL_WIDTH, row_h], egui::Label::new("Wireframe"));
            ui.checkbox(&mut self.is_wireframe, "");
        });
    }
}

impl ModelRenderer {
    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> anyhow::Result<()> {
        if let Some(v) = value.get("model_path").and_then(|v| v.as_str()) {
            self.model_path = v.to_string();
        }
        if let Some(v) = value.get("material_override").and_then(|v| v.as_str()) {
            self.material_override = if v.is_empty() {
                None
            } else {
                Some(v.to_string())
            };
        }

        if let Some(v) = value.get("is_wireframe").and_then(|v| v.as_bool()) {
            self.is_wireframe = v;
        }
        Ok(())
    }

    pub fn from_path(path: &str) -> Self {
        Self {
            model: None,
            model_path: path.to_string(),
            material_override: None,
            is_wireframe: false,
        }
    }
}
