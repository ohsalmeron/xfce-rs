use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::collections::HashMap;
use anyhow::{Result, Context};
use tracing::{info, warn, error};
use serde::{Deserialize, Serialize};
use std::fs;

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
        // Look for plugins in target/release first (production), then target/debug (development)
        let plugin_dir = if Path::new("target/release").exists() {
            PathBuf::from("target/release")
        } else {
            PathBuf::from("target/debug")
        };

        Self {
            plugin_dir,
            running_plugins: HashMap::new(),
        }
    }

    /// Discover all available plugin binaries in the plugin directory
    pub fn discover_available_plugins(&self) -> Vec<PluginInfo> {
        let mut plugins = Vec::new();

        // Scan plugin directory for all xfce-rs-* binaries
        if let Ok(entries) = fs::read_dir(&self.plugin_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            // Check if it's a plugin binary (starts with xfce-rs-)
                            if file_name.starts_with("xfce-rs-") && !file_name.ends_with(".toml") {
                                // Generate a description from the binary name
                                let description = file_name
                                    .strip_prefix("xfce-rs-")
                                    .unwrap_or(file_name)
                                    .replace("-", " ")
                                    .split_whitespace()
                                    .map(|word| {
                                        let mut chars = word.chars();
                                        match chars.next() {
                                            None => String::new(),
                                            Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str(),
                                        }
                                    })
                                    .collect::<Vec<String>>()
                                    .join(" ");

                                plugins.push(PluginInfo {
                                    name: file_name.to_string(),
                                    binary: path.clone(),
                                    description,
                                    detached: false, // All plugins are embedded by default
                                });
                                info!("Discovered plugin: {} at {:?}", file_name, path);
                            }
                        }
                    }
                }
            }
        } else {
            warn!("Failed to read plugin directory: {:?}", self.plugin_dir);
        }

        plugins
    }

    /// Discover plugins (backward compatibility - returns all available)
    pub fn discover_plugins(&self) -> Vec<PluginInfo> {
        self.discover_available_plugins()
    }

    pub fn start_plugin(&mut self, plugin: &PluginInfo, position: Option<(f32, f32, f32, f32)>) -> Result<()> {
        if self.running_plugins.contains_key(&plugin.name) {
            warn!("Plugin {} is already running", plugin.name);
            return Ok(());
        }

        info!("Starting plugin: {} ({:?})", plugin.name, plugin.binary);

        let mut cmd = Command::new(&plugin.binary);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Pass position arguments if provided (for embedded plugins)
        if let Some((x, y, width, height)) = position {
            if !plugin.detached {
                cmd.arg("--panel-x").arg(x.to_string());
                cmd.arg("--panel-y").arg(y.to_string());
                cmd.arg("--panel-width").arg(width.to_string());
                cmd.arg("--panel-height").arg(height.to_string());
                info!("Starting plugin {} at position: ({}, {}) size: {}x{}", plugin.name, x, y, width, height);
            }
        }

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
