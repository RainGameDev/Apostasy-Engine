use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AudioLayer {
    #[default]
    #[serde(rename = "Effect", alias = "SoundEffect")]
    SoundEffect,
    Music,
    FootStep,
    Ambience,
    Voices,
    UI,
    Weather,
}

impl AudioLayer {
    pub const ALL: &'static [AudioLayer] = &[
        AudioLayer::SoundEffect,
        AudioLayer::Music,
        AudioLayer::FootStep,
        AudioLayer::Ambience,
        AudioLayer::Voices,
        AudioLayer::UI,
        AudioLayer::Weather,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AudioLayer::SoundEffect => "Effect",
            AudioLayer::Music => "Music",
            AudioLayer::FootStep => "FootStep",
            AudioLayer::Ambience => "Ambience",
            AudioLayer::Voices => "Voices",
            AudioLayer::UI => "UI",
            AudioLayer::Weather => "Weather",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Effect" | "SoundEffect" => Some(AudioLayer::SoundEffect),
            "Music" => Some(AudioLayer::Music),
            "FootStep" => Some(AudioLayer::FootStep),
            "Ambience" => Some(AudioLayer::Ambience),
            "Voices" => Some(AudioLayer::Voices),
            "UI" => Some(AudioLayer::UI),
            "Weather" => Some(AudioLayer::Weather),
            _ => None,
        }
    }
}

impl fmt::Display for AudioLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
