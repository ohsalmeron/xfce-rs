use anyhow::Result;
use x11rb::protocol::xproto::{PropMode, ConnectionExt, AtomEnum, WindowClass, CreateWindowAux};
use x11rb::connection::Connection;
use x11rb::wrapper::ConnectionExt as _;

use crate::core::context::Context;

pub fn setup_hints(ctx: &Context) -> Result<()> {
    // 1. Create a dummy window for _NET_SUPPORTING_WM_CHECK
    let check_win = ctx.conn.generate_id()?;
    ctx.conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        check_win,
        ctx.root_window,
        -1, -1, 1, 1, 0,
        WindowClass::INPUT_OUTPUT,
        0,
        &CreateWindowAux::new(),
    )?;

    // 2. Set _NET_SUPPORTING_WM_CHECK on the check window to itself
    ctx.conn.change_property32(
        PropMode::REPLACE,
        check_win,
        ctx.atoms._NET_SUPPORTING_WM_CHECK,
        AtomEnum::WINDOW,
        &[check_win],
    )?;

    // 3. Set _NET_WM_NAME on check window
    let name = "xfwm4-rs";
    ctx.conn.change_property8(
        PropMode::REPLACE,
        check_win,
        ctx.atoms._NET_WM_NAME,
        ctx.atoms.UTF8_STRING,
        name.as_bytes(),
    )?;

    // 4. Set _NET_SUPPORTING_WM_CHECK on root window
    ctx.conn.change_property32(
        PropMode::REPLACE,
        ctx.root_window,
        ctx.atoms._NET_SUPPORTING_WM_CHECK,
        AtomEnum::WINDOW,
        &[check_win],
    )?;

    // 5. Set _NET_SUPPORTED on root window
    let supported = [
        ctx.atoms._NET_SUPPORTED,
        ctx.atoms._NET_CLIENT_LIST,
        ctx.atoms._NET_NUMBER_OF_DESKTOPS,
        ctx.atoms._NET_CURRENT_DESKTOP,
        ctx.atoms._NET_ACTIVE_WINDOW,
        ctx.atoms._NET_WM_NAME,
        ctx.atoms._NET_SUPPORTING_WM_CHECK,
        ctx.atoms._NET_WM_STATE,
        ctx.atoms._NET_WM_STATE_FULLSCREEN,
        ctx.atoms._NET_WM_STATE_MAXIMIZED_VERT,
        ctx.atoms._NET_WM_STATE_MAXIMIZED_HORZ,
        ctx.atoms._NET_WM_WINDOW_TYPE,
        ctx.atoms._NET_WM_WINDOW_TYPE_NORMAL,
        ctx.atoms._NET_WM_WINDOW_TYPE_DOCK,
        ctx.atoms._NET_WM_WINDOW_TYPE_DIALOG,
        ctx.atoms._NET_WM_STRUT,
        ctx.atoms._NET_WM_STRUT_PARTIAL,
        ctx.atoms._NET_WORKAREA,
        ctx.atoms.WM_DELETE_WINDOW,
        ctx.atoms.WM_TAKE_FOCUS,
        ctx.atoms.WM_TRANSIENT_FOR,
    ];
    
    ctx.conn.change_property32(
        PropMode::REPLACE,
        ctx.root_window,
        ctx.atoms._NET_SUPPORTED,
        AtomEnum::ATOM,
        &supported,
    )?;

    ctx.conn.change_property32(
        PropMode::REPLACE,
        ctx.root_window,
        ctx.atoms._NET_NUMBER_OF_DESKTOPS,
        AtomEnum::CARDINAL,
        &[4],
    )?;
    
    ctx.conn.change_property32(
        PropMode::REPLACE,
        ctx.root_window,
        ctx.atoms._NET_CURRENT_DESKTOP,
        AtomEnum::CARDINAL,
        &[0],
    )?;

    Ok(())
}
