use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub mod audio_listener;
pub mod audio_player;
pub mod layers;
pub mod sound;

use apostasy_macros::Resource;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::track::TrackBuilder;
use kira::{AudioManager, AudioManagerSettings, DefaultBackend};

use layers::AudioLayer;

#[derive(Resource, Clone)]
pub struct Audio {
    pub manager: Arc<Mutex<AudioManager>>,
    pub tracks: HashMap<AudioLayer, Arc<Mutex<kira::track::TrackHandle>>>,

    pub master_volume: u32,
    pub music_volume: u32,
    pub sound_effects_volume: u32,
    pub footstep_volume: u32,
    pub ambience_volume: u32,
    pub voices_volume: u32,
    pub ui_volume: u32,
    pub weather_volume: u32,
}

impl Audio {
    pub fn new() -> anyhow::Result<Self> {
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())?;
        let mut tracks = HashMap::new();
        for &layer in AudioLayer::ALL {
            let handle = manager
                .add_sub_track(TrackBuilder::new())
                .map_err(|_| anyhow::anyhow!("failed to create sub-track for {:?}", layer))?;
            tracks.insert(layer, Arc::new(Mutex::new(handle)));
        }
        Ok(Self {
            manager: Arc::new(Mutex::new(manager)),
            tracks,
            master_volume: 50,
            music_volume: 50,
            sound_effects_volume: 50,
            footstep_volume: 50,
            ambience_volume: 50,
            voices_volume: 50,
            ui_volume: 50,
            weather_volume: 50,
        })
    }

    /// Applies all stored volume levels to the live kira tracks.
    /// Safe to call even before the AudioManager is initialised.
    pub fn apply_volumes(&self) {
        fn pct_to_db(pct: u32) -> kira::Decibels {
            if pct == 0 {
                kira::Decibels::SILENCE
            } else {
                kira::Decibels(20.0 * (pct as f32 / 100.0).log10())
            }
        }

        if let Ok(mut mgr) = self.manager.lock() {
            mgr.main_track()
                .set_volume(pct_to_db(self.master_volume), kira::Tween::default());
        }

        let layer_vols = [
            (AudioLayer::Music, self.music_volume),
            (AudioLayer::SoundEffect, self.sound_effects_volume),
            (AudioLayer::FootStep, self.footstep_volume),
            (AudioLayer::Ambience, self.ambience_volume),
            (AudioLayer::Voices, self.voices_volume),
            (AudioLayer::UI, self.ui_volume),
            (AudioLayer::Weather, self.weather_volume),
        ];
        for (layer, vol) in layer_vols {
            if let Some(track) = self.tracks.get(&layer)
                && let Ok(mut handle) = track.lock()
            {
                handle.set_volume(pct_to_db(vol), kira::Tween::default());
            }
        }
    }

    pub fn play_sound(
        &self,
        layer: AudioLayer,
        data: StaticSoundData,
    ) -> anyhow::Result<StaticSoundHandle> {
        if let Some(track_arc) = self.tracks.get(&layer) {
            let mut track = track_arc
                .lock()
                .map_err(|e| anyhow::anyhow!("track lock: {e}"))?;
            Ok(track.play(data)?)
        } else {
            let mut mgr = self
                .manager
                .lock()
                .map_err(|e| anyhow::anyhow!("manager lock: {e}"))?;
            Ok(mgr.play(data)?)
        }
    }
}
