pub mod client;
pub mod manager;
pub mod frame;
pub mod draw;
pub mod placement;
pub mod cursors;
pub mod compositor;
pub mod settings;
pub mod session;
pub mod error;

pub const LAYER_DESKTOP: u16 = 0;
pub const LAYER_BELOW: u16 = 2;
pub const LAYER_NORMAL: u16 = 4;
pub const LAYER_ONTOP: u16 = 6;
pub const LAYER_DOCK: u16 = 8;
pub const LAYER_FULLSCREEN: u16 = 10;
pub const LAYER_NOTIFICATION: u16 = 14;
