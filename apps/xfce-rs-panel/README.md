# XFCE.rs Panel

A modern Rust-based panel system for XFCE.rs that can host plugin binaries.

## Features

- **Plugin System**: Discovers and launches plugin binaries automatically
- **Detached Mode**: Plugins can run as separate windows (current implementation)
- **Embedded Mode**: Support for embedding plugins (future enhancement)
- **Iced Framework**: Modern, performant UI built with Iced
- **Glassmorphism**: Beautiful transparent glass-style panel

## Running the Panel

### Prerequisites

First, build the plugins:
```bash
cd /home/bizkit/GitHub/xfce-rs
cargo build --release --workspace
```

### Run the Panel

```bash
# From workspace root
cargo run --release --bin xfce-rs-panel

# Or build first, then run
cargo build --release --bin xfce-rs-panel
./target/release/xfce-rs-panel
```

### What Happens

1. **Panel Starts**: A transparent panel window appears (default: bottom of screen, 48px height)
2. **Plugin Discovery**: Panel automatically discovers available plugins in `target/debug` or `target/release`
3. **Plugin Launch**: All discovered plugins are automatically started as separate processes
4. **Plugin Display**: Each plugin appears as a slot in the panel showing its name/description

## Current Plugins

The panel automatically discovers and launches:
- **xfce-rs-clock** - Clock plugin (shows time/date)
- **xfce-rs-separator** - Separator widget
- **xfce-rs-showdesktop** - Show desktop button

## Configuration

### Window Settings

Edit `panel/src/main.rs` to customize:

```rust
.window(iced::window::Settings {
    size: iced::Size::new(1920.0, 48.0),  // Width x Height
    position: iced::window::Position::Specific(iced::Point::new(0.0, 1032.0)), // X, Y position
    transparent: true,  // Transparent background
    decorations: false, // No window decorations
    resizable: false,   // Fixed size
    ..Default::default()
})
```

### Plugin Discovery

Plugins are discovered in `plugin_manager.rs`. The panel looks for binaries in:
- `target/debug/` (development)
- `target/release/` (release builds)

## Architecture

### Components

- **PanelApp**: Main application that manages the panel UI
- **PluginManager**: Discovers, starts, and stops plugin processes
- **PluginSlot**: UI representation of a plugin slot in the panel

### Plugin Communication

Currently, plugins run as **detached** processes (separate windows). The panel:
1. Spawns plugin binaries as child processes
2. Monitors their status
3. Can start/stop them on demand

Future enhancements:
- **Embedded Mode**: Embed plugin windows directly in panel slots
- **IPC Communication**: D-Bus or custom protocol for panel ↔ plugin communication
- **Plugin Configuration**: Panel can send configuration to plugins

## Development

### Adding a New Plugin

1. Create plugin binary in `panel-plugins/`
2. Add to `PluginManager::discover_plugins()` in `panel/src/plugin_manager.rs`:

```rust
let plugin_binaries = [
    ("xfce-rs-clock", "Clock Plugin", false),
    ("xfce-rs-separator", "Separator", false),
    ("xfce-rs-showdesktop", "Show Desktop", false),
    ("xfce-rs-your-plugin", "Your Plugin", false), // Add here
];
```

3. Build the plugin: `cargo build --release --bin xfce-rs-your-plugin`
4. Restart the panel - it will auto-discover and launch your plugin

### Debugging

Run with logging:
```bash
RUST_LOG=debug cargo run --bin xfce-rs-panel
```

## Notes

- Plugins are **self-contained binaries** - they don't need special linking
- Plugins use **Iced framework** for consistent UI
- Panel automatically **starts all discovered plugins** on launch
- Plugins run as **separate processes** - crash isolation
- Panel **monitors plugin processes** and can restart them if needed

## Future Enhancements

- [ ] Embedded plugin windows (X11/Wayland embedding)
- [ ] Plugin configuration UI
- [ ] D-Bus communication protocol
- [ ] Plugin hot-reload
- [ ] Panel position/size configuration file
- [ ] Plugin ordering/dragging
- [ ] Panel autohide functionality
