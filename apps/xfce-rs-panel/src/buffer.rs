//! Shared memory buffer handling.

use wayland_client::{
    protocol::{wl_buffer, wl_shm},
    QueueHandle,
};
use anyhow::Result;
use std::os::unix::io::{AsRawFd, BorrowedFd};

pub struct Buffer {
    pub buffer: wl_buffer::WlBuffer,
    pub memfd: memfd::Memfd,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
}

impl Buffer {
    pub fn create(
        shm: &wl_shm::WlShm,
        width: u32,
        height: u32,
        qh: &QueueHandle<crate::PanelState>,
    ) -> Result<Self> {
        let stride = width * 4; // ARGB32, 4 bytes per pixel
        let size = stride * height;

        // Create shared memory file using memfd
        let memfd_name = format!("xfce-rs-panel-{}", std::process::id());
        let memfd = memfd::MemfdOptions::default()
            .allow_sealing(false)
            .create(&memfd_name)?;
        
        let file = memfd.as_file();
        file.set_len(size as u64)?;

        // Create buffer from shm pool
        let fd = unsafe { BorrowedFd::borrow_raw(memfd.as_raw_fd()) };
        let pool = shm.create_pool(fd, size as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        pool.destroy();

        Ok(Buffer {
            buffer,
            memfd,
            width,
            height,
            stride,
        })
    }

    pub fn data_ptr(&self) -> Result<*mut u8> {
        let size = (self.stride * self.height) as usize;
        unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.memfd.as_file().as_raw_fd(),
                0,
            );
            if ptr == libc::MAP_FAILED {
                return Err(anyhow::anyhow!("mmap failed"));
            }
            Ok(ptr as *mut u8)
        }
    }

    pub fn unmap_data(&self, ptr: *mut u8) {
        let size = (self.stride * self.height) as usize;
        unsafe {
            libc::munmap(ptr as *mut std::ffi::c_void, size);
        }
    }
}
