
pub const TITLE_HEIGHT: u16 = 24;
pub const BORDER_WIDTH: u16 = 4;

#[derive(Debug, Clone, Copy)]
pub struct FrameGeometry {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub client_x: i16,
    pub client_y: i16,
}

impl FrameGeometry {

    pub const RESIZE_HANDLE_SIZE: i16 = 10;

    pub fn hit_test(width: u16, height: u16, x: i16, y: i16) -> FramePart {
        // x, y are relative to the frame window (0,0 is top-left of frame)
        
        let w = width as i16;
        let h = height as i16;
        let border = BORDER_WIDTH as i16;
        let title_h = TITLE_HEIGHT as i16;

        // Outer bounds check
        if x < 0 || y < 0 || x >= w || y >= h {
            return FramePart::None;
        }

        // Corners
        let resize_margin = Self::RESIZE_HANDLE_SIZE;
        
        if x < resize_margin && y < resize_margin { return FramePart::CornerTopLeft; }
        if x > w - resize_margin && y < resize_margin { return FramePart::CornerTopRight; }
        if x < resize_margin && y > h - resize_margin { return FramePart::CornerBottomLeft; }
        if x > w - resize_margin && y > h - resize_margin { return FramePart::CornerBottomRight; }

        // Borders
        if x < border { return FramePart::LeftBorder; }
        if x > w - border { return FramePart::RightBorder; }
        if y > h - border { return FramePart::BottomBorder; }
        
        // Buttons
        // Close Button (Right - 20)
        let close_x = w - 20;
        let btn_y = 6;
        let btn_size = 12;
        if y >= btn_y && y < btn_y + btn_size && x >= close_x && x < close_x + btn_size {
            return FramePart::CloseButton;
        }

        // Maximize Button (Right - 40)
        let max_x = w - 40;
        if y >= btn_y && y < btn_y + btn_size && x >= max_x && x < max_x + btn_size {
            return FramePart::MaximizeButton;
        }

        // Minimize Button (Right - 60)
        let min_x = w - 60;
        if y >= btn_y && y < btn_y + btn_size && x >= min_x && x < min_x + btn_size {
            return FramePart::MinimizeButton;
        }

        // Top Edge vs TitleBar
        if y < resize_margin {
             return FramePart::TopBorder;
        }

        // If y is in titlebar area (and not top border/corner/buttons)
        if y < title_h + border {
            return FramePart::TitleBar;
        }
        
        FramePart::ClientArea
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FramePart {
    TitleBar,
    ClientArea,
    LeftBorder,
    RightBorder,
    BottomBorder,
    TopBorder, 
    CornerTopLeft,
    CornerTopRight,
    CornerBottomLeft,
    CornerBottomRight,
    CloseButton,
    MaximizeButton,
    MinimizeButton,
    None,
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_hit_test_execution() {
        let w = 808;
        let h = 632;
        let _border = 4;
        let _title = 24;
        
        // Top Left Corner
        assert_eq!(FrameGeometry::hit_test(w, h, 0, 0), FramePart::CornerTopLeft);
        
        // Title Bar (click at 100, 10)
        assert_eq!(FrameGeometry::hit_test(w, h, 100, 10), FramePart::TitleBar);
        
        // Close Button (Right - 20) = 788. Button size 12. click at 790, 8
        assert_eq!(FrameGeometry::hit_test(w, h, 790, 8), FramePart::CloseButton);
        
        // Client Area (click at 100, 100)
        assert_eq!(FrameGeometry::hit_test(w, h, 100, 100), FramePart::ClientArea);
        
        // Bottom Right
        assert_eq!(FrameGeometry::hit_test(w, h, 807, 631), FramePart::CornerBottomRight);
    }
}
