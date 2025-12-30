use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::rust_connection::RustConnection;
use x11rb::protocol::xproto::ConnectionExt;

use crate::ewmh::atoms::AtomCollection;

pub struct Context {
    pub conn: RustConnection,
    pub screen_num: usize,
    pub root_window: u32,
    pub root_depth: u8,
    pub atoms: AtomCollection,
    pub screen_width: u16,
    pub screen_height: u16,
}

impl Context {
    pub fn new() -> Result<Self> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root_window = screen.root;
        let root_depth = screen.root_depth;
        
        let atoms = AtomCollection::new(&conn)?.reply()?;
        let screen_width = screen.width_in_pixels;
        let screen_height = screen.height_in_pixels;
        
        // Select events on root window
        use x11rb::protocol::xproto::{ChangeWindowAttributesAux, EventMask};
        let values = ChangeWindowAttributesAux::new()
            .event_mask(EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY);
        conn.change_window_attributes(root_window, &values)?;
        
        Ok(Self { conn, screen_num, root_window, root_depth, atoms, screen_width, screen_height })
    }
}
