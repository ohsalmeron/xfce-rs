use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Window, ConnectionExt as XProtoExt};
use x11rb::protocol::render::{Picture, PictType, ConnectionExt as RenderExt, CreatePictureAux};
use x11rb::protocol::composite::{ConnectionExt as CompositeExt, Redirect};
use x11rb::protocol::xfixes::ConnectionExt as XFixesExt;
use x11rb::protocol::shape::{ConnectionExt as ShapeExt, SK, SO};
use tracing::{error, warn, debug, info};
use crate::window::error::{log_warn, log_and_ignore};

pub struct Compositor {
    pub root: Window,
    pub overlay_window: Window,
    pub root_picture: Picture,
    pub active: bool,
}

impl Compositor {
    pub fn new<C: Connection>(_conn: &C, root: Window, _screen_num: usize) -> Result<Self> {
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

        // 3. Find format matching the overlay window's depth
        let geom = conn.get_geometry(self.overlay_window)?.reply()?;
        let target_depth = geom.depth;
        let formats = conn.render_query_pict_formats()?.reply()?;
        let mut root_format = x11rb::NONE;
        
        for fmt in &formats.formats {
            if fmt.type_ == PictType::DIRECT && fmt.depth == target_depth {
                root_format = fmt.id;
                break;
            }
        }

        if root_format == x11rb::NONE {
            if let Some(first) = formats.formats.first() {
                 root_format = first.id;
            }
        }
        debug!("Compositor using PictFormat {} for Overlay Window {} (depth {})", root_format, self.overlay_window, target_depth);

        // 4. Create Picture for Overlay
        self.root_picture = conn.generate_id()?;
        conn.render_create_picture(self.root_picture, self.overlay_window, root_format, &CreatePictureAux::new())?;
        info!("🎬 Compositor root picture {} created for overlay {} with depth {}", self.root_picture, self.overlay_window, target_depth);
        
        // Making overlay window input-transparent so clicks pass through to windows below
        if let Ok(region) = conn.generate_id() {
            if let Err(e) = XFixesExt::xfixes_create_region(conn, region, &[]) {
                error!("Failed to create XFixes region for overlay transparency: {}", e);
            } else {
                log_warn(ShapeExt::shape_mask(conn, SO::SET, SK::BOUNDING, self.overlay_window, 0, 0, x11rb::NONE), "shape_mask for overlay bounding");
                log_warn(XFixesExt::xfixes_set_window_shape_region(conn, self.overlay_window, SK::INPUT, 0, 0, region), "xfixes_set_window_shape_region for overlay input");
                log_and_ignore(XFixesExt::xfixes_destroy_region(conn, region), "xfixes_destroy_region cleanup");
            }
        }
        
        // Ensure overlay is mapped
        if let Err(e) = conn.map_window(self.overlay_window) {
            error!("Failed to map overlay window: {}", e);
        }
        
        self.active = true;
        Ok(())
    }

    pub fn find_format<C: Connection>(conn: &C, depth: u8) -> Result<x11rb::protocol::render::Pictformat> {
        let formats = conn.render_query_pict_formats()?.reply()?;
        // Prioritize direct formats with the exact depth
        for fmt in &formats.formats {
            if fmt.type_ == PictType::DIRECT && fmt.depth == depth {
                return Ok(fmt.id);
            }
        }
        // Fallback: any direct format
        for fmt in &formats.formats {
            if fmt.type_ == PictType::DIRECT {
                debug!("Falling back to direct format with depth {}", fmt.depth);
                return Ok(fmt.id);
            }
        }
        // Ultimate fallback: first available format
        Ok(formats.formats.first().map(|f| f.id).unwrap_or(x11rb::NONE))
    }

    pub fn paint<C: Connection>(
        &self,
        conn: &C,
        screen_w: u16,
        screen_h: u16,
        clients: impl Iterator<Item = (Option<Picture>, Picture, i16, i16, u16, u16, u16, u16, u16, u16)>,
    ) -> Result<()> {
        if !self.active { return Ok(()); }
        
        use x11rb::protocol::xproto::Rectangle;
        use x11rb::protocol::render::Color;

        // Clear with dark slate-blue
        let rect = x11rb::protocol::xproto::Rectangle {
            x: 0, y: 0, width: screen_w, height: screen_h,
        };
        
        conn.render_fill_rectangles(
            x11rb::protocol::render::PictOp::SRC,
            self.root_picture,
            Color { red: 0x2424, green: 0x2424, blue: 0x3030, alpha: 0xffff },
            &[rect],
        )?;

        // Create a vector to avoid double iteration issues
        let client_list: Vec<_> = clients.collect();

        // 1. Draw all shadows first
        for (frame_pic_opt, _, x, y, frame_w, frame_h, _, _, _, _) in &client_list {
            if frame_pic_opt.is_none() { continue; }
            let shadow_rect = Rectangle {
                x: x.wrapping_add(6),
                y: y.wrapping_add(6),
                width: *frame_w,
                height: *frame_h,
            };
            
            if let Err(e) = conn.render_fill_rectangles(
                x11rb::protocol::render::PictOp::OVER,
                self.root_picture,
                Color { red: 0, green: 0, blue: 0, alpha: 0x7000 }, // ~44% alpha
                &[shadow_rect],
            ) {
                warn!("Failed to render shadow rectangle: {}", e);
            }
        }

        // 2. Draw all windows (Frame + Content)
        for (frame_pic_opt, content_pic, x, y, frame_w, frame_h, border, title_h, client_w, client_h) in &client_list {
            // Composite Frame (decorations) if present
            if let Some(frame_pic) = frame_pic_opt {
                debug!("Compositing frame picture {} to root picture {} at ({}, {}) with size {}x{}", frame_pic, self.root_picture, x, y, frame_w, frame_h);
                if let Err(e) = conn.render_composite(
                    x11rb::protocol::render::PictOp::OVER,
                    *frame_pic,
                    x11rb::NONE,
                    self.root_picture,
                    0, 0,
                    0, 0,
                    *x, *y,
                    *frame_w, *frame_h,
                ) {
                    warn!("Failed to composite frame picture: {}", e);
                }
            }

            // Composite Client Content (terminal)
            if *client_w > 0 && *client_h > 0 {
                debug!("Compositing content picture {} to root picture {} at ({}, {}) with size {}x{}", content_pic, self.root_picture, *x + *border as i16, *y + (*title_h + *border) as i16, client_w, client_h);
                if let Err(e) = conn.render_composite(
                    x11rb::protocol::render::PictOp::OVER,
                    *content_pic,
                    x11rb::NONE,
                    self.root_picture,
                    0, 0,
                    0, 0,
                    *x + *border as i16, *y + (*title_h + *border) as i16,
                    *client_w, *client_h,
                ) {
                    warn!("Failed to composite content picture: {}", e);
                }
            }
        }
        conn.flush()?;
        Ok(())
    }

    pub fn set_cursor<C: Connection>(&self, conn: &C, cursor: x11rb::protocol::xproto::Cursor) -> Result<()> {
        if self.overlay_window != x11rb::NONE {
            use x11rb::protocol::xproto::ChangeWindowAttributesAux;
            let values = ChangeWindowAttributesAux::new().cursor(cursor);
            conn.change_window_attributes(self.overlay_window, &values)?;
        }
        Ok(())
    }
}
