#!/bin/bash
# Automated interaction test for xfwm4-rs in Xephyr

set -e

XEPHYR_DISPLAY=":1"
LOG_DIR="$(pwd)/logs"
TEST_DURATION=10  # seconds to run test

echo "=== xfwm4-rs Automated Interaction Test ==="
echo "Starting at: $(date)"
echo ""

# Cleanup
echo "[1/7] Cleaning up old sessions..."
pkill -f "Xephyr $XEPHYR_DISPLAY" 2>/dev/null || true
sleep 1

# Start Xephyr
echo "[2/7] Launching Xephyr..."
RESOLUTION="1920x1080x32"
Xephyr $XEPHYR_DISPLAY -br -ac -screen $RESOLUTION > "$LOG_DIR/xephyr.log" 2>&1 &
XEPHYR_PID=$!
sleep 2

if ! ps -p $XEPHYR_PID > /dev/null; then
    echo "❌ FAILED: Xephyr didn't start"
    exit 1
fi
echo "✓ Xephyr running (PID: $XEPHYR_PID)"

# Start WM and components in background
echo "[3/7] Starting window manager and XFCE components..."
dbus-run-session -- bash -c "
    export DISPLAY=$XEPHYR_DISPLAY
    export RUST_LOG=info
    
    xfsettingsd > \"$LOG_DIR/xfsettingsd.log\" 2>&1 &
    xfce4-panel > \"$LOG_DIR/xfce4-panel.log\" 2>&1 &
    xfdesktop > \"$LOG_DIR/xfdesktop.log\" 2>&1 &
    xfce4-terminal > \"$LOG_DIR/xfce4-terminal.log\" 2>&1 &
    
    /home/bizkit/GitHub/xfce-rs/target/release/xfwm4-rs > \"$LOG_DIR/xfwm4-rs.log\" 2>&1 &
    WM_PID=\$!
    
    # Wait for WM to initialize
    sleep 3
    
    # Check if WM is still running
    if ! ps -p \$WM_PID > /dev/null; then
        echo \"❌ FAILED: Window manager crashed during startup\"
        exit 1
    fi
    
    echo \"✓ Window manager running (PID: \$WM_PID)\"
    
    # Keep session alive for test duration
    sleep $TEST_DURATION
    
    # Cleanup
    kill \$WM_PID 2>/dev/null || true
" &
SESSION_PID=$!

sleep 4

# Run interaction tests
echo "[4/7] Running automated interaction tests..."

# Test 1: Move mouse around
echo "  Test 1: Mouse movement..."
DISPLAY=$XEPHYR_DISPLAY xdotool mousemove 500 500 2>/dev/null || echo "  ⚠ xdotool not available, skipping mouse tests"
sleep 0.5
DISPLAY=$XEPHYR_DISPLAY xdotool mousemove 1000 500 2>/dev/null || true
sleep 0.5
DISPLAY=$XEPHYR_DISPLAY xdotool mousemove 500 300 2>/dev/null || true

# Test 2: Click on screen
echo "  Test 2: Mouse clicks..."
DISPLAY=$XEPHYR_DISPLAY xdotool click 1 2>/dev/null || true
sleep 0.5

# Test 3: Type some text (if terminal is focused)
echo "  Test 3: Keyboard input..."
DISPLAY=$XEPHYR_DISPLAY xdotool type "test" 2>/dev/null || true
sleep 0.5

# Test 4: Try to open terminal (Ctrl+Alt+T if configured)
echo "  Test 4: Hotkey test..."
DISPLAY=$XEPHYR_DISPLAY xdotool key ctrl+alt+t 2>/dev/null || true
sleep 1

echo "✓ Interaction tests completed"

# Wait a bit more to see if WM crashes
sleep 2

# Check results
echo "[5/7] Analyzing results..."

# Check if WM is still running
if ! pgrep -f "xfwm4-rs" > /dev/null; then
    echo "❌ FAILED: Window manager crashed during test"
    TEST_RESULT=1
else
    echo "✓ Window manager still running"
    TEST_RESULT=0
fi

# Check logs for errors
ERROR_COUNT=$(grep -c "ERROR" "$LOG_DIR/xfwm4-rs.log" 2>/dev/null || echo "0")
if [ "$ERROR_COUNT" -gt 0 ]; then
    echo "⚠ Found $ERROR_COUNT errors in logs"
    grep "ERROR" "$LOG_DIR/xfwm4-rs.log" | tail -5
fi

# Check for managed windows
MANAGED_COUNT=$(grep -c "Managing window" "$LOG_DIR/xfwm4-rs.log" 2>/dev/null || echo "0")
echo "✓ Managed $MANAGED_COUNT windows"

if [ "$MANAGED_COUNT" -lt 2 ]; then
    echo "⚠ Warning: Expected at least 2 managed windows (panel + terminal)"
fi

# Cleanup
echo "[6/7] Cleaning up..."
kill $SESSION_PID 2>/dev/null || true
pkill -P $SESSION_PID 2>/dev/null || true
kill $XEPHYR_PID 2>/dev/null || true
sleep 1

# Final health check
echo "[7/7] Running health check..."
./health-check.sh

# Final verdict
echo ""
echo "=== Test Results ==="
if [ $TEST_RESULT -eq 0 ] && [ "$ERROR_COUNT" -eq 0 ]; then
    echo "✅ ALL TESTS PASSED"
    exit 0
elif [ $TEST_RESULT -eq 0 ]; then
    echo "⚠️  TESTS PASSED WITH WARNINGS ($ERROR_COUNT errors logged)"
    exit 2
else
    echo "❌ TESTS FAILED"
    exit 1
fi
