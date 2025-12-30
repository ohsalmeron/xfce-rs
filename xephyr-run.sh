#!/bin/bash
# xephyr-run.sh: Test the XFCE-RS session in an isolated window

# 1. Compile everything in release mode first if needed
# cargo build --release --workspace

# 2. Add local release binaries to PATH
export PATH="$(pwd)/target/release:$PATH"

# 3. Find an available display number
DISPLAY_NUM=""
for i in {1..20}; do
    if [ ! -f "/tmp/.X$i-lock" ]; then
        DISPLAY_NUM=":$i"
        break
    fi
done

if [ -z "$DISPLAY_NUM" ]; then
    echo "Error: No available X display found (tried :1 to :20)"
    exit 1
fi

# 4. Clean up any stale sockets for this specific display
sudo rm -f "/tmp/.X11-unix/X${DISPLAY_NUM#:}" || true

# 5. Start Xephyr
echo "Starting Xephyr on $DISPLAY_NUM..."
Xephyr $DISPLAY_NUM -ac -screen 1280x720 -reset -terminate &
XEPHYR_PID=$!

# Wait for Xephyr to start
sleep 2

# 6. Run the session script inside Xephyr
echo "Launching XFCE-RS Session on $DISPLAY_NUM..."
# Ensure the session script is executable
chmod +x ./packaging/xfce-rs-session
DISPLAY=$DISPLAY_NUM ./packaging/xfce-rs-session

# 7. Cleanup when session ends
kill $XEPHYR_PID 2>/dev/null || true
echo "Xephyr session terminated."
