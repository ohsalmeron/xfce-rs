// PulseAudio sink input management for per-application volume control
// Using pulsectl-rs for real PulseAudio integration
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, debug};
use once_cell::sync::Lazy;
use pulsectl::controllers::{SinkController, AppControl};

// PulseAudio constants
const PA_VOLUME_NORM: u32 = 0x10000; // 65536
const PA_PROP_APPLICATION_NAME: &str = "application.name";
const PA_PROP_APPLICATION_ICON_NAME: &str = "application.icon_name";
const PA_PROP_APPLICATION_ID: &str = "application.id";

#[derive(Debug, Clone, PartialEq)]
pub struct SinkInput {
    pub index: u32,
    pub name: String,
    pub application_name: String,
    pub application_icon: Option<String>,
    pub volume: f32,
    pub muted: bool,
    pub sink_index: u32,
}

pub struct SinkInputManager {
    inputs: Arc<Mutex<HashMap<u32, SinkInput>>>,
}

impl SinkInputManager {
    pub fn new() -> Self {
        Self {
            inputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_sink_inputs(&self) -> Result<Vec<SinkInput>> {
        // Run blocking PulseAudio operations in a blocking task
        // Create controller in the blocking task since it's not Send
        let inputs_cache = self.inputs.clone();
        
        let result = tokio::task::spawn_blocking(move || {
            Self::get_sink_inputs_blocking(inputs_cache)
        }).await.map_err(|e| anyhow::anyhow!("Task error: {}", e))??;
        
        Ok(result)
    }

    fn get_sink_inputs_blocking(
        inputs_cache: Arc<Mutex<HashMap<u32, SinkInput>>>,
    ) -> Result<Vec<SinkInput>> {
        // Create controller in this thread
        let mut controller = SinkController::create()
            .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {}", e))?;
        
        // Get applications (sink inputs)
        let apps = controller.list_applications()
            .map_err(|e| anyhow::anyhow!("Failed to list applications: {}", e))?;
        
        let mut sink_inputs = Vec::new();
        
        for app in apps {
            let index = app.index;
            let name = app.name.clone().unwrap_or_else(|| format!("Unknown-{}", index));
            
            // Get application name from proplist
            let application_name = app.proplist
                .get_str(PA_PROP_APPLICATION_NAME)
                .unwrap_or_else(|| name.clone());
            
            // Get application icon from proplist
            let application_icon = app.proplist
                .get_str(PA_PROP_APPLICATION_ICON_NAME)
                .or_else(|| app.proplist.get_str(PA_PROP_APPLICATION_ID));
            
            // Calculate volume percentage
            // ChannelVolumes has a get() method that returns a slice of Volume
            let volume_percent = if app.volume.get().len() > 0 {
                let vol = app.volume.get()[0];
                (vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0
            } else {
                0.0
            };
            
            let muted = app.mute;
            let sink_index = app.connection_id;
            
            debug!("Sink input {}: {} (app: {}, volume: {:.1}%, muted: {})", 
                index, name, application_name, volume_percent, muted);
            
            let sink_input = SinkInput {
                index,
                name: name.clone(),
                application_name: application_name.clone(),
                application_icon: application_icon.map(|s| s.to_string()),
                volume: volume_percent,
                muted,
                sink_index,
            };
            
            sink_inputs.push(sink_input);
        }
        
        // Update cache
        let mut cache = inputs_cache.lock().unwrap();
        cache.clear();
        for input in &sink_inputs {
            cache.insert(input.index, input.clone());
        }
        
        info!("Found {} sink inputs", sink_inputs.len());
        Ok(sink_inputs)
    }

    pub async fn set_sink_input_volume(&self, index: u32, volume: f32) -> Result<()> {
        // Note: UI state is updated immediately in main.rs for smooth slider movement
        // This function only updates PulseAudio
        
        // Set volume in PulseAudio
        let volume_clone = volume;
        tokio::task::spawn_blocking(move || {
            Self::set_sink_input_volume_blocking(index, volume_clone)
        }).await.map_err(|e| anyhow::anyhow!("Task error: {}", e))??;
        
        // Update cache after successful PulseAudio update
        {
            let mut inputs = self.inputs.lock().unwrap();
            if let Some(input) = inputs.get_mut(&index) {
                input.volume = volume;
            }
        }
        
        Ok(())
    }

    fn set_sink_input_volume_blocking(
        index: u32,
        volume: f32,
    ) -> Result<()> {
        // Create controller in this thread
        let mut controller = SinkController::create()
            .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {}", e))?;
        
        // Get current app info to get channel map and current volume
        let mut app = controller.get_app_by_index(index)
            .map_err(|e| anyhow::anyhow!("Failed to get app by index {}: {}", index, e))?;
        
        // Get current average volume
        let current_vol = if app.volume.get().len() > 0 {
            app.volume.get()[0]
        } else {
            libpulse_binding::volume::Volume(PA_VOLUME_NORM)
        };
        
        // Calculate current percentage
        let current_percent = (current_vol.0 as f32 / PA_VOLUME_NORM as f32) * 100.0;
        let delta_percent = volume - current_percent;
        
        // If already close to target, skip (optimization)
        if delta_percent.abs() < 0.1 {
            return Ok(());
        }
        
        // Use increase/decrease methods which are safe
        // Calculate the volume delta needed (as a percentage of NORM)
        let delta_volume = if delta_percent > 0.0 {
            // Need to increase - calculate delta as percentage
            let delta_ratio = delta_percent / 100.0;
            let delta_vol = libpulse_binding::volume::Volume((delta_ratio * PA_VOLUME_NORM as f32) as u32);
            app.volume.increase(delta_vol)
        } else {
            // Need to decrease
            let delta_ratio = delta_percent.abs() / 100.0;
            let delta_vol = libpulse_binding::volume::Volume((delta_ratio * PA_VOLUME_NORM as f32) as u32);
            app.volume.decrease(delta_vol)
        };
        
        // Use the result from increase/decrease - this is the safe way
        let channel_volumes = delta_volume.ok_or_else(|| {
            anyhow::anyhow!("Failed to calculate new volume (increase/decrease returned None)")
        })?;
        
        // Set the volume using introspect API
        let op = controller.handler.introspect.set_sink_input_volume(
            index,
            &channel_volumes,
            None,
        );
        controller.handler.wait_for_operation(op)
            .map_err(|e| anyhow::anyhow!("Failed to set volume: {}", e))?;
        
        debug!("Set sink input {} volume to {:.1}%", index, volume);
        Ok(())
    }

    pub async fn set_sink_input_mute(&self, index: u32, muted: bool) -> Result<()> {
        // Update local cache immediately for UI responsiveness
        {
            let mut inputs = self.inputs.lock().unwrap();
            if let Some(input) = inputs.get_mut(&index) {
                input.muted = muted;
            }
        }
        
        // Set mute in PulseAudio
        let muted_clone = muted;
        tokio::task::spawn_blocking(move || {
            Self::set_sink_input_mute_blocking(index, muted_clone)
        }).await.map_err(|e| anyhow::anyhow!("Task error: {}", e))??;
        
        Ok(())
    }

    fn set_sink_input_mute_blocking(
        index: u32,
        muted: bool,
    ) -> Result<()> {
        // Create controller in this thread
        let mut controller = SinkController::create()
            .map_err(|e| anyhow::anyhow!("Failed to create SinkController: {}", e))?;
        
        controller.set_app_mute(index, muted)
            .map_err(|e| anyhow::anyhow!("Failed to set mute: {}", e))?;
        
        info!("Set sink input {} mute to {}", index, muted);
        Ok(())
    }
}

// Global manager instance
static MANAGER: Lazy<Arc<SinkInputManager>> = Lazy::new(|| {
    Arc::new(SinkInputManager::new())
});

// Public API functions
pub async fn get_sink_inputs() -> Result<Vec<SinkInput>> {
    MANAGER.get_sink_inputs().await
}

pub async fn set_sink_input_volume(index: u32, volume: f32) -> Result<()> {
    MANAGER.set_sink_input_volume(index, volume).await
}

pub async fn set_sink_input_mute(index: u32, muted: bool) -> Result<()> {
    MANAGER.set_sink_input_mute(index, muted).await
}
