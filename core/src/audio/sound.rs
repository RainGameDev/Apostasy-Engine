use std::fmt;

use kira::sound::static_sound::StaticSoundData;

use crate::assets::audio::resolve_audio_path;

#[derive(Clone, Default)]
pub struct Sound {
    pub data: Option<StaticSoundData>,
    pub path: String,
    pub volume: f32,
}

impl Sound {
    pub fn from_path(path: &str) -> anyhow::Result<Self> {
        let resolved = resolve_audio_path(path)
            .ok_or_else(|| anyhow::anyhow!("Audio file not found: {}", path))?;
        let data = StaticSoundData::from_file(&resolved)?;
        Ok(Self {
            data: Some(data),
            path: path.to_string(),
            volume: 0.0,
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
            .field("loaded", &self.data.is_some())
            .finish()
    }
}
