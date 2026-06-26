use std::fmt;
use std::sync::{Arc, Mutex};

use kira::sound::PlaybackState;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};

use crate::assets::audio::resolve_audio_path;

#[derive(Clone)]
pub struct Sound {
    pub data: Option<StaticSoundData>,
    pub path: String,
    pub volume: f32,
    pub pitch: f32,
    pub looping: bool,
    pub auto_play: bool,
    pub auto_played: bool,
    pub spatial: bool,
    pub max_distance: f32,
    /// Handle to the currently playing instance, if any.
    pub handle: Arc<Mutex<Option<StaticSoundHandle>>>,
}

impl Default for Sound {
    fn default() -> Self {
        Self {
            data: None,
            path: String::new(),
            volume: 0.0,
            pitch: 1.0,
            looping: false,
            auto_play: false,
            auto_played: false,
            spatial: false,
            max_distance: 20.0,
            handle: Arc::new(Mutex::new(None)),
        }
    }
}

impl Sound {
    pub fn is_playing(&self) -> bool {
        self.handle
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|h| h.state()))
            .map(|s| s == PlaybackState::Playing || s == PlaybackState::Pausing)
            .unwrap_or(false)
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

    /// Try to reload `data` from `self.path`. Returns an error if the file can't be loaded.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        let resolved = resolve_audio_path(&self.path)
            .ok_or_else(|| anyhow::anyhow!("Audio file not found: {}", self.path))?;
        self.data = Some(StaticSoundData::from_file(&resolved)?);
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
