# Quick Start - Running Panel Plugins

These plugins are standalone Iced applications that you can run directly to see them in action.

## 🕐 Clock Plugin

**What it does:** Shows current time and date, updates every second

**Run it:**
```bash
cd /home/bizkit/GitHub/xfce-rs
cargo run --bin xfce-rs-clock
```

**What you'll see:**
- A small window (200x48px) in the center of your screen
- Current time in HH:MM format (e.g., "14:30")
- Current date below (e.g., "Monday, January 15")
- Glassmorphism styling with transparent background
- Updates every second automatically

**To close:** Click the window or press Ctrl+C in terminal

---

## ➖ Separator Plugin

**What it does:** Visual separator/divider widget

**Run it:**
```bash
cd /home/bizkit/GitHub/xfce-rs
cargo run --bin xfce-rs-separator
```

**What you'll see:**
- A thin vertical line (8x48px) in the center of your screen
- Simple separator with subtle border
- Transparent background

**To close:** Click the window or press Ctrl+C in terminal

---

## 🖥️ Show Desktop Plugin

**What it does:** Button to toggle show/hide desktop (minimize all windows)

**Run it:**
```bash
cd /home/bizkit/GitHub/xfce-rs
cargo run --bin xfce-rs-showdesktop
```

**What you'll see:**
- A square button (48x48px) in the center of your screen
- Desktop icon (🖥️ when desktop is visible, 📋 when hidden)
- Click to toggle show desktop state
- Uses wmctrl to control windows (if available)

**To close:** Click the window or press Ctrl+C in terminal

---

## 🚀 Running All at Once

You can run multiple plugins simultaneously in different terminals:

```bash
# Terminal 1
cargo run --bin xfce-rs-clock

# Terminal 2  
cargo run --bin xfce-rs-separator

# Terminal 3
cargo run --bin xfce-rs-showdesktop
```

## 🎨 Customizing Window Position/Size

Edit the window settings in each plugin's `src/main.rs`:

```rust
.window(iced::window::Settings {
    size: iced::Size::new(200.0, 48.0),  // Change width/height here
    position: iced::window::Position::Centered,  // Or use Specific(x, y)
    transparent: true,  // Set to false for solid background
    decorations: false,  // Set to true for window decorations
    ..Default::default()
})
```

## 🐛 Debug Mode

Run with logging to see what's happening:

```bash
RUST_LOG=debug cargo run --bin xfce-rs-clock
```

## 📝 Notes

- All plugins use **transparent backgrounds** and **no window decorations** by default
- They're designed to look like panel widgets floating on your desktop
- Window positions are centered by default - you can drag them if needed
- The plugins are fully functional standalone applications
