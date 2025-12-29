#!/bin/bash
# Run the Separator plugin
cd "$(dirname "$0")/.."
cargo run --release --bin xfce-rs-separator
