# Running the XFCE.rs Panel

## Quick Start

### 1. Build Everything

```bash
cd /home/bizkit/GitHub/xfce-rs
cargo build --release --workspace
```

This builds:
- The panel (`xfce-rs-panel`)
- All plugins (`xfce-rs-clock`, `xfce-rs-separator`, `xfce-rs-showdesktop`)

### 2. Run the Panel

```bash
cargo run --release --bin xfce-rs-panel
```

**What you'll see:**
- A transparent panel bar at the bottom of your screen (1920x48px)
- Panel automatically discovers and launches all available plugins
- Each plugin runs as a separate window
- Plugin slots appear in the panel showing plugin names/descriptions

### 3. Plugin Windows

When the panel starts, it automatically launches:
- **Clock Plugin** - Shows time and date (updates every second)
- **Separator** - Visual separator widget
- **Show Desktop** - Button to toggle show desktop

Each plugin appears as its own window. The panel shows status indicators for each plugin.

## Running Individual Plugins

You can also run plugins standalone (without the panel):

```bash
# Clock
cargo run --release --bin xfce-rs-clock

# Separator  
cargo run --release --bin xfce-rs-separator

# Show Desktop
cargo run --release --bin xfce-rs-showdesktop
```

## Panel Configuration

### Change Panel Position/Size

Edit `panel/src/main.rs`:

```rust
.window(iced::window::Settings {
    size: iced::Size::new(1920.0, 48.0),  // Change width/height
    position: iced::window::Position::Specific(iced::Point::new(0.0, 1032.0)), // Change X, Y
    // ...
})
```

### Panel Positions

- **Top**: `position: iced::window::Position::Specific(iced::Point::new(0.0, 0.0))`
- **Bottom**: `position: iced::window::Position::Specific(iced::Point::new(0.0, 1032.0))` (default)
- **Left**: `position: iced::window::Position::Specific(iced::Point::new(0.0, 0.0))` + rotate size
- **Right**: `position: iced::window::Position::Specific(iced::Point::new(1872.0, 0.0))` + rotate size

## Debugging

### Panel Logs
```bash
RUST_LOG=debug cargo run --bin xfce-rs-panel
```

### Plugin Logs
```bash
RUST_LOG=debug cargo run --bin xfce-rs-clock
```

## How It Works

1. **Panel starts** → Discovers plugins in `target/debug` or `target/release`
2. **Plugin discovery** → Scans for known plugin binaries
3. **Plugin launch** → Spawns each plugin as a child process
4. **Status tracking** → Panel monitors plugin processes
5. **UI display** → Shows plugin slots in the panel bar

## Troubleshooting

### Plugins Not Found

Make sure plugins are built:
```bash
cargo build --release --bin xfce-rs-clock
cargo build --release --bin xfce-rs-separator  
cargo build --release --bin xfce-rs-showdesktop
```

### Panel Not Starting

Check if another panel is running:
```bash
ps aux | grep xfce-rs-panel
```

### Plugins Not Launching

Check panel logs:
```bash
RUST_LOG=debug cargo run --bin xfce-rs-panel 2>&1 | grep -i plugin
```

## Next Steps

- Customize panel appearance
- Add more plugins
- Configure plugin positions
- Implement embedded mode (embed plugin windows in panel)
