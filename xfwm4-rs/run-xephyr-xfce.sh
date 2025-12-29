#!/bin/bash
# run-xephyr-xfce.sh: Test xfwm4-rs in a full XFCE environment inside Xephyr

XEPHYR_DISPLAY=":1"

# Automatically detect host resolution
RESOLUTION=$(xrandr | grep -w connected | grep -o '[0-9]\+x[0-9]\+' | head -n1)
if [ -z "$RESOLUTION" ]; then
    RESOLUTION="1600x900"
fi

echo "Checking for existing Xephyr on $XEPHYR_DISPLAY..."
pkill -f "Xephyr $XEPHYR_DISPLAY"

echo "Launching Xephyr $XEPHYR_DISPLAY at $RESOLUTION..."
# Use -screen size for "fullscreen" feel, or -fullscreen if you want to lock the mouse
Xephyr $XEPHYR_DISPLAY -ac -screen $RESOLUTION &
sleep 2

echo "Launching xfwm4-rs..."
cd $(dirname "$0")
DISPLAY=$XEPHYR_DISPLAY RUST_LOG=info ./target/release/xfwm4-rs &
sleep 1

echo "Launching XFCE components..."
# We use export to ensure all children see the right display
export DISPLAY=$XEPHYR_DISPLAY
xfsettingsd &
xfce4-panel &
xfdesktop &
xfce4-terminal &

echo "Xephyr session is ready on display $XEPHYR_DISPLAY"
echo "To exit, close the Xephyr window."
wait
