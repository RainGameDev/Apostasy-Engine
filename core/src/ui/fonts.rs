use apostasy_macros::Resource;
use egui::{Context, FontData, FontDefinitions, FontFamily};
use std::path::Path;
use std::sync::Arc;

#[derive(Resource, Clone)]
pub struct FontRegistry {
    pub fonts: Vec<String>,
    pub active: String,
    defs: FontDefinitions,
    needs_apply: bool,
}

impl Default for FontRegistry {
    fn default() -> Self {
        Self {
            fonts: Vec::new(),
            active: String::new(),
            defs: FontDefinitions::default(),
            needs_apply: false,
        }
    }
}

impl FontRegistry {
    /// Scan `dir` for .ttf / .otf / .ttc files and add them to the registry.
    /// The first font loaded becomes the active font if none is set yet.
    /// Does nothing if the directory does not exist.
    pub fn load_dir(&mut self, dir: &Path) {
        for (name, data) in scan_font_dir(dir) {
            self.defs
                .font_data
                .insert(name.clone(), Arc::new(FontData::from_owned(data)));
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                let list = self.defs.families.entry(family).or_default();
                if !list.contains(&name) {
                    list.push(name.clone());
                }
            }
            if !self.fonts.contains(&name) {
                self.fonts.push(name.clone());
            }
        }
        self.fonts.sort();
        if self.active.is_empty() {
            if let Some(first) = self.fonts.first() {
                self.active = first.clone();
            }
        }
        self.needs_apply = true;
    }

    pub fn set_active(&mut self, name: impl Into<String>) {
        let name = name.into();
        if name != self.active && self.fonts.contains(&name) {
            self.active = name;
            self.needs_apply = true;
        }
    }

    /// Apply font changes to the egui context if the active font changed since last call.
    pub fn apply_if_needed(&mut self, ctx: &Context) {
        if self.needs_apply {
            self.apply(ctx);
            self.needs_apply = false;
        }
    }

    fn apply(&self, ctx: &Context) {
        let mut defs = self.defs.clone();
        for list in defs.families.values_mut() {
            if let Some(pos) = list.iter().position(|n| n == &self.active) {
                list.remove(pos);
                list.insert(0, self.active.clone());
            }
        }
        ctx.set_fonts(defs);
    }
}

fn scan_font_dir(dir: &Path) -> Vec<(String, Vec<u8>)> {
    if !dir.exists() {
        return Vec::new();
    }
    let mut fonts = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("ttf" | "otf" | "ttc")
            ) {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if let Ok(data) = std::fs::read(&path) {
                    fonts.push((name, data));
                }
            }
        }
    }
    fonts.sort_by(|a, b| a.0.cmp(&b.0));
    fonts
}
