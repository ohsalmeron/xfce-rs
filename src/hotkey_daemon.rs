//! X11 hotkey daemon for Navigator
//! 
//! Grabs the Super_L key globally and toggles Navigator visibility.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use std::sync::mpsc;
use tracing::{debug, info, error};

/// Super_L keycode on most systems
const SUPER_L_KEYCODE: u8 = 133;

/// Message sent from daemon to main app
#[derive(Debug, Clone)]
pub enum DaemonMessage {
    ToggleVisibility,
}

/// Start the X11 hotkey daemon in a background thread.
/// Returns a receiver that emits ToggleVisibility when Super is pressed.
pub fn start_daemon() -> Option<mpsc::Receiver<DaemonMessage>> {
    let (tx, rx) = mpsc::channel();
    
    std::thread::spawn(move || {
        if let Err(e) = run_daemon(tx) {
            error!("Hotkey daemon failed: {}", e);
        }
    });
    
    Some(rx)
}

fn run_daemon(tx: mpsc::Sender<DaemonMessage>) -> anyhow::Result<()> {
    // Connect to X server
    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    
    info!("Hotkey daemon connected to X11, screen {}", screen_num);
    
    // Grab Super_L key on the root window
    // We need to grab with and without NumLock/CapsLock modifiers
    let modifiers = [
        ModMask::from(0u16),           // No modifiers
        ModMask::LOCK,                  // CapsLock
        ModMask::M2,                    // NumLock (usually Mod2)
        ModMask::LOCK | ModMask::M2,    // Both
    ];
    
    for &mods in &modifiers {
        match conn.grab_key(
            false,
            root,
            mods,
            SUPER_L_KEYCODE,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        ) {
            Ok(cookie) => {
                if let Err(e) = cookie.check() {
                    debug!("Could not grab Super_L with mods {:?}: {}", mods, e);
                } else {
                    debug!("Grabbed Super_L with modifiers {:?}", mods);
                }
            }
            Err(e) => {
                debug!("Grab request failed: {}", e);
            }
        }
    }
    
    conn.flush()?;
    info!("Hotkey daemon listening for Super_L key");
    
    // Track key state for press/release detection
    let mut super_pressed = false;
    
    // Event loop
    loop {
        let event = conn.wait_for_event()?;
        
        match event {
            Event::KeyPress(e) => {
                if e.detail == SUPER_L_KEYCODE {
                    super_pressed = true;
                    debug!("Super_L pressed");
                }
            }
            Event::KeyRelease(e) => {
                if e.detail == SUPER_L_KEYCODE && super_pressed {
                    super_pressed = false;
                    debug!("Super_L released, toggling visibility");
                    let _ = tx.send(DaemonMessage::ToggleVisibility);
                }
            }
            _ => {}
        }
    }
}
