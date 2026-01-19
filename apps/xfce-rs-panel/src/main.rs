//! Minimal Wayland panel for XFCE.rs.

mod wayland;
mod buffer;
mod render;
mod config;

use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::{
        wl_registry::WlRegistry,
        wl_compositor::WlCompositor,
        wl_shm::WlShm,
        wl_shm_pool::WlShmPool,
        wl_buffer::WlBuffer,
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::ZwlrLayerShellV1,
    zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
};
use anyhow::{Context, Result};
use std::time::Duration;

pub struct PanelState {
    pub wayland: wayland::WaylandState,
    pub layer_surface: Option<wayland::LayerSurfaceState>,
    pub buffer: Option<buffer::Buffer>,
    pub qh: QueueHandle<Self>,
    pub event_queue: EventQueue<Self>,
    pub config: config::PanelConfig,
}

impl PanelState {
    pub fn new() -> Result<Self> {
        env_logger::init();
        log::info!("XFCE.rs Panel starting");

        let config = config::PanelConfig::load()?;

        let conn = Connection::connect_to_env()
            .context("Failed to connect to Wayland display")?;
        
        log::info!("Connected to Wayland display");
        
        let (globals, event_queue) = registry_queue_init::<Self>(&conn)
            .context("Failed to initialize wayland registry")?;
        
        let qh = event_queue.handle();

        let wayland = wayland::WaylandState::new(&globals, &qh)?;

        if wayland.layer_shell.is_none() {
            return Err(anyhow::anyhow!("Layer shell protocol not available"));
        }

        Ok(PanelState {
            wayland,
            layer_surface: None,
            buffer: None,
            qh,
            event_queue,
            config,
        })
    }

    pub fn create_layer_surface(&mut self) -> Result<()> {
        let layer_shell = self.wayland.layer_shell.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Layer shell not available"))?;

        let surface = self.wayland.create_surface(&self.qh);
        
        let layer = match self.config.position {
            config::PanelPosition::Top => wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer::Top,
            config::PanelPosition::Bottom => wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer::Bottom,
        };

        use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Anchor;
        let anchor = Anchor::Left | Anchor::Right | match self.config.position {
            config::PanelPosition::Top => Anchor::Top,
            config::PanelPosition::Bottom => Anchor::Bottom,
        };

        let layer_surface = wayland::create_layer_surface(
            layer_shell,
            &surface,
            None,
            "xfce-rs-panel",
            layer,
            anchor,
            0, // width (0 = full width)
            self.config.height,
            self.config.height as i32, // exclusive zone
            &self.qh,
        )?;

        self.layer_surface = Some(wayland::LayerSurfaceState {
            surface,
            layer_surface,
            width: 1920, // Will be set by configure event
            height: self.config.height,
            configured: false,
        });

        // Create initial buffer with default size to make panel visible immediately
        // We'll recreate it with correct size when configure event arrives
        log::info!("Creating initial buffer with default size");
        if let Ok(buf) = buffer::Buffer::create(
            &self.wayland.shm,
            1920, // Default width
            self.config.height,
            &self.qh,
        ) {
            // Fill with semi-transparent dark background (ARGB: 0xCC333333)
            if let Err(e) = render::fill_color(&buf, 0xCC333333) {
                log::error!("Failed to render initial buffer: {}", e);
            } else {
                if let Some(ref layer_surface_state) = self.layer_surface {
                    layer_surface_state.surface.attach(Some(&buf.buffer), 0, 0);
                    layer_surface_state.surface.commit();
                    self.buffer = Some(buf);
                    log::info!("Initial buffer attached and committed");
                }
            }
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.event_queue.flush()?;
        Ok(())
    }
}

// Dispatch implementations
wayland_client::delegate_dispatch!(PanelState: [WlRegistry: GlobalListContents] => PanelState);
wayland_client::delegate_dispatch!(PanelState: [WlCompositor: ()] => PanelState);
wayland_client::delegate_dispatch!(PanelState: [WlShm: ()] => PanelState);
wayland_client::delegate_dispatch!(PanelState: [WlSurface: ()] => PanelState);
wayland_client::delegate_dispatch!(PanelState: [WlShmPool: ()] => PanelState);
wayland_client::delegate_dispatch!(PanelState: [WlBuffer: ()] => PanelState);
wayland_client::delegate_dispatch!(PanelState: [ZwlrLayerShellV1: ()] => PanelState);

// Handle layer surface events
impl Dispatch<ZwlrLayerSurfaceV1, ()> for PanelState {
    fn event(
        state: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as Proxy>::Event,
        _: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event;
        log::debug!("Layer surface event received");
        match event {
            Event::Configure { serial, width, height } => {
                log::info!("Layer surface configured: {}x{} (serial: {})", width, height, serial);
                
                if let Some(ref mut layer_surface_state) = state.layer_surface {
                    layer_surface_state.width = width;
                    layer_surface_state.height = height;
                    layer_surface_state.configured = true;
                    
                    proxy.ack_configure(serial);
                    
                    // Create buffer with new size
                    if let Ok(buf) = buffer::Buffer::create(
                        &state.wayland.shm,
                        width,
                        height,
                        &state.qh,
                    ) {
                        // Fill with semi-transparent dark background (ARGB: 0xCC333333)
                        if let Err(e) = render::fill_color(&buf, 0xCC333333) {
                            log::error!("Failed to render panel: {}", e);
                        } else {
                            layer_surface_state.surface.attach(Some(&buf.buffer), 0, 0);
                            layer_surface_state.surface.commit();
                            state.buffer = Some(buf);
                        }
                    }
                }
            }
            Event::Closed => {
                log::info!("Layer surface closed by compositor");
            }
            _ => {}
        }
    }
}

fn main() -> Result<()> {
    let mut app = PanelState::new()?;
    
    // Helper function to dispatch events (workaround for borrow checker)
    fn dispatch_events(app: &mut PanelState) -> Result<usize> {
        unsafe {
            let queue_ptr: *mut EventQueue<PanelState> = &mut app.event_queue;
            let state_ptr: *mut PanelState = app;
            Ok((*queue_ptr).dispatch_pending(&mut *state_ptr)?)
        }
    }
    
    // Dispatch events to get globals
    dispatch_events(&mut app)?;
    app.flush()?;
    std::thread::sleep(Duration::from_millis(100));
    dispatch_events(&mut app)?;
    app.flush()?;

    // Create layer surface
    log::info!("Creating layer surface");
    app.create_layer_surface()?;
    app.flush()?;

    // Dispatch events in a loop
    log::info!("Entering event loop");
    loop {
        dispatch_events(&mut app)?;
        app.flush()?;
        std::thread::sleep(Duration::from_millis(16)); // ~60 FPS
    }
}
