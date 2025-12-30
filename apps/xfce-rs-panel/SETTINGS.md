# Panel Settings System

## Overview

The panel now has an integrated settings system - **everything is in a single binary** (`xfce-rs-panel`). No separate settings application needed!

## How It Works

### Single Binary Architecture

- **Panel**: Main panel application
- **Settings UI**: Integrated overlay (opens when you right-click → Settings)
- **Plugins**: Separate binaries (as intended)

### Accessing Settings

1. **Right-click on the panel** → Context menu appears
2. Click **"Settings"** → Settings overlay opens
3. Make your changes
4. Click **"Save"** → Settings saved to `~/.config/xfce-rs/panel.toml`
5. Click **"Close"** or click outside → Settings overlay closes

### Settings Application

**Settings apply as follows:**

- **Immediate**: Dark mode, theme changes
- **On Save**: All settings saved to config file
- **On Restart**: Window size, position, mode changes (requires panel restart)

This is normal behavior - even xfce4-panel requires restart for some changes.

### Settings File

Location: `~/.config/xfce-rs/panel.toml`

Example:
```toml
size = 48
icon_size = 0
dark_mode = false
position = "Bottom"
position_locked = false
span_monitors = false
autohide = "Never"
autohide_size = 3
popdown_speed = 25
mode = "Horizontal"
nrows = 1
length = null
length_max = null
enable_struts = true
keep_below = true
```

### Applying Settings

**To apply window size/position changes:**
1. Save settings in the UI
2. Close the panel (or restart it)
3. Run panel again - it will use new settings

**Other settings** (like dark mode) apply immediately via the theme system.

## Architecture

```
xfce-rs-panel (single binary)
├── Panel UI
├── Settings UI (overlay)
├── Plugin Manager
└── Settings Manager

Plugins (separate binaries)
├── xfce-rs-clock
├── xfce-rs-separator
└── xfce-rs-showdesktop
```

## Future Enhancements

- [ ] Live window resizing (requires platform-specific code)
- [ ] Live window repositioning (requires platform-specific code)
- [ ] Settings validation
- [ ] Settings import/export
- [ ] Per-plugin settings
