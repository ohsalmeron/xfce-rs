use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::Window;
use x11rb::protocol::render::{Picture, PictType, ConnectionExt as RenderExt, CreatePictureAux};
use x11rb::protocol::composite::{ConnectionExt as CompositeExt, Redirect};

pub struct Compositor {
    pub root: Window,
    pub overlay_window: Window,
    pub root_picture: Picture,
    pub active: bool,
}

impl Compositor {
    pub fn new<C: Connection>(_conn: &C, root: Window) -> Result<Self> {
        // We will defer activation to explicit call to avoid freezing screen during startup
        // Placeholder
        Ok(Self {
            root,
            overlay_window: x11rb::NONE,
            root_picture: x11rb::NONE,
            active: false,
        })
    }

    pub fn enable<C: Connection>(&mut self, conn: &C) -> Result<()> {
        if self.active { return Ok(()); }
        
        // 1. Redirect Subwindows (Manual)
        conn.composite_redirect_subwindows(self.root, Redirect::MANUAL)?;
        
        // 2. Get Overlay Window
        let overlay = conn.composite_get_overlay_window(self.root)?.reply()?.overlay_win;
        self.overlay_window = overlay;

        // 3. Find format for the root visual
        // We assume 24-bit or 32-bit usually.
        // We get standard formats.
        let formats = conn.render_query_pict_formats()?.reply()?;
        
        // Simplistic choice: Find the format that matches the root visual?
        // Or simplified: Find the standard RGB24 format.
        let mut root_format = x11rb::NONE;
        
        for fmt in &formats.formats {
            if fmt.type_ == PictType::DIRECT && fmt.depth == 24 {
                // Check masks for RGB (approximate)
                if fmt.direct.red_mask == 0xff && fmt.direct.green_mask == 0xff && fmt.direct.blue_mask == 0xff {
                    root_format = fmt.id;
                    break;
                }
            }
        }
        
        if root_format == x11rb::NONE {
            // Fallback to first compatible?
            if let Some(first) = formats.formats.first() {
                 root_format = first.id;
            }
        }

        // 4. Create Picture for Overlay
        self.root_picture = conn.generate_id()?;
        conn.render_create_picture(self.root_picture, self.overlay_window, root_format, &CreatePictureAux::new())?;
        
        // Allow input to pass through overlay?
        // Overlay window is usually input-transparent or we must shape it.
        // xfixes::set_window_shape_region(conn, overlay, ShapeType::INPUT, 0, 0, x11rb::NONE)?;
        // But for now let's assume we want to catch input or it passes through.
        
        self.active = true;
        Ok(())
    }

    pub fn find_format<C: Connection>(conn: &C, depth: u8) -> Result<x11rb::protocol::render::Pictformat> {
        let formats = conn.render_query_pict_formats()?.reply()?;
        for fmt in &formats.formats {
            if fmt.type_ == PictType::DIRECT && fmt.depth == depth {
                return Ok(fmt.id);
            }
        }
         // Fallback
        Ok(formats.formats.first().map(|f| f.id).unwrap_or(x11rb::NONE))
    }

    pub fn paint<C: Connection>(
        &self,
        conn: &C,
        clients: impl Iterator<Item = (Picture, i16, i16, u16, u16)>,
    ) -> Result<()> {
        if !self.active { return Ok(()); }
        
        // Clear overlay (fill with transparent - assuming ARGB visual for overlay?)
        // If Overlay is just a window, we might need to clear it.
        // For MVP, enable "wobbly" effect by NOT clearing? ;) 
        // No, we should clear to avoid trails.
        
        use x11rb::protocol::xproto::Rectangle;
        use x11rb::protocol::render::Color;

        // Clear with transparent black
        let rect = x11rb::protocol::xproto::Rectangle {
            x: 0, y: 0, width: 3840, height: 2160, // TODO: Get screen size
        };
        
        // To clear, we can composite Clear or Src with 0 alpha.
        conn.render_fill_rectangles(
            x11rb::protocol::render::PictOp::CLEAR,
            self.root_picture,
            Color { red: 0, green: 0, blue: 0, alpha: 0 },
            &[rect],
        )?;

        for (pic, x, y, w, h) in clients {
            // Draw simple drop shadow (offset +10, semi-transparent black)
            let shadow_rect = Rectangle {
                x: x.wrapping_add(10),
                y: y.wrapping_add(10),
                width: w,
                height: h,
            };
            
            conn.render_fill_rectangles(
                x11rb::protocol::render::PictOp::OVER,
                self.root_picture,
                Color { red: 0, green: 0, blue: 0, alpha: 0x4000 }, // ~25% alpha
                &[shadow_rect],
            )?;

            conn.render_composite(
                x11rb::protocol::render::PictOp::OVER,
                pic,
                x11rb::NONE,
                self.root_picture,
                0, 0,
                0, 0,
                x, y,
                w, h,
            )?;
        }
        Ok(())
    }
}
