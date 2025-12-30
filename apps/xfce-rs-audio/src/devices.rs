// Audio device management module
pub struct DeviceManager {
    // Device manager is handled by PulseAudioManager
    // This module provides helper functions for device filtering and sorting
}

impl DeviceManager {

    /// Filter out monitor sources (unless they're the default source)
    pub fn filter_devices(
        outputs: Vec<crate::AudioDevice>,
        inputs: Vec<crate::AudioDevice>,
        default_source_name: Option<&str>,
    ) -> (Vec<crate::AudioDevice>, Vec<crate::AudioDevice>) {
        // Filter inputs: remove monitor sources unless they're default
        let filtered_inputs: Vec<crate::AudioDevice> = inputs
            .into_iter()
            .filter(|device| {
                // Keep if it's the default source
                if let Some(default) = default_source_name {
                    if device.name == default {
                        return true;
                    }
                }
                // Remove monitor sources
                !device.name.ends_with(".monitor")
            })
            .collect();

        (outputs, filtered_inputs)
    }

    /// Sort devices by description, with default device first
    pub fn sort_devices(mut devices: Vec<crate::AudioDevice>) -> Vec<crate::AudioDevice> {
        devices.sort_by(|a, b| {
            // Default device first
            match (a.is_default, b.is_default) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.description.cmp(&b.description),
            }
        });
        devices
    }
}
