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

#[derive(Debug, Clone)]
pub struct DevicePort {
    pub name: String,
    pub description: String,
    pub priority: u32,
    pub available: String,
}

#[derive(Debug, Clone)]
pub struct AudioDeviceDetails {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub is_default: bool,

    pub volume_percent: f32,
    pub muted: bool,

    pub state: String,
    pub driver: Option<String>,
    pub card: Option<u32>,

    pub sample_spec: String,
    pub channel_map: String,
    pub latency_usec: u64,
    pub configured_latency_usec: u64,

    pub ports: Vec<DevicePort>,
    pub active_port: Option<String>,
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

