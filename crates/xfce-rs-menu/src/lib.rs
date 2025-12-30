use thiserror::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Error types for menu operations
#[derive(Error, Debug)]
pub enum MenuError {
    #[error("Desktop file not found: {path}")]
    DesktopFileNotFound { path: String },
    
    #[error("Invalid desktop file format: {reason}")]
    InvalidDesktopFile { reason: String },
    
    #[error("Menu file not found: {path}")]
    MenuFileNotFound { path: String },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Desktop entry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub description: String,
    pub categories: Vec<String>,
    pub terminal: bool,
    pub no_display: bool,
    pub hidden: bool,
}

impl Default for DesktopEntry {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            exec: "".to_string(),
            icon: "application-x-executable".to_string(),
            description: "".to_string(),
            categories: Vec::new(),
            terminal: false,
            no_display: false,
            hidden: false,
        }
    }
}

/// Desktop menu structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopMenu {
    pub name: String,
    pub icon: Option<String>,
    pub entries: Vec<MenuEntry>,
    pub submenus: HashMap<String, DesktopMenu>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MenuEntry {
    Separator,
    Application(DesktopEntry),
    Submenu(String),
}

/// Menu parser for freedesktop.org menu specification
#[derive(Debug)]
pub struct MenuParser {
    desktop_dirs: Vec<PathBuf>,
    menu_dirs: Vec<PathBuf>,
}

impl MenuParser {
    pub fn new() -> Self {
        let mut parser = Self {
            desktop_dirs: Vec::new(),
            menu_dirs: Vec::new(),
        };
        
        // Add standard desktop directories
        if let Some(home) = dirs::home_dir() {
            parser.desktop_dirs.push(home.join(".local/share/applications"));
            parser.desktop_dirs.push(home.join(".share/applications"));
        }
        
        // Add system desktop directories
        parser.desktop_dirs.push(PathBuf::from("/usr/share/applications"));
        parser.desktop_dirs.push(PathBuf::from("/usr/local/share/applications"));
        
        // Add menu directories
        if let Some(home) = dirs::home_dir() {
            parser.menu_dirs.push(home.join(".config/menus"));
        }
        parser.menu_dirs.push(PathBuf::from("/etc/xdg/menus"));
        
        parser
    }
    
    /// Parse all desktop files
    pub fn parse_desktop_entries(&self) -> Result<Vec<DesktopEntry>, MenuError> {
        let mut entries = Vec::new();
        
        for desktop_dir in &self.desktop_dirs {
            if !desktop_dir.exists() {
                continue;
            }
            
            for entry in WalkDir::new(desktop_dir)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path().extension().map_or(false, |ext| ext == "desktop")
                })
            {
                if let Ok(desktop_entry) = self.parse_desktop_file(entry.path()) {
                    if !desktop_entry.no_display && !desktop_entry.hidden {
                        entries.push(desktop_entry);
                    }
                }
            }
        }
        
        // Sort entries by name
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
    
    /// Parse a single .desktop file
    fn parse_desktop_file(&self, path: &std::path::Path) -> Result<DesktopEntry, MenuError> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| MenuError::DesktopFileNotFound { 
                path: path.to_string_lossy().to_string() 
            })?;
        
        let mut entry = DesktopEntry::default();
        let mut in_desktop_entry = false;
        
        for line in content.lines() {
            let line = line.trim();
            
            // Skip comments and empty lines
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            
            // Check for [Desktop Entry] section
            if line == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            }
            
            if line.starts_with('[') && line != "[Desktop Entry]" {
                in_desktop_entry = false;
                continue;
            }
            
            if !in_desktop_entry {
                continue;
            }
            
            // Parse key=value pairs
            if let Some((key, value)) = line.split_once('=') {
                match key.trim() {
                    "Name" => entry.name = value.trim().to_string(),
                    "Exec" => entry.exec = value.trim().to_string(),
                    "Icon" => entry.icon = value.trim().to_string(),
                    "Comment" => entry.description = value.trim().to_string(),
                    "Categories" => {
                        entry.categories = value
                            .split(';')
                            .filter(|s| !s.is_empty())
                            .map(|s| s.trim().to_string())
                            .collect();
                    }
                    "Terminal" => entry.terminal = value.trim() == "true",
                    "NoDisplay" => entry.no_display = value.trim() == "true",
                    "Hidden" => entry.hidden = value.trim() == "true",
                    _ => {}
                }
            }
        }
        
        Ok(entry)
    }
    
    /// Generate menu hierarchy from entries
    pub fn generate_menu(&self, entries: &[DesktopEntry]) -> DesktopMenu {
        let mut menu = DesktopMenu {
            name: "Applications".to_string(),
            icon: Some("applications-other".to_string()),
            entries: Vec::new(),
            submenus: HashMap::new(),
        };
        
        // Simplified category grouping
        let mut categories: HashMap<String, Vec<DesktopEntry>> = HashMap::new();
        
        for entry in entries {
            if entry.categories.is_empty() {
                // Uncategorized application
                menu.entries.push(MenuEntry::Application(entry.clone()));
            } else {
                // Use first category for simplicity
                let category = entry.categories[0].clone();
                categories.entry(category).or_insert_with(Vec::new).push(entry.clone());
            }
        }
        
        // Create submenus for common categories
        for (category_name, category_entries) in categories {
            menu.submenus.insert(
                category_name.clone(),
                DesktopMenu {
                    name: category_name.clone(),
                    icon: Some("application-x-executable".to_string()),
                    entries: category_entries
                        .into_iter()
                        .map(MenuEntry::Application)
                        .collect(),
                    submenus: HashMap::new(),
                },
            );
        }
        
        menu
    }
    
    /// Search desktop entries by query
    pub fn search_entries<'a>(&self, entries: &'a [DesktopEntry], query: &str) -> Vec<&'a DesktopEntry> {
        let query_lower = query.to_lowercase();
        entries
            .iter()
            .filter(|entry| {
                entry.name.to_lowercase().contains(&query_lower)
                    || entry.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }
}

impl Default for MenuParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    
    #[test]
    fn test_desktop_entry_parsing() {
        let desktop_content = r#"
[Desktop Entry]
Name=Test Application
Exec=test-app
Icon=test-icon
Comment=A test application
Categories=Development;Utility;
Terminal=false
"#;
        
        let temp_dir = tempdir().unwrap();
        let desktop_file = temp_dir.path().join("test.desktop");
        fs::write(&desktop_file, desktop_content).unwrap();
        
        let parser = MenuParser::new();
        let entry = parser.parse_desktop_file(&desktop_file).unwrap();
        
        assert_eq!(entry.name, "Test Application");
        assert_eq!(entry.exec, "test-app");
        assert_eq!(entry.icon, "test-icon");
        assert_eq!(entry.description, "A test application");
        assert_eq!(entry.categories, vec!["Development", "Utility"]);
        assert!(!entry.terminal);
    }
    
    #[test]
    fn test_search_entries() {
        let entries = vec![
            DesktopEntry {
                name: "Text Editor".to_string(),
                description: "Edit text files".to_string(),
                categories: vec!["Utility".to_string()],
                ..Default::default()
            },
            DesktopEntry {
                name: "Image Editor".to_string(),
                description: "Edit images".to_string(),
                categories: vec!["Graphics".to_string()],
                ..Default::default()
            },
        ];
        
        let parser = MenuParser::new();
        let results = parser.search_entries(&entries, "edit");
        
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.name.contains("Edit")));
        
        let results = parser.search_entries(&entries, "text");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Text Editor");
    }
}