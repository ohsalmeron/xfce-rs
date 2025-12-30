use iced::widget::{
    column, container, row, text, button, slider, scrollable, space,
    mouse_area,
};
use iced::{Alignment, Element, Length, Task, Theme, Color, window, Subscription};
use xfce_rs_ui::styles;
use xfce_rs_ui::colors;
use tracing::{debug, warn, info};

mod pulseaudio;
mod mpris;
mod devices;
mod notifications;
mod sink_inputs;

use xfce_rs_audio::{AudioDevice, AudioDeviceDetails, DevicePort, NowPlaying};

pub fn main() -> iced::Result {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    info!("Audio application starting");
    
    iced::application(AudioApp::new, AudioApp::update, AudioApp::view)
        .title(AudioApp::title)
        .theme(AudioApp::theme)
        .style(AudioApp::style)
        .subscription(AudioApp::subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(900.0, 650.0),
            position: iced::window::Position::Centered,
            transparent: true,
            decorations: false,
            ..Default::default()
        })
        .run()
}

struct AudioApp {
    // Volume state
    #[allow(unused)] // Used in view_volume_controls (line 509, 519, 522)
    volume: f32,
    #[allow(unused)] // Used in view_volume_controls (line 509)
    muted: bool,
    mic_volume: f32,
    mic_muted: bool,
    
    // Currently playing
    now_playing: Option<NowPlaying>,
    
    // Devices
    output_devices: Vec<AudioDevice>,
    input_devices: Vec<AudioDevice>,
    selected_output: Option<usize>,
    selected_input: Option<usize>,
    selected_output_details: Option<AudioDeviceDetails>,
    selected_input_details: Option<AudioDeviceDetails>,
    
    // Per-app volume controls
    sink_inputs: Vec<sink_inputs::SinkInput>,
    show_app_volumes: bool,
    
    // UI state
    show_devices: bool,
    notification: Option<String>,
    
    // Debouncing for app volume updates
    pending_app_volume_updates: std::collections::HashMap<u32, f32>,
    // Debouncing for master volume updates
    pending_master_volume: Option<f32>,
    pending_mic_volume: Option<f32>,
    // MPRIS metadata per sink input (keyed by application_name)
    sink_input_mpris_metadata: std::collections::HashMap<String, NowPlaying>,
}


#[derive(Debug, Clone)]
enum Message {
    VolumeChanged(f32),
    VolumeChangedDebounced(f32), // Debounced version that actually calls PulseAudio
    ToggleMute,
    MicVolumeChanged(f32),
    MicVolumeChangedDebounced(f32), // Debounced version that actually calls PulseAudio
    ToggleMicMute,
    PlayPause,
    Previous,
    Next,
    Seek(u64),
    SelectOutputDevice(usize),
    SelectInputDevice(usize),
    OutputDeviceDetailsUpdate(Option<AudioDeviceDetails>),
    InputDeviceDetailsUpdate(Option<AudioDeviceDetails>),
    SetOutputPort(u32, String),
    SetInputPort(u32, String),
    ToggleDevices,
    #[allow(dead_code)]
    ToggleAppVolumes,
    AppVolumeChanged(u32, f32),
    AppVolumeChangedDebounced(u32, f32), // Debounced version that actually calls PulseAudio
    AppMuteToggled(u32),
    SinkInputsUpdate(Vec<sink_inputs::SinkInput>),
    NowPlayingUpdate(Option<NowPlaying>),
    VolumeUpdate(f32, bool),
    MicVolumeUpdate(f32, bool),
    DevicesUpdate(Vec<AudioDevice>, Vec<AudioDevice>),
    ClearNotification,
    WindowDragged,
    Minimize,
    Maximize,
    Close,
    PollUpdates,
}

impl AudioApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                volume: 50.0,
                muted: false,
                mic_volume: 50.0,
                mic_muted: false,
                now_playing: None,
                output_devices: Vec::new(),
                input_devices: Vec::new(),
                selected_output: None,
                selected_input: None,
                selected_output_details: None,
                selected_input_details: None,
                sink_inputs: Vec::new(),
                show_app_volumes: true, // Show by default
                show_devices: false,
                notification: None,
                pending_app_volume_updates: std::collections::HashMap::new(),
                pending_master_volume: None,
                pending_mic_volume: None,
                sink_input_mpris_metadata: std::collections::HashMap::new(),
            },
            Task::batch(vec![
                // Initialize PulseAudio connection
                Task::perform(
                    async {
                        debug!("Initializing PulseAudio connection...");
                        if let Err(e) = pulseaudio::init().await {
                            warn!("Failed to initialize PulseAudio: {}", e);
                        } else {
                            debug!("PulseAudio initialized successfully");
                        }
                        // Get initial volume state
                        let vol_result = pulseaudio::get_volume().await;
                        match vol_result {
                            Ok((vol, muted)) => {
                                debug!("Initial volume: {:.1}%, muted: {}", vol, muted);
                                (vol, muted)
                            }
                            Err(e) => {
                                warn!("Failed to get initial volume: {}", e);
                                (50.0, false)
                            }
                        }
                    },
                    |(vol, muted)| {
                        debug!("VolumeUpdate message: {:.1}%, muted: {}", vol, muted);
                        Message::VolumeUpdate(vol, muted)
                    },
                ),
                // Initialize MPRIS
                Task::perform(
                    async {
                        debug!("Initializing MPRIS connection...");
                        if let Err(e) = mpris::init().await {
                            warn!("Failed to initialize MPRIS: {}", e);
                        } else {
                            debug!("MPRIS initialized successfully");
                        }
                        // Get initial now playing state
                        match mpris::get_now_playing().await {
                            Ok(Some(np)) => {
                                debug!("Initial now playing: {} - {}", np.artist, np.title);
                                Some(np)
                            }
                            Ok(None) => {
                                debug!("No active MPRIS player");
                                None
                            }
                            Err(e) => {
                                warn!("Failed to get initial now playing: {}", e);
                                None
                            }
                        }
                    },
                    |np| {
                        if let Some(ref np) = np {
                            debug!("NowPlayingUpdate message: {} - {}", np.artist, np.title);
                        } else {
                            debug!("NowPlayingUpdate message: None");
                        }
                        Message::NowPlayingUpdate(np)
                    },
                ),
                // Get initial devices
                Task::perform(
                    async {
                        debug!("Fetching initial device list...");
                        match pulseaudio::get_devices().await {
                            Ok((outputs, inputs)) => {
                                debug!("Initial devices: {} outputs, {} inputs", outputs.len(), inputs.len());
                                (outputs, inputs)
                            }
                            Err(e) => {
                                warn!("Failed to get initial devices: {}", e);
                                (Vec::new(), Vec::new())
                            }
                        }
                    },
                    |(outputs, inputs)| {
                        debug!("DevicesUpdate message: {} outputs, {} inputs", outputs.len(), inputs.len());
                        Message::DevicesUpdate(outputs, inputs)
                    },
                ),
                // Get initial mic volume
                Task::perform(
                    async {
                        debug!("Fetching initial mic volume...");
                        match pulseaudio::get_mic_volume().await {
                            Ok((vol, muted)) => {
                                debug!("Initial mic volume: {:.1}%, muted: {}", vol, muted);
                                (vol, muted)
                            }
                            Err(e) => {
                                warn!("Failed to get initial mic volume: {}", e);
                                (50.0, false)
                            }
                        }
                    },
                    |(vol, muted)| {
                        debug!("MicVolumeUpdate message: {:.1}%, muted: {}", vol, muted);
                        Message::MicVolumeUpdate(vol, muted)
                    },
                ),
                // Get initial sink inputs (app volumes)
                Task::perform(
                    async {
                        debug!("Fetching initial sink inputs (app volumes)...");
                        match sink_inputs::get_sink_inputs().await {
                            Ok(inputs) => {
                                debug!("Initial sink inputs: {} applications", inputs.len());
                                inputs
                            }
                            Err(e) => {
                                warn!("Failed to get initial sink inputs: {}", e);
                                Vec::new()
                            }
                        }
                    },
                    |inputs| {
                        debug!("SinkInputsUpdate message: {} applications", inputs.len());
                        Message::SinkInputsUpdate(inputs)
                    },
                ),
            ]),
        )
    }

    fn title(&self) -> String {
        String::from("Audio Control")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn style(&self, theme: &Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        // Poll for updates every 2 seconds (reduced from 500ms for better performance)
        iced::time::every(std::time::Duration::from_secs(2))
            .map(|_| Message::PollUpdates)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::VolumeChanged(vol) => {
                // Update UI immediately for smooth slider movement
                self.volume = vol;
                
                // Store pending update for debouncing
                self.pending_master_volume = Some(vol);
                
                // Schedule debounced update after 50ms
                let vol_clone = vol;
                Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        vol_clone
                    },
                    |v| Message::VolumeChangedDebounced(v),
                )
            }
            Message::VolumeChangedDebounced(vol) => {
                // Only apply if this is still the latest value
                if let Some(&latest_vol) = self.pending_master_volume.as_ref() {
                    if (latest_vol - vol).abs() < 0.1 {
                        // This is still the latest, apply it
                        self.pending_master_volume = None;
                        let muted = self.muted;
                        Task::perform(
                            pulseaudio::set_volume(vol),
                            move |_| Message::VolumeUpdate(vol, muted),
                        )
                    } else {
                        // A newer update came in, ignore this one
                        Task::none()
                    }
                } else {
                    // Already processed or cancelled
                    Task::none()
                }
            }
            Message::ToggleMute => {
                self.muted = !self.muted;
                let muted = self.muted;
                let volume = self.volume;
                Task::perform(
                    pulseaudio::set_mute(muted),
                    move |_| Message::VolumeUpdate(volume, muted),
                )
            }
            Message::MicVolumeChanged(vol) => {
                // Update UI immediately for smooth slider movement
                self.mic_volume = vol;
                
                // Store pending update for debouncing
                self.pending_mic_volume = Some(vol);
                
                // Schedule debounced update after 50ms
                let vol_clone = vol;
                Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        vol_clone
                    },
                    |v| Message::MicVolumeChangedDebounced(v),
                )
            }
            Message::MicVolumeChangedDebounced(vol) => {
                // Only apply if this is still the latest value
                if let Some(&latest_vol) = self.pending_mic_volume.as_ref() {
                    if (latest_vol - vol).abs() < 0.1 {
                        // This is still the latest, apply it
                        self.pending_mic_volume = None;
                        let mic_muted = self.mic_muted;
                        Task::perform(
                            pulseaudio::set_mic_volume(vol),
                            move |_| Message::MicVolumeUpdate(vol, mic_muted),
                        )
                    } else {
                        // A newer update came in, ignore this one
                        Task::none()
                    }
                } else {
                    // Already processed or cancelled
                    Task::none()
                }
            }
            Message::ToggleMicMute => {
                self.mic_muted = !self.mic_muted;
                let mic_muted = self.mic_muted;
                let mic_volume = self.mic_volume;
                Task::perform(
                    pulseaudio::set_mic_mute(mic_muted),
                    move |_| Message::MicVolumeUpdate(mic_volume, mic_muted),
                )
            }
            Message::PlayPause => {
                let now_playing = self.now_playing.clone();
                Task::perform(
                    mpris::play_pause(),
                    move |_| Message::NowPlayingUpdate(now_playing),
                )
            }
            Message::Previous => {
                let now_playing = self.now_playing.clone();
                Task::perform(
                    mpris::previous(),
                    move |_| Message::NowPlayingUpdate(now_playing),
                )
            }
            Message::Next => {
                let now_playing = self.now_playing.clone();
                Task::perform(
                    mpris::next(),
                    move |_| Message::NowPlayingUpdate(now_playing),
                )
            }
            Message::Seek(pos) => {
                let now_playing = self.now_playing.clone();
                Task::perform(
                    mpris::seek(pos),
                    move |_| Message::NowPlayingUpdate(now_playing),
                )
            }
            Message::SelectOutputDevice(idx) => {
                debug!("SelectOutputDevice called with index {}", idx);
                if let Some(device) = self.output_devices.get(idx) {
                    debug!("Selecting output device: index={}, name={}, description={}", device.index, device.name, device.description);
                    self.selected_output = Some(idx);
                    let device_index = device.index;
                    Task::batch(vec![
                        Task::perform(
                            async move {
                                pulseaudio::set_default_output(device_index).await.ok();
                                pulseaudio::get_devices().await.unwrap_or((Vec::new(), Vec::new()))
                            },
                            |(outputs, inputs)| Message::DevicesUpdate(outputs, inputs),
                        ),
                        Task::perform(
                            pulseaudio::get_output_device_details(device_index),
                            |details| {
                                debug!("Output device details task completed: success={}", details.is_ok());
                                Message::OutputDeviceDetailsUpdate(details.ok())
                            },
                        ),
                    ])
                } else {
                    warn!("SelectOutputDevice: device at index {} not found (total devices: {})", idx, self.output_devices.len());
                    Task::none()
                }
            }
            Message::SelectInputDevice(idx) => {
                debug!("SelectInputDevice called with index {}", idx);
                if let Some(device) = self.input_devices.get(idx) {
                    debug!("Selecting input device: index={}, name={}, description={}", device.index, device.name, device.description);
                    self.selected_input = Some(idx);
                    let device_index = device.index;
                    Task::batch(vec![
                        Task::perform(
                            async move {
                                pulseaudio::set_default_input(device_index).await.ok();
                                pulseaudio::get_devices().await.unwrap_or((Vec::new(), Vec::new()))
                            },
                            |(outputs, inputs)| Message::DevicesUpdate(outputs, inputs),
                        ),
                        Task::perform(
                            pulseaudio::get_input_device_details(device_index),
                            |details| {
                                debug!("Input device details task completed: success={}", details.is_ok());
                                Message::InputDeviceDetailsUpdate(details.ok())
                            },
                        ),
                    ])
                } else {
                    warn!("SelectInputDevice: device at index {} not found (total devices: {})", idx, self.input_devices.len());
                    Task::none()
                }
            }
            Message::OutputDeviceDetailsUpdate(details) => {
                match &details {
                    Some(d) => debug!("Output device details updated: index={}, {} ports, state={}, driver={:?}", d.index, d.ports.len(), d.state, d.driver),
                    None => warn!("Output device details fetch returned None"),
                }
                self.selected_output_details = details;
                Task::none()
            }
            Message::InputDeviceDetailsUpdate(details) => {
                match &details {
                    Some(d) => debug!("Input device details updated: index={}, {} ports, state={}, driver={:?}", d.index, d.ports.len(), d.state, d.driver),
                    None => warn!("Input device details fetch returned None"),
                }
                self.selected_input_details = details;
                Task::none()
            }
            Message::SetOutputPort(device_index, port_name) => {
                let port_name_clone = port_name.clone();
                Task::batch(vec![
                    Task::perform(
                        pulseaudio::set_output_device_port(device_index, port_name_clone),
                        |_| Message::ClearNotification,
                    ),
                        Task::perform(
                            pulseaudio::get_output_device_details(device_index),
                            |details| {
                                debug!("Output device details task completed after port change: success={}", details.is_ok());
                                Message::OutputDeviceDetailsUpdate(details.ok())
                            },
                        ),
                ])
            }
            Message::SetInputPort(device_index, port_name) => {
                let port_name_clone = port_name.clone();
                Task::batch(vec![
                    Task::perform(
                        pulseaudio::set_input_device_port(device_index, port_name_clone),
                        |_| Message::ClearNotification,
                    ),
                        Task::perform(
                            pulseaudio::get_input_device_details(device_index),
                            |details| {
                                debug!("Input device details task completed after port change: success={}", details.is_ok());
                                Message::InputDeviceDetailsUpdate(details.ok())
                            },
                        ),
                ])
            }
            Message::ToggleDevices => {
                self.show_devices = !self.show_devices;
                debug!("ToggleDevices: show_devices={}, current output devices={}, input devices={}, selected_output={:?}, selected_input={:?}", 
                    self.show_devices, self.output_devices.len(), self.input_devices.len(), self.selected_output, self.selected_input);
                
                if self.show_devices {
                    // Always refresh devices first to ensure we have latest data
                    debug!("Refreshing devices list before showing device panel");
                    Task::perform(
                        pulseaudio::get_devices(),
                        |result| {
                            let (outputs, inputs) = result.unwrap_or((Vec::new(), Vec::new()));
                            debug!("Device refresh completed: {} outputs, {} inputs", outputs.len(), inputs.len());
                            Message::DevicesUpdate(outputs, inputs)
                        }
                    )
                } else {
                    Task::none()
                }
            }
                Message::ToggleAppVolumes => {
                    self.show_app_volumes = !self.show_app_volumes;
                    // Always fetch when showing
                    Task::perform(
                        sink_inputs::get_sink_inputs(),
                        |inputs| Message::SinkInputsUpdate(inputs.unwrap_or_default()),
                    )
                }
            Message::AppVolumeChanged(index, volume) => {
                // Update UI immediately for smooth slider movement
                if let Some(input) = self.sink_inputs.iter_mut().find(|i| i.index == index) {
                    input.volume = volume;
                }
                
                // Store pending update for debouncing
                self.pending_app_volume_updates.insert(index, volume);
                
                // Schedule debounced update after 50ms for smoother feel
                let index_clone = index;
                Task::perform(
                    async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        (index_clone, volume)
                    },
                    |(idx, vol)| Message::AppVolumeChangedDebounced(idx, vol),
                )
            }
            Message::AppVolumeChangedDebounced(index, volume) => {
                // Only apply if this is still the latest value (not overwritten)
                if let Some(&latest_volume) = self.pending_app_volume_updates.get(&index) {
                    if (latest_volume - volume).abs() < 0.1 {
                        // This is still the latest, apply it
                        self.pending_app_volume_updates.remove(&index);
                        Task::perform(
                            sink_inputs::set_sink_input_volume(index, volume),
                            |_| Message::ClearNotification,
                        )
                    } else {
                        // A newer update came in, ignore this one
                        Task::none()
                    }
                } else {
                    // Already processed or cancelled
                    Task::none()
                }
            }
            Message::AppMuteToggled(index) => {
                let muted = self.sink_inputs.iter()
                    .find(|i| i.index == index)
                    .map(|i| !i.muted)
                    .unwrap_or(false);
                Task::perform(
                    sink_inputs::set_sink_input_mute(index, muted),
                    |_| Message::ClearNotification,
                )
            }
            Message::SinkInputsUpdate(inputs) => {
                self.sink_inputs = inputs.clone();
                
                // Match sink inputs to MPRIS players
                // Get current now_playing to match against
                let now_playing = self.now_playing.clone();
                
                // Update MPRIS metadata map
                if let Some(np) = now_playing {
                    // Try to match by player name to application name
                    for input in &self.sink_inputs {
                        let app_name_lower = input.application_name.to_lowercase();
                        let player_name_lower = np.player_name.to_lowercase();
                        
                        // Match if application name contains player name or vice versa
                        if app_name_lower.contains(&player_name_lower) || 
                           player_name_lower.contains(&app_name_lower) ||
                           app_name_lower == player_name_lower {
                            self.sink_input_mpris_metadata.insert(input.application_name.clone(), np.clone());
                            debug!("Matched MPRIS metadata for sink input: {} -> {}", input.application_name, np.title);
                            break; // Only match one player for now
                        }
                    }
                }
                
                Task::none()
            }
            Message::NowPlayingUpdate(np) => {
                self.now_playing = np.clone();
                
                // Update MPRIS metadata for matching sink inputs
                if let Some(ref np_meta) = np {
                    for input in &self.sink_inputs {
                        let app_name_lower = input.application_name.to_lowercase();
                        let player_name_lower = np_meta.player_name.to_lowercase();
                        
                        // Match if application name contains player name or vice versa
                        if app_name_lower.contains(&player_name_lower) || 
                           player_name_lower.contains(&app_name_lower) ||
                           app_name_lower == player_name_lower {
                            self.sink_input_mpris_metadata.insert(input.application_name.clone(), np_meta.clone());
                            debug!("Updated MPRIS metadata for sink input: {} -> {}", input.application_name, np_meta.title);
                        }
                    }
                }
                
                Task::none()
            }
            Message::VolumeUpdate(vol, muted) => {
                self.volume = vol;
                self.muted = muted;
                Task::none()
            }
            Message::MicVolumeUpdate(vol, muted) => {
                self.mic_volume = vol;
                self.mic_muted = muted;
                Task::none()
            }
            Message::DevicesUpdate(outputs, inputs) => {
                debug!("DevicesUpdate received: {} outputs, {} inputs", outputs.len(), inputs.len());
                // Filter and sort devices
                let (filtered_outputs, filtered_inputs) = devices::DeviceManager::filter_devices(
                    outputs,
                    inputs,
                    None, // We don't have default source name here, filtering happens in PulseAudio
                );
                self.output_devices = devices::DeviceManager::sort_devices(filtered_outputs);
                self.input_devices = devices::DeviceManager::sort_devices(filtered_inputs);
                debug!("After filtering/sorting: {} output devices, {} input devices", self.output_devices.len(), self.input_devices.len());
                
                // If show_devices is true and no device selected, auto-select defaults
                if self.show_devices {
                    let mut tasks = Vec::new();
                    
                    if self.selected_output.is_none() {
                        if let Some((idx, device)) = self.output_devices.iter().enumerate().find(|(_, d)| d.is_default) {
                            debug!("Auto-selecting default output device: index={}, name={}", device.index, device.name);
                            self.selected_output = Some(idx);
                            let device_index = device.index;
                            tasks.push(Task::perform(
                                pulseaudio::get_output_device_details(device_index),
                                |details| {
                                    debug!("Auto-selected output device details task completed: success={}", details.is_ok());
                                    Message::OutputDeviceDetailsUpdate(details.ok())
                                },
                            ));
                        } else {
                            debug!("No default output device found to auto-select");
                        }
                    }
                    
                    if self.selected_input.is_none() {
                        // First try to find default device
                        if let Some((idx, device)) = self.input_devices.iter().enumerate().find(|(_, d)| d.is_default) {
                            debug!("Auto-selecting default input device: index={}, name={}", device.index, device.name);
                            self.selected_input = Some(idx);
                            let device_index = device.index;
                            tasks.push(Task::perform(
                                pulseaudio::get_input_device_details(device_index),
                                |details| {
                                    debug!("Auto-selected input device details task completed: success={}", details.is_ok());
                                    Message::InputDeviceDetailsUpdate(details.ok())
                                },
                            ));
                        } else if let Some((idx, device)) = self.input_devices.first().map(|d| (0, d)) {
                            // If no default, select first available input device
                            debug!("No default input device found, selecting first available: index={}, name={}", device.index, device.name);
                            self.selected_input = Some(idx);
                            let device_index = device.index;
                            tasks.push(Task::perform(
                                pulseaudio::get_input_device_details(device_index),
                                |details| {
                                    debug!("Auto-selected first input device details task completed: success={}", details.is_ok());
                                    Message::InputDeviceDetailsUpdate(details.ok())
                                },
                            ));
                        } else {
                            debug!("No input devices available to auto-select");
                        }
                    }
                    
                    if !tasks.is_empty() {
                        return Task::batch(tasks);
                    }
                }
                
                Task::none()
            }
            Message::ClearNotification => {
                self.notification = None;
                Task::none()
            }
            Message::WindowDragged => {
                window::latest().and_then(|id| window::drag(id))
            }
            Message::Minimize => {
                window::latest().and_then(|id| window::minimize(id, true))
            }
            Message::Maximize => {
                window::latest().and_then(|id| window::maximize(id, true))
            }
            Message::Close => {
                window::latest().and_then(|id| window::close(id))
            }
            Message::PollUpdates => {
                // Poll for volume updates
                let current_vol = self.volume;
                let current_muted = self.muted;
                let current_mic_vol = self.mic_volume;
                let current_mic_muted = self.mic_muted;
                let current_now_playing = self.now_playing.clone();
                let current_sink_inputs = self.sink_inputs.clone();
                
                let current_vol_clone = current_vol;
                let current_muted_clone = current_muted;
                let current_mic_vol_clone = current_mic_vol;
                let current_mic_muted_clone = current_mic_muted;
                let current_now_playing_clone = current_now_playing.clone();
                
                Task::batch(vec![
                    // Poll PulseAudio volume
                    Task::perform(
                        async move { pulseaudio::get_volume().await.unwrap_or((current_vol_clone, current_muted_clone)) },
                        move |(vol, muted)| {
                            let vol_diff = (vol - current_vol_clone).abs();
                            if vol_diff > 0.1 || muted != current_muted_clone {
                                Message::VolumeUpdate(vol, muted)
                            } else {
                                Message::ClearNotification
                            }
                        },
                    ),
                    // Poll mic volume
                    Task::perform(
                        async move { pulseaudio::get_mic_volume().await.unwrap_or((current_mic_vol_clone, current_mic_muted_clone)) },
                        move |(vol, muted)| {
                            let vol_diff = (vol - current_mic_vol_clone).abs();
                            if vol_diff > 0.1 || muted != current_mic_muted_clone {
                                Message::MicVolumeUpdate(vol, muted)
                            } else {
                                Message::ClearNotification
                            }
                        },
                    ),
                    // Poll MPRIS now playing
                    Task::perform(
                        async move { mpris::get_now_playing().await.ok().flatten() },
                        move |np| {
                            if np != current_now_playing_clone {
                                Message::NowPlayingUpdate(np)
                            } else {
                                Message::ClearNotification
                            }
                        },
                    ),
                    // Poll sink inputs if app volumes are shown
                    if self.show_app_volumes {
                        let current_sink_inputs_clone = current_sink_inputs.clone();
                        Task::perform(
                            async move { sink_inputs::get_sink_inputs().await.unwrap_or_default() },
                            move |inputs| {
                                // Simple comparison: check if lengths differ or any index changed
                                let changed = inputs.len() != current_sink_inputs_clone.len() ||
                                    inputs.iter().any(|i| {
                                        !current_sink_inputs_clone.iter().any(|c| c.index == i.index && c.volume == i.volume && c.muted == i.muted)
                                    });
                                if changed {
                                    Message::SinkInputsUpdate(inputs)
                                } else {
                                    Message::ClearNotification
                                }
                            },
                        )
                    } else {
                        Task::none()
                    },
                ])
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let header = self.view_header();
        
        // Show Now Playing if we have any real metadata (title is not just "Playing from X")
        let now_playing = if let Some(np) = &self.now_playing {
            // Show if title is not the fallback pattern - even if artist is unknown, show the title
            let is_real_metadata = np.title != format!("Playing from {}", np.player_name) && 
                                   !np.title.starts_with("Playing from");
            if is_real_metadata {
                self.view_now_playing()
            } else {
                Element::from(space().height(0))
            }
        } else {
            Element::from(space().height(0))
        };
        
        let volume_controls = self.view_volume_controls();
        
        // Per-app volume controls are ALWAYS shown - this is the main feature
        let app_volume_controls = self.view_app_volume_controls();
        
        let device_controls = if self.show_devices {
            self.view_device_controls()
        } else {
            Element::from(space().height(0))
        };

        let main_content = column![
            header,
            volume_controls,
            app_volume_controls,  // Primary feature - show prominently
            now_playing,  // Secondary - only if we have real metadata
            device_controls,
        ]
        .spacing(20)
        .padding(30);

        let mut layers = vec![
            // Glass base
            container(space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme| styles::glass_base(theme))
                .into(),
            // Highlights
            container(space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme| styles::glass_highlight_top(theme))
                .into(),
            container(space())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme| styles::glass_highlight_bottom(theme))
                .into(),
            // Drag area
            mouse_area(container(space()).width(Length::Fill).height(Length::Fill))
                .on_press(Message::WindowDragged)
                .into(),
            // Content
            container(main_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        ];

        // Notification
        if let Some(note) = &self.notification {
            layers.push(
                container(
                    container(text(note).color(Color::WHITE))
                        .padding(15)
                        .style(|theme| styles::glass_base(theme))
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::End)
                .padding(40)
                .into()
            );
        }

        iced::widget::Stack::with_children(layers).into()
    }

    fn view_header(&self) -> Element<'_, Message> {
        row![
            // Window controls
            row![
                button(space().width(12).height(12))
                    .on_press(Message::Close)
                    .style(|theme, status| styles::window_control(theme, status, colors::CONTROL_CLOSE))
                    .width(12).height(12),
                button(space().width(12).height(12))
                    .on_press(Message::Minimize)
                    .style(|theme, status| styles::window_control(theme, status, colors::CONTROL_MIN))
                    .width(12).height(12),
                button(space().width(12).height(12))
                    .on_press(Message::Maximize)
                    .style(|theme, status| styles::window_control(theme, status, colors::CONTROL_MAX))
                    .width(12).height(12),
            ]
            .spacing(8)
            .padding(10),
            
            // Title
            row![
                text("🎵 Audio Control").size(20).color(colors::TEXT_PRIMARY),
            ]
            .width(Length::Fill)
            .align_y(Alignment::Center),
        ]
        .height(40)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_now_playing(&self) -> Element<'_, Message> {
        if let Some(np) = &self.now_playing {
            let play_pause_icon = if np.playing { "⏸" } else { "▶" };
            
            column![
                // Album art placeholder
                container(
                    text("🎵").size(120)
                )
                .width(300)
                .height(300)
                .style(|theme| styles::glass_base(theme))
                .center_x(Length::Fill),
                
                // Track info
                column![
                    // Show title - always show if we have it
                    text(&np.title).size(24).color(colors::TEXT_PRIMARY),
                    // Show artist or hide if unknown
                    if np.artist != "Unknown Artist" && !np.artist.is_empty() {
                        text(&np.artist).size(18).color(colors::TEXT_SECONDARY)
                    } else {
                        text("").size(18)
                    },
                    // Show album or hide if unknown
                    if np.album != "Unknown Album" && !np.album.is_empty() {
                        text(&np.album).size(14).color(colors::TEXT_SECONDARY)
                    } else {
                        text("").size(14)
                    },
                    text(format!("Source: {}", np.player_name)).size(12).color(colors::TEXT_SECONDARY),
                ]
                .spacing(5)
                .align_x(Alignment::Center),
                
                // Progress bar
                slider(0.0..=np.length.max(1) as f64, np.position as f64, |v| Message::Seek(v as u64))
                    .width(Length::Fill),
                
                // Controls
                row![
                    button(text("⏮").size(24))
                        .on_press(Message::Previous)
                        .style(|theme, status| styles::app_card(theme, status))
                        .padding(10),
                    button(text(play_pause_icon).size(32))
                        .on_press(Message::PlayPause)
                        .style(|theme, status| styles::app_card(theme, status))
                        .padding(15),
                    button(text("⏭").size(24))
                        .on_press(Message::Next)
                        .style(|theme, status| styles::app_card(theme, status))
                        .padding(10),
                ]
                .spacing(15),
            ]
            .spacing(15)
            .into()
        } else {
            container(
                column![
                    text("No media playing").size(18).color(colors::TEXT_SECONDARY),
                ]
                .align_x(Alignment::Center)
            )
            .width(Length::Fill)
            .height(400)
            .style(|theme| styles::glass_base(theme))
            .center_x(Length::Fill)
            .into()
        }
    }

    fn view_volume_controls(&self) -> Element<'_, Message> {
        let mute_icon = if self.muted { "🔇" } else { "🔊" };
        let mic_mute_icon = if self.mic_muted { "🎤🚫" } else { "🎤" };
        
        column![
            // Output volume
            row![
                button(text(mute_icon).size(24))
                    .on_press(Message::ToggleMute)
                    .style(|theme, status| styles::app_card(theme, status))
                    .padding(8),
                slider(0.0..=100.0, self.volume, Message::VolumeChanged)
                    .width(Length::Fill)
                    .step(1.0),
                text(format!("{:.0}%", self.volume)).size(14).color(colors::TEXT_SECONDARY).width(50),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            
            // Input volume
            row![
                button(text(mic_mute_icon).size(24))
                    .on_press(Message::ToggleMicMute)
                    .style(|theme, status| styles::app_card(theme, status))
                    .padding(8),
                slider(0.0..=100.0, self.mic_volume, Message::MicVolumeChanged)
                    .width(Length::Fill)
                    .step(1.0),
                text(format!("{:.0}%", self.mic_volume)).size(14).color(colors::TEXT_SECONDARY).width(50),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            
            // App volumes are always shown now, so remove this toggle
            
            // Device toggle
            button(text(if self.show_devices { "Hide Devices" } else { "Show Devices" }).size(14))
                .on_press(Message::ToggleDevices)
                .style(|theme, status| styles::app_card(theme, status))
                .padding(10),
        ]
        .spacing(10)
        .into()
    }

    fn view_device_controls(&self) -> Element<'_, Message> {
        let output_details = self.view_device_details_panel(true);
        let input_details = self.view_device_details_panel(false);

        column![
            text("Output Devices").size(16).color(colors::TEXT_PRIMARY),
            scrollable(
                column(
                    self.output_devices.iter().enumerate().map(|(idx, device)| {
                        let is_selected = self.selected_output == Some(idx);
                        let is_default = device.is_default;
                        let description = device.description.clone();
                        button(
                            column![
                                text(description).size(14).color(colors::TEXT_PRIMARY),
                                if is_default {
                                    text("Default").size(12).color(colors::ACCENT_PRIMARY)
                                } else {
                                    text("").size(12)
                                },
                            ]
                            .spacing(2)
                        )
                        .on_press(Message::SelectOutputDevice(idx))
                        .style(move |theme, status| {
                            if is_selected {
                                styles::app_card(theme, iced::widget::button::Status::Active)
                            } else {
                                styles::app_card(theme, status)
                            }
                        })
                        .width(Length::Fill)
                        .padding(10)
                        .into()
                    }).collect::<Vec<Element<Message>>>()
                )
                .spacing(5)
            )
            .height(120),

            space().height(10),
            output_details,
            space().height(10),
            
            text("Input Devices").size(16).color(colors::TEXT_PRIMARY),
            scrollable(
                column(
                    self.input_devices.iter().enumerate().map(|(idx, device)| {
                        let is_selected = self.selected_input == Some(idx);
                        let is_default = device.is_default;
                        let description = device.description.clone();
                        button(
                            column![
                                text(description).size(14).color(colors::TEXT_PRIMARY),
                                if is_default {
                                    text("Default").size(12).color(colors::ACCENT_PRIMARY)
                                } else {
                                    text("").size(12)
                                },
                            ]
                            .spacing(2)
                        )
                        .on_press(Message::SelectInputDevice(idx))
                        .style(move |theme, status| {
                            if is_selected {
                                styles::app_card(theme, iced::widget::button::Status::Active)
                            } else {
                                styles::app_card(theme, status)
                            }
                        })
                        .width(Length::Fill)
                        .padding(10)
                        .into()
                    }).collect::<Vec<Element<Message>>>()
                )
                .spacing(5)
            )
            .height(120),

            space().height(10),
            input_details,
        ]
        .spacing(10)
        .into()
    }

    fn view_device_details_panel(&self, is_output: bool) -> Element<'_, Message> {
        let details_opt = if is_output {
            self.selected_output_details.clone()
        } else {
            self.selected_input_details.clone()
        };

        let title = if is_output { "🔊 Output Device Details" } else { "🎤 Input Device Details" };
        
        let is_device_selected = if is_output {
            self.selected_output.is_some()
        } else {
            self.selected_input.is_some()
        };

        let Some(details) = details_opt else {
            let message = if is_device_selected {
                "⏳ Loading device details..."
            } else {
                "👆 Click a device above to see detailed information"
            };
            return container(
                column![
                    text(title).size(16).color(colors::TEXT_PRIMARY),
                    text(message).size(12).color(colors::TEXT_SECONDARY),
                ]
                .spacing(8),
            )
            .padding(16)
            .width(Length::Fill)
            .style(|theme| styles::glass_base(theme))
            .into();
        };

        let device_index = details.index;
        let active_port = details.active_port.clone().unwrap_or_default();

        let ports_row: Element<Message> = if details.ports.is_empty() {
            container(text("No ports exposed by server").size(12).color(colors::TEXT_SECONDARY))
                .width(Length::Fill)
                .into()
        } else {
            let mut port_buttons: Vec<Element<Message>> = Vec::new();
            for p in details.ports.iter() {
                if p.name.is_empty() {
                    continue;
                }
                let is_active = p.name == active_port;
                let port_name = p.name.clone();
                let label = if p.description.is_empty() {
                    port_name.clone()
                } else {
                    format!("{} ({})", p.description, p.available)
                };

                let msg = if is_output {
                    Message::SetOutputPort(device_index, port_name)
                } else {
                    Message::SetInputPort(device_index, port_name)
                };

                port_buttons.push(
                    button(text(label).size(12))
                        .on_press(msg)
                        .style(move |theme, status| {
                            if is_active {
                                styles::app_card(theme, iced::widget::button::Status::Active)
                            } else {
                                styles::app_card(theme, status)
                            }
                        })
                        .padding(8)
                        .into(),
                );
            }

            scrollable(row(port_buttons).spacing(8))
                .height(Length::Shrink)
                .into()
        };

        container(
            column![
                text(title).size(14).color(colors::TEXT_PRIMARY),
                text(details.description.clone()).size(12).color(colors::TEXT_SECONDARY),
                text(format!(
                    "State: {}   Driver: {}   Card: {}",
                    details.state,
                    details.driver.clone().unwrap_or_else(|| "unknown".to_string()),
                    details.card.map(|c| c.to_string()).unwrap_or_else(|| "n/a".to_string()),
                ))
                .size(12)
                .color(colors::TEXT_SECONDARY),
                text(format!(
                    "Sample: {}   Channels: {}",
                    details.sample_spec,
                    details.channel_map
                ))
                .size(12)
                .color(colors::TEXT_SECONDARY),
                text(format!(
                    "Latency: {} µs   Configured: {} µs",
                    details.latency_usec, details.configured_latency_usec
                ))
                .size(12)
                .color(colors::TEXT_SECONDARY),
                text("Ports").size(13).color(colors::TEXT_PRIMARY),
                ports_row,
            ]
            .spacing(8),
        )
        .padding(12)
        .style(|theme| styles::glass_base(theme))
        .into()
    }

    fn view_app_volume_controls(&self) -> Element<'_, Message> {
        container(
            column![
                // Title - make it prominent
                text("Application Volumes").size(20).color(colors::TEXT_PRIMARY).width(Length::Fill),
                
                // App list or empty state
                if self.sink_inputs.is_empty() {
                    Element::from(
                        container(
                            text("No applications playing audio").size(14).color(colors::TEXT_SECONDARY)
                        )
                        .padding(20)
                        .width(Length::Fill)
                    )
                } else {
                    scrollable(
                        column(
                            self.sink_inputs.iter().map(|input| -> Element<Message> {
                                let mute_icon = if input.muted { "🔇" } else { "🔊" };
                                let app_name = input.application_name.clone();
                                let app_icon = "🎵".to_string(); // For now use emoji, can load real icons later
                                let input_index = input.index;
                                let input_volume = input.volume;
                                
                                container(
                                    row![
                                        // App icon - larger and more prominent
                                        container(
                                            text(app_icon.clone()).size(28)
                                        )
                                        .width(48)
                                        .height(48)
                                        .center_x(Length::Fill)
                                        .center_y(Length::Fill),
                                        // App name, MPRIS metadata, and volume info
                                        column![
                                            text(app_name.clone()).size(16).color(colors::TEXT_PRIMARY),
                                            // Show MPRIS metadata if available
                                            if let Some(mpris_meta) = self.sink_input_mpris_metadata.get(&app_name) {
                                                if !mpris_meta.title.is_empty() && mpris_meta.title != format!("Playing from {}", mpris_meta.player_name) {
                                                    column![
                                                        text(format!("{} - {}", mpris_meta.title, mpris_meta.artist))
                                                            .size(12)
                                                            .color(colors::TEXT_SECONDARY),
                                                    ]
                                                    .spacing(2)
                                                } else {
                                                    column![].spacing(2)
                                                }
                                            } else {
                                                column![].spacing(2)
                                            },
                                            text(format!("{:.0}%", input_volume)).size(12).color(colors::TEXT_SECONDARY),
                                        ]
                                        .width(Length::Fill)
                                        .spacing(4),
                                        // Volume slider - make it prominent and wider
                                        slider(0.0..=100.0, input_volume, move |v| Message::AppVolumeChanged(input_index, v))
                                            .width(250)
                                            .step(1.0),
                                        // Mute button - larger
                                        button(text(mute_icon).size(24))
                                            .on_press(Message::AppMuteToggled(input_index))
                                            .style(|theme, status| styles::app_card(theme, status))
                                            .padding(10),
                                    ]
                                    .spacing(20)
                                    .align_y(Alignment::Center)
                                    .padding(15)
                                )
                                .style(|theme| styles::glass_base(theme))
                                .padding(8)
                                .into()
                            }).collect::<Vec<Element<Message>>>()
                        )
                        .spacing(10)
                    )
                    .height(400)  // More height for better visibility
                    .into()
                },
            ]
            .spacing(15)
        )
        .width(Length::Fill)
        .padding(20)
        .style(|theme| styles::glass_base(theme))
        .into()
    }
}

