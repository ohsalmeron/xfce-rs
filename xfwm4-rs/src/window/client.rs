use x11rb::protocol::xproto::Window;
use x11rb::protocol::render::Picture;

#[derive(Debug, Clone)]
pub struct Client {
    /// The window ID of the application window
    pub window: Window,
    /// The window ID of the frame decorations (if any)
    pub frame: Option<Window>,
    /// The Render Picture for composition
    pub picture: Option<Picture>,
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
    pub is_fullscreen: bool,
    pub saved_geometry: Option<(i16, i16, u16, u16)>,
    pub damage: Option<x11rb::protocol::damage::Damage>,
    pub strut: Option<Vec<u32>>,
    pub transient_for: Option<Window>,
    pub layer: u16,
}

impl Client {
    pub fn new(window: Window, x: i16, y: i16, width: u16, height: u16) -> Self {
        Self {
            window,
            frame: None,
            picture: None,
            x,
            y,
            width,
            height,
            visible: false,
            name: String::from("Unnamed"),
            workspace: 0,
            window_type: Vec::new(),
            is_maximized: false,
            is_fullscreen: false,
            saved_geometry: None,
            damage: None,
            strut: None,
            transient_for: None,
            layer: 4, // Normal layer
        }
    }
}
