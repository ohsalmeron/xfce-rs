#!/bin/bash
# install-session.sh: Deploys local XFCE-RS session components

set -e

# 1. Build release binaries
echo "Building XFCE-RS in release mode..."
cargo build --release --workspace

# 2. Install binaries to /usr/local/bin
echo "Installing binaries..."
sudo install -m 755 target/release/xfwm4-rs /usr/local/bin/xfwm4-rs
sudo install -m 755 target/release/xfce-rs-panel /usr/local/bin/xfce-rs-panel
sudo install -m 755 target/release/navigator /usr/local/bin/navigator

# 3. Install session script
echo "Installing session script..."
sudo install -m 755 packaging/xfce-rs-session /usr/local/bin/xfce-rs-session

# 4. Install desktop entry
echo "Installing desktop entry to /usr/share/xsessions..."
sudo install -m 644 packaging/xfce-rs.desktop /usr/share/xsessions/xfce-rs.desktop

echo "--------------------------------------------------"
echo "XFCE-RS Session installed successfully!"
echo "Log out and select 'XFCE-RS' from your display manager (LightDM/SDDM)."
echo "--------------------------------------------------"
