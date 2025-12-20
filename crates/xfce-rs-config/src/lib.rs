use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::error;

/// Error types for configuration operations
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Invalid configuration format: {reason}")]
    InvalidFormat { reason: String },
    
    #[error("Configuration property not found: {channel}.{property}")]
    PropertyNotFound { channel: String, property: String },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

/// Configuration value types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfigValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Float(f64),
    Array(Vec<ConfigValue>),
}

/// Configuration channel containing properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChannel {
    pub properties: HashMap<String, ConfigValue>,
}

impl ConfigChannel {
    pub fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }
    
    pub fn get(&self, property: &str) -> Option<&ConfigValue> {
        self.properties.get(property)
    }
    
    pub fn set(&mut self, property: String, value: ConfigValue) {
        self.properties.insert(property, value);
    }
    
    pub fn remove(&mut self, property: &str) -> Option<ConfigValue> {
        self.properties.remove(property)
    }
}

/// Configuration change watcher
pub type ConfigWatcher = Box<dyn Fn(&str, &str, &ConfigValue) + Send + Sync>;

/// Main configuration system
pub struct XfceConfig {
    channels: RwLock<HashMap<String, ConfigChannel>>,
    config_path: String,
    _watchers: Vec<ConfigWatcher>,
}

impl std::fmt::Debug for XfceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XfceConfig")
            .field("config_path", &self.config_path)
            .field("channels", &"RwLock<HashMap<...>>")
            .field("_watchers", &"<ConfigWatchers>")
            .finish()
    }
}

impl XfceConfig {
    pub fn new(config_path: impl Into<String>) -> Result<Self, ConfigError> {
        let config_path = config_path.into();
        let config = Self::load_from_file(&config_path)?;
        
        Ok(Self {
            channels: RwLock::new(config),
            config_path,
            _watchers: Vec::new(),
        })
    }
    
    /// Load configuration from file
    fn load_from_file(path: &str) -> Result<HashMap<String, ConfigChannel>, ConfigError> {
        if !std::path::Path::new(path).exists() {
            return Ok(HashMap::new());
        }
        
        let content = std::fs::read_to_string(path)?;
        let config: HashMap<String, ConfigChannel> = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// Save configuration to file
    pub async fn save(&self) -> Result<(), ConfigError> {
        let channels = self.channels.read().await;
        let content = toml::to_string_pretty(&*channels)
            .map_err(|e| ConfigError::InvalidFormat { reason: e.to_string() })?;
        
        if let Some(parent) = std::path::Path::new(&self.config_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        tokio::fs::write(&self.config_path, content).await?;
        Ok(())
    }
    
    /// Get a configuration property
    pub async fn get_property(&self, channel: &str, property: &str) -> Result<ConfigValue, ConfigError> {
        let channels = self.channels.read().await;
        
        channels
            .get(channel)
            .ok_or_else(|| ConfigError::PropertyNotFound {
                channel: channel.to_string(),
                property: property.to_string(),
            })?
            .get(property)
            .cloned()
            .ok_or_else(|| ConfigError::PropertyNotFound {
                channel: channel.to_string(),
                property: property.to_string(),
            })
    }
    
    /// Set a configuration property
    pub async fn set_property(&self, channel: &str, property: &str, value: ConfigValue) -> Result<(), ConfigError> {
        {
            let mut channels = self.channels.write().await;
            
            let channel_entry = channels.entry(channel.to_string()).or_insert_with(ConfigChannel::new);
            channel_entry.set(property.to_string(), value.clone());
        }
        
        self.save().await?;
        Ok(())
    }
    
    /// List all channels
    pub async fn list_channels(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }
    
    /// List properties in a channel
    pub async fn list_properties(&self, channel: &str) -> Result<Vec<String>, ConfigError> {
        let channels = self.channels.read().await;
        
        let channel = channels.get(channel)
            .ok_or_else(|| ConfigError::PropertyNotFound {
                channel: channel.to_string(),
                property: "".to_string(),
            })?;
        
        Ok(channel.properties.keys().cloned().collect())
    }
}

impl Default for XfceConfig {
    fn default() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            config_path: dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("xfce-rs")
                .join("config.toml")
                .to_string_lossy()
                .to_string(),
            _watchers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_config_basic_operations() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let config = XfceConfig::new(config_path.to_string_lossy()).unwrap();
        
        // Test setting and getting a property
        config.set_property("test", "string_prop", ConfigValue::String("test".to_string())).await.unwrap();
        let value = config.get_property("test", "string_prop").await.unwrap();
        assert_eq!(value, ConfigValue::String("test".to_string()));
        
        // Test integer property
        config.set_property("test", "int_prop", ConfigValue::Integer(42)).await.unwrap();
        let value = config.get_property("test", "int_prop").await.unwrap();
        assert_eq!(value, ConfigValue::Integer(42));
    }
    
    #[tokio::test]
    async fn test_channel_listing() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        let config = XfceConfig::new(config_path.to_string_lossy()).unwrap();
        
        config.set_property("channel1", "prop1", ConfigValue::Boolean(true)).await.unwrap();
        config.set_property("channel2", "prop2", ConfigValue::Float(3.14)).await.unwrap();
        
        let channels = config.list_channels().await;
        assert_eq!(channels.len(), 2);
        assert!(channels.contains(&"channel1".to_string()));
        assert!(channels.contains(&"channel2".to_string()));
    }
}