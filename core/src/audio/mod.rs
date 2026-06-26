use std::sync::{Arc, Mutex};
pub mod audio_listener;
pub mod audio_player;
pub mod sound;

use apostasy_macros::Resource;
use kira::AudioManager;

#[derive(Resource, Clone)]
pub struct Audio {
    pub manager: Arc<Mutex<AudioManager>>,
}
