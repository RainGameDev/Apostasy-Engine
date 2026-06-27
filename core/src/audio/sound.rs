use std::fmt;
use std::sync::{Arc, Mutex};

use kira::sound::FromFileError;
use kira::sound::PlaybackState;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::streaming::StreamingSoundHandle;
use kira::{Decibels, Panning, Tween};

use crate::assets::audio::resolve_audio_path;
use crate::audio::layers::AudioLayer;

/// Wraps either a static or streaming sound handle with a uniform control interface.
pub enum SoundHandle {
    Static(StaticSoundHandle),
    Streaming(StreamingSoundHandle<FromFileError>),
}

impl SoundHandle {
    pub fn state(&self) -> PlaybackState {
        match self {
            Self::Static(h) => h.state(),
            Self::Streaming(h) => h.state(),
        }
    }

    pub fn pause(&mut self, tween: Tween) {
        match self {
            Self::Static(h) => h.pause(tween),
            Self::Streaming(h) => h.pause(tween),
        }
    }

    pub fn resume(&mut self, tween: Tween) {
        match self {
            Self::Static(h) => h.resume(tween),
            Self::Streaming(h) => h.resume(tween),
        }
    }

    pub fn stop(&mut self, tween: Tween) {
        match self {
            Self::Static(h) => h.stop(tween),
            Self::Streaming(h) => h.stop(tween),
        }
    }

    pub fn set_volume(&mut self, volume: Decibels, tween: Tween) {
        match self {
            Self::Static(h) => h.set_volume(volume, tween),
            Self::Streaming(h) => h.set_volume(volume, tween),
        }
    }

    pub fn set_panning(&mut self, panning: Panning, tween: Tween) {
        match self {
            Self::Static(h) => h.set_panning(panning, tween),
            Self::Streaming(h) => h.set_panning(panning, tween),
        }
    }
}

fn is_active(h: &SoundHandle) -> bool {
    matches!(
        h.state(),
        PlaybackState::Playing | PlaybackState::Pausing | PlaybackState::Paused
    )
}

#[derive(Clone)]
pub struct Sound {
    pub data: Option<StaticSoundData>,
    pub path: String,
    pub layer: AudioLayer,
    pub volume: f32,
    pub pitch: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub looping: bool,
    pub auto_play: bool,
    pub auto_played: bool,
    pub spatial: bool,
    pub max_distance: f32,
    /// All currently active (playing or paused) instances.
    pub handle: Arc<Mutex<Vec<SoundHandle>>>,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            data: None,
            path: String::new(),
            layer: AudioLayer::default(),
            volume: 0.0,
            pitch: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            looping: false,
            auto_play: false,
            auto_played: false,
            spatial: false,
            max_distance: 20.0,
            handle: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Sound {
    pub fn is_playing(&self) -> bool {
        self.handle
            .lock()
            .ok()
            .map(|g| {
                g.iter()
                    .any(|h| matches!(h.state(), PlaybackState::Playing | PlaybackState::Pausing))
            })
            .unwrap_or(false)
    }

    pub fn is_paused(&self) -> bool {
        self.handle
            .lock()
            .ok()
            .map(|g| {
                !g.iter()
                    .any(|h| matches!(h.state(), PlaybackState::Playing | PlaybackState::Pausing))
                    && g.iter().any(|h| h.state() == PlaybackState::Paused)
            })
            .unwrap_or(false)
    }

    /// Returns true if a path is set and the sound is ready to play.
    pub fn is_streaming(&self) -> bool {
        self.layer == AudioLayer::Music
    }

    /// Returns true if the sound is ready to play.
    pub fn is_ready(&self) -> bool {
        if self.is_streaming() {
            !self.path.is_empty()
        } else {
            self.data.is_some()
        }
    }

    /// Removes stopped/finished handles from the pool.
    pub fn prune_handles(&self) {
        if let Ok(mut g) = self.handle.lock() {
            g.retain(is_active);
        }
    }

    pub fn from_path(path: &str) -> anyhow::Result<Self> {
        let resolved = resolve_audio_path(path)
            .ok_or_else(|| anyhow::anyhow!("Audio file not found: {}", path))?;
        let data = StaticSoundData::from_file(&resolved)?;
        Ok(Self {
            data: Some(data),
            path: path.to_string(),
            ..Default::default()
        })
    }

    /// Reload or validate the sound file.
    /// For streaming (Music) sounds, just checks the file exists without loading it.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        let resolved = resolve_audio_path(&self.path)
            .ok_or_else(|| anyhow::anyhow!("Audio file not found: {}", self.path))?;
        if self.is_streaming() {
            // Don't load into memory — streaming reads from disk at play time.
            let _ = resolved;
            self.data = None;
        } else {
            self.data = Some(StaticSoundData::from_file(&resolved)?);
        }
        Ok(())
    }
}

impl fmt::Debug for Sound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sound")
            .field("path", &self.path)
            .field("volume", &self.volume)
            .field("auto_play", &self.auto_play)
            .field("loaded", &self.data.is_some())
            .field("playing", &self.is_playing())
            .finish()
    }
}
