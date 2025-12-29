#!/bin/bash
# Run the Show Desktop plugin
cd "$(dirname "$0")/.."
cargo run --release --bin xfce-rs-showdesktop
