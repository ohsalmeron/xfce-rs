// PulseAudio integration module - Real implementation using pulsectl-rs
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, debug};
use once_cell::sync::Lazy;
use pulsectl::controllers::{SinkController, SourceController, DeviceControl};
use pulsectl::controllers::types::{DeviceInfo, ServerInfo};

// PulseAudio constants
const PA_VOLUME_NORM: u32 = 0x10000; // 65536

pub struct PulseAudioManager {
    sinks: Arc<Mutex<HashMap<String, SinkInfo>>>,
    sources: Arc<Mutex<HashMap<String, SourceInfo>>>,
    default_sink: Arc<Mutex<Option<String>>>,
    default_source: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone)]
pub struct SinkInfo {
    pub name: String,
    pub description: String,
    pub index: u32,
    pub volume: f32,
    pub muted: bool,
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub name: String,
    pub description: String,
    pub index: u32,
    pub volume: f32,
    pub muted: bool,
}

impl PulseAudioManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            sinks: Arc::new(Mutex::new(HashMap::new())),
            sources: Arc::new(Mutex::new(HashMap::new())),
            default_sink: Arc::new(Mutex::new(None)),
            default_source: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to PulseAudio daemon");
        // Refresh device lists
        self.refresh_devices().await?;
        Ok(())
    }

    async fn refresh_devices(&self) -> Result<()> {
        let sinks = self.sinks.clone();
        let sources = self.sources.clone();
        let default_sink = self.default_sink.clone();
        let default_source = self.default_source.clone();
        
        tokio::task::spawn_blocking(move || {
            Self::refresh_devices_blocking(
                sinks,
                sources,
                default_sink,
                default_source,
            )
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(())
    }

    fn refresh_devices_blocking(
        sinks: Arc<Mutex<HashMap<String, SinkInfo>>>,
        sources: Arc<Mutex<HashMap<String, SourceInfo>>>,
        default_sink: Arc<Mutex<Option<String>>>,
        default_source: Arc<Mutex<Option<String>>>,
    ) -> Result<()> {
        // Get sinks (output devices)
        let mut sink_controller = SinkController::create()
            .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {}", e))?;
        
        let server_info = sink_controller.get_server_info()
            .map_err(|e| anyhow::anyhow!("Failed to get server info: {}", e))?;
        
        let default_sink_name = server_info.default_sink_name.clone();
        *default_sink.lock().unwrap() = default_sink_name.clone();
        
        let devices = sink_controller.list_devices()
            .map_err(|e| anyhow::anyhow!("Failed to list sinks: {}", e))?;
        
        let mut sinks_map = sinks.lock().unwrap();
        sinks_map.clear();
        for device in devices {
            let volume_percent = if device.volume.get().len() > 0 {
                let vol = device.volume.get()[0];
                (vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0
            } else {
                0.0
            };
            
            sinks_map.insert(device.name.clone().unwrap_or_default(), SinkInfo {
                name: device.name.clone().unwrap_or_default(),
                description: device.description.clone().unwrap_or_default(),
                index: device.index,
                volume: volume_percent,
                muted: device.mute,
            });
        }
        
        // Get sources (input devices)
        let mut source_controller = SourceController::create()
            .map_err(|e| anyhow::anyhow!("Failed to create SourceController: {}", e))?;
        
        let server_info = source_controller.get_server_info()
            .map_err(|e| anyhow::anyhow!("Failed to get server info: {}", e))?;
        
        let default_source_name = server_info.default_source_name.clone();
        *default_source.lock().unwrap() = default_source_name.clone();
        
        let devices = source_controller.list_devices()
            .map_err(|e| anyhow::anyhow!("Failed to list sources: {}", e))?;
        
        let mut sources_map = sources.lock().unwrap();
        sources_map.clear();
        for device in devices {
            let volume_percent = if device.volume.get().len() > 0 {
                let vol = device.volume.get()[0];
                (vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0
            } else {
                0.0
            };
            
            sources_map.insert(device.name.clone().unwrap_or_default(), SourceInfo {
                name: device.name.clone().unwrap_or_default(),
                description: device.description.clone().unwrap_or_default(),
                index: device.index,
                volume: volume_percent,
                muted: device.mute,
            });
        }
        
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        // Always consider connected if we can create controllers
        true
    }

    pub async fn get_volume(&self) -> Result<(f32, bool)> {
        // Get default sink volume
        let default_sink_name = self.default_sink.lock().unwrap().clone();
        let sink_name = default_sink_name.ok_or_else(|| anyhow::anyhow!("No default sink"))?;
        
        tokio::task::spawn_blocking(move || -> Result<(f32, bool), anyhow::Error> {
            let mut controller = SinkController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {:?}", e))?;
            
            let device = controller.get_device_by_name(&sink_name)
                .map_err(|e| anyhow::anyhow!("Failed to get default sink: {:?}", e))?;
            
            let volume_percent = if device.volume.get().len() > 0 {
                let vol = device.volume.get()[0];
                (vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0
            } else {
                0.0
            };
            
            Ok((volume_percent, device.mute))
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }

    pub async fn set_volume(&self, volume: f32) -> Result<()> {
        let default_sink_name = self.default_sink.lock().unwrap().clone();
        let sink_name = default_sink_name.ok_or_else(|| anyhow::anyhow!("No default sink"))?;
        let volume_clone = volume;
        
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let mut controller = SinkController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {:?}", e))?;
            
            let mut device = controller.get_device_by_name(&sink_name)
                .map_err(|e| anyhow::anyhow!("Failed to get default sink: {:?}", e))?;
            
            // Calculate volume delta
            let current_vol = if device.volume.get().len() > 0 {
                device.volume.get()[0]
            } else {
                libpulse_binding::volume::Volume(PA_VOLUME_NORM)
            };
            
            let current_percent = (current_vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0;
            let delta_percent = volume_clone - current_percent;
            
            if delta_percent.abs() < 0.1 {
                return Ok(());
            }
            
            // Use increase/decrease
            let delta_volume = if delta_percent > 0.0 {
                let delta_ratio = delta_percent / 100.0;
                let delta_vol = libpulse_binding::volume::Volume((delta_ratio * PA_VOLUME_NORM as f32) as u32);
                device.volume.increase(delta_vol)
            } else {
                let delta_ratio = delta_percent.abs() / 100.0;
                let delta_vol = libpulse_binding::volume::Volume((delta_ratio * PA_VOLUME_NORM as f32) as u32);
                device.volume.decrease(delta_vol)
            };
            
            let channel_volumes = delta_volume.ok_or_else(|| {
                anyhow::anyhow!("Failed to calculate new volume")
            })?;
            
            controller.set_device_volume_by_name(&sink_name, &channel_volumes);
            
            info!("Set sink volume to {:.1}%", volume_clone);
            Ok(())
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }

    pub async fn set_mute(&self, muted: bool) -> Result<()> {
        let default_sink_name = self.default_sink.lock().unwrap().clone();
        let sink_name = default_sink_name.ok_or_else(|| anyhow::anyhow!("No default sink"))?;
        let muted_clone = muted;
        
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let mut controller = SinkController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {:?}", e))?;
            
            controller.set_device_mute_by_name(&sink_name, muted_clone);
            
            info!("Set sink mute to {}", muted_clone);
            Ok(())
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }

    pub async fn get_mic_volume(&self) -> Result<(f32, bool)> {
        // Get default source volume
        let default_source_name = self.default_source.lock().unwrap().clone();
        let source_name = default_source_name.ok_or_else(|| anyhow::anyhow!("No default source"))?;
        
        tokio::task::spawn_blocking(move || -> Result<(f32, bool), anyhow::Error> {
            let mut controller = SourceController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SourceController: {:?}", e))?;
            
            let device = controller.get_device_by_name(&source_name)
                .map_err(|e| anyhow::anyhow!("Failed to get default source: {:?}", e))?;
            
            let volume_percent = if device.volume.get().len() > 0 {
                let vol = device.volume.get()[0];
                (vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0
            } else {
                0.0
            };
            
            Ok((volume_percent, device.mute))
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }

    pub async fn set_mic_volume(&self, volume: f32) -> Result<()> {
        let default_source_name = self.default_source.lock().unwrap().clone();
        let source_name = default_source_name.ok_or_else(|| anyhow::anyhow!("No default source"))?;
        let volume_clone = volume;
        
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let mut controller = SourceController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SourceController: {:?}", e))?;
            
            let mut device = controller.get_device_by_name(&source_name)
                .map_err(|e| anyhow::anyhow!("Failed to get default source: {:?}", e))?;
            
            // Calculate volume delta
            let current_vol = if device.volume.get().len() > 0 {
                device.volume.get()[0]
            } else {
                libpulse_binding::volume::Volume(PA_VOLUME_NORM)
            };
            
            let current_percent = (current_vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0;
            let delta_percent = volume_clone - current_percent;
            
            if delta_percent.abs() < 0.1 {
                return Ok(());
            }
            
            // Use increase/decrease
            let delta_volume = if delta_percent > 0.0 {
                let delta_ratio = delta_percent / 100.0;
                let delta_vol = libpulse_binding::volume::Volume((delta_ratio * PA_VOLUME_NORM as f32) as u32);
                device.volume.increase(delta_vol)
            } else {
                let delta_ratio = delta_percent.abs() / 100.0;
                let delta_vol = libpulse_binding::volume::Volume((delta_ratio * PA_VOLUME_NORM as f32) as u32);
                device.volume.decrease(delta_vol)
            };
            
            let channel_volumes = delta_volume.ok_or_else(|| {
                anyhow::anyhow!("Failed to calculate new volume")
            })?;
            
            controller.set_device_volume_by_name(&source_name, &channel_volumes);
            
            info!("Set source volume to {:.1}%", volume_clone);
            Ok(())
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }

    pub async fn set_mic_mute(&self, muted: bool) -> Result<()> {
        let default_source_name = self.default_source.lock().unwrap().clone();
        let source_name = default_source_name.ok_or_else(|| anyhow::anyhow!("No default source"))?;
        let muted_clone = muted;
        
        tokio::task::spawn_blocking(move || -> Result<_, anyhow::Error> {
            let mut controller = SourceController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SourceController: {:?}", e))?;
            
            controller.set_device_mute_by_name(&source_name, muted_clone);
            
            info!("Set source mute to {}", muted_clone);
            Ok(())
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }

    pub async fn get_devices(&self) -> Result<(Vec<crate::AudioDevice>, Vec<crate::AudioDevice>)> {
        // Refresh devices first
        self.refresh_devices().await?;

        let sinks = self.sinks.lock().unwrap();
        let sources = self.sources.lock().unwrap();
        let default_sink = self.default_sink.lock().unwrap().clone();
        let default_source = self.default_source.lock().unwrap().clone();

        let output_devices: Vec<crate::AudioDevice> = sinks.values()
            .map(|sink| crate::AudioDevice {
                name: sink.name.clone(),
                description: sink.description.clone(),
                index: sink.index,
                is_default: Some(&sink.name) == default_sink.as_ref(),
            })
            .collect();

        let input_devices: Vec<crate::AudioDevice> = sources.values()
            .map(|source| crate::AudioDevice {
                name: source.name.clone(),
                description: source.description.clone(),
                index: source.index,
                is_default: Some(&source.name) == default_source.as_ref(),
            })
            .collect();

        Ok((output_devices, input_devices))
    }

    pub async fn set_default_output(&self, device_name: &str) -> Result<()> {
        let device_name = device_name.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let mut controller = SinkController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {:?}", e))?;
            
            controller.set_default_device(&device_name)
                .map_err(|e| anyhow::anyhow!("Failed to set default sink: {:?}", e))?;
            
            info!("Default output set to {}", device_name);
            Ok(())
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }

    pub async fn set_default_input(&self, device_name: &str) -> Result<()> {
        let device_name = device_name.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let mut controller = SourceController::create()
                .map_err(|e| anyhow::anyhow!("Failed to create SourceController: {:?}", e))?;
            
            controller.set_default_device(&device_name)
                .map_err(|e| anyhow::anyhow!("Failed to set default source: {:?}", e))?;
            
            info!("Default input set to {}", device_name);
            Ok(())
        }).await
            .map_err(|e| anyhow::anyhow!("Task error: {}", e))?
    }
}

// Global manager instance
static MANAGER: once_cell::sync::Lazy<Arc<PulseAudioManager>> = once_cell::sync::Lazy::new(|| {
    Arc::new(PulseAudioManager::new().expect("Failed to create PulseAudio manager"))
});

// Public API functions
pub async fn init() -> Result<()> {
    info!("Initializing PulseAudio connection");
    MANAGER.connect().await?;
    Ok(())
}

pub async fn set_volume(volume: f32) -> Result<()> {
    MANAGER.set_volume(volume).await
}

pub async fn set_mute(muted: bool) -> Result<()> {
    MANAGER.set_mute(muted).await
}

pub async fn set_mic_volume(volume: f32) -> Result<()> {
    MANAGER.set_mic_volume(volume).await
}

pub async fn set_mic_mute(muted: bool) -> Result<()> {
    MANAGER.set_mic_mute(muted).await
}

pub async fn set_default_output(device_index: u32) -> Result<()> {
    let (outputs, _) = MANAGER.get_devices().await?;
    if let Some(device) = outputs.iter().find(|d| d.index == device_index) {
        MANAGER.set_default_output(&device.name).await
    } else {
        Err(anyhow::anyhow!("Device not found"))
    }
}

pub async fn set_default_input(device_index: u32) -> Result<()> {
    let (_, inputs) = MANAGER.get_devices().await?;
    if let Some(device) = inputs.iter().find(|d| d.index == device_index) {
        MANAGER.set_default_input(&device.name).await
    } else {
        Err(anyhow::anyhow!("Device not found"))
    }
}

pub async fn get_devices() -> Result<(Vec<crate::AudioDevice>, Vec<crate::AudioDevice>)> {
    MANAGER.get_devices().await
}

pub async fn get_volume() -> Result<(f32, bool)> {
    MANAGER.get_volume().await
}

pub async fn get_mic_volume() -> Result<(f32, bool)> {
    MANAGER.get_mic_volume().await
}
