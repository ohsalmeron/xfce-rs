use anyhow::Result;
use x11rb::protocol::xproto::{ConnectionExt, Window, CreateGCAux, Rectangle};
use x11rb::connection::Connection;

use crate::core::context::Context;

pub fn draw_decoration(ctx: &Context, frame: Window, title: &str, width: u16, _height: u16, title_height: u16) -> Result<()> {
    if title_height == 0 { return Ok(()); }
    // 1. Create Graphics Context
    let gc = ctx.conn.generate_id()?;
    let font = ctx.conn.generate_id()?;
    
    // Open a font. "fixed" is almost always available.
    // We ignore error here, hoping it works. 
    // In robust code we'd check or have a fallback.
    let _ = ctx.conn.open_font(font, b"fixed"); 
    
    // Create GC with font and colors
    let values = CreateGCAux::new()
        .foreground(0xffffff) // White text
        .background(0x333333) // Dark Gray background (matches frame)
        .font(font);
        
    ctx.conn.create_gc(gc, frame, &values)?;
    
    // 3. Draw Title
    // Position: x=10, y=16 (baseline guess for 24px height)
    ctx.conn.image_text8(frame, gc, 10, 16, title.as_bytes())?;
    
    // 4. Draw Close Button (Mock)
    // Close at Right - 20
    let close_x = width as i16 - 20;
    let btn_y = 6;
    let btn_size = 12;
    
    // Red color for Close
    let gc_red = ctx.conn.generate_id()?;
    let values_red = CreateGCAux::new().foreground(0xff5555);
    ctx.conn.create_gc(gc_red, frame, &values_red)?;
    
    let close_btn = Rectangle { x: close_x, y: btn_y, width: btn_size, height: btn_size };
    ctx.conn.poly_fill_rectangle(frame, gc_red, &[close_btn])?;
    
    // Cleanup
    let _ = ctx.conn.free_gc(gc);
    let _ = ctx.conn.free_gc(gc_red);
    let _ = ctx.conn.close_font(font);
    
    Ok(())
}
