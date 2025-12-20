// XFCE App Finder - Tauri Backend
// Scans .desktop files and provides search/launch functionality

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use freedesktop_desktop_entry::{DesktopEntry, Iter as DesktopIter};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::State;

/// Represents a desktop application entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub terminal: bool,
}

/// Application state holding cached entries
pub struct AppState {
    entries: Mutex<Vec<AppEntry>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
}

/// Scan system for .desktop files and return all applications
fn scan_desktop_entries() -> Vec<AppEntry> {
    let mut entries = Vec::new();
    let mut seen: HashMap<String, bool> = HashMap::new();

    // Get XDG data directories
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/share:/usr/local/share".to_string());
    
    let mut search_paths: Vec<PathBuf> = data_dirs
        .split(':')
        .map(|p| PathBuf::from(p).join("applications"))
        .collect();

    // Add user-local applications directory
    if let Some(home) = dirs::home_dir() {
        search_paths.push(home.join(".local/share/applications"));
    }

    // Preferred locales for desktop entries
    let locales: &[&str] = &["en_US", "en"];

    // Scan each directory - DesktopIter::new takes an iterator of paths
    for entry_path in DesktopIter::new(search_paths.clone().into_iter()) {
        if let Ok(bytes) = std::fs::read_to_string(&entry_path) {
            if let Ok(desktop) = DesktopEntry::from_str(&entry_path, &bytes, Some(locales)) {
                // Skip NoDisplay and Hidden entries
                if desktop.no_display() || desktop.hidden() {
                    continue;
                }

                // Skip entries without Exec
                let exec = match desktop.exec() {
                    Some(e) => e.to_string(),
                    None => continue,
                };

                let id = entry_path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Skip duplicates
                if seen.contains_key(&id) {
                    continue;
                }
                seen.insert(id.clone(), true);

                // Methods take &[L] directly, not Option
                let name = desktop.name(locales)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| id.clone());

                let app = AppEntry {
                    id,
                    name,
                    generic_name: desktop.generic_name(locales).map(|s| s.to_string()),
                    comment: desktop.comment(locales).map(|s| s.to_string()),
                    exec,
                    icon: desktop.icon().map(|s| s.to_string()),
                    categories: desktop.categories()
                        .map(|cats| cats.into_iter().map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                    keywords: desktop.keywords(locales)
                        .map(|kws| kws.into_iter().map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                    terminal: desktop.terminal(),
                };

                entries.push(app);
            }
        }
    }

    // Sort alphabetically by name
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    println!("Found {} applications", entries.len());
    if entries.is_empty() {
        println!("WARNING: No applications found! Searched in: {:?}", search_paths);
    }
    entries
}

/// Tauri command: Refresh and return all applications
#[tauri::command]
fn get_applications(state: State<'_, AppState>) -> Vec<AppEntry> {
    let apps = scan_desktop_entries();
    let mut cached = state.entries.lock().unwrap();
    *cached = apps.clone();
    apps
}

/// Tauri command: Search applications by query
#[tauri::command]
fn search_applications(query: &str, state: State<'_, AppState>) -> Vec<AppEntry> {
    let cached = state.entries.lock().unwrap();
    
    if query.is_empty() {
        return cached.clone();
    }

    let matcher = SkimMatcherV2::default();
    let mut scored: Vec<(i64, AppEntry)> = cached
        .iter()
        .filter_map(|app| {
            // Match against name, generic_name, comment, and keywords
            let name_score = matcher.fuzzy_match(&app.name, query).unwrap_or(0);
            let generic_score = app.generic_name.as_ref()
                .and_then(|g| matcher.fuzzy_match(g, query))
                .unwrap_or(0);
            let comment_score = app.comment.as_ref()
                .and_then(|c| matcher.fuzzy_match(c, query))
                .unwrap_or(0);
            let keyword_score = app.keywords.iter()
                .filter_map(|k| matcher.fuzzy_match(k, query))
                .max()
                .unwrap_or(0);

            let total = name_score.max(generic_score).max(comment_score).max(keyword_score);
            
            if total > 0 {
                Some((total, app.clone()))
            } else {
                None
            }
        })
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, app)| app).collect()
}

/// Tauri command: Launch an application
#[tauri::command]
fn launch_application(exec: &str) -> Result<(), String> {
    // Parse the Exec string (remove %f, %u, %F, %U placeholders)
    let cleaned = exec
        .replace("%f", "")
        .replace("%F", "")
        .replace("%u", "")
        .replace("%U", "")
        .replace("%d", "")
        .replace("%D", "")
        .replace("%n", "")
        .replace("%N", "")
        .replace("%k", "")
        .replace("%v", "")
        .replace("%c", "")
        .trim()
        .to_string();

    // Split into command and args
    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".to_string());
    }

    let program = parts[0];
    let args = &parts[1..];

    Command::new(program)
        .args(args)
        .spawn()
        .map_err(|e| format!("Failed to launch: {}", e))?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_applications,
            search_applications,
            launch_application
        ])
        .run(tauri::generate_context!())
        .expect("error while running XFCE App Finder");
}
