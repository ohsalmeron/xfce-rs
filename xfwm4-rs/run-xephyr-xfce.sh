#!/bin/bash
# run-xephyr-xfce.sh: Test xfwm4-rs in a full, isolated XFCE environment

XEPHYR_DISPLAY=":1"
LOG_DIR="/home/bizkit/GitHub/xfce-rs/xfwm4-rs/logs"
mkdir -p "$LOG_DIR"

# Automatically detect host resolution
RESOLUTION=$(xrandr | grep -w connected | grep -o '[0-9]\+x[0-9]\+' | head -n1)
if [ -z "$RESOLUTION" ]; then
    RESOLUTION="1600x900"
fi

# Ensure we use the correct host display (avoiding conflict if terminal is on :1)
if [ "$DISPLAY" == "$XEPHYR_DISPLAY" ] || [ -z "$DISPLAY" ]; then
    # Try to find the first active X11 socket
    DETECTED=$(ls /tmp/.X11-unix/X* 2>/dev/null | head -n1 | sed 's/.*X/:/')
    export DISPLAY=${DETECTED:-:0}
    echo "Terminal DISPLAY was $XEPHYR_DISPLAY or empty, adjusted to $DISPLAY for host."
fi

echo "Cleaning up old sessions..."
pkill -f "Xephyr $XEPHYR_DISPLAY"
sleep 1

echo "Launching Xephyr $XEPHYR_DISPLAY at $RESOLUTION..."
Xephyr $XEPHYR_DISPLAY -br -ac -screen ${RESOLUTION}x32 > "$LOG_DIR/xephyr.log" 2>&1 &
XEPHYR_PID=$!
sleep 2

if ! ps -p $XEPHYR_PID > /dev/null; then
    echo "Error: Xephyr failed to start. Check $LOG_DIR/xephyr.log"
    exit 1
fi

echo "Launching isolated session via dbus-run-session..."
dbus-run-session -- bash -c "
    export DISPLAY=$XEPHYR_DISPLAY
    export RUST_LOG=debug
    
    echo 'Starting XFCE components...'
    xfsettingsd > \"$LOG_DIR/xfsettingsd.log\" 2>&1 &
    xfce4-panel > \"$LOG_DIR/xfce4-panel.log\" 2>&1 &
    xfdesktop > \"$LOG_DIR/xfdesktop.log\" 2>&1 &
    xfce4-terminal > \"$LOG_DIR/xfce4-terminal.log\" 2>&1 &
    
    echo 'Starting xfwm4-rs (foreground)...'
    /home/bizkit/GitHub/xfce-rs/target/release/xfwm4-rs > \"$LOG_DIR/xfwm4-rs.log\" 2>&1
    
    echo 'xfwm4-rs exited, running health check...'
    /home/bizkit/GitHub/xfce-rs/xfwm4-rs/health-check.sh
"

echo "Cleaning up..."
pkill -P $$
