# XFCE.rs Panel Plugins

Rust-based panel plugins for XFCE.rs, built with the Iced framework.

## Available Plugins

### Clock Plugin (`xfce-rs-clock`)
Displays the current time and date with automatic updates every second.

**Run:**
```bash
# From workspace root
cargo run --release --bin xfce-rs-clock

# Or use the helper script
./panel-plugins/run-clock.sh
```

### Separator Plugin (`xfce-rs-separator`)
A visual separator widget for organizing panel items.

**Run:**
```bash
# From workspace root
cargo run --release --bin xfce-rs-separator

# Or use the helper script
./panel-plugins/run-separator.sh
```

### Show Desktop Plugin (`xfce-rs-showdesktop`)
Button to toggle showing/hiding all windows (show desktop).

**Run:**
```bash
# From workspace root
cargo run --release --bin xfce-rs-showdesktop

# Or use the helper script
./panel-plugins/run-showdesktop.sh
```

## Building All Plugins

```bash
# Build all plugins in release mode
cargo build --release --workspace

# Build specific plugin
cargo build --release --bin xfce-rs-clock
cargo build --release --bin xfce-rs-separator
cargo build --release --bin xfce-rs-showdesktop
```

## Running in Development Mode

For faster iteration during development:

```bash
# Run with debug build (faster compilation)
cargo run --bin xfce-rs-clock

# Run with logging
RUST_LOG=debug cargo run --bin xfce-rs-clock
```

## Window Settings

All plugins are configured with:
- **Transparent background** - blends with desktop
- **No window decorations** - frameless appearance
- **Centered position** - appears in center of screen
- **Glassmorphism styling** - modern glass effect UI

You can modify window settings in each plugin's `main.rs` file:
- Window size: `iced::Size::new(width, height)`
- Position: `iced::window::Position::Centered` or custom coordinates
- Transparency: `transparent: true/false`
- Decorations: `decorations: false` (no title bar)

## Integration with XFCE Panel

These plugins are designed to work as external plugins with the xfce4-panel wrapper system. To integrate:

1. Build the plugins in release mode
2. Create `.desktop` files pointing to the binaries
3. Place them in the appropriate plugin directory
4. The panel wrapper will launch them as external processes

## Development

Each plugin follows the same structure:
- `src/main.rs` - Main application entry point
- Uses Iced framework for UI
- Shares styling from `xfce-rs-ui` crate
- Follows the same patterns as `audio` and `navigator` plugins
