use thiserror::Error;
use sysinfo::System;
use regex::Regex;
use tokio::process;
use tracing::error;

/// Error types for utilities
#[derive(Error, Debug)]
pub enum UtilError {
    #[error("Process execution failed: {command}")]
    ProcessFailed { command: String },
    
    #[error("System information unavailable")]
    SystemInfoUnavailable,
    
    #[error("Invalid path: {path}")]
    InvalidPath { path: String },
    
    #[error("Regex compilation failed: {0}")]
    RegexError(#[from] regex::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// System information utilities
pub struct SystemInfo {
    system: System,
}

impl SystemInfo {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        Self { system }
    }
    
    /// Get CPU usage percentage
    pub fn cpu_usage(&self) -> f32 {
        self.system.global_cpu_info().cpu_usage()
    }
    
    /// Get memory usage information
    pub fn memory_usage(&self) -> (u64, u64) {
        let total = self.system.total_memory();
        let used = self.system.used_memory();
        (used, total)
    }
    
    /// Get list of running processes
    pub fn running_processes(&self) -> Vec<ProcessInfo> {
        self.system.processes()
            .values()
            .map(|process| ProcessInfo {
                pid: process.pid().as_u32(),
                name: process.name().to_string(),
                cpu_usage: process.cpu_usage(),
                memory: process.memory(),
                cmd: process.cmd().join(" "),
            })
            .collect()
    }
    
    /// Get disk usage information (simplified)
    pub fn disk_usage(&self, path: &str) -> Result<DiskUsage, UtilError> {
        // For now, return a placeholder implementation
        Ok(DiskUsage {
            total: 1000000000, // 1GB placeholder
            available: 500000000, // 500MB placeholder
            used: 500000000, // 500MB placeholder
            mount_point: path.to_string(),
        })
    }
    
    /// Check if a process is running
    pub fn is_process_running(&mut self, name: &str) -> bool {
        self.system.refresh_processes();
        self.system.processes()
            .values()
            .any(|process| process.name().contains(name))
    }
    
    /// Find process by name
    pub fn find_process(&mut self, name: &str) -> Option<u32> {
        self.system.refresh_processes();
        self.system.processes()
            .values()
            .find(|process| process.name().contains(name))
            .map(|process| process.pid().as_u32())
    }
}

/// Process information
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory: u64,
    pub cmd: String,
}

/// Disk usage information
#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub total: u64,
    pub available: u64,
    pub used: u64,
    pub mount_point: String,
}

impl DiskUsage {
    /// Calculate usage percentage
    pub fn usage_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.used as f64 / self.total as f64) * 100.0
    }
}

/// File system utilities
pub struct FileSystemUtils;

impl FileSystemUtils {
    /// Get file icon based on MIME type
    pub fn get_file_icon(file_path: &str) -> String {
        let path = std::path::Path::new(file_path);
        
        if path.is_dir() {
            return "folder".to_string();
        }
        
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        match extension.to_lowercase().as_str() {
            "txt" | "md" => "text-plain",
            "pdf" => "application-pdf",
            "jpg" | "jpeg" | "png" | "gif" | "svg" => "image",
            "mp4" | "avi" | "mkv" | "mov" => "video",
            "mp3" | "ogg" | "flac" | "wav" => "audio",
            "zip" | "tar" | "gz" | "7z" => "archive",
            "rs" | "c" | "cpp" | "py" | "js" | "html" | "css" => "text-code",
            "exe" | "deb" | "rpm" => "application-x-executable",
            _ => "text-x-generic",
        }
        .to_string()
    }
    
    /// Check if path exists
    pub fn path_exists(path: &str) -> bool {
        std::path::Path::new(path).exists()
    }
    
    /// Get file size in human readable format
    pub fn format_file_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = size as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }
    
    /// Validate filename
    pub fn is_valid_filename(filename: &str) -> bool {
        let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
        !filename.chars().any(|c| invalid_chars.contains(&c))
            && !filename.is_empty()
            && filename.len() <= 255
    }
    
    /// Sanitize filename
    pub fn sanitize_filename(filename: &str) -> String {
        let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
        filename
            .chars()
            .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
            .collect::<String>()
            .trim()
            .to_string()
    }
}

/// Process utilities
pub struct ProcessUtils;

impl ProcessUtils {
    /// Execute a command and return output
    pub async fn execute_command(command: &str, args: &[&str]) -> Result<String, UtilError> {
        let output = process::Command::new(command)
            .args(args)
            .output()
            .await
            .map_err(|_| UtilError::ProcessFailed { 
                command: format!("{} {}", command, args.join(" ")) 
            })?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(UtilError::ProcessFailed { 
                command: format!("{} {} failed", command, args.join(" ")) 
            })
        }
    }
    
    /// Check if a command is available in PATH
    pub async fn command_exists(command: &str) -> bool {
        match process::Command::new("which")
            .arg(command)
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
    
    /// Kill process by PID
    pub async fn kill_process(pid: u32) -> Result<(), UtilError> {
        let output = process::Command::new("kill")
            .arg(pid.to_string())
            .output()
            .await?;
        
        if output.status.success() {
            Ok(())
        } else {
            Err(UtilError::ProcessFailed { 
                command: format!("kill {}", pid) 
            })
        }
    }
}

/// String utilities
pub struct StringUtils;

impl StringUtils {
    /// Truncate string to specified length
    pub fn truncate(s: &str, max_length: usize) -> String {
        if s.len() <= max_length {
            s.to_string()
        } else {
            format!("{}...", &s[..max_length.saturating_sub(3)])
        }
    }
    
    /// Extract number from string using regex
    pub fn extract_number(s: &str) -> Option<f64> {
        let re = Regex::new(r"[-+]?\d*\.?\d+").ok()?;
        re.find(s)?.as_str().parse().ok()
    }
    
    /// Check if string contains only ASCII characters
    pub fn is_ascii(s: &str) -> bool {
        s.is_ascii()
    }
    
    /// Convert to title case
    pub fn to_title_case(s: &str) -> String {
        s.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                }
            })
            .collect::<Vec<String>>()
            .join(" ")
    }
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_file_icon_detection() {
        assert_eq!(FileSystemUtils::get_file_icon("test.txt"), "text-plain");
        assert_eq!(FileSystemUtils::get_file_icon("image.jpg"), "image");
        assert_eq!(FileSystemUtils::get_file_icon("video.mp4"), "video");
        assert_eq!(FileSystemUtils::get_file_icon("/some/path"), "folder");
    }
    
    #[test]
    fn test_file_size_formatting() {
        assert_eq!(FileSystemUtils::format_file_size(512), "512 B");
        assert_eq!(FileSystemUtils::format_file_size(1536), "1.5 KB");
        assert_eq!(FileSystemUtils::format_file_size(1048576), "1.0 MB");
    }
    
    #[test]
    fn test_filename_validation() {
        assert!(FileSystemUtils::is_valid_filename("valid_name.txt"));
        assert!(!FileSystemUtils::is_valid_filename("invalid/name.txt"));
        assert!(!FileSystemUtils::is_valid_filename(""));
    }
    
    #[test]
    fn test_string_utilities() {
        assert_eq!(StringUtils::truncate("short", 10), "short");
        assert_eq!(StringUtils::truncate("very long string", 10), "very lo...");
        assert_eq!(StringUtils::extract_number("Version 2.3.1"), Some(2.3));
        assert_eq!(StringUtils::to_title_case("hello world"), "Hello World");
    }
    
    #[test]
    fn test_disk_usage_percent() {
        let usage = DiskUsage {
            total: 1000,
            used: 500,
            available: 500,
            mount_point: "/test".to_string(),
        };
        assert_eq!(usage.usage_percent(), 50.0);
    }
}