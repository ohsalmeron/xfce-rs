use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Cursor;
use x11rb::resource_manager::new_from_default;
use x11rb::cursor::Handle;

pub struct Cursors {
    pub normal: Cursor,
    pub move_: Cursor,
    pub resize_nw: Cursor, // Top-Left
    pub resize_ne: Cursor, // Top-Right
    pub resize_sw: Cursor, // Bottom-Left
    pub resize_se: Cursor, // Bottom-Right
    pub resize_n: Cursor,  // Top
    pub resize_s: Cursor,  // Bottom
    pub resize_e: Cursor,  // Right
    pub resize_w: Cursor,  // Left
    pub hand: Cursor,      // For buttons
}

impl Cursors {
    pub fn new<C: Connection>(conn: &C, screen_num: usize) -> Result<Self> {
        let db = new_from_default(conn)?;
        let handle = Handle::new(conn, screen_num, &db)?.reply()?;
        
        let load = |name: &str| -> Result<Cursor> {
             Ok(handle.load_cursor(conn, name)?)
        };
        
        Ok(Self {
            normal: load("left_ptr")?,
            move_: load("fleur")?, // or "move"
            resize_nw: load("top_left_corner")?,
            resize_ne: load("top_right_corner")?,
            resize_sw: load("bottom_left_corner")?,
            resize_se: load("bottom_right_corner")?,
            resize_n: load("top_side")?,
            resize_s: load("bottom_side")?,
            resize_e: load("right_side")?,
            resize_w: load("left_side")?,
            hand: load("hand2")?,
        })
    }
}
