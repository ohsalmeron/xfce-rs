// Audio control plugin library
pub mod pulseaudio;
pub mod mpris;
pub mod devices;
pub mod notifications;
pub mod sink_inputs;

// Types used across modules
#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
    pub index: u32,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_art: Option<String>,
    pub position: u64,
    pub length: u64,
    pub playing: bool,
    pub player_name: String,
}

