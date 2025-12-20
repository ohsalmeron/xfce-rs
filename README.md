# XFCE.rs - A Modern Rust Desktop Environment

[![Crates.io](https://img.shields.io/crates/v/xfce-rs.svg)](https://crates.io/crates/xfce-rs)
[![Documentation](https://docs.rs/xfce-rs/badge.svg)](https://docs.rs/xfce-rs)
[![License](https://img.shields.io/badge/License-GPLv2+-blue.svg)](LICENSE)
[![Build Status](https://github.com/ohsalmeron/xfce-rs/workflows/CI/badge.svg)](https://github.com/ohsalmeron/xfce-rs/actions)

A complete rewrite of the XFCE desktop environment in Rust, maintaining the philosophy of simplicity, modularity, and low resource usage while leveraging modern programming language features for enhanced safety and performance.

## 🎯 Project Goals

- **Memory Safety**: Eliminate entire classes of bugs through Rust's ownership model
- **Performance**: Better resource utilization and faster performance than the original
- **Modularity**: Maintain XFCE's component-based architecture
- **Modern Standards**: Full Wayland support with X11 compatibility
- **Developer Friendly**: Modern tooling and excellent documentation

## 🏗️ Architecture

XFCE.rs consists of several core components organized as a Cargo workspace:

### Core Libraries
- **`xfce-rs-config`** - Configuration system (replaces xfconf)
- **`xfce-rs-ipc`** - Inter-process communication framework
- **`xfce-rs-utils`** - Core utilities and helper functions
- **`xfce-rs-ui`** - Reusable UI components and theming
- **`xfce-rs-menu`** - Desktop menu system (freedesktop.org compliant)

### Desktop Components
- **`xfce-rs-wm`** - Wayland window manager (replaces xfwm4)
- **`xfce-rs-desktop`** - Desktop manager (replaces xfdesktop)
- **`xfce-rs-panel`** - Panel system with plugin architecture
- **`xfce-rs-thunar`** - File manager (replaces Thunar)
- **`xfce-rs-appfinder`** - Application launcher and finder

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ with stable toolchain
- Wayland development libraries
- For X11 compatibility: X11 development libraries

### Installation from Source

```bash
# Clone the repository
git clone https://github.com/ohsalmeron/xfce-rs.git
cd xfce-rs

# Build all components
cargo build --release

# Install (optional)
sudo cargo install --path .
```

### Development Setup

```bash
# Set up development environment
cargo install cargo-watch cargo-tarpaulin

# Run tests
cargo test --workspace

# Run with hot reload during development
cargo watch -x run
```

## 📚 Documentation

- [Architecture Overview](docs/architecture.md)
- [API Documentation](https://docs.rs/xfce-rs)
- [Development Guide](docs/development.md)
- [Migration Guide from XFCE](docs/migration.md)

## 🖥️ Usage

### Starting the Desktop Environment

```bash
# Start the full desktop environment
xfce-rs-session

# Start individual components
xfce-rs-wm          # Window manager
xfce-rs-desktop     # Desktop manager
xfce-rs-panel       # Panel
xfce-rs-thunar      # File manager
```

### Configuration

XFCE.rs uses a TOML-based configuration system located at:
- `~/.config/xfce-rs/config.toml` - Main configuration
- `~/.config/xfce-rs/themes/` - Theme definitions
- `~/.config/xfce-rs/plugins/` - Plugin configurations

Example configuration:

```toml
[general]
theme = "default"
workspace_count = 4
auto_save_session = true

[panel]
position = "bottom"
size = 48
autohide = false

[desktop]
show_icons = true
wallpaper = "/path/to/wallpaper.png"
```

## 🧪 Development Status

This project is currently in **active development**.

### Phase 1: Foundation ✅
- [x] Project structure
- [x] Configuration system
- [x] IPC framework
- [x] Basic UI components

### Phase 2: Window Manager 🚧
- [ ] Wayland compositor
- [ ] Window management logic
- [ ] Theme system
- [ ] Workspace management

### Phase 3: Desktop Environment 📋
- [ ] Desktop manager
- [ ] Panel system
- [ ] Basic applications

### Phase 4: Application Ecosystem 📋
- [ ] File manager
- [ ] Application launcher
- [ ] Settings manager

### Phase 5: Integration 📋
- [ ] Session management
- [ ] Migration tools
- [ ] Performance optimization

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Workflow

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests and documentation
5. Run the test suite (`cargo test --workspace`)
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

### Code Style

We use `rustfmt` and `clippy` for code formatting and linting:

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

## 📄 License

This project is licensed under the GPLv2+ License - see the [LICENSE](LICENSE) file for details. This maintains compatibility with the original XFCE project while allowing for future contributions.

## 🙏 Acknowledgments

- The original [XFCE](https://www.xfce.org/) project for inspiration and design philosophy
- [System76 COSMIC](https://system76.com/cosmic) for pioneering Rust desktop development
- All the great dependencies we're using.

## 🔗 Related Projects

- [COSMIC Desktop](https://github.com/pop-os/cosmic-desktop) - System76's Rust desktop
- [Alacritty](https://github.com/alacritty/alacritty) - Terminal emulator in Rust
- [WezTerm](https://github.com/wez/wezterm) - Cross-platform terminal emulator

## 📞 Support

- 📖 [Documentation](https://docs.rs/xfce-rs)
- 🐛 [Issue Tracker](https://github.com/ohsalmeron/xfce-rs/issues)
- 💬 [Discussions](https://github.com/ohsalmeron/xfce-rs/discussions)
- 🎙️ [Matrix Chat](https://matrix.to/#/#xfce-rs:matrix.org)

---

**Note**: This is an independent project not affiliated with the official XFCE development team.