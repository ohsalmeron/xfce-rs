use anyhow::{Result, Context};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, AtomEnum, Window, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperExt;
use x11rb::atom_manager;
// Tracing imports removed - not used

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub window_id: u32,
    pub name: String,
    pub is_active: bool,
    pub is_minimized: bool,
    pub skip_taskbar: bool,
    pub is_desktop: bool,
    pub is_dock: bool,
}

atom_manager! {
    pub AtomCollection: AtomCollectionCookie {
        _NET_CLIENT_LIST,
        _NET_ACTIVE_WINDOW,
        _NET_WM_NAME,
        _NET_WM_STATE,
        _NET_WM_STATE_HIDDEN,
        _NET_WM_STATE_SKIP_TASKBAR,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_DOCK,
        UTF8_STRING,
    }
}

pub struct WindowTracker {
    conn: RustConnection,
    screen_num: usize,
    root_window: Window,
    atoms: AtomCollection,
}

impl WindowTracker {
    pub fn new() -> Result<Self> {
        let (conn, screen_num) = x11rb::connect(None)
            .context("Failed to connect to X11 server")?;
        
        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;
        
        let atoms = AtomCollection::new(&conn)?.reply()?;
        
        Ok(Self {
            conn,
            screen_num,
            root_window,
            atoms,
        })
    }

    pub fn get_windows(&self) -> Result<Vec<WindowInfo>> {
        // Get _NET_CLIENT_LIST from root window
        let reply = self.conn.get_property(
            false,
            self.root_window,
            self.atoms._NET_CLIENT_LIST,
            AtomEnum::WINDOW,
            0,
            1024,
        )?.reply()
        .context("Failed to get _NET_CLIENT_LIST")?;

        let mut windows = Vec::new();
        
        if reply.type_ == u32::from(AtomEnum::WINDOW) && reply.format == 32 {
            if let Some(window_ids) = reply.value32() {
                for window_id in window_ids {
                    if let Ok(window_info) = self.get_window_info(window_id) {
                        // Filter out windows that should be skipped
                        if !window_info.skip_taskbar && !window_info.is_desktop && !window_info.is_dock {
                            windows.push(window_info);
                        }
                    }
                }
            }
        }

        Ok(windows)
    }

    fn get_window_info(&self, window: Window) -> Result<WindowInfo> {
        // Get window name
        let mut name = "Unnamed".to_string();
        for &atom in &[self.atoms._NET_WM_NAME, self.atoms.UTF8_STRING, AtomEnum::WM_NAME.into()] {
            if let Ok(reply) = self.conn.get_property(false, window, atom, AtomEnum::ANY, 0, 1024)?.reply() {
                if !reply.value.is_empty() {
                    if let Ok(s) = String::from_utf8(reply.value) {
                        name = s;
                        break;
                    }
                }
            }
        }

        // Get window state
        let mut is_minimized = false;
        let mut skip_taskbar = false;
        let mut is_desktop = false;
        let mut is_dock = false;

        // Check _NET_WM_STATE
        if let Ok(reply) = self.conn.get_property(
            false,
            window,
            self.atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            0,
            1024,
        )?.reply() {
            if let Some(states) = reply.value32() {
                for state in states {
                    if state == self.atoms._NET_WM_STATE_HIDDEN {
                        is_minimized = true;
                    }
                    if state == self.atoms._NET_WM_STATE_SKIP_TASKBAR {
                        skip_taskbar = true;
                    }
                }
            }
        }

        // Check window type
        if let Ok(reply) = self.conn.get_property(
            false,
            window,
            self.atoms._NET_WM_WINDOW_TYPE,
            AtomEnum::ATOM,
            0,
            1024,
        )?.reply() {
            if let Some(types) = reply.value32() {
                for window_type in types {
                    if window_type == self.atoms._NET_WM_WINDOW_TYPE_DESKTOP {
                        is_desktop = true;
                    }
                    if window_type == self.atoms._NET_WM_WINDOW_TYPE_DOCK {
                        is_dock = true;
                    }
                }
            }
        }

        // Check if window is active
        let is_active = self.get_active_window() == Some(window);

        Ok(WindowInfo {
            window_id: window,
            name,
            is_active,
            is_minimized,
            skip_taskbar,
            is_desktop,
            is_dock,
        })
    }

    fn get_active_window(&self) -> Option<Window> {
        if let Ok(reply) = self.conn.get_property(
            false,
            self.root_window,
            self.atoms._NET_ACTIVE_WINDOW,
            AtomEnum::WINDOW,
            0,
            1,
        ) {
            if let Ok(prop) = reply.reply() {
                if prop.type_ == u32::from(AtomEnum::WINDOW) && prop.format == 32 {
                    if let Some(mut windows) = prop.value32() {
                        return windows.next();
                    }
                }
            }
        }
        None
    }

    pub fn activate_window(&self, window: Window) -> Result<()> {
        // Send _NET_ACTIVE_WINDOW client message
        use x11rb::protocol::xproto::{ClientMessageData, ClientMessageEvent, EventMask};
        
        let data = ClientMessageData::from([
            window,
            2, // Source indication: 2 = application
            0, // Timestamp (0 = current)
            0, // Requestor window (0 = none)
            0, // Unused
        ]);

        let event = ClientMessageEvent {
            response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: self.root_window,
            type_: self.atoms._NET_ACTIVE_WINDOW,
            data,
        };

        self.conn.send_event(
            false,
            self.root_window,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )
        .context("Failed to send _NET_ACTIVE_WINDOW message")?;
        
        // Flush the connection
        self.conn.flush()
            .context("Failed to flush X11 connection")?;

        // If window is minimized, unminimize it by removing _NET_WM_STATE_HIDDEN
        if let Ok(reply) = self.conn.get_property(
            false,
            window,
            self.atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            0,
            1024,
        )?.reply() {
            if let Some(states) = reply.value32() {
                let states_vec: Vec<u32> = states.collect();
                let has_hidden = states_vec.contains(&self.atoms._NET_WM_STATE_HIDDEN);
                
                if has_hidden {
                    // Remove HIDDEN state to unminimize
                    let new_states: Vec<u32> = states_vec
                        .into_iter()
                        .filter(|&state| state != self.atoms._NET_WM_STATE_HIDDEN)
                        .collect();
                    
                    self.conn.change_property32(
                        PropMode::REPLACE,
                        window,
                        self.atoms._NET_WM_STATE,
                        AtomEnum::ATOM,
                        &new_states,
                    )
                    .context("Failed to change window state")?;
                    
                    // Flush the connection
                    self.conn.flush()
                        .context("Failed to flush X11 connection")?;
                }
            }
        }

        Ok(())
    }
}
