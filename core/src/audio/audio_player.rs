use anyhow::Result;
use apostasy_macros::Component;
use kira::{AudioManager, AudioManagerSettings, DefaultBackend};

use crate::audio::Audio;
use crate::audio::sound::Sound;
use crate::ecs::components::Inspect;
use crate::ecs::world::World;
use crate::egui::{DragAndDrop, StrokeKind};
use crate::ui::{DRAG_SIZE, LABEL_WIDTH};
use apostasy_macros::update;

#[derive(Clone, Component, Debug, Default)]
pub struct AudioPlayer {
    pub audio: Vec<Sound>,
    #[doc(hidden)]
    pub pending_play: Option<usize>,
}

impl Inspect for AudioPlayer {
    fn inspect(&mut self, ui: &mut crate::egui::Ui) {
        let row_h = DRAG_SIZE.y;
        let mut to_remove: Option<usize> = None;
        let mut to_reload: Option<usize> = None;

        let has_audio_drag = DragAndDrop::has_payload_of_type::<String>(ui.ctx());

        for (i, sound) in self.audio.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                let salt = format!("salt {}", i);
                ui.indent(salt, |ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [LABEL_WIDTH, row_h],
                                crate::egui::Label::new(format!("Sound {}", i)),
                            );
                            let path_w = ui.available_width()
                                - 24.0 * 2.0
                                - ui.spacing().item_spacing.x * 2.0;
                            let resp = ui.add_sized(
                                [path_w, row_h],
                                crate::egui::TextEdit::singleline(&mut sound.path)
                                    .hint_text("drag audio here or type a path…"),
                            );
                            if resp.lost_focus() && resp.changed() {
                                to_reload = Some(i);
                            }
                            // Drag and drop highlight
                            if has_audio_drag && ui.rect_contains_pointer(resp.rect) {
                                ui.painter().rect_stroke(
                                    resp.rect.expand(2.0),
                                    3.0,
                                    crate::egui::Stroke::new(
                                        2.0,
                                        crate::egui::Color32::from_rgb(100, 180, 255),
                                    ),
                                    StrokeKind::Outside,
                                );
                            }
                            if let Some(payload) = resp.dnd_release_payload::<String>()
                                && let Some(path) = payload.strip_prefix("audio:")
                            {
                                sound.path = path.to_string();
                                to_reload = Some(i);
                            }

                            if ui
                                .add_sized(
                                    [24.0, row_h],
                                    crate::egui::Button::new("▶").sense(if sound.data.is_some() {
                                        crate::egui::Sense::click()
                                    } else {
                                        crate::egui::Sense::hover()
                                    }),
                                )
                                .on_hover_text(if sound.data.is_some() {
                                    "Play sound"
                                } else {
                                    "Load a file first"
                                })
                                .clicked()
                                && sound.data.is_some()
                            {
                                self.pending_play = Some(i);
                            }
                            if ui
                                .add_sized([24.0, row_h], crate::egui::Button::new("✕"))
                                .clicked()
                            {
                                to_remove = Some(i);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [LABEL_WIDTH, row_h],
                                crate::egui::Label::new("Volume (dB)"),
                            );
                            ui.add(
                                crate::egui::Slider::new(&mut sound.volume, -60.0_f32..=6.0_f32)
                                    .clamping(egui::SliderClamping::Always),
                            );
                        });
                    });
                    ui.separator();
                });

                let loaded = sound.data.is_some();

                if !loaded && !sound.path.is_empty() {
                    ui.label(
                        crate::egui::RichText::new("⚠ Not loaded - press Enter or unfocus path")
                            .small()
                            .color(crate::egui::Color32::YELLOW),
                    );
                } else if !loaded {
                    ui.label(crate::egui::RichText::new("No file set").small().weak());
                }
            });
            ui.add_space(4.0);
        }

        if let Some(i) = to_reload
            && let Err(e) = self.audio[i].reload()
        {
            crate::log_warn!(
                "AudioPlayer: failed to load '{}': {}",
                self.audio[i].path,
                e
            );
        }
        if let Some(i) = to_remove {
            self.audio.remove(i);
        }

        if ui.button("+ Add Sound").clicked() {
            self.audio.push(Sound::default());
        }
    }
}

#[update(mode = "all")]
pub fn audio_player_pending(world: &mut World) -> Result<()> {
    // Collect pending play requests first to avoid borrow conflicts.
    let requests: Vec<(crate::worldspaces::cell::EntityId, usize)> = world
        .get_entities_with_component::<AudioPlayer>()
        .into_iter()
        .filter_map(|id| {
            world
                .get_component::<AudioPlayer>(id)
                .and_then(|p| p.pending_play.map(|idx| (id, idx)))
        })
        .collect();

    if requests.is_empty() {
        return Ok(());
    }

    // Ensure Audio resource exists.
    if !world.has_resource::<Audio>() {
        match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(mgr) => {
                world.insert_resource(Audio {
                    manager: std::sync::Arc::new(std::sync::Mutex::new(mgr)),
                });
            }
            Err(e) => {
                crate::log_warn!("AudioPlayer: failed to create AudioManager: {}", e);
                // Clear all pending_play so we don't retry every frame.
                for (id, _) in &requests {
                    if let Some(p) = world.get_component_mut::<AudioPlayer>(*id) {
                        p.pending_play = None;
                    }
                }
                return Ok(());
            }
        }
    }

    let manager_arc = world.get_resource::<Audio>()?.manager.clone();

    for (id, idx) in requests {
        if let Ok(mut mgr) = manager_arc.lock()
            && let Some(player) = world.get_component::<AudioPlayer>(id)
            && let Err(e) = player.play(idx, &mut mgr)
        {
            crate::log_warn!("AudioPlayer: play failed: {}", e);
        }
        if let Some(player) = world.get_component_mut::<AudioPlayer>(id) {
            player.pending_play = None;
        }
    }

    Ok(())
}

impl AudioPlayer {
    pub fn deserialize(&mut self, _value: &serde_yaml::Value) -> Result<()> {
        Ok(())
    }

    /// Plays a sound from the AudioPlayer by index. Returns an error if the sound has no loaded data.
    pub fn play(&self, index: usize, manager: &mut AudioManager) -> Result<()> {
        let sound = &self.audio[index];
        let data = sound
            .data
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Sound at index {} has no loaded data", index))?
            .volume(kira::Decibels(sound.volume));
        manager.play(data)?;
        Ok(())
    }
}
