use anyhow::Result;
use x11rb::protocol::xproto::{ConnectionExt, Window, CreateGCAux, ChangeGCAux, Rectangle};
use x11rb::connection::Connection;
use tracing::debug;

use crate::core::context::Context;

pub fn draw_decoration(ctx: &Context, frame: Window, title: &str, width: u16, height: u16, title_height: u16) -> Result<()> {
    if width == 0 || height == 0 { return Ok(()); }

    // 1. Create IDs
    let gc = ctx.conn.generate_id()?;
    let font = ctx.conn.generate_id()?;

    // Try to open a font. 10x20 is bigger and clearer than fixed.
    let mut font_opened = true;
    if let Err(_) = ctx.conn.open_font(font, b"10x20") {
        if let Err(e) = ctx.conn.open_font(font, b"fixed") {
            debug!("Failed to open font 'fixed': {}. Continuing without text.", e);
            font_opened = false;
        }
    }
    
    // Create GC with colors
    let values = CreateGCAux::new()
        .foreground(0x3c3c3c) // Dark charcoal background
        .font(font);
        
    ctx.conn.create_gc(gc, frame, &values)?;
    
    // 2. Clear Background (fills the entire frame including borders)
    let bg_rect = Rectangle { x: 0, y: 0, width, height };
    ctx.conn.poly_fill_rectangle(frame, gc, &[bg_rect])?;
    
    if title_height > 0 && font_opened {
        // 3. Draw Title Text
        ctx.conn.change_gc(gc, &ChangeGCAux::new().foreground(0xe0e0e0))?;
        if !title.is_empty() {
            // Adjust y for better vertical centering with 10x20 font
            // 10x20 font usually has baseline around 15-16
            let text_y = 15 + (title_height as i16 / 10); 
            if let Err(e) = ctx.conn.image_text8(frame, gc, 12, text_y, title.as_bytes()) {
                debug!("Failed to draw title text: {}", e);
            }
        }
        
        // 4. Draw Close Button (Mock)
        let close_x = width as i16 - 20;
        let btn_y = 6;
        let btn_size = 12;
        
        let gc_red = ctx.conn.generate_id()?;
        let values_red = CreateGCAux::new().foreground(0xff5555);
        ctx.conn.create_gc(gc_red, frame, &values_red)?;
        
        let close_btn = Rectangle { x: close_x, y: btn_y, width: btn_size, height: btn_size };
        ctx.conn.poly_fill_rectangle(frame, gc_red, &[close_btn])?;
        let _ = ctx.conn.free_gc(gc_red);
    }
    
    // Cleanup
    let _ = ctx.conn.free_gc(gc);
    if font_opened {
        let _ = ctx.conn.close_font(font);
    }
    
    Ok(())
}
