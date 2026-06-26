use anyhow::Result;
use apostasy_macros::Component;
use kira::sound::static_sound::StaticSoundHandle;
use kira::{Decibels, Panning, PlaybackRate, Tween};

use crate::audio::Audio;
use crate::audio::layers::AudioLayer;
use crate::audio::sound::Sound;
use crate::ecs::components::Inspect;
use crate::ecs::world::World;
use crate::egui::{DragAndDrop, StrokeKind};
use crate::rendering::components::camera::{ActiveCamera, EditorCamera};
use crate::ui::{DRAG_SIZE, LABEL_WIDTH};
use apostasy_macros::update;

#[derive(Clone, Component, Debug, Default)]
pub struct AudioPlayer {
    pub audio: Vec<Sound>,
    #[doc(hidden)]
    pub pending_play: Option<usize>,
    #[doc(hidden)]
    pub pending_stop: Option<usize>,
    #[doc(hidden)]
    pub pending_pause: Option<usize>,
    #[doc(hidden)]
    pub pending_resume: Option<usize>,
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
                                - 24.0 * 3.0
                                - ui.spacing().item_spacing.x * 3.0;
                            let resp = ui.add_sized(
                                [path_w, row_h],
                                crate::egui::TextEdit::singleline(&mut sound.path)
                                    .hint_text("drag audio here or type a path…"),
                            );
                            if resp.lost_focus() && resp.changed() {
                                to_reload = Some(i);
                            }
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

                            let is_playing = sound.is_playing();
                            let is_paused = sound.is_paused();
                            let has_data = sound.data.is_some();

                            // Play / pause / resume button
                            let (play_label, play_hover, play_clickable) = if is_playing {
                                ("⏸", "Pause", true)
                            } else if is_paused {
                                ("▶", "Resume", true)
                            } else if has_data {
                                ("▶", "Play sound", true)
                            } else {
                                ("▶", "Load a file first", false)
                            };

                            if ui
                                .add_sized(
                                    [24.0, row_h],
                                    crate::egui::Button::new(play_label).sense(if play_clickable {
                                        crate::egui::Sense::click()
                                    } else {
                                        crate::egui::Sense::hover()
                                    }),
                                )
                                .on_hover_text(play_hover)
                                .clicked()
                                && play_clickable
                            {
                                if is_playing {
                                    self.pending_pause = Some(i);
                                } else if is_paused {
                                    self.pending_resume = Some(i);
                                } else {
                                    self.pending_play = Some(i);
                                }
                            }

                            // Stop button (only when active)
                            let stop_clickable = is_playing || is_paused;
                            if ui
                                .add_sized(
                                    [24.0, row_h],
                                    crate::egui::Button::new("⏹").sense(if stop_clickable {
                                        crate::egui::Sense::click()
                                    } else {
                                        crate::egui::Sense::hover()
                                    }),
                                )
                                .on_hover_text(if stop_clickable { "Stop" } else { "Not playing" })
                                .clicked()
                                && stop_clickable
                            {
                                self.pending_stop = Some(i);
                            }

                            if ui
                                .add_sized([24.0, row_h], crate::egui::Button::new("-"))
                                .clicked()
                            {
                                to_remove = Some(i);
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, row_h], crate::egui::Label::new("Layer"));
                            crate::egui::ComboBox::from_id_salt(format!("audio_layer_{}_{}", i, i))
                                .selected_text(sound.layer.as_str())
                                .show_ui(ui, |ui| {
                                    for &layer in AudioLayer::ALL {
                                        ui.selectable_value(
                                            &mut sound.layer,
                                            layer,
                                            layer.as_str(),
                                        );
                                    }
                                });
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
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, row_h], crate::egui::Label::new("Pitch"));
                            ui.add(
                                crate::egui::Slider::new(&mut sound.pitch, 0.1_f32..=4.0_f32)
                                    .clamping(egui::SliderClamping::Always),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [LABEL_WIDTH, row_h],
                                crate::egui::Label::new("Fade In (s)"),
                            );
                            ui.add(
                                crate::egui::Slider::new(&mut sound.fade_in, 0.0_f32..=10.0_f32)
                                    .clamping(egui::SliderClamping::Always),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [LABEL_WIDTH, row_h],
                                crate::egui::Label::new("Fade Out (s)"),
                            );
                            ui.add(
                                crate::egui::Slider::new(&mut sound.fade_out, 0.0_f32..=10.0_f32)
                                    .clamping(egui::SliderClamping::Always),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, row_h], crate::egui::Label::new("Loop"));
                            ui.checkbox(&mut sound.looping, "");
                        });
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [LABEL_WIDTH, row_h],
                                crate::egui::Label::new("Auto Play"),
                            );
                            ui.checkbox(&mut sound.auto_play, "");
                        });
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, row_h], crate::egui::Label::new("Spatial"));
                            ui.checkbox(&mut sound.spatial, "");
                        });
                        if sound.spatial {
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [LABEL_WIDTH, row_h],
                                    crate::egui::Label::new("Max Distance"),
                                );
                                ui.add(
                                    crate::egui::Slider::new(
                                        &mut sound.max_distance,
                                        0.1_f32..=500.0_f32,
                                    )
                                    .clamping(egui::SliderClamping::Always),
                                );
                            });
                        }
                    });
                    ui.separator();
                });

                let loaded = sound.data.is_some();
                if !loaded && !sound.path.is_empty() {
                    ui.label(
                        crate::egui::RichText::new("⚠ Not loaded — press Enter or unfocus path")
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
    let play_requests: Vec<(crate::worldspaces::cell::EntityId, usize)> = world
        .get_entities_with_component::<AudioPlayer>()
        .into_iter()
        .filter_map(|id| {
            world
                .get_component::<AudioPlayer>(id)
                .and_then(|p| p.pending_play.map(|idx| (id, idx)))
        })
        .collect();

    let stop_requests: Vec<(crate::worldspaces::cell::EntityId, usize)> = world
        .get_entities_with_component::<AudioPlayer>()
        .into_iter()
        .filter_map(|id| {
            world
                .get_component::<AudioPlayer>(id)
                .and_then(|p| p.pending_stop.map(|idx| (id, idx)))
        })
        .collect();

    let pause_requests: Vec<(crate::worldspaces::cell::EntityId, usize)> = world
        .get_entities_with_component::<AudioPlayer>()
        .into_iter()
        .filter_map(|id| {
            world
                .get_component::<AudioPlayer>(id)
                .and_then(|p| p.pending_pause.map(|idx| (id, idx)))
        })
        .collect();

    let resume_requests: Vec<(crate::worldspaces::cell::EntityId, usize)> = world
        .get_entities_with_component::<AudioPlayer>()
        .into_iter()
        .filter_map(|id| {
            world
                .get_component::<AudioPlayer>(id)
                .and_then(|p| p.pending_resume.map(|idx| (id, idx)))
        })
        .collect();

    if play_requests.is_empty()
        && stop_requests.is_empty()
        && pause_requests.is_empty()
        && resume_requests.is_empty()
    {
        return Ok(());
    }

    // Handle stops — no AudioManager needed.
    for (id, idx) in &stop_requests {
        if let Some(player) = world.get_component::<AudioPlayer>(*id)
            && let Some(sound) = player.audio.get(*idx)
            && let Ok(mut guard) = sound.handle.lock()
        {
            if let Some(h) = guard.as_mut() {
                let tween = if sound.fade_out > 0.0 {
                    Tween {
                        duration: std::time::Duration::from_secs_f32(sound.fade_out),
                        ..Default::default()
                    }
                } else {
                    Tween::default()
                };
                h.stop(tween);
            }
            *guard = None;
        }
        if let Some(player) = world.get_component_mut::<AudioPlayer>(*id) {
            player.pending_stop = None;
        }
    }

    // Handle pauses — no AudioManager needed.
    for (id, idx) in &pause_requests {
        if let Some(player) = world.get_component::<AudioPlayer>(*id)
            && let Some(sound) = player.audio.get(*idx)
            && let Ok(mut guard) = sound.handle.lock()
            && let Some(h) = guard.as_mut()
        {
            let tween = if sound.fade_out > 0.0 {
                Tween {
                    duration: std::time::Duration::from_secs_f32(sound.fade_out),
                    ..Default::default()
                }
            } else {
                Tween::default()
            };
            h.pause(tween);
        }
        if let Some(player) = world.get_component_mut::<AudioPlayer>(*id) {
            player.pending_pause = None;
        }
    }

    // Handle resumes — no AudioManager needed.
    for (id, idx) in &resume_requests {
        if let Some(player) = world.get_component::<AudioPlayer>(*id)
            && let Some(sound) = player.audio.get(*idx)
            && let Ok(mut guard) = sound.handle.lock()
            && let Some(h) = guard.as_mut()
        {
            let tween = if sound.fade_in > 0.0 {
                Tween {
                    duration: std::time::Duration::from_secs_f32(sound.fade_in),
                    ..Default::default()
                }
            } else {
                Tween::default()
            };
            h.resume(tween);
        }
        if let Some(player) = world.get_component_mut::<AudioPlayer>(*id) {
            player.pending_resume = None;
        }
    }

    if play_requests.is_empty() {
        return Ok(());
    }

    // Ensure Audio resource exists.
    if !world.has_resource::<Audio>() {
        match Audio::new() {
            Ok(audio) => {
                world.insert_resource(audio);
            }
            Err(e) => {
                crate::log_warn!("AudioPlayer: failed to create AudioManager: {}", e);
                for (id, _) in &play_requests {
                    if let Some(p) = world.get_component_mut::<AudioPlayer>(*id) {
                        p.pending_play = None;
                    }
                }
                return Ok(());
            }
        }
    }

    let audio = world.get_resource::<Audio>()?.clone();

    for (id, idx) in play_requests {
        let handle_arc = world
            .get_component::<AudioPlayer>(id)
            .and_then(|p| p.audio.get(idx))
            .map(|s| s.handle.clone());

        if let Some(player) = world.get_component::<AudioPlayer>(id) {
            match player.play(idx, &audio) {
                Ok(handle) => {
                    if let Some(arc) = handle_arc
                        && let Ok(mut g) = arc.lock()
                    {
                        *g = Some(handle);
                    }
                }
                Err(e) => {
                    crate::log_warn!("AudioPlayer: play failed: {}", e);
                }
            }
        }
        if let Some(player) = world.get_component_mut::<AudioPlayer>(id) {
            player.pending_play = None;
        }
    }

    Ok(())
}

impl AudioPlayer {
    pub fn serialize(&self) -> Option<serde_yaml::Value> {
        let sounds: Vec<serde_yaml::Value> = self
            .audio
            .iter()
            .map(|s| {
                let mut map = serde_yaml::Mapping::new();
                map.insert("path".into(), s.path.clone().into());
                map.insert("layer".into(), s.layer.as_str().into());
                map.insert("volume".into(), (s.volume as f64).into());
                map.insert("pitch".into(), (s.pitch as f64).into());
                map.insert("fade_in".into(), (s.fade_in as f64).into());
                map.insert("fade_out".into(), (s.fade_out as f64).into());
                map.insert("looping".into(), s.looping.into());
                map.insert("auto_play".into(), s.auto_play.into());
                map.insert("spatial".into(), s.spatial.into());
                map.insert("max_distance".into(), (s.max_distance as f64).into());
                serde_yaml::Value::Mapping(map)
            })
            .collect();

        let mut map = serde_yaml::Mapping::new();
        map.insert("type".into(), "AudioPlayer".into());
        map.insert("sounds".into(), serde_yaml::Value::Sequence(sounds));
        Some(serde_yaml::Value::Mapping(map))
    }

    pub fn deserialize(&mut self, value: &serde_yaml::Value) -> Result<()> {
        let Some(seq) = value.get("sounds").and_then(|v| v.as_sequence()) else {
            return Ok(());
        };

        self.audio.clear();
        for entry in seq {
            let mut sound = crate::audio::sound::Sound::default();

            if let Some(p) = entry.get("path").and_then(|v| v.as_str()) {
                sound.path = p.to_string();
            }
            if let Some(l) = entry.get("layer").and_then(|v| v.as_str())
                && let Some(layer) = AudioLayer::from_str(l)
            {
                sound.layer = layer;
            }
            if let Some(v) = entry.get("volume").and_then(|v| v.as_f64()) {
                sound.volume = v as f32;
            }
            if let Some(v) = entry.get("pitch").and_then(|v| v.as_f64()) {
                sound.pitch = v as f32;
            }
            if let Some(v) = entry.get("fade_in").and_then(|v| v.as_f64()) {
                sound.fade_in = v as f32;
            }
            if let Some(v) = entry.get("fade_out").and_then(|v| v.as_f64()) {
                sound.fade_out = v as f32;
            }
            if let Some(v) = entry.get("looping").and_then(|v| v.as_bool()) {
                sound.looping = v;
            }
            if let Some(v) = entry.get("auto_play").and_then(|v| v.as_bool()) {
                sound.auto_play = v;
            }

            if let Some(v) = entry.get("spatial").and_then(|v| v.as_bool()) {
                sound.spatial = v;
            }
            if let Some(v) = entry.get("max_distance").and_then(|v| v.as_f64()) {
                sound.max_distance = v as f32;
            }

            if !sound.path.is_empty()
                && let Err(e) = sound.reload()
            {
                crate::log_warn!("AudioPlayer: failed to load '{}': {}", sound.path, e);
            }

            self.audio.push(sound);
        }
        Ok(())
    }

    /// Plays a sound by index, returning the handle.
    /// Returns an error if no data is loaded.
    pub fn play(&self, index: usize, audio: &Audio) -> Result<StaticSoundHandle> {
        let sound = &self.audio[index];
        let mut data = sound
            .data
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Sound at index {} has no loaded data", index))?
            .volume(kira::Decibels(sound.volume))
            .playback_rate(PlaybackRate(sound.pitch as f64));
        if sound.looping {
            data = data.loop_region(..);
        }
        if sound.fade_in > 0.0 {
            data = data.fade_in_tween(Tween {
                duration: std::time::Duration::from_secs_f32(sound.fade_in),
                ..Default::default()
            });
        }
        audio.play_sound(sound.layer, data)
    }
}

#[update(mode = "game")]
pub fn auto_play_audio(world: &mut World) -> Result<()> {
    // Collect sounds that have auto_play set but haven't been triggered yet.
    let mut to_play: Vec<(crate::worldspaces::cell::EntityId, Vec<usize>)> = Vec::new();
    world.query::<&AudioPlayer>().for_each(|id, player| {
        let indices: Vec<usize> = player
            .audio
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                if s.auto_play && !s.auto_played && s.data.is_some() {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        if !indices.is_empty() {
            to_play.push((id, indices));
        }
    });

    if to_play.is_empty() {
        return Ok(());
    }

    if !world.has_resource::<Audio>() {
        match Audio::new() {
            Ok(audio) => {
                world.insert_resource(audio);
            }
            Err(e) => {
                crate::log_warn!("auto_play_audio: failed to create AudioManager: {}", e);
                return Ok(());
            }
        }
    }

    let audio = world.get_resource::<Audio>()?.clone();

    for (id, indices) in to_play {
        for idx in indices {
            let handle_arc = world
                .get_component::<AudioPlayer>(id)
                .and_then(|p| p.audio.get(idx))
                .map(|s| s.handle.clone());

            if let Some(player) = world.get_component::<AudioPlayer>(id) {
                match player.play(idx, &audio) {
                    Ok(handle) => {
                        if let Some(arc) = handle_arc
                            && let Ok(mut g) = arc.lock()
                        {
                            *g = Some(handle);
                        }
                    }
                    Err(e) => {
                        crate::log_warn!("auto_play_audio: play failed: {}", e);
                    }
                }
            }

            // Mark as triggered so it doesn't fire again until the scene reloads.
            if let Some(player) = world.get_component_mut::<AudioPlayer>(id)
                && let Some(s) = player.audio.get_mut(idx)
            {
                s.auto_played = true;
            }
        }
    }

    Ok(())
}

#[update(mode = "all")]
pub fn update_spatial_audio(world: &mut World) -> Result<()> {
    use crate::audio::audio_listener::AudioListener;
    use crate::ecs::components::transform::Transform;
    use cgmath::InnerSpace;

    // Query for the listener: entity with Transform + AudioListener tag.
    // Falls back to EditorCamera then ActiveCamera if none is set.
    let mut listener_data = None;
    world
        .query::<&Transform>()
        .with_tag::<AudioListener>()
        .for_each(|_, t| {
            if listener_data.is_none() {
                listener_data = Some((t.global_position, t.calculate_global_right()));
            }
        });

    if listener_data.is_none() {
        world
            .query::<&Transform>()
            .with_tag::<EditorCamera>()
            .for_each(|_, t| {
                if listener_data.is_none() {
                    listener_data = Some((t.global_position, t.calculate_global_right()));
                }
            });
    }

    if listener_data.is_none() {
        world
            .query::<&Transform>()
            .with_tag::<ActiveCamera>()
            .for_each(|_, t| {
                if listener_data.is_none() {
                    listener_data = Some((t.global_position, t.calculate_global_right()));
                }
            });
    }

    let (listener_pos, listener_right) = match listener_data {
        Some(l) => l,
        None => return Ok(()),
    };

    // Query all entities that have both AudioPlayer and Transform.
    let spatial_sounds: Vec<_> = {
        let mut acc = Vec::new();
        world
            .query::<(&AudioPlayer, &Transform)>()
            .for_each(|_, (player, transform)| {
                let source_pos = transform.global_position;
                for s in player.audio.iter().filter(|s| s.spatial && s.is_playing()) {
                    acc.push((source_pos, s.volume, s.max_distance, s.handle.clone()));
                }
            });
        acc
    };

    for (source_pos, base_volume_db, max_distance, handle_arc) in spatial_sounds {
        let delta = source_pos - listener_pos;
        let dist = delta.magnitude();

        let t = (1.0_f32 - (dist / max_distance).min(1.0)).max(0.0);

        let volume_db = if t <= 0.0 {
            Decibels::SILENCE
        } else {
            let attenuation_db = 20.0 * t.log10();
            Decibels(base_volume_db + attenuation_db)
        };

        let pan = if dist > 0.001 {
            (delta / dist).dot(listener_right).clamp(-1.0, 1.0)
        } else {
            0.0
        };

        if let Ok(mut guard) = handle_arc.lock()
            && let Some(h) = guard.as_mut()
        {
            h.set_volume(volume_db, Tween::default());
            h.set_panning(Panning(pan), Tween::default());
        }
    }

    Ok(())
}
