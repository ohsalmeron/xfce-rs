#!/bin/bash
# Run the Clock plugin
cd "$(dirname "$0")/.."
cargo run --release --bin xfce-rs-clock
