#!/bin/bash
# Quick automated verification test

set -e

cd "$(dirname "$0")"
LOG_DIR="logs"

echo "=== Quick Verification Test ==="

# Clean and rebuild
echo "[1/4] Building WM..."
cargo build -p xfwm4-rs --release > /dev/null 2>&1

# Start Xephyr in background
echo "[2/4] Starting Xephyr..."
pkill -f "Xephyr :1" 2>/dev/null || true
sleep 1
Xephyr :1 -br -ac -screen 1920x1080x32 > "$LOG_DIR/xephyr.log" 2>&1 &
XEPHYR_PID=$!
sleep 2

# Start WM
echo "[3/4] Starting WM for 5 seconds..."
# Binary is in target/release/xfwm4-rs relative to project root
# Script is in xfwm4-rs subfolder
WM_BIN="../target/release/xfwm4-rs"
if [ ! -f "$WM_BIN" ]; then
    echo "❌ Binary not found at $WM_BIN, trying relative to script"
    WM_BIN="./target/release/xfwm4-rs"
fi

DISPLAY=:1 RUST_LOG=info "$WM_BIN" > "$LOG_DIR/quick-test.log" 2>&1 &
WM_PID=$!

sleep 5

# Check if still running
if ps -p $WM_PID > /dev/null; then
    echo "✓ WM still running after 5s"
    RESULT=0
else
    echo "❌ WM crashed"
    RESULT=1
fi

# Cleanup
kill $WM_PID 2>/dev/null || true
kill $XEPHYR_PID 2>/dev/null || true

# Show critical info
echo "[4/4] Log summary:"
echo "Errors: $(grep -c 'ERROR' "$LOG_DIR/quick-test.log" 2>/dev/null || echo 0)"
echo "Focus events: $(grep -c 'FOCUS' "$LOG_DIR/quick-test.log" 2>/dev/null || echo 0)"
echo "Managed windows: $(grep -c 'Managing window' "$LOG_DIR/quick-test.log" 2>/dev/null || echo 0)"
tail -10 "$LOG_DIR/quick-test.log"

exit $RESULT
