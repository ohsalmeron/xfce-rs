use anyhow::{Result, Context};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, PropMode, AtomEnum};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperExt;
use x11rb::atom_manager;
use tracing::{info, warn};

atom_manager! {
    pub AtomCollection: AtomCollectionCookie {
        _NET_CLIENT_LIST,
        _NET_WM_NAME,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_STATE,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_STATE_SKIP_PAGER,
        _NET_WM_STATE_STICKY,
        UTF8_STRING,
    }
}

/// Set panel window properties to make it appear as a dock and skip taskbar
pub fn set_panel_window_properties(window_name: &str) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(None)
        .context("Failed to connect to X11 server")?;
    
    let screen = &conn.setup().roots[screen_num];
    let root_window = screen.root;
    
    let atoms = AtomCollection::new(&conn)?.reply()?;
    
    // Find window by name
    let window_id = find_window_by_name(&conn, root_window, window_name, &atoms)
        .context("Failed to find panel window")?;
    
    info!("Found panel window: {}, setting properties", window_id);
    
    // Set window type to DOCK
    conn.change_property32(
        PropMode::REPLACE,
        window_id,
        atoms._NET_WM_WINDOW_TYPE,
        AtomEnum::ATOM,
        &[atoms._NET_WM_WINDOW_TYPE_DOCK],
    )
    .context("Failed to set _NET_WM_WINDOW_TYPE")?;
    
    // Set window state: SKIP_TASKBAR, SKIP_PAGER, STICKY
    let states = [
        atoms._NET_WM_STATE_SKIP_TASKBAR,
        atoms._NET_WM_STATE_SKIP_PAGER,
        atoms._NET_WM_STATE_STICKY,
    ];
    conn.change_property32(
        PropMode::REPLACE,
        window_id,
        atoms._NET_WM_STATE,
        AtomEnum::ATOM,
        &states,
    )
    .context("Failed to set _NET_WM_STATE")?;
    
    conn.flush()
        .context("Failed to flush X11 connection")?;
    
    info!("Successfully set panel window properties");
    Ok(())
}

fn find_window_by_name(
    conn: &RustConnection,
    root: u32,
    name: &str,
    atoms: &AtomCollection,
) -> Result<u32> {
    // Get all client windows
    let reply = conn.get_property(
        false,
        root,
        atoms._NET_CLIENT_LIST,
        AtomEnum::WINDOW,
        0,
        1024,
    )?
    .reply()?;
    
    if reply.type_ == u32::from(AtomEnum::WINDOW) && reply.format == 32 {
        if let Some(windows) = reply.value32() {
            for window_id in windows {
                // Get window name
                let name_reply = conn.get_property(
                    false,
                    window_id,
                    atoms._NET_WM_NAME,
                    atoms.UTF8_STRING,
                    0,
                    1024,
                )?
                .reply();
                
                if let Ok(prop) = name_reply {
                    if prop.type_ == atoms.UTF8_STRING && !prop.value.is_empty() {
                        if let Ok(window_name_str) = String::from_utf8(prop.value) {
                            if window_name_str == name {
                                return Ok(window_id);
                            }
                        }
                    }
                }
                
                // Fallback to WM_NAME
                let wm_name_reply = conn.get_property(
                    false,
                    window_id,
                    u32::from(AtomEnum::WM_NAME),
                    u32::from(AtomEnum::STRING),
                    0,
                    1024,
                )?
                .reply();
                
                if let Ok(prop) = wm_name_reply {
                    if prop.type_ == u32::from(AtomEnum::STRING) && !prop.value.is_empty() {
                        if let Ok(window_name_str) = String::from_utf8(prop.value) {
                            if window_name_str == name {
                                return Ok(window_id);
                            }
                        }
                    }
                }
            }
        }
    }
    
    Err(anyhow::anyhow!("Window '{}' not found", name))
}
