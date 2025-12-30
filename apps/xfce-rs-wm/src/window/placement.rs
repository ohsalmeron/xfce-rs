pub fn center_window(screen_width: u16, screen_height: u16, win_width: u16, win_height: u16) -> (i16, i16) {
    let x = (screen_width as i32 - win_width as i32) / 2;
    let y = (screen_height as i32 - win_height as i32) / 2;
    (x.max(0) as i16, y.max(0) as i16)
}

pub fn cascade_placement(
    screen_width: u16, 
    screen_height: u16, 
    win_width: u16, 
    win_height: u16, 
    existing_origins: &[(i16, i16)]
) -> (i16, i16) {
    let start_x: i16 = 20;
    let start_y: i16 = 40;
    let step: i16 = 25;
    
    let mut x = start_x;
    let mut y = start_y;
    
    loop {
        // Check if this origin is taken (approximate)
        let mut overlap = false;
        for &(ex, ey) in existing_origins {
            if (x - ex).abs() < 15 && (y - ey).abs() < 15 {
                overlap = true;
                break;
            }
        }
        
        if !overlap {
            // Check bounds
            if (x as i32 + win_width as i32) > screen_width as i32 || (y as i32 + win_height as i32) > screen_height as i32 {
                // Reset to top left if we run off screen
                x = start_x + 10;
                y = start_y + 10;
                // Ideally we'd have a 'lap' counter to offset reset
                break; 
            }
            break;
        }
        
        x += step;
        y += step;
    }
    
    (x, y)
}
