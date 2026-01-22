# xfce-rs - The xfce Desktop Environment rewritten in Rust

A complete rewrite of the XFCE desktop environment in Rust, maintaining the philosophy of simplicity, modularity, and low resource usage while leveraging modern programming language features for enhanced safety and performance.

## 🎯 Project Goals

- **Memory Safety**: Eliminate entire classes of bugs through Rust's ownership model
- **Performance**: Better resource utilization and faster performance than the original
- **Modularity**: Maintain XFCE's component-based architecture
- **Developer Friendly**: Modern tooling and excellent documentation

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

## 📦 Components

- **Window Manager** (`apps/xfce-rs-wm`): A Wayland compositor built with Smithay.
- **Panel** (`apps/xfce-rs-panel`): The desktop panel and taskbar.
- **Desktop** (`apps/xfce-rs-desktop`): Background and icon manager.
- **File Manager** (`apps/xfce-rs-thunar`): Fast and lightweight file manager.
- **Navigator** (`apps/xfce-rs-navigator`): Application launcher and finder.
- **Audio** (`apps/xfce-rs-audio`): Volume control and audio management.

---

**Note**: This is an independent project not affiliated with the official XFCE development team.