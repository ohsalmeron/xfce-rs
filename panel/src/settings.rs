use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use dirs;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PanelSettings {
    // Panel appearance
    pub size: u32,              // Panel height/width (16-128)
    pub icon_size: u32,         // Icon size (0-256, 0 = auto)
    pub dark_mode: bool,        // Dark mode
    
    // Panel position
    pub position: PanelPosition,
    pub position_locked: bool,  // Lock position
    pub span_monitors: bool,    // Span across all monitors
    
    // Panel behavior
    pub autohide: AutohideBehavior,
    pub autohide_size: u32,     // Size when hidden (1-10)
    pub popdown_speed: u32,     // Popdown animation speed (1-100)
    
    // Panel layout
    pub mode: PanelMode,         // Horizontal or vertical
    pub nrows: u32,              // Number of rows (1-6)
    pub length: Option<u32>,    // Fixed length (None = auto)
    pub length_max: Option<u32>, // Maximum length
    
    // Advanced
    pub enable_struts: bool,    // Enable struts (reserve screen space)
    pub keep_below: bool,       // Keep panel below other windows
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PanelPosition {
    Top,
    Bottom,
    Left,
    Right,
}

impl std::fmt::Display for PanelPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PanelPosition::Top => write!(f, "Top"),
            PanelPosition::Bottom => write!(f, "Bottom"),
            PanelPosition::Left => write!(f, "Left"),
            PanelPosition::Right => write!(f, "Right"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PanelMode {
    Horizontal,
    Vertical,
}

impl std::fmt::Display for PanelMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PanelMode::Horizontal => write!(f, "Horizontal"),
            PanelMode::Vertical => write!(f, "Vertical"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AutohideBehavior {
    Never,
    Intelligently,  // Hide when window overlaps
    Always,
}

impl std::fmt::Display for AutohideBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutohideBehavior::Never => write!(f, "Never"),
            AutohideBehavior::Intelligently => write!(f, "Intelligently"),
            AutohideBehavior::Always => write!(f, "Always"),
        }
    }
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            size: 48,
            icon_size: 0,  // Auto
            dark_mode: false,
            position: PanelPosition::Bottom,
            position_locked: false,
            span_monitors: false,
            autohide: AutohideBehavior::Never,
            autohide_size: 3,
            popdown_speed: 25,
            mode: PanelMode::Horizontal,
            nrows: 1,
            length: None,
            length_max: None,
            enable_struts: true,
            keep_below: true,
        }
    }
}

impl PanelSettings {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("xfce-rs")
            .join("panel.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(settings) = toml::from_str(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn get_window_size(&self, screen_width: f32, screen_height: f32) -> (f32, f32) {
        match self.mode {
            PanelMode::Horizontal => {
                let width = self.length.unwrap_or(screen_width as u32) as f32;
                let height = self.size as f32;
                (width, height)
            }
            PanelMode::Vertical => {
                let width = self.size as f32;
                let height = self.length.unwrap_or(screen_height as u32) as f32;
                (width, height)
            }
        }
    }

    pub fn get_window_position(&self, screen_width: f32, screen_height: f32) -> (f32, f32) {
        let (width, height) = self.get_window_size(screen_width, screen_height);
        match self.position {
            PanelPosition::Top => (0.0, 0.0),
            PanelPosition::Bottom => (0.0, screen_height - height),
            PanelPosition::Left => (0.0, 0.0),
            PanelPosition::Right => (screen_width - width, 0.0),
        }
    }
}
