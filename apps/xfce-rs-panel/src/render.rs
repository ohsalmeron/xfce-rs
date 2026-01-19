//! Simple rendering functions.

use crate::buffer::Buffer;
use anyhow::Result;
use std::os::unix::io::AsRawFd;

/// Fill buffer with a solid color.
pub fn fill_color(buffer: &Buffer, color: u32) -> Result<()> {
    let stride = buffer.width * 4;
    let size = stride * buffer.height;
    
    unsafe {
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            size as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            buffer.memfd.as_file().as_raw_fd(),
            0,
        );
        if ptr == libc::MAP_FAILED {
            return Err(anyhow::anyhow!("mmap failed for rendering"));
        }
        
        let pixels = std::slice::from_raw_parts_mut(ptr as *mut u32, (size / 4) as usize);
        // Fill with color (ARGB format, little endian)
        for pixel in pixels.iter_mut() {
            *pixel = color.to_le();
        }
        
        libc::munmap(ptr, size as usize);
    }
    
    Ok(())
}
