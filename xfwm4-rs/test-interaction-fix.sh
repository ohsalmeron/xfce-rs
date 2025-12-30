#!/bin/bash
# Interaction verification test

set -e

cd "$(dirname "$0")"
LOG_DIR="logs"
mkdir -p "$LOG_DIR"

echo "=== Interaction Verification Test ==="

# Clean and rebuild
echo "[1/5] Building WM..."
cargo build -p xfwm4-rs --release > /dev/null 2>&1

# Start Xephyr
echo "[2/5] Starting Xephyr..."
pkill -f "Xephyr :1" 2>/dev/null || true
sleep 1
Xephyr :1 -br -ac -screen 1024x768x32 > "$LOG_DIR/xephyr.log" 2>&1 &
XEPHYR_PID=$!
sleep 2

# Start WM
echo "[3/5] Starting WM..."
DISPLAY=:1 RUST_LOG=debug ./target/release/xfwm4-rs > "$LOG_DIR/interaction.log" 2>&1 &
WM_PID=$!
sleep 2

# Start a terminal inside
echo "[4/5] Launching terminal and simulating click..."
DISPLAY=:1 xfce4-terminal &
TERM_PID=$!
sleep 3

# Click in the middle of the terminal
# xfce4-terminal usually opens at 0,0 or similar.
DISPLAY=:1 xdotool mousemove 200 200 click 1
sleep 2

# Cleanup
kill $WM_PID 2>/dev/null || true
kill $XEPHYR_PID 2>/dev/null || true
kill $TERM_PID 2>/dev/null || true

echo "[5/5] Verification Results:"
FOCUS_COUNT=$(grep -c "✓ FOCUS: Successfully set input focus" "$LOG_DIR/interaction.log" || echo 0)
REPLAY_COUNT=$(grep -c "✓ Replayed pointer to client" "$LOG_DIR/interaction.log" || echo 0)

echo "Focus count: $FOCUS_COUNT"
echo "Replay count: $REPLAY_COUNT"

if [ "$FOCUS_COUNT" -gt 0 ] && [ "$REPLAY_COUNT" -gt 0 ]; then
    echo "✅ SUCCESS: Interaction working!"
    exit 0
else
    echo "❌ FAILURE: Interaction issues remaining."
    echo "Recent Logs:"
    grep -E "(FOCUS|Replayed|ButtonPress)" "$LOG_DIR/interaction.log" | tail -n 10
    exit 1
fi
