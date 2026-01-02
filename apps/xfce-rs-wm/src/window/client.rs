use x11rb::protocol::xproto::Window;
use x11rb::protocol::render::Picture;

#[derive(Debug, Clone)]
pub struct Client {
    /// The window ID of the application window
    pub window: Window,
    /// The window ID of the frame decorations (if any)
    pub frame: Option<Window>,
    /// The Render Picture for the frame decorations
    pub picture: Option<Picture>,
    /// The Render Picture for the client content
    pub content_picture: Option<Picture>,
    // Fields tracking window geometry/state, used for layout and rendering logic (Phase 2)
    // Detailed usage planned for decoration rendering implementation.
    #[allow(dead_code)]
    pub x: i16,
    #[allow(dead_code)]
    pub y: i16,
    #[allow(dead_code)]
    pub width: u16,
    #[allow(dead_code)]
    pub height: u16,
    #[allow(dead_code)]
    pub visible: bool,
    #[allow(dead_code)]
    pub name: String,
    // -1 (0xFFFFFFFF) = All Workspaces
    // -1 (0xFFFFFFFF) = All Workspaces
    pub workspace: u32,
    pub window_type: Vec<u32>,
    pub is_maximized: bool,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
    pub is_sticky: bool,
    pub saved_geometry: Option<(i16, i16, u16, u16)>,
    pub damage: Option<x11rb::protocol::damage::Damage>,
    pub strut: Option<Vec<u32>>,
    pub transient_for: Option<Window>,
    pub group_leader: Option<Window>,
    pub client_leader: Option<Window>,
    pub user_time: u32,
    pub user_time_window: Option<Window>,
    pub is_modal: bool,
    pub frame_extents: (u32, u32, u32, u32),
    pub gravity: i32,
    pub layer: u16,
    pub is_desktop: bool,
    pub is_dock: bool,
    pub is_csd: bool,
    pub accepts_input: bool,
    pub pid: u32,
    pub is_urgent: bool,
    pub sync_counter: Option<u32>,
    pub sync_next_value: u64,
    pub sync_waiting: bool,
    pub is_shaped: bool,
    pub sync_alarm: Option<u32>,
    pub opacity: u32,
    pub demands_attention: bool,
    pub skip_taskbar: bool,
    pub skip_pager: bool,
    pub is_shaded: bool,
    pub is_above: bool,
    pub is_below: bool,
    pub startup_id: Option<String>,
}




impl Client {
    pub fn new(window: Window, x: i16, y: i16, width: u16, height: u16) -> Self {
        Self {
            window,
            frame: None,
            picture: None,
            content_picture: None,
            x,
            y,
            width,
            height,
            visible: false,
            name: String::from("Unnamed"),
            workspace: 0,
            window_type: Vec::new(),
            is_maximized: false,
            is_minimized: false,
            is_fullscreen: false,
            is_sticky: false,
            saved_geometry: None,
            damage: None,
            strut: None,
            transient_for: None,
            group_leader: None,
            client_leader: None,
            user_time: 0,
            user_time_window: None,
            is_modal: false,
            frame_extents: (0, 0, 0, 0),
            gravity: 1, // NorthWestGravity
            layer: 4, // Normal layer
            is_desktop: false,
            is_dock: false,
            is_csd: false,
            accepts_input: true,
            pid: 0,
            is_urgent: false,
            sync_counter: None,
            sync_next_value: 0,
            sync_waiting: false,
            is_shaped: false,
            sync_alarm: None,
            opacity: 0xFFFFFFFF,
            demands_attention: false,
            skip_taskbar: false,
            skip_pager: false,
            is_shaded: false,
            is_above: false,
            is_below: false,
            startup_id: None,
        }
    }
}





