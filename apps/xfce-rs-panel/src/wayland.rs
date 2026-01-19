//! Wayland protocol handling for the panel.

use wayland_client::{
    globals::GlobalList,
    protocol::{wl_compositor, wl_output, wl_seat, wl_shm, wl_surface},
    QueueHandle,
};
use anyhow::{Context, Result};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{self, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};

pub struct WaylandState {
    pub compositor: wl_compositor::WlCompositor,
    pub shm: wl_shm::WlShm,
    pub layer_shell: Option<ZwlrLayerShellV1>,
    pub outputs: Vec<wl_output::WlOutput>,
    pub seats: Vec<wl_seat::WlSeat>,
}

pub struct LayerSurfaceState {
    pub surface: wl_surface::WlSurface,
    pub layer_surface: ZwlrLayerSurfaceV1,
    pub width: u32,
    pub height: u32,
    pub configured: bool,
}

impl WaylandState {
    pub fn new(globals: &GlobalList, qh: &QueueHandle<crate::PanelState>) -> Result<Self> {
        log::info!("Querying Wayland globals");
        
        // Bind compositor
        let compositor = globals
            .bind(qh, 1..=4, ())
            .context("Failed to bind wl_compositor")?;
        log::debug!("Bound wl_compositor");

        // Bind shm
        let shm = globals
            .bind(qh, 1..=1, ())
            .context("Failed to bind wl_shm")?;
        log::debug!("Bound wl_shm");

        // Bind layer shell protocol
        let layer_shell_result: Result<ZwlrLayerShellV1, _> = globals.bind(qh, 4..=4, ());
        let layer_shell = match layer_shell_result {
            Ok(ls) => {
                log::debug!("Bound zwlr_layer_shell_v1");
                Some(ls)
            }
            Err(e) => {
                log::warn!("Layer shell protocol not available: {:?}", e);
                None
            }
        };

        Ok(WaylandState {
            compositor,
            shm,
            layer_shell,
            outputs: Vec::new(),
            seats: Vec::new(),
        })
    }

    pub fn create_surface(&self, qh: &QueueHandle<crate::PanelState>) -> wl_surface::WlSurface {
        self.compositor.create_surface(qh, ())
    }
}

pub fn create_layer_surface(
    layer_shell: &ZwlrLayerShellV1,
    surface: &wl_surface::WlSurface,
    output: Option<&wl_output::WlOutput>,
    namespace: &str,
    layer: zwlr_layer_shell_v1::Layer,
    anchor: zwlr_layer_surface_v1::Anchor,
    width: u32,
    height: u32,
    exclusive_zone: i32,
    qh: &QueueHandle<crate::PanelState>,
) -> Result<ZwlrLayerSurfaceV1> {
    // Create layer surface
    let layer_surface = layer_shell.get_layer_surface(
        surface,
        output,
        layer,
        namespace.to_string(),
        qh,
        (),
    );
    
    // Configure layer surface
    layer_surface.set_anchor(anchor);
    layer_surface.set_size(width, height);
    layer_surface.set_exclusive_zone(exclusive_zone);
    layer_surface.set_keyboard_interactivity(
        zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive
    );
    
    // Initial commit (no buffer) - required by protocol
    surface.commit();
    
    Ok(layer_surface)
}
