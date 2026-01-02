# xfwm4-rs Special Compatibility Cases & Hacks

This document outlines the special logic ported from the original `xfwm4` C source code to ensure compatibility with various applications (Steam, Wine, Java, LibreOffice, etc.).

## 1. Window Gravity (ICCCM)
**Source**: `client.c:clientCoordGravitate`
**Case**: Applications use `WM_NORMAL_HINTS.win_gravity` to specify how their window should move when the size changes or when decorations are added.
**Implementation**:
- We store the **client window's** root-relative position.
- For `NorthWestGravity` (default), the frame is placed at the requested (x,y), but the client is offset by the decoration size.
- For `StaticGravity`, the client stays at the requested (x,y), and the frame is shifted "backwards" to accommodate.
- **Hack**: We force the client's internal gravity to `NorthWest` after reparenting to prevent "double gravity" effects when the WM resizes the frame.

## 2. Focus Stealing Prevention (EWMH)
**Source**: `focus.c:clientFocusNew`
**Case**: Slow-loading apps grabbing focus while the user is typing in another window.
**Logic**:
- Check `_NET_WM_USER_TIME`. If it's 0 (mapped by helper) or older than the currently focused window's last interaction, focus is denied.
- Focus is denied if the user is currently dragging/resizing another window.
- Higher layer windows (docks) don't steal focus from normal windows usually, but modal dialogs *always* get focus.

## 3. User Time Window Proxy (`_NET_WM_USER_TIME_WINDOW`)
**Source**: `netwm.c:getNetWMUserTimeWindow`
**Case**: Some toolkits (Gtk3+) use a dedicated window for timestamps to avoid updating the main window properties constantly.
**Implementation**: We monitor both the main window and the proxy window for property changes.

## 4. Modal Redirection
**Source**: `transients.c:clientGetModalFor`
**Case**: If a window has a modal child, clicking the parent should focus the child instead.
**Implementation**: `focus_window` recursively checks for modal transients.

## 5. Stacking Hacks
**Source**: `client.c:clientAdjustConfig`
**Case**: `WINDOW_DESKTOP` windows should never be brought to the top by client requests (`ConfigureRequest`).
**Implementation**: We filter `StackMode` and `Sibling` changes for desktop windows.

## 6. Redundant ConfigureRequest Filtering
**Source**: `client.c:clientAdjustConfig`
**Case**: Buggy apps send constantly `ConfigureRequest` with the same values, causing flicker.
**Implementation**: We skip processing if the requested geometry matches the current one.

## 7. GTK Frame Extents (`_GTK_FRAME_EXTENTS`)
**Source**: `client.c:clientGetGtkFrameExtents`
**Case**: Modern GTK apps draw their own shadows/borders.
**Implementation**: If this property is set, we disable WM decorations to avoid "double frames" and use the extents for tiling/snapping accurately.

## 8. Client Liveness (`_NET_WM_PING`)
**Source**: `netwm.c:clientSendNetWMPing`
**Case**: Detecting "Application not responding" states.
**Implementation**: Periodically send `_NET_WM_PING` and wait for `pong` response. If no response, the app might be frozen.

## 9. Input Focus behavior
**Source**: `focus.c:clientAcceptFocus`
**Case**: Some windows should not receive focus directly via `XSetInputFocus` (e.g. windows that handle input internally or via `WM_TAKE_FOCUS`).
**Implementation**: We respect `WM_HINTS.input` and the `WM_TAKE_FOCUS` protocol. If a window supports `WM_TAKE_FOCUS`, we send it a message. We only call `set_input_focus` if the window explicitly accepts input via hints.

## 10. Multi-factor Implementation & PID Tracking
**Source**: `transients.c` and `client.c`
**Case**: Modern applications require PID and Hostname tracking for better grouping and session management.
**Implementation**: We read `_NET_WM_PID` during client management. This allows for future features like "Kill process" or grouping windows by application even without a common group leader.

## 11. Fullscreen & Maximized Constraints
**Source**: `client.c:clientAdjustConfig`
**Case**: Applications should not be able to move or resize themselves while in fullscreen or maximized states.
**Implementation**: We filter out geometry change requests in `ConfigureRequest` if the window is currently in one of these states, preventing buggy apps from "popping out" of fullscreen.

## 12. Modern Toolkit Focus Signaling (`_NET_WM_STATE_FOCUSED`)
**Source**: Modern EWMH additions
**Case**: Toolkits like **Iced**, **Winit/WGPU**, and **Gtk4** use this state to enable/disable UI animations (like cursor blinking) and rendering optimizations.
**Implementation**: We add/remove `_NET_WM_STATE_FOCUSED` from the `_NET_WM_STATE` property when a window gains/loses focus.

## 13. Smart Placement (Cascade)
**Source**: `client.c:clientPlace`
**Case**: Windows that don't specify a position (requested at 0,0) should not all stack directly on top of each other in the top-left corner.
**Implementation**: We use a cascading algorithm that finds the next available slot with a slight offset, improving window visibility on launch.

## 14. Interactive Resize Synchronization (`_NET_WM_SYNC_REQUEST`)
**Source**: `xsync.c`
**Case**: Prevent flickering and "lagging behind" visuals during window resizing, especially for GPU-accelerated apps.
**Implementation**: We use the `XSync` extension and `_NET_WM_SYNC_REQUEST` protocol. The WM sends a sync request before resizing, and the client increments a counter once it has finished drawing the new frame. The WM waits for this signal (via Alarms) to maintain smooth visuals.

## 15. EWMH Frame Extents Signaling (`_NET_FRAME_EXTENTS`)
**Source**: Standard EWMH implementation
**Case**: Applications need to know the size of the WM decorations to calculate their own layout or handle CSD correctly.
**Implementation**: We set `_NET_FRAME_EXTENTS` on mapped windows so applications can query exactly how much "edge" the WM is adding.
