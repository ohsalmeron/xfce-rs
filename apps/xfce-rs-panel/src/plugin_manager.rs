use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::collections::HashMap;
use anyhow::{Result, Context};
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub binary: PathBuf,
    pub description: String,
    pub detached: bool, // If true, runs as separate window; if false, embedded
}

pub struct PluginManager {
    plugin_dir: PathBuf,
    running_plugins: HashMap<String, std::process::Child>,
}

impl PluginManager {
    pub fn new() -> Self {
        // Look for plugins in target/debug or target/release
        let plugin_dir = if Path::new("target/debug").exists() {
            PathBuf::from("target/debug")
        } else {
            PathBuf::from("target/release")
        };

        Self {
            plugin_dir,
            running_plugins: HashMap::new(),
        }
    }

    pub fn discover_plugins(&self) -> Vec<PluginInfo> {
        let mut plugins = Vec::new();

        // Look for our Rust panel plugins
        let plugin_binaries = [
            ("xfce-rs-clock", "Clock Plugin", false),
            ("xfce-rs-separator", "Separator", false),
            ("xfce-rs-showdesktop", "Show Desktop", false),
        ];

        for (bin_name, desc, detached) in plugin_binaries.iter() {
            let binary_path = self.plugin_dir.join(bin_name);
            if binary_path.exists() {
                let binary_path_clone = binary_path.clone();
                plugins.push(PluginInfo {
                    name: bin_name.to_string(),
                    binary: binary_path,
                    description: desc.to_string(),
                    detached: *detached,
                });
                info!("Found plugin: {} at {:?}", bin_name, binary_path_clone);
            } else {
                warn!("Plugin binary not found: {:?}", binary_path);
            }
        }

        plugins
    }

    pub fn start_plugin(&mut self, plugin: &PluginInfo) -> Result<()> {
        if self.running_plugins.contains_key(&plugin.name) {
            warn!("Plugin {} is already running", plugin.name);
            return Ok(());
        }

        info!("Starting plugin: {} ({:?})", plugin.name, plugin.binary);

        let mut cmd = Command::new(&plugin.binary);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn()
            .with_context(|| format!("Failed to spawn plugin: {}", plugin.name))?;

        self.running_plugins.insert(plugin.name.clone(), child);
        info!("Plugin {} started successfully", plugin.name);

        Ok(())
    }

    pub fn stop_plugin(&mut self, name: &str) -> Result<()> {
        if let Some(mut child) = self.running_plugins.remove(name) {
            info!("Stopping plugin: {}", name);
            child.kill()
                .with_context(|| format!("Failed to kill plugin: {}", name))?;
            let _ = child.wait();
            info!("Plugin {} stopped", name);
        }
        Ok(())
    }

    pub fn stop_all(&mut self) {
        let names: Vec<String> = self.running_plugins.keys().cloned().collect();
        for name in names {
            if let Err(e) = self.stop_plugin(&name) {
                error!("Error stopping plugin {}: {}", name, e);
            }
        }
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}
