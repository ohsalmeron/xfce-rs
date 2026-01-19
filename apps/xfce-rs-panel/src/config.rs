//! Configuration loading and management.

use serde::{Deserialize, Serialize};
use anyhow::Result;
use dirs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub position: PanelPosition,
    pub height: u32,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PanelPosition {
    Top,
    Bottom,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            position: PanelPosition::Bottom,
            height: 48,
            plugins: vec![
                "clock".to_string(),
            ],
        }
    }
}

impl PanelConfig {
    pub fn load() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot find config directory"))?
            .join("xfce-rs");
        
        let config_file = config_dir.join("panel.toml");
        
        if !config_file.exists() {
            // Create default config
            std::fs::create_dir_all(&config_dir)?;
            let default_config = Self::default();
            let toml = toml::to_string_pretty(&default_config)?;
            std::fs::write(&config_file, toml)?;
            log::info!("Created default config at {:?}", config_file);
            return Ok(default_config);
        }

        let content = std::fs::read_to_string(&config_file)?;
        let config: Self = toml::from_str(&content)?;
        log::debug!("Loaded config from {:?}", config_file);
        Ok(config)
    }
}
