use std::collections::HashMap;
use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Window, ConnectionExt, CreateWindowAux, WindowClass, EventMask, AtomEnum, PropMode, MapState, SubwindowMode, ConfigWindow, ConfigureWindowAux};
use x11rb::protocol::composite::ConnectionExt as CompositeExt;
use x11rb::protocol::damage::{ConnectionExt as DamageExt, ReportLevel, Damage};
use x11rb::protocol::render::{ConnectionExt as RenderExt, CreatePictureAux, Picture};
use x11rb::protocol::xfixes::ConnectionExt as XFixesExt;
use x11rb::protocol::shape::{ConnectionExt as ShapeExt, SO, SK};
use x11rb::protocol::sync::ConnectionExt as SyncExt;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::protocol::Event;
use tracing::{info, debug, warn, error};

use crate::core::context::Context;
use crate::window::client::Client;
use crate::window::frame::{FrameGeometry, FramePart, TITLE_HEIGHT, BORDER_WIDTH};
use crate::window::draw::draw_decoration;
use crate::window::placement::{center_window, cascade_placement};
use crate::window::cursors::Cursors;
use crate::window::compositor::Compositor;
use crate::window::settings::SettingsManager;
use crate::window::error::{ErrorTracker, log_warn};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapZone {
    None,
    Left,
    Right,
    Top,
}


#[derive(Debug, Clone, Copy, PartialEq)]

pub enum DragState {
    None,
    Moving {
        window: Window,
        start_pointer_x: i16,
        start_pointer_y: i16,
        start_frame_x: i16,
        start_frame_y: i16,
        snap: SnapZone,
    },
    Resizing {
        window: Window,
        start_pointer_x: i16,
        start_pointer_y: i16,
        start_width: u16,
        start_height: u16,
    },
}

#[derive(Debug, Clone)]
pub struct UnmanagedWindow {
    pub picture: Picture,
    pub damage: Option<Damage>,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

pub struct WindowManager {
    pub ctx: Context,
    pub clients: HashMap<Window, Client>,
    pub drag_state: DragState,
    pub current_workspace: u32,
    pub cursors: Cursors,
    pub compositor: Compositor,
    pub last_click_time: u32,
    pub last_click_window: Window,
    pub mru_stack: Vec<Window>,
    pub focused_window: Option<Window>,
    pub settings_manager: SettingsManager,
    pub unmanaged_windows: HashMap<Window, UnmanagedWindow>,
    pub error_tracker: ErrorTracker,
}

impl WindowManager {
    pub fn new(ctx: Context, settings_manager: SettingsManager) -> Result<Self> {
        let error_tracker = ErrorTracker::new();

        // Initialize extensions with error checking
        let _ = error_tracker.warn_if_failed(
            ctx.conn.composite_query_version(0, 4)?.reply().map(|_| ()),
            "query composite version",
            crate::window::error::ErrorCategory::Compositor
        );
        let _ = error_tracker.warn_if_failed(
            ctx.conn.damage_query_version(1, 1)?.reply().map(|_| ()),
            "query damage version",
            crate::window::error::ErrorCategory::X11
        );
        let _ = error_tracker.warn_if_failed(
            XFixesExt::xfixes_query_version(&ctx.conn, 5, 0)?.reply().map(|_| ()),
            "query xfixes version",
            crate::window::error::ErrorCategory::X11
        );
        let _ = error_tracker.warn_if_failed(
            ShapeExt::shape_query_version(&ctx.conn)?.reply().map(|_| ()),
            "query shape version",
            crate::window::error::ErrorCategory::X11
        );

        let cursors = Cursors::new(&ctx.conn, ctx.screen_num)?;
        let mut compositor = Compositor::new(&ctx.conn, ctx.root_window, ctx.screen_num)?;

        // Enable compositor immediately
        if let Err(e) = compositor.enable(&ctx.conn) {
             error_tracker.record_compositor_error("enable compositor", e);
        } else {
             info!("Compositor enabled.");
             log_warn(compositor.set_cursor(&ctx.conn, cursors.normal), "set compositor cursor");
        }
        
        // Set root cursor
        use x11rb::protocol::xproto::ChangeWindowAttributesAux;
        log_warn(ctx.conn.change_window_attributes(ctx.root_window, &ChangeWindowAttributesAux::new().cursor(cursors.normal)), "set root cursor");
        // Select input events for the root window to receive necessary events
        let event_mask = EventMask::SUBSTRUCTURE_REDIRECT
            | EventMask::SUBSTRUCTURE_NOTIFY
            | EventMask::PROPERTY_CHANGE
            | EventMask::BUTTON_PRESS
            | EventMask::BUTTON_RELEASE
            | EventMask::KEY_PRESS
            | EventMask::KEY_RELEASE;
        log_warn(
            ctx.conn.change_window_attributes(
                ctx.root_window,
                &ChangeWindowAttributesAux::new().event_mask(event_mask),
            ),
            "set root window event mask",
        );
        
        // Grab Alt+Tab (Mod1 + 23)
        let modifiers = [
             x11rb::protocol::xproto::ModMask::M1,
             x11rb::protocol::xproto::ModMask::M1 | x11rb::protocol::xproto::ModMask::LOCK,
             x11rb::protocol::xproto::ModMask::M1 | x11rb::protocol::xproto::ModMask::M2,
             x11rb::protocol::xproto::ModMask::M1 | x11rb::protocol::xproto::ModMask::LOCK | x11rb::protocol::xproto::ModMask::M2,
        ];
        
        for mods in modifiers {
             if let Err(e) = ctx.conn.grab_key(
                 false,
                 ctx.root_window,
                 mods,
                 23, // Tab
                 x11rb::protocol::xproto::GrabMode::ASYNC,
                 x11rb::protocol::xproto::GrabMode::ASYNC
             ) {
                 warn!("Failed to grab Alt+Tab with modifiers {:?}: {}", mods, e);
             }
        }

        Ok(Self {
            ctx,
            clients: HashMap::new(),
            drag_state: DragState::None,
            current_workspace: 0,
            cursors,
            compositor,
            last_click_time: 0,
            last_click_window: x11rb::NONE,
            mru_stack: Vec::new(),
            focused_window: None,
            settings_manager,
            unmanaged_windows: HashMap::new(),
            error_tracker,
        })
    }

    pub fn scan_windows(&mut self) -> Result<()> {
        let tree = self.ctx.conn.query_tree(self.ctx.root_window)?.reply()?;
        info!("Scanning {} windows...", tree.children.len());

        let mut to_manage = Vec::new();

        for &win in &tree.children {
            if let Ok(attrs) = self.ctx.conn.get_window_attributes(win)?.reply() {
                if !attrs.override_redirect && attrs.map_state != x11rb::protocol::xproto::MapState::UNMAPPED {
                    to_manage.push((win, attrs));
                }
            }
        }

        for (win, _attrs) in to_manage {
            self.manage_window(win)?;
        }
        Ok(())
    }

    pub fn manage_window(&mut self, win: Window) -> Result<()> {
        // 1. Get Window Name (with fallbacks)
        let mut name = "Unnamed".to_string();
        for &atom in &[self.ctx.atoms._NET_WM_NAME, self.ctx.atoms.UTF8_STRING, AtomEnum::WM_NAME.into()] {
            if let Ok(reply) = self.ctx.conn.get_property(false, win, atom, AtomEnum::ANY, 0, 1024)?.reply() {
                if !reply.value.is_empty() {
                    if let Ok(s) = String::from_utf8(reply.value) {
                        name = s;
                        break;
                    }
                }
            }
        }
        debug!("Managing window {} ({})", win, name);
        
        // Check for _NET_WM_DESKTOP
        let mut workspace = self.current_workspace;

        let reply = self.ctx.conn.get_property(
            false,
            win,
            self.ctx.atoms._NET_WM_DESKTOP,
            AtomEnum::CARDINAL,
            0,
            1,
        )?.reply();
        
        if let Ok(prop) = reply {
            if prop.type_ == u32::from(AtomEnum::CARDINAL) && prop.format == 32 && prop.value_len == 1 {
                if let Some(w) = prop.value32().and_then(|mut i| i.next()) {
                     workspace = w;
                     debug!("Window {} is on workspace {}", win, workspace);
                }
            }
        }
        
        let geom = self.ctx.conn.get_geometry(win)?.reply()?;
        let mut x = geom.x;
        let mut y = geom.y;
        
        // Fetch Window Type
        let mut window_types = Vec::new();
        let mut is_dialog = false;
        let type_reply = self.ctx.conn.get_property(false, win, self.ctx.atoms._NET_WM_WINDOW_TYPE, AtomEnum::ATOM, 0, 1024)?.reply();
        if let Ok(prop) = type_reply {
            if prop.type_ == u32::from(AtomEnum::ATOM) && prop.format == 32 {
                for atom in prop.value32().unwrap() {
                    window_types.push(atom);
                    if atom == self.ctx.atoms._NET_WM_WINDOW_TYPE_DIALOG { is_dialog = true; }
                }
            }
        }

        let is_dock = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_DOCK);
        let is_desktop = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_DESKTOP);

        // Fetch Window State
        let mut is_fullscreen = false;
        let mut is_maximized = false;
        let mut is_modal = false;
        let mut is_sticky = false;
        let mut demands_attention = false;
        let mut skip_taskbar = false;
        let mut skip_pager = false;
        let mut is_shaded = false;
        let mut is_above = false;
        let mut is_below = false;
        if let Ok(reply) = self.ctx.conn.get_property(false, win, self.ctx.atoms._NET_WM_STATE, AtomEnum::ATOM, 0, 1024)?.reply() {
            if reply.type_ == u32::from(AtomEnum::ATOM) && reply.format == 32 {
                for atom in reply.value32().unwrap() {
                    if atom == self.ctx.atoms._NET_WM_STATE_FULLSCREEN { is_fullscreen = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_MAXIMIZED_VERT || atom == self.ctx.atoms._NET_WM_STATE_MAXIMIZED_HORZ { is_maximized = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_MODAL { is_modal = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_STICKY { is_sticky = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_DEMANDS_ATTENTION { demands_attention = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_SKIP_TASKBAR { skip_taskbar = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_SKIP_PAGER { skip_pager = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_SHADED { is_shaded = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_ABOVE { is_above = true; }
                    else if atom == self.ctx.atoms._NET_WM_STATE_BELOW { is_below = true; }
                }
            }
        }

        if is_sticky { workspace = 0xFFFFFFFF; }

        // Smart Placement if position is 0,0 (ported from xfwm4 clientPlace)
        if x == 0 && y == 0 && !is_dock && !is_desktop {
             let (nx, ny) = self.place_window(geom.width, geom.height);
             x = nx;
             y = ny;
             debug!("Smart placed window {} at ({}, {})", win, x, y);
        }
        
        // Fetch Transient For
        let mut transient_for = None;
        let trans_reply = self.ctx.conn.get_property(false, win, self.ctx.atoms.WM_TRANSIENT_FOR, AtomEnum::WINDOW, 0, 1)?.reply();
        if let Ok(prop) = trans_reply {
            if prop.type_ == u32::from(AtomEnum::WINDOW) && prop.format == 32 {
                if let Some(parent) = prop.value32().and_then(|mut i| i.next()) {
                    transient_for = Some(parent);
                    is_dialog = true;
                }
            }
        }
        
        let (group_leader, accepts_input, is_urgent) = self.read_wm_hints(win);
        let client_leader = self.read_client_leader(win);

        let user_time_window = self.read_user_time_window(win);
        let startup_id = self.read_startup_id(win);
        let user_time = if let Some(utw) = user_time_window {
             self.read_user_time(utw)
        } else {
             self.read_user_time(win)
        };
        let pid = self.read_pid(win);
        let frame_extents = self.read_frame_extents(win);

        let (gravity, _min_w, _min_h, _max_w, _max_h) = self.read_size_hints(win);
        let sync_counter = self.read_sync_counter(win);
        let is_shaped = self.read_is_shaped(win);
        
        let is_splash = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_SPLASH);
        let is_utility = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_UTILITY);
        let is_toolbar = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_TOOLBAR);
        let is_menu = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_MENU);

        let (motif_decor, motif_title) = self.read_motif_hints(win);
        
        let is_csd = self.has_csd_hint(win);
        let (border, title) = if is_fullscreen || is_desktop || is_dock || !motif_decor || is_csd || is_splash || is_menu { (0, 0) } else if !motif_title || is_toolbar || is_utility { (BORDER_WIDTH, 0) } else { (BORDER_WIDTH, TITLE_HEIGHT) };
        
        use crate::window::{LAYER_DOCK, LAYER_NORMAL, LAYER_FULLSCREEN, LAYER_DESKTOP, LAYER_ONTOP, LAYER_BELOW, LAYER_NOTIFICATION};
        let layer = if is_desktop {
            LAYER_DESKTOP
        } else if is_dock {
            LAYER_DOCK
        } else if is_fullscreen {
            LAYER_FULLSCREEN
        } else if is_above {
            LAYER_ONTOP
        } else if is_below {
            LAYER_BELOW
        } else if is_splash || is_menu {
            LAYER_ONTOP 
        } else if window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_NOTIFICATION) {
            LAYER_NOTIFICATION
        } else {
            LAYER_NORMAL
        };
        
        // Final Frame coordinates calculation
        let (frame_x, frame_y) = if x == 0 && y == 0 && !is_dock && !is_desktop {
             let (nx, ny) = self.place_window(geom.width, geom.height);
             debug!("Smart placed window {} at ({}, {})", win, nx, ny);
             (nx, ny)
        } else if (x <= 1 || y <= 1) && !is_dock && !is_desktop && !is_splash && !is_menu {
             // Handle "near corner" placement with centering or cascading
             let screen = &self.ctx.conn.setup().roots[self.ctx.screen_num];
             if is_dialog || is_utility {
                 let (nx, ny) = center_window(screen.width_in_pixels, screen.height_in_pixels, geom.width, geom.height);
                 (nx, ny)
             } else {
                  let origins: Vec<(i16, i16)> = self.clients.values().map(|c| (c.x, c.y)).collect();
                  let (nx, ny) = cascade_placement(screen.width_in_pixels, screen.height_in_pixels, geom.width, geom.height, &origins);
                  (nx, ny)
             }
        } else {
             // Explicitly provided coordinates are for client area (usually)
             let mut tx = x;
            let mut ty = y;
             Self::gravitate(gravity, 1, border, title, &mut tx, &mut ty);
             (tx - border as i16, ty - (title + border) as i16)
        };

        let (fix_x, fix_y, fix_w, fix_h) = if is_desktop {
            (0, 0, self.ctx.screen_width as u16, self.ctx.screen_height as u16)
        } else {
            (frame_x, frame_y, geom.width, geom.height)
        };

        let frame_geom = FrameGeometry {
            x: fix_x,
            y: fix_y,
            width: fix_w + (2 * border),
            height: fix_h + title + (2 * border),
            client_x: border as i16,
            client_y: (title + border) as i16,
        };
        debug!("Frame geometry for window {}: {:?}", win, frame_geom);
        let frame_win = self.ctx.conn.generate_id()?;
        
        // Listen for frame events (decorations) and motion
        let values = CreateWindowAux::new()
            .event_mask(EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT | EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::PROPERTY_CHANGE)
            .background_pixel(0)
            .border_pixel(0x000000);
            
        self.ctx.conn.create_window(
            self.ctx.root_depth,
            frame_win,
            self.ctx.root_window,
            frame_geom.x,
            frame_geom.y,
            frame_geom.width,
            frame_geom.height,
            if is_dock { 0 } else { 1 },
            WindowClass::INPUT_OUTPUT,
            0,
            &values,
        )?;

        if !is_dock && !is_desktop {
            // Passive grab for click-to-focus on the client window
            // SYNC mode is crucial: it let's us "AllowEvents(REPLAY_POINTER)" so the app gets the click
            use x11rb::protocol::xproto::{ButtonIndex, ModMask, GrabMode};
            self.ctx.conn.grab_button(
                true,
                win,
                EventMask::BUTTON_PRESS,
                GrabMode::SYNC, 
                GrabMode::ASYNC,
                x11rb::NONE,
                x11rb::NONE,
                ButtonIndex::ANY,
                ModMask::ANY,
            )?;
        }
        
        self.ctx.conn.reparent_window(win, frame_win, frame_geom.client_x, frame_geom.client_y)?;
        
        // HACK: Force NorthWest gravity on the client window to avoid it moving 
        // relative to the frame when the frame resizes. Ported from xfwm4 client.c.
        let _ = self.ctx.conn.change_window_attributes(win, &x11rb::protocol::xproto::ChangeWindowAttributesAux::new().win_gravity(Some(x11rb::protocol::xproto::Gravity::NORTH_WEST)));
        
        // Initial stacking order: Desktops stay at the bottom, others go to top
        let mut aux = x11rb::protocol::xproto::ConfigureWindowAux::new();
        if is_desktop {
            aux = aux.stack_mode(x11rb::protocol::xproto::StackMode::BELOW);
        } else {
            aux = aux.stack_mode(x11rb::protocol::xproto::StackMode::ABOVE);
        }
        let _ = self.ctx.conn.configure_window(frame_win, &aux);
        debug!("Created frame window {} for client {} (stacking: {:?})", frame_win, win, if is_desktop { "BELOW" } else { "ABOVE" });
        
        if let Some(utw) = user_time_window {
            if utw != win {
                let _ = self.ctx.conn.change_window_attributes(utw, &x11rb::protocol::xproto::ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE));
            }
        }
        
        if workspace == self.current_workspace || workspace == 0xFFFFFFFF {


             self.ctx.conn.map_window(frame_win)?;
             self.ctx.conn.map_window(win)?;
             let _ = self.update_window_shape(win);
        }
        
        let mut client = Client::new(
            win,
            frame_geom.x,
            frame_geom.y,
            fix_w,
            fix_h
        );
        client.frame = Some(frame_win);
        client.is_csd = is_csd;
        client.name = name;
        client.workspace = workspace;
        client.window_type = window_types;
        client.transient_for = transient_for;
        client.group_leader = group_leader;
        client.client_leader = client_leader;
        client.user_time = user_time;
        client.user_time_window = user_time_window;
        client.is_modal = is_modal;
        client.is_fullscreen = is_fullscreen;
        client.is_maximized = is_maximized;
        client.is_sticky = is_sticky;
        client.demands_attention = demands_attention;
        client.skip_taskbar = skip_taskbar;
        client.skip_pager = skip_pager;
        client.is_shaded = is_shaded;
        client.is_above = is_above;
        client.is_below = is_below;
        client.startup_id = startup_id;

        client.frame_extents = frame_extents;
        client.gravity = gravity;
        client.layer = layer;

        client.is_desktop = is_desktop;
        client.is_dock = is_dock;
        client.is_fullscreen = is_fullscreen;
        client.accepts_input = accepts_input;
        client.pid = pid;
        client.is_urgent = is_urgent;
        client.sync_counter = sync_counter;
        client.is_shaped = is_shaped;
        client.opacity = self.read_opacity(win);

        // Select Shape events
        let _ = ShapeExt::shape_select_input(&self.ctx.conn, win, true);
        
        // Send initial ConfigureNotify to let client know its position
        self.send_configure_notify(win);

        // Set EWMH Frame Extents (Standard and GTK variants)
        let (border, title) = if client.is_desktop || client.is_dock || client.is_fullscreen { (0, 0) } else { (crate::window::frame::BORDER_WIDTH, crate::window::frame::TITLE_HEIGHT) };
        let extents = [
            border as u32, // left
            border as u32, // right
            (title + border) as u32, // top
            border as u32, // bottom
        ];
        let _ = self.ctx.conn.change_property32(PropMode::REPLACE, win, self.ctx.atoms._NET_FRAME_EXTENTS, AtomEnum::CARDINAL, &extents);
        let _ = self.ctx.conn.change_property32(PropMode::REPLACE, win, self.ctx.atoms._GTK_FRAME_EXTENTS, AtomEnum::CARDINAL, &extents);




        
        if self.compositor.active {
             let frame_geom = self.ctx.conn.get_geometry(frame_win)?.reply()?;
             debug!("Frame {} depth: {}", frame_win, frame_geom.depth);
             
             if let Ok(format) = Compositor::find_format(&self.ctx.conn, frame_geom.depth) {
                  // 1. Picture for the frame (decorations)
                  if let Ok(pict) = self.ctx.conn.generate_id() {
                      match self.ctx.conn.render_create_picture(
                          pict, 
                          frame_win, 
                          format, 
                          &CreatePictureAux::new()
                      ) {
                          Ok(_) => { 
                              debug!("Created Picture {} (depth {}) for frame {}", pict, frame_geom.depth, frame_win);
                              client.picture = Some(pict); 
                          },
                          Err(e) => self.error_tracker.record_compositor_error("create frame picture", e),
                      }
                  }

                  // 2. Picture for the client (content)
                  if let Ok(pict) = self.ctx.conn.generate_id() {
                      let win_geom = self.ctx.conn.get_geometry(win)?.reply()?;
                      debug!("Client {} depth: {}", win, win_geom.depth);
                      if let Ok(win_format) = Compositor::find_format(&self.ctx.conn, win_geom.depth) {
                          match self.ctx.conn.render_create_picture(
                              pict,
                              win,
                              win_format,
                              &CreatePictureAux::new()
                          ) {
                              Ok(_) => {
                                  debug!("Created Picture {} (depth {}) for client window {}", pict, win_geom.depth, win);
                                  client.content_picture = Some(pict);
                              },
                              Err(e) => self.error_tracker.record_compositor_error("create client picture", e),
                          }
                      }
                  }
             }

             if let Ok(dmg) = self.ctx.conn.generate_id() {
                 debug!("Creating damage {} for window {}", dmg, win);
                 match self.ctx.conn.damage_create(dmg, win, ReportLevel::NON_EMPTY) {
                     Ok(_) => client.damage = Some(dmg),
                     Err(e) => self.error_tracker.record_x11_error("create damage resource", e),
                 }
             }
        }
        
        if let Ok(strut) = self.read_strut_property(win) {
             client.strut = strut;
        }


        let (border, title) = if client.is_desktop || client.is_dock || client.is_fullscreen { (0, 0) } else { (crate::window::frame::BORDER_WIDTH, crate::window::frame::TITLE_HEIGHT) };
        let width = geom.width + (2 * border);
        let height = geom.height + title + (2 * border);
        debug!("Drawing decoration for frame {} (title: {})", frame_win, client.name);
        let _ = self.error_tracker.warn_if_failed(
            draw_decoration(&self.ctx, frame_win, &client.name, width, height, title),
            "draw initial decoration",
            crate::window::error::ErrorCategory::Window
        );
        
        self.clients.insert(win, client);
        self.mru_stack.retain(|&w| w != win);
        self.mru_stack.insert(0, win);
        
        // Create XSync Alarm if supported
        if let Err(e) = self.client_create_xsync_alarm(win) {
             warn!("Failed to create XSync alarm for window {}: {}", win, e);
        }
        
        // Focus the new window (ported from xfwm4 clientFrame)
        let _ = self.focus_window(win);
        
        Ok(())
    }

    pub fn unmanage_window(&mut self, win: Window) -> Result<()> {
        if self.clients.contains_key(&win) {
            debug!("Unmanaging window {}", win);
            if let Some(client) = self.clients.remove(&win) {
                if let Some(frame) = client.frame {
                    let _ = self.ctx.conn.destroy_window(frame);
                }
                
                if let Some(pict) = client.picture {
                    let _ = self.ctx.conn.render_free_picture(pict);
                }
                if let Some(pict) = client.content_picture {
                    let _ = self.ctx.conn.render_free_picture(pict);
                }
                
                if let Some(dmg) = client.damage {
                     let _ = self.ctx.conn.damage_destroy(dmg);
                }
                
                let (b, t) = if client.is_desktop || client.is_dock || client.is_fullscreen { (0, 0) } else { (crate::window::frame::BORDER_WIDTH, crate::window::frame::TITLE_HEIGHT) };
                let client_x = client.x + b as i16;
                let client_y = client.y + (t + b) as i16;
                let _ = self.ctx.conn.reparent_window(win, self.ctx.root_window, client_x, client_y);
            }
            self.mru_stack.retain(|&w| w != win);
            
            // Focus next window in MRU stack (ported from xfwm4 clientFocusTop)
            if let Some(&next) = self.mru_stack.first() {
                let _ = self.focus_window(next);
            }
        }
        Ok(())
    }

    pub fn find_client_by_frame(&self, frame: Window) -> Option<&Client> {
        self.clients.values().find(|c| c.frame == Some(frame))
    }

    pub fn update_current_desktop_prop(&self) -> Result<()> {
        self.ctx.conn.change_property32(
            PropMode::REPLACE,
            self.ctx.root_window,
            self.ctx.atoms._NET_CURRENT_DESKTOP,
            AtomEnum::CARDINAL,
            &[self.current_workspace],
        )?;
        Ok(())
    }

    pub fn send_delete_window(&self, window: Window) -> Result<()> {
        use x11rb::protocol::xproto::{ClientMessageEvent, ClientMessageData, EventMask};
        
        let event = ClientMessageEvent {
            response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
            format: 32,
            window,
            type_: self.ctx.atoms.WM_PROTOCOLS,
            data: ClientMessageData::from([
                 self.ctx.atoms.WM_DELETE_WINDOW,
                 x11rb::CURRENT_TIME,
                 0, 0, 0
            ]),
            sequence: 0,
        };
        
        self.ctx.conn.send_event(false, window, EventMask::NO_EVENT, event)?;
        Ok(())
    }

    pub fn get_cursor_for_part(&self, part: FramePart) -> x11rb::protocol::xproto::Cursor {
        match part {
             FramePart::CornerTopLeft => self.cursors.resize_nw,
             FramePart::CornerTopRight => self.cursors.resize_ne,
             FramePart::CornerBottomLeft => self.cursors.resize_sw,
             FramePart::CornerBottomRight => self.cursors.resize_se,
             FramePart::LeftBorder => self.cursors.resize_w,
             FramePart::RightBorder => self.cursors.resize_e,
             FramePart::TopBorder => self.cursors.resize_n,
             FramePart::BottomBorder => self.cursors.resize_s,
             FramePart::CloseButton => self.cursors.hand,
             FramePart::TitleBar => self.cursors.move_,
             _ => self.cursors.normal,
        }
    }

    pub fn apply_snap(&mut self, window: Window, zone: SnapZone) -> Result<()> {
        let (wa_x, wa_y, wa_w, wa_h) = self.calculate_workarea();
        use crate::window::frame::{BORDER_WIDTH, TITLE_HEIGHT};
        
        if zone == SnapZone::Top {
            return self.toggle_maximize(window);
        }

        let (new_x, new_y, f_w, f_h) = match zone {
            SnapZone::Left => (wa_x, wa_y, wa_w / 2, wa_h),
            SnapZone::Right => (wa_x + (wa_w / 2) as i16, wa_y, wa_w / 2, wa_h),
            _ => return Ok(()),
        };

        if let Some(client) = self.clients.get_mut(&window) {
            if let Some(frame) = client.frame {
                if !client.is_maximized && !client.is_fullscreen {
                    client.saved_geometry = Some((client.x, client.y, client.width, client.height));
                }

                let c_w = f_w.saturating_sub((2 * BORDER_WIDTH) as u16);
                let c_h = f_h.saturating_sub((TITLE_HEIGHT + 2 * BORDER_WIDTH) as u16);

                let _ = self.ctx.conn.configure_window(frame, &x11rb::protocol::xproto::ConfigureWindowAux::new().x(new_x as i32).y(new_y as i32).width(f_w as u32).height(f_h as u32));
                let _ = self.ctx.conn.configure_window(window, &x11rb::protocol::xproto::ConfigureWindowAux::new().width(c_w as u32).height(c_h as u32));
                
                client.x = new_x; client.y = new_y; client.width = c_w; client.height = c_h;
                client.is_maximized = false;
            }
        }
        self.update_net_wm_state(window)?;
        Ok(())
    }

    pub fn paint(&self) -> Result<()> {
        if !self.compositor.active { return Ok(()); }
        debug!("Compositor painting...");

        let mut layered_clients: Vec<(u16, usize, &Client)> = self.mru_stack.iter().enumerate().filter_map(|(idx, &win_id)| {
            self.clients.get(&win_id).map(|c| (c.layer, idx, c))
        }).collect();
        
        // Sort by layer (ascending), then by mru index (descending - Painter's Algorithm)
        layered_clients.sort_by(|a, b| {
            if a.0 != b.0 {
                a.0.cmp(&b.0)
            } else {
                b.1.cmp(&a.1)
            }
        });

        let sorted_clients = layered_clients.into_iter().filter_map(|(_, _, client)| {
            if (client.workspace == self.current_workspace || client.workspace == 4294967295) && !client.is_minimized {
                if let Some(content_pic) = client.content_picture {
                   // Docks and Desktops have no borders
                   let (b, t) = if client.is_desktop || client.is_dock || client.is_fullscreen { 
                       (0, 0) 
                   } else { 
                       (crate::window::frame::BORDER_WIDTH, crate::window::frame::TITLE_HEIGHT) 
                   };
                   
                   let w = client.width + (2 * b);
                   let h = client.height + t + (2 * b);
                   let has_shadow = !client.is_csd && !client.is_desktop && !client.is_dock;
                   return Some((client.picture, content_pic, client.x, client.y, w, h, b, t, client.width, client.height, has_shadow, client.opacity));
                }
            }
            None
        });

        let unmanaged_list = self.unmanaged_windows.values().map(|u| {
            (None, u.picture, u.x, u.y, u.width, u.height, 0, 0, u.width, u.height, false, 0xFFFFFFFF)
        });
        
        let all_items = sorted_clients.chain(unmanaged_list);

        self.compositor.paint(&self.ctx.conn, self.ctx.screen_width, self.ctx.screen_height, all_items)?;
        Ok(())
    }

    pub fn toggle_maximize(&mut self, window: Window) -> Result<()> {
        let (maximized, saved_geom, frame_win, client_width, client_height, start_x, start_y) = {
             if let Some(client) = self.clients.get(&window) {
                 if client.frame.is_none() { return Ok(()); }
                 (
                     client.is_maximized, 
                     client.saved_geometry, 
                     client.frame.unwrap(),
                     client.width,
                     client.height,
                     client.x,
                     client.y
                 )
             } else {
                 return Ok(());
             }
        };

        if maximized {
             if let Some((x, y, w, h)) = saved_geom {
                 use x11rb::protocol::xproto::ConfigureWindowAux;
                 let frame_w = w as u32 + (2 * BORDER_WIDTH) as u32;
                 let frame_h = h as u32 + TITLE_HEIGHT as u32 + (2 * BORDER_WIDTH) as u32;
                 
                 let values = ConfigureWindowAux::new().x(x as i32).y(y as i32).width(frame_w).height(frame_h);
                 self.ctx.conn.configure_window(frame_win, &values)?;
                 
                 let c_values = ConfigureWindowAux::new().width(w as u32).height(h as u32);
                 self.ctx.conn.configure_window(window, &c_values)?;
                 
                 if let Some(client) = self.clients.get_mut(&window) {
                     client.is_maximized = false;
                     client.x = x;
                     client.y = y;
                     client.width = w;
                     client.height = h;
                 }
                 self.update_net_wm_state(window)?;
             }
        } else {
             let (wa_x, wa_y, wa_w, wa_h) = self.calculate_workarea();
             let saved = (start_x, start_y, client_width, client_height);
             
             let new_client_w = (wa_w as u32).saturating_sub((2 * BORDER_WIDTH) as u32);
             let new_client_h = (wa_h as u32).saturating_sub((TITLE_HEIGHT + 2 * BORDER_WIDTH) as u32);
             
             use x11rb::protocol::xproto::ConfigureWindowAux;
             let values = ConfigureWindowAux::new().x(wa_x as i32).y(wa_y as i32).width(wa_w as u32).height(wa_h as u32);
             self.ctx.conn.configure_window(frame_win, &values)?;
             
             let c_values = ConfigureWindowAux::new().width(new_client_w).height(new_client_h);
             self.ctx.conn.configure_window(window, &c_values)?;
             
             if let Some(client) = self.clients.get_mut(&window) {
                 client.is_maximized = true;
                 client.saved_geometry = Some(saved);
                 client.x = wa_x;
                 client.y = wa_y;
                 client.width = new_client_w as u16;
                 client.height = new_client_h as u16;
             }
             self.update_net_wm_state(window)?;
        }
        Ok(())
    }

    pub fn toggle_minimize(&mut self, window: Window) -> Result<()> {
        let (minimized, frame_win) = {
            if let Some(client) = self.clients.get(&window) {
                if client.frame.is_none() { return Ok(()); }
                (client.is_minimized, client.frame.unwrap())
            } else {
                return Ok(());
            }
        };

        if minimized {
            // Restore: Map frame and client
            self.ctx.conn.map_window(frame_win)?;
            self.ctx.conn.map_window(window)?;
            
            if let Some(client) = self.clients.get_mut(&window) {
                client.is_minimized = false;
            }
            let _ = self.focus_window(window);
        } else {
            // Minimize: Unmap frame and client
            self.ctx.conn.unmap_window(frame_win)?;
            self.ctx.conn.unmap_window(window)?;
            
            if let Some(client) = self.clients.get_mut(&window) {
                client.is_minimized = true;
            }
        }
        
        self.update_net_wm_state(window)?;
        Ok(())
    }

    pub fn toggle_fullscreen(&mut self, window: Window) -> Result<()> {
        let (fullscreen, saved_geom, frame_win, client_width, client_height, start_x, start_y) = {
             if let Some(client) = self.clients.get(&window) {
                 if client.frame.is_none() { return Ok(()); }
                 (
                     client.is_fullscreen, 
                     client.saved_geometry, 
                     client.frame.unwrap(),
                     client.width,
                     client.height,
                     client.x,
                     client.y
                 )
             } else {
                 return Ok(());
             }
        };

        if fullscreen {
             if let Some((x, y, w, h)) = saved_geom {
                 use x11rb::protocol::xproto::ConfigureWindowAux;
                 let frame_w = w as u32 + (2 * BORDER_WIDTH) as u32;
                 let frame_h = h as u32 + TITLE_HEIGHT as u32 + (2 * BORDER_WIDTH) as u32;
                 
                 let values = ConfigureWindowAux::new().x(x as i32).y(y as i32).width(frame_w).height(frame_h);
                 self.ctx.conn.configure_window(frame_win, &values)?;
                 
                 let c_values = ConfigureWindowAux::new().width(w as u32).height(h as u32);
                 self.ctx.conn.configure_window(window, &c_values)?;
                 
                 if let Some(client) = self.clients.get_mut(&window) {
                     client.is_fullscreen = false;
                     client.x = x;
                     client.y = y;
                     client.width = w;
                     client.height = h;
                 }
                 self.update_net_wm_state(window)?;
             }
        } else {
             let screen = &self.ctx.conn.setup().roots[self.ctx.screen_num];
             let screen_w = screen.width_in_pixels;
             let screen_h = screen.height_in_pixels;
             let saved = (start_x, start_y, client_width, client_height);
             
             use x11rb::protocol::xproto::ConfigureWindowAux;
             let values = ConfigureWindowAux::new().x(0).y(0).width(screen_w as u32).height(screen_h as u32);
             self.ctx.conn.configure_window(frame_win, &values)?;
             
             let c_values = ConfigureWindowAux::new().width(screen_w as u32).height(screen_h as u32);
             self.ctx.conn.configure_window(window, &c_values)?;
             
             if let Some(client) = self.clients.get_mut(&window) {
                 client.is_fullscreen = true;
                 client.saved_geometry = Some(saved);
                 client.x = 0;
                 client.y = 0;
                 client.width = screen_w;
                 client.height = screen_h;
             }
             self.update_net_wm_state(window)?;
        }
        Ok(())
    }

    fn update_net_wm_state(&self, window: Window) -> Result<()> {
        let client = if let Some(c) = self.clients.get(&window) { c } else { return Ok(()); };
        let mut states = Vec::new();
        if client.is_maximized {
            states.push(self.ctx.atoms._NET_WM_STATE_MAXIMIZED_VERT);
            states.push(self.ctx.atoms._NET_WM_STATE_MAXIMIZED_HORZ);
        }
        if client.is_fullscreen {
            states.push(self.ctx.atoms._NET_WM_STATE_FULLSCREEN);
        }
        if client.is_minimized {
            states.push(self.ctx.atoms._NET_WM_STATE_HIDDEN);
        }
        if Some(window) == self.focused_window {
            states.push(self.ctx.atoms._NET_WM_STATE_FOCUSED);
        }
        if client.demands_attention {
            states.push(self.ctx.atoms._NET_WM_STATE_DEMANDS_ATTENTION);
        }
        if client.skip_taskbar {
            states.push(self.ctx.atoms._NET_WM_STATE_SKIP_TASKBAR);
        }
        if client.skip_pager {
            states.push(self.ctx.atoms._NET_WM_STATE_SKIP_PAGER);
        }
        if client.is_shaded {
            states.push(self.ctx.atoms._NET_WM_STATE_SHADED);
        }
        if client.is_above {
            states.push(self.ctx.atoms._NET_WM_STATE_ABOVE);
        }
        if client.is_below {
            states.push(self.ctx.atoms._NET_WM_STATE_BELOW);
        }
        
        self.ctx.conn.change_property32(
            PropMode::REPLACE,
            window,
            self.ctx.atoms._NET_WM_STATE,
            AtomEnum::ATOM,
            &states
        )?;
        Ok(())
    }

    fn read_strut_property(&self, window: Window) -> Result<Option<Vec<u32>>> {
        let partial_atom = self.ctx.atoms._NET_WM_STRUT_PARTIAL;
        let strut_atom = self.ctx.atoms._NET_WM_STRUT;
        
        if let Ok(reply) = self.ctx.conn.get_property(false, window, partial_atom, AtomEnum::CARDINAL, 0, 12)?.reply() {
             if reply.type_ == u32::from(AtomEnum::CARDINAL) && reply.value_len == 12 {
                 return Ok(Some(reply.value32().map(|i| i.collect()).unwrap_or_default()));
             }
        }

        if let Ok(reply) = self.ctx.conn.get_property(false, window, strut_atom, AtomEnum::CARDINAL, 0, 4)?.reply() {
            if reply.type_ == u32::from(AtomEnum::CARDINAL) && reply.value_len == 4 {
                 return Ok(Some(reply.value32().map(|i| i.collect()).unwrap_or_default()));
            }
        }
        Ok(None)
    }
    
    fn calculate_workarea(&self) -> (i16, i16, u16, u16) {
        let screen = &self.ctx.conn.setup().roots[self.ctx.screen_num];
        let screen_w = screen.width_in_pixels as i32;
        let screen_h = screen.height_in_pixels as i32;
        
        let mut left_margin = 0;
        let mut right_margin = 0;
        let mut top_margin = 0;
        let mut bottom_margin = 0;
        
        for client in self.clients.values() {
            if let Some(strut) = &client.strut {
                 if strut.len() >= 4 {
                     left_margin = left_margin.max(strut[0] as i32);
                     right_margin = right_margin.max(strut[1] as i32);
                     top_margin = top_margin.max(strut[2] as i32);
                     bottom_margin = bottom_margin.max(strut[3] as i32);
                 }
            }
        }
        (left_margin as i16, top_margin as i16, (screen_w - left_margin - right_margin).max(1) as u16, (screen_h - top_margin - bottom_margin).max(1) as u16)
    }

    fn update_net_workarea(&self) -> Result<()> {
        let (x, y, w, h) = self.calculate_workarea();
        let single_wa = [x as u32, y as u32, w as u32, h as u32];
        let mut workarea = Vec::with_capacity(16);
        for _ in 0..4 {
            workarea.extend_from_slice(&single_wa);
        }
        self.ctx.conn.change_property32(PropMode::REPLACE, self.ctx.root_window, self.ctx.atoms._NET_WORKAREA, AtomEnum::CARDINAL, &workarea)?;
        Ok(())
    }

    pub fn switch_workspace(&mut self, workspace: u32) -> Result<()> {
        if workspace == self.current_workspace { return Ok(()); }
        self.current_workspace = workspace;
        for client in self.clients.values() {
            if client.workspace == 0xFFFFFFFF { continue; }
            if let Some(frame) = client.frame {
                if client.workspace == workspace {
                    self.ctx.conn.map_window(frame)?;
                    self.ctx.conn.map_window(client.window)?;
                } else {
                    self.ctx.conn.unmap_window(frame)?;
                }
            }
        }
        self.update_current_desktop_prop()?;
        if let Some(&top_win) = self.mru_stack.iter().find(|&&w| {
             if let Some(c) = self.clients.get(&w) {
                 return c.workspace == workspace || c.workspace == 0xFFFFFFFF;
             }
             false
        }) {
             let _ = self.focus_window(top_win);
        }
        Ok(())
    }

    fn is_protocol_supported(&self, window: Window, protocol: x11rb::protocol::xproto::Atom) -> bool {
        let protocols_atom = self.ctx.atoms.WM_PROTOCOLS;
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, protocols_atom, AtomEnum::ATOM, 0, 100) {
            if let Ok(reply) = cookie.reply() {
                 if reply.format == 32 {
                     if let Some(mut vals) = reply.value32() {
                         return vals.any(|a| a == protocol);
                     }
                 }
            }
        }
        false
    }

        pub fn focus_window(&mut self, window: Window) -> Result<()> {
        use x11rb::protocol::xproto::{InputFocus, ClientMessageEvent, ClientMessageData, EventMask};
        
        info!("🎯 FOCUS: Attempting to focus window {}", window);
        
        let mut target_window = window;
        if let Some(client) = self.clients.get(&window) {
            // Check for modals (secret sauce part 1)
            if let Some(modal) = self.get_modal_for(client) {
                info!("🎯 FOCUS: Window {} has modal {}, focusing modal instead", window, modal);
                target_window = modal;
            }
        }

    let mut update_new_state = false;
    let (accepts_input, layer, user_time, is_modal, name) = {
        if let Some(client) = self.clients.get_mut(&target_window) {
            if client.demands_attention {
                client.demands_attention = false;
                update_new_state = true;
            }
            (client.accepts_input, client.layer, client.user_time, client.is_modal, client.name.clone())
        } else {
            return Ok(());
        }
    };

    if update_new_state {
        let _ = self.update_net_wm_state(target_window);
    }

    // Focus Stealing Prevention
    if let Some(&current_focus) = self.mru_stack.first() {
        if current_focus != target_window {
            if let Some(focused_client) = self.clients.get(&current_focus) {
                let mut prevent = false;
                if focused_client.layer > layer {
                    prevent = true;
                }
                if user_time == 0 || Self::timestamp_is_before(user_time, focused_client.user_time) {
                    prevent = true;
                }
                if self.drag_state != DragState::None {
                    prevent = true;
                }
                if prevent && !is_modal {
                     info!("🎯 FOCUS: Prevention active for window {}", target_window);
                     return Ok(());
                }
            }
        }
    }

    info!("🎯 FOCUS: Focusing window {}, name='{}'", target_window, name);
    
    let supports_take_focus = self.is_protocol_supported(target_window, self.ctx.atoms.WM_TAKE_FOCUS);
    if supports_take_focus {
         let event = ClientMessageEvent {
            response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
            format: 32,
            window: target_window,
            type_: self.ctx.atoms.WM_PROTOCOLS,
            data: ClientMessageData::from([self.ctx.atoms.WM_TAKE_FOCUS, x11rb::CURRENT_TIME, 0, 0, 0]),
            sequence: 0,
        };
        let _ = self.ctx.conn.send_event(false, target_window, EventMask::NO_EVENT, event);
    }

    if accepts_input {
        match self.ctx.conn.set_input_focus(InputFocus::POINTER_ROOT, target_window, x11rb::CURRENT_TIME) {
            Ok(_) => {
                let old_focus = self.focused_window;
                self.focused_window = Some(target_window);
                let _ = self.ctx.conn.change_property32(PropMode::REPLACE, self.ctx.root_window, self.ctx.atoms._NET_ACTIVE_WINDOW, AtomEnum::WINDOW, &[target_window]);
                if let Some(old) = old_focus {
                    let _ = self.update_net_wm_state(old);
                }
                let _ = self.update_net_wm_state(target_window);
            },
            Err(e) => error!("❌ FOCUS: Failed for window {}: {}", target_window, e),
        }
    }
        
        self.mru_stack.retain(|&w| w != target_window);
        self.mru_stack.insert(0, target_window);
        Ok(())
    }

    fn read_motif_hints(&self, window: Window) -> (bool, bool) {
        let motif_atom = self.ctx.atoms._MOTIF_WM_HINTS;
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, motif_atom, AtomEnum::ANY, 0, 5) {
            if let Ok(reply) = cookie.reply() {
                 if reply.format == 32 && reply.value_len >= 3 {
                     if let Some(mut vals) = reply.value32() {
                         let flags = vals.next().unwrap_or(0);
                         let _functions = vals.next().unwrap_or(0);
                         let decorations = vals.next().unwrap_or(1);
                         const MWM_HINT_DECORATIONS: u32 = 1 << 1;
                         if flags & MWM_HINT_DECORATIONS != 0 {
                              return (decorations != 0, decorations != 0);
                         }
                     }
                 }
            }
        }
        (true, true)
    }

    fn read_wm_hints(&self, window: Window) -> (Option<Window>, bool, bool) {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms.WM_HINTS, AtomEnum::ANY, 0, 9) {
            if let Ok(reply) = cookie.reply() {
                if reply.format == 32 && reply.value_len >= 1 {
                    if let Some(mut vals) = reply.value32() {
                        let flags = vals.next().unwrap_or(0);
                        let input = vals.next().unwrap_or(1);
                        let _initial_state = vals.next().unwrap_or(1);
                        let _icon_pixmap = vals.next().unwrap_or(0);
                        let _icon_window = vals.next().unwrap_or(0);
                        let _icon_x = vals.next().unwrap_or(0);
                        let _icon_y = vals.next().unwrap_or(0);
                        let _icon_mask = vals.next().unwrap_or(0);
                        let window_group = vals.next().unwrap_or(0);

                        let group_leader = if (flags & (1 << 6)) != 0 { Some(window_group) } else { None };
                        let accepts_input = if (flags & (1 << 0)) != 0 { input != 0 } else { true };
                        let is_urgent = (flags & (1 << 8)) != 0;
                        return (group_leader, accepts_input, is_urgent);
                    }
                }
            }
        }
        (None, true, false)
    }


    fn read_user_time(&self, window: Window) -> u32 {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._NET_WM_USER_TIME, AtomEnum::CARDINAL, 0, 1) {
             if let Ok(reply) = cookie.reply() {
                 if let Some(val) = reply.value32().and_then(|mut i| i.next()) {
                     return val;
                 }
             }
        }
        0
    }

    fn read_opacity(&self, window: Window) -> u32 {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._NET_WM_WINDOW_OPACITY, AtomEnum::CARDINAL, 0, 1) {
             if let Ok(reply) = cookie.reply() {
                 if let Some(val) = reply.value32().and_then(|mut i| i.next()) {
                     return val;
                 }
             }
        }
        0xFFFFFFFF
    }

    fn read_pid(&self, window: Window) -> u32 {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._NET_WM_PID, AtomEnum::CARDINAL, 0, 1) {
             if let Ok(reply) = cookie.reply() {
                 if let Some(val) = reply.value32().and_then(|mut i| i.next()) {
                     return val;
                 }
             }
        }
        0
    }

    fn read_frame_extents(&self, window: Window) -> (u32, u32, u32, u32) {

        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._GTK_FRAME_EXTENTS, AtomEnum::CARDINAL, 0, 4) {
            if let Ok(reply) = cookie.reply() {
                if let Some(mut vals) = reply.value32() {
                    let left = vals.next().unwrap_or(0);
                    let right = vals.next().unwrap_or(0);
                    let top = vals.next().unwrap_or(0);
                    let bottom = vals.next().unwrap_or(0);
                    return (left, right, top, bottom);
                }
            }
        }
        (0, 0, 0, 0)
    }

    fn read_sync_counter(&self, window: Window) -> Option<u32> {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._NET_WM_SYNC_REQUEST_COUNTER, AtomEnum::CARDINAL, 0, 1) {
            if let Ok(reply) = cookie.reply() {
                if reply.format == 32 && reply.value_len >= 1 {
                    if let Some(mut vals) = reply.value32() {
                        return vals.next();
                    }
                }
            }
        }
        None
    }

    fn read_is_shaped(&self, window: Window) -> bool {
        if let Ok(reply) = ShapeExt::shape_query_extents(&self.ctx.conn, window) {
            if let Ok(reply) = reply.reply() {
                return reply.bounding_shaped;
            }
        }
        false
    }

    fn is_modal(&self, window: Window) -> bool {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._NET_WM_STATE, AtomEnum::ATOM, 0, 1024) {
            if let Ok(reply) = cookie.reply() {
                if let Some(vals) = reply.value32() {
                    for atom in vals {
                        if atom == self.ctx.atoms._NET_WM_STATE_MODAL {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn same_group(&self, c1: &Client, c2: &Client) -> bool {
        if c1.window == c2.window { return true; }
        if let Some(g1) = c1.group_leader {
            if let Some(g2) = c2.group_leader {
                if g1 == g2 { return true; }
            }
            if g1 == c2.window { return true; }
        }
        if let Some(g2) = c2.group_leader {
            if g2 == c1.window { return true; }
        }
        false
    }

    fn is_transient_for(&self, c1: &Client, c2: &Client) -> bool {
        if let Some(transient_for) = c1.transient_for {
            if transient_for != self.ctx.root_window {
                return transient_for == c2.window;
            } else if c2.transient_for.is_none() {
                // Transients for group ONLY apply to top-level windows (not other transients)
                // This ported logic from xfwm4/src/transients.c
                return self.same_group(c1, c2);
            }
        }
        false
    }

    fn is_modal_for(&self, c1: &Client, c2: &Client) -> bool {
        if c1.is_modal {
            return self.is_transient_for(c1, c2);
        }
        false
    }

    fn get_modal_for(&self, client: &Client) -> Option<Window> {
        // Search mru stack for a modal window that is transient for this client or its group
        for &win in self.mru_stack.iter() {
            if let Some(other) = self.clients.get(&win) {
                if self.is_modal_for(other, client) {
                    return Some(win);
                }
            }
        }
        None
    }

    fn timestamp_is_before(time1: u32, time2: u32) -> bool {
        if time1 == 0 { return true; }
        if time2 == 0 { return false; }
        
        // Wrapping sub for 32-bit timestamps
        let diff = time2.wrapping_sub(time1);
        diff < (u32::MAX >> 1)
    }

    fn read_user_time_window(&self, window: Window) -> Option<Window> {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._NET_WM_USER_TIME_WINDOW, AtomEnum::WINDOW, 0, 1) {
            if let Ok(reply) = cookie.reply() {
                if let Some(val) = reply.value32().and_then(|mut i| i.next()) {
                    return Some(val);
                }
            }
        }
        None
    }

    fn read_client_leader(&self, window: Window) -> Option<Window> {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms.WM_CLIENT_LEADER, AtomEnum::WINDOW, 0, 1) {
            if let Ok(reply) = cookie.reply() {
                if let Some(val) = reply.value32().and_then(|mut i| i.next()) {
                    return Some(val);
                }
            }
        }
        None
    }

    fn read_size_hints(&self, window: Window) -> (i32, i16, i16, u16, u16) {
        // Returns (gravity, min_w, min_h, max_w, max_h)
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, AtomEnum::WM_NORMAL_HINTS, AtomEnum::ANY, 0, 18) {
            if let Ok(reply) = cookie.reply() {
                if reply.format == 32 && reply.value_len >= 15 {
                    if let Some(vals) = reply.value32() {
                        let data: Vec<u32> = vals.collect();
                        let flags = data[0];
                        let min_w = if flags & (1 << 4) != 0 { data[5] as i16 } else { 0 };
                        let min_h = if flags & (1 << 4) != 0 { data[6] as i16 } else { 0 };
                        let max_w = if flags & (1 << 5) != 0 { data[7] as u16 } else { 0 };
                        let max_h = if flags & (1 << 5) != 0 { data[8] as u16 } else { 0 };
                        let gravity = if flags & (1 << 8) != 0 && data.len() >= 18 { data[17] as i32 } else { 1 };
                        
                        return (gravity, min_w, min_h, max_w, max_h);
                    }
                }
            }
        }
        (1, 0, 0, 0, 0)
    }

    fn gravitate(gravity: i32, mode: i32, border: u16, title: u16, x: &mut i16, y: &mut i16) {
        let fl = border as i16;
        let fr = border as i16;
        let ft = (title + border) as i16;
        let fb = border as i16;

        let (dx, dy) = match gravity {
            5 => ((fl - fr + 1) / 2, (ft - fb + 1) / 2), // Center
            2 => ((fl - fr + 1) / 2, ft),               // North
            8 => ((fl - fr + 1) / 2, -fb),              // South
            6 => (-fr, (ft - fb + 1) / 2),              // East
            4 => (fl, (ft - fb + 1) / 2),               // West
            1 => (fl, ft),                              // NorthWest
            3 => (-fr, ft),                             // NorthEast
            7 => (fl, -fb),                             // SouthWest
            9 => (-fr, -fb),                            // SouthEast
            _ => (0, 0),                                // Static or others
        };

        *x += dx * mode as i16;
        *y += dy * mode as i16;
    }

    fn send_configure_notify(&self, window: Window) {
        if let Some(client) = self.clients.get(&window) {
            let (b, t) = if client.is_desktop || client.is_dock || client.is_fullscreen || client.is_csd { (0, 0) } else { (crate::window::frame::BORDER_WIDTH, crate::window::frame::TITLE_HEIGHT) };
            
            let event = x11rb::protocol::xproto::ConfigureNotifyEvent {
                response_type: x11rb::protocol::xproto::CONFIGURE_NOTIFY_EVENT,
                sequence: 0,
                event: window,
                window,
                above_sibling: x11rb::NONE,
                x: client.x + b as i16,
                y: client.y + (t + b) as i16,
                width: client.width,
                height: client.height,
                border_width: 0,
                override_redirect: false,
            };
            let _ = self.ctx.conn.send_event(false, window, EventMask::STRUCTURE_NOTIFY, event);
        }
    }

    fn find_client_by_user_time_window(&self, window: Window) -> Option<Window> {

        self.clients.iter().find(|(_, c)| c.user_time_window == Some(window)).map(|(&w, _)| w)
    }

    fn has_csd_hint(&self, window: Window) -> bool {


        if let Ok(cookie) = self.ctx.conn.get_property(
            false,
            window,
            self.ctx.atoms._GTK_FRAME_EXTENTS,
            AtomEnum::CARDINAL,
            0,
            4
        ) {
            if let Ok(reply) = cookie.reply() {
                return !reply.value.is_empty();
            }
        }
        false
    }

    #[allow(dropping_copy_types)]


    pub fn handle_event(&mut self, event: Event) -> Result<bool> {
        debug!("Received event: {:?}", event);
        let mut needs_paint = false;
        match event {
            Event::MapRequest(event) => {
                let attrs = self.ctx.conn.get_window_attributes(event.window)?.reply()?;
                if !attrs.override_redirect && !self.clients.contains_key(&event.window) {
                    drop(attrs);
                    if let Err(_) = self.manage_window(event.window) { } else { needs_paint = true; }
                } else if attrs.override_redirect {
                    let _ = self.ctx.conn.map_window(event.window);
                }
            }
            Event::ConfigureRequest(event) => {
                let sibling_resolved = if event.value_mask.contains(ConfigWindow::SIBLING) {
                    self.find_client_by_frame(event.sibling).map(|c| c.window).unwrap_or(event.sibling)
                } else {
                    event.sibling
                };

                if let Some(client) = self.clients.get_mut(&event.window) {
                    let mut mask = event.value_mask;
                    
                    if client.is_fullscreen || client.is_maximized {
                         mask = ConfigWindow::from(u16::from(mask) & !(u16::from(ConfigWindow::X) | u16::from(ConfigWindow::Y) | u16::from(ConfigWindow::WIDTH) | u16::from(ConfigWindow::HEIGHT)));
                    }

                    let (b, t) = if client.is_fullscreen || client.is_desktop || client.is_dock || client.is_csd { (0, 0) } else { (BORDER_WIDTH, TITLE_HEIGHT) };
                    
                    let mut req_x = if mask.contains(ConfigWindow::X) { event.x } else { client.x + b as i16 };
                    let mut req_y = if mask.contains(ConfigWindow::Y) { event.y } else { client.y + (t + b) as i16 };
                    let req_w = if mask.contains(ConfigWindow::WIDTH) { event.width } else { client.width };
                    let req_h = if mask.contains(ConfigWindow::HEIGHT) { event.height } else { client.height };

                    // 1. Gravitation of requested coordinates
                    if mask.intersects(ConfigWindow::X | ConfigWindow::Y) {
                         let mut tx = req_x;
                         let mut ty = req_y;
                         Self::gravitate(client.gravity, 1, b, t, &mut tx, &mut ty);
                         if mask.contains(ConfigWindow::X) { req_x = tx; }
                         if mask.contains(ConfigWindow::Y) { req_y = ty; }
                    }

                    // 2. Gravitation due to size change (dw, dh)
                    let mut dw = 0i16;
                    let mut dh = 0i16;
                    match client.gravity {
                        5 => { // Center
                            dw = ((client.width as i32 - req_w as i32) / 2) as i16;
                            dh = ((client.height as i32 - req_h as i32) / 2) as i16;
                        },
                        2 => { // North
                            dw = ((client.width as i32 - req_w as i32) / 2) as i16;
                        },
                        8 => { // South
                            dw = ((client.width as i32 - req_w as i32) / 2) as i16;
                            dh = (client.height as i32 - req_h as i32) as i16;
                        },
                        6 => { // East
                            dw = (client.width as i32 - req_w as i32) as i16;
                            dh = ((client.height as i32 - req_h as i32) / 2) as i16;
                        },
                        4 => { // West
                            dh = ((client.height as i32 - req_h as i32) / 2) as i16;
                        },
                        3 => { // NorthEast
                            dw = (client.width as i32 - req_w as i32) as i16;
                        },
                        7 => { // SouthWest
                            dh = (client.height as i32 - req_h as i32) as i16;
                        },
                        9 => { // SouthEast
                            dw = (client.width as i32 - req_w as i32) as i16;
                            dh = (client.height as i32 - req_h as i32) as i16;
                        },
                        _ => {}
                    }

                    if !mask.contains(ConfigWindow::X) && mask.contains(ConfigWindow::WIDTH) && dw != 0 {
                        req_x = (client.x + b as i16) + dw;
                        mask |= ConfigWindow::X;
                    }
                    if !mask.contains(ConfigWindow::Y) && mask.contains(ConfigWindow::HEIGHT) && dh != 0 {
                        req_y = (client.y + (t + b) as i16) + dh;
                        mask |= ConfigWindow::Y;
                    }

                    let frame_x = req_x - b as i16;
                    let frame_y = req_y - (t + b) as i16;

                    if mask.contains(ConfigWindow::X) && frame_x == client.x { mask.remove(ConfigWindow::X); }
                    if mask.contains(ConfigWindow::Y) && frame_y == client.y { mask.remove(ConfigWindow::Y); }
                    if mask.contains(ConfigWindow::WIDTH) && req_w == client.width { mask.remove(ConfigWindow::WIDTH); }
                    if mask.contains(ConfigWindow::HEIGHT) && req_h == client.height { mask.remove(ConfigWindow::HEIGHT); }
                    if client.is_desktop { mask.remove(ConfigWindow::SIBLING | ConfigWindow::STACK_MODE); }

                    if mask.intersects(ConfigWindow::X | ConfigWindow::Y | ConfigWindow::WIDTH | ConfigWindow::HEIGHT | ConfigWindow::SIBLING | ConfigWindow::STACK_MODE) {
                        if let Some(frame) = client.frame {
                            let (b, t) = if client.is_fullscreen || client.is_desktop || client.is_dock || client.is_csd { (0, 0) } else { (BORDER_WIDTH, TITLE_HEIGHT) };
                            
                            let mut aux = x11rb::protocol::xproto::ConfigureWindowAux::new();
                            if mask.contains(ConfigWindow::X) { aux = aux.x(req_x as i32); client.x = req_x; }
                            if mask.contains(ConfigWindow::Y) { aux = aux.y(req_y as i32); client.y = req_y; }
                            
                            let mut resized = false;
                            if mask.contains(ConfigWindow::WIDTH) { 
                                let fw = req_w + (2 * b);
                                aux = aux.width(fw as u32); 
                                client.width = req_w; 
                                resized = true;
                            }
                            if mask.contains(ConfigWindow::HEIGHT) { 
                                let fh = req_h + t + (2 * b);
                                aux = aux.height(fh as u32); 
                                client.height = req_h; 
                                resized = true;
                            }
                            if mask.contains(ConfigWindow::SIBLING) { aux = aux.sibling(sibling_resolved); }
                            if mask.contains(ConfigWindow::STACK_MODE) { aux = aux.stack_mode(event.stack_mode); }
                            
                            let _ = self.ctx.conn.configure_window(frame, &aux);
                            
                            if resized {
                                let _ = self.ctx.conn.configure_window(event.window, &x11rb::protocol::xproto::ConfigureWindowAux::new().width(client.width as u32).height(client.height as u32));
                                if let Err(_) = draw_decoration(&self.ctx, event.window, &client.name, client.width + 2*b, client.height + t + 2*b, t) { }
                                let _ = self.update_window_shape(event.window);
                            }
                        }
                        needs_paint = true;
                        self.send_configure_notify(event.window);
                    } else {
                        self.send_configure_notify(event.window);
                    }
                } else {
                    // Unmanaged window
                    let mut aux = ConfigureWindowAux::new();
                    if event.value_mask.contains(ConfigWindow::X) { aux = aux.x(Some(event.x as i32)); }
                    if event.value_mask.contains(ConfigWindow::Y) { aux = aux.y(Some(event.y as i32)); }
                    if event.value_mask.contains(ConfigWindow::WIDTH) { aux = aux.width(Some(event.width as u32)); }
                    if event.value_mask.contains(ConfigWindow::HEIGHT) { aux = aux.height(Some(event.height as u32)); }
                    if event.value_mask.contains(ConfigWindow::STACK_MODE) { aux = aux.stack_mode(Some(event.stack_mode)); }
                    let _ = self.ctx.conn.configure_window(event.window, &aux);
                }
            }

            Event::MapNotify(event) => {
                if event.window != self.compositor.overlay_window 
                    && !self.clients.contains_key(&event.window) 
                    && !self.unmanaged_windows.contains_key(&event.window)
                    && self.find_client_by_frame(event.window).is_none()
                {
                    // Potentially an override_redirect window (menu/tooltip)
                    if let Ok(attrs) = self.ctx.conn.get_window_attributes(event.window) {
                        if let Ok(reply) = attrs.reply() {
                            if reply.map_state != MapState::UNMAPPED {
                                 if let Ok(geom) = self.ctx.conn.get_geometry(event.window)?.reply() {
                                     if let Ok(format) = Compositor::find_format(&self.ctx.conn, geom.depth) {
                                         if let Ok(pict) = self.ctx.conn.generate_id() {
                                             if let Ok(_) = self.ctx.conn.render_create_picture(pict, event.window, format, &CreatePictureAux::new().subwindowmode(SubwindowMode::INCLUDE_INFERIORS)) {
                                                 let mut damage = None;
                                                 if let Ok(dmg) = self.ctx.conn.generate_id() {
                                                     if let Ok(_) = self.ctx.conn.damage_create(dmg, event.window, ReportLevel::NON_EMPTY) {
                                                         damage = Some(dmg);
                                                     }
                                                 }
                                                 info!("🔍 Tracking unmanaged window {} (x={}, y={}, w={}, h={})", event.window, geom.x, geom.y, geom.width, geom.height);
                                                 self.unmanaged_windows.insert(event.window, UnmanagedWindow {
                                                     picture: pict,
                                                     damage,
                                                     x: geom.x,
                                                     y: geom.y,
                                                     width: geom.width,
                                                     height: geom.height,
                                                 });
                                                 needs_paint = true;
                                             }
                                         }
                                     }
                                 }
                            }
                        }
                    }
                }
            }
            Event::UnmapNotify(event) => { 
                let _ = self.unmanage_window(event.window); 
                if let Some(unmanaged) = self.unmanaged_windows.remove(&event.window) {
                    info!("🔍 Stopped tracking unmanaged window {}", event.window);
                    let _ = self.ctx.conn.render_free_picture(unmanaged.picture);
                    if let Some(dmg) = unmanaged.damage { let _ = self.ctx.conn.damage_destroy(dmg); }
                }
                needs_paint = true; 
            }
            Event::DestroyNotify(event) => { 
                let _ = self.unmanage_window(event.window); 
                if let Some(unmanaged) = self.unmanaged_windows.remove(&event.window) {
                    info!("🔍 Stopped tracking unmanaged window (destroyed) {}", event.window);
                    let _ = self.ctx.conn.render_free_picture(unmanaged.picture);
                    if let Some(dmg) = unmanaged.damage { let _ = self.ctx.conn.damage_destroy(dmg); }
                }
                needs_paint = true; 
            }
            Event::ConfigureNotify(event) => {
                if let Some(unmanaged) = self.unmanaged_windows.get_mut(&event.window) {
                    unmanaged.x = event.x;
                    unmanaged.y = event.y;
                    unmanaged.width = event.width;
                    unmanaged.height = event.height;
                    needs_paint = true;
                }
            }
            Event::DamageNotify(event) => { 
                if self.clients.contains_key(&event.drawable) { needs_paint = true; }
                if self.unmanaged_windows.contains_key(&event.drawable) { needs_paint = true; }
                let _ = self.ctx.conn.damage_subtract(event.damage, x11rb::NONE, x11rb::NONE); 
            }
            Event::ShapeNotify(event) => {
                let win = event.affected_window;
                let is_shaped = event.shaped;
                if let Some(client) = self.clients.get_mut(&win) {
                    client.is_shaped = is_shaped;
                    debug!("Shape updated for window {} (shaped: {})", win, is_shaped);
                    let _ = self.update_window_shape(win);
                    needs_paint = true;
                }
            }
            Event::SyncAlarmNotify(event) => {
                if let Some(client) = self.clients.values_mut().find(|c| c.sync_alarm == Some(event.alarm)) {
                    client.sync_waiting = false;
                    debug!("XSync Alarm for window {} - waiting finished", client.window);
                }
            }
            Event::PropertyNotify(event) => {
                 let mut target_win = event.window;
                 if !self.clients.contains_key(&target_win) {
                     if let Some(w) = self.find_client_by_user_time_window(event.window) {
                         target_win = w;
                     } else {
                         return Ok(false);
                     }
                 }

                 if event.atom == self.ctx.atoms._NET_WM_WINDOW_OPACITY {
                      let opacity = self.read_opacity(target_win);
                      if let Some(client) = self.clients.get_mut(&target_win) {
                          client.opacity = opacity;
                          needs_paint = true;
                      }
                 } else if event.atom == self.ctx.atoms._NET_WM_STRUT || event.atom == self.ctx.atoms._NET_WM_STRUT_PARTIAL {
                      if let Ok(strut) = self.read_strut_property(target_win) {
                          if let Some(client) = self.clients.get_mut(&target_win) {
                               client.strut = strut;
                               let _ = self.update_net_workarea();
                          }
                      }
                 } else if event.atom == self.ctx.atoms._NET_WM_NAME {
                      if let Some(client) = self.clients.get_mut(&target_win) {
                           let name_reply = self.ctx.conn.get_property(false, target_win, self.ctx.atoms._NET_WM_NAME, self.ctx.atoms.UTF8_STRING, 0, 1024)?.reply();
                           if let Ok(prop) = name_reply {
                               if let Ok(name) = String::from_utf8(prop.value) { client.name = name;
                                   if let Some(frame) = client.frame {
                                       let _ = self.ctx.conn.send_event(false, frame, EventMask::EXPOSURE, x11rb::protocol::xproto::ExposeEvent { response_type: x11rb::protocol::xproto::EXPOSE_EVENT, sequence: 0, window: frame, x: 0, y: 0, width: 0, height: 0, count: 0 });
                                   }
                               }
                           }
                      }
                 } else if event.atom == self.ctx.atoms._GTK_FRAME_EXTENTS {
                      let frame_extents = self.read_frame_extents(target_win);
                      let is_csd = self.has_csd_hint(target_win);
                      if let Some(client) = self.clients.get_mut(&target_win) {
                          if client.is_csd != is_csd || client.frame_extents != frame_extents {
                              client.is_csd = is_csd;
                              client.frame_extents = frame_extents;
                              debug!("CSD/Extents changed for window {} (csd: {}, extents: {:?})", target_win, is_csd, frame_extents);
                              needs_paint = true;
                          }
                      }
                 } else if event.atom == self.ctx.atoms._NET_WM_USER_TIME {
                      let user_time = self.read_user_time(event.window); // Read from event.window which might be utw
                      if let Some(client) = self.clients.get_mut(&target_win) {
                           client.user_time = user_time;
                           debug!("User time updated for window {} to {}", target_win, user_time);
                      }
                 } else if event.atom == self.ctx.atoms.WM_HINTS {
                      let (group_leader, accepts_input, is_urgent) = self.read_wm_hints(target_win);
                      if let Some(client) = self.clients.get_mut(&target_win) {
                           client.group_leader = group_leader;
                           client.accepts_input = accepts_input;
                           client.is_urgent = is_urgent;
                           debug!("WM_HINTS updated for window {} (accepts_input: {}, urgent: {})", target_win, accepts_input, is_urgent);
                      }
                 } else if event.atom == self.ctx.atoms.WM_TRANSIENT_FOR {
                      let trans_reply = self.ctx.conn.get_property(false, target_win, self.ctx.atoms.WM_TRANSIENT_FOR, AtomEnum::WINDOW, 0, 1)?.reply();
                      if let Ok(prop) = trans_reply {
                          if let Some(parent) = prop.value32().and_then(|mut i| i.next()) {
                              if let Some(client) = self.clients.get_mut(&target_win) {
                                  client.transient_for = Some(parent);
                              }
                          }
                      }
                 } else if event.atom == self.ctx.atoms._NET_WM_STATE {
                      let is_modal = self.is_modal(target_win);
                      if let Some(client) = self.clients.get_mut(&target_win) {
                           client.is_modal = is_modal;
                      }
                 } else if event.atom == self.ctx.atoms._NET_WM_USER_TIME_WINDOW {
                      let utw = self.read_user_time_window(target_win);
                      if let Some(w) = utw {
                           let _ = self.ctx.conn.change_window_attributes(w, &x11rb::protocol::xproto::ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE));
                           let user_time = self.read_user_time(w);
                           if let Some(client) = self.clients.get_mut(&target_win) {
                                client.user_time_window = Some(w);
                                client.user_time = user_time;
                           }
                      } else {
                           if let Some(client) = self.clients.get_mut(&target_win) {
                                client.user_time_window = None;
                           }
                      }
                 }



            }
            Event::Expose(event) => {
                if event.count == 0 {
                    if let Some(client) = self.find_client_by_frame(event.window) {
                        let (border, title) = if client.is_fullscreen || client.is_desktop || client.is_dock || client.is_csd { (0, 0) } else { (BORDER_WIDTH, TITLE_HEIGHT) };
                        if let Err(_) = draw_decoration(&self.ctx, event.window, &client.name, client.width + 2*border, client.height + title + 2*border, title) { }
                        needs_paint = true;
                    }
                    if event.window == self.compositor.overlay_window || event.window == self.ctx.root_window { needs_paint = true; }
                }
            }
            Event::ClientMessage(event) => {
                 if event.type_ == self.ctx.atoms._NET_CURRENT_DESKTOP {
                     if let Some(new_idx) = event.data.as_data32().get(0) { let _ = self.switch_workspace(*new_idx); needs_paint = true; }
                 } else if event.type_ == self.ctx.atoms._NET_ACTIVE_WINDOW {
                     if let Some(client) = self.clients.get(&event.window) {
                         if let Some(frame) = client.frame { let _ = self.ctx.conn.configure_window(frame, &x11rb::protocol::xproto::ConfigureWindowAux::new().stack_mode(x11rb::protocol::xproto::StackMode::ABOVE)); } 
                         let _ = self.focus_window(event.window);
                         needs_paint = true;
                     }
                 } else if event.type_ == self.ctx.atoms.WM_PROTOCOLS {
                      let data = event.data.as_data32();
                      if data[0] == self.ctx.atoms._NET_WM_PING {
                          debug!("🏓 PONG: Window {} is alive!", event.window);
                      }
                 } else if event.type_ == self.ctx.atoms._NET_WM_STATE {
                    let data = event.data.as_data32();
                    let action = data[0]; // 0: remove, 1: add, 2: toggle
                    let atoms = [data[1], data[2]];

                    for atom in atoms {
                        if atom == 0 { continue; }
                        
                        let mut toggle_fs = false;
                        let mut toggle_max = false;
                        
                        if let Some(client) = self.clients.get_mut(&event.window) {
                            if atom == self.ctx.atoms._NET_WM_STATE_FULLSCREEN {
                                let next = match action {
                                    0 => false, 1 => true, 2 => !client.is_fullscreen, _ => client.is_fullscreen,
                                };
                                if next != client.is_fullscreen { toggle_fs = true; }
                            } else if atom == self.ctx.atoms._NET_WM_STATE_MAXIMIZED_VERT || atom == self.ctx.atoms._NET_WM_STATE_MAXIMIZED_HORZ {
                                let next = match action {
                                    0 => false, 1 => true, 2 => !client.is_maximized, _ => client.is_maximized,
                                };
                                if next != client.is_maximized { toggle_max = true; }
                            } else if atom == self.ctx.atoms._NET_WM_STATE_MODAL {
                                client.is_modal = match action {
                                    0 => false, 1 => true, 2 => !client.is_modal, _ => client.is_modal,
                                };
                            } else if atom == self.ctx.atoms._NET_WM_STATE_DEMANDS_ATTENTION {
                                client.demands_attention = match action {
                                    0 => false, 1 => true, 2 => !client.demands_attention, _ => client.demands_attention,
                                };
                            } else if atom == self.ctx.atoms._NET_WM_STATE_STICKY {
                                client.is_sticky = match action {
                                    0 => false, 1 => true, 2 => !client.is_sticky, _ => client.is_sticky,
                                };
                                client.workspace = if client.is_sticky { 0xFFFFFFFF } else { self.current_workspace };
                            } else if atom == self.ctx.atoms._NET_WM_STATE_SKIP_TASKBAR {
                                client.skip_taskbar = match action {
                                    0 => false, 1 => true, 2 => !client.skip_taskbar, _ => client.skip_taskbar,
                                };
                            } else if atom == self.ctx.atoms._NET_WM_STATE_SKIP_PAGER {
                                client.skip_pager = match action {
                                    0 => false, 1 => true, 2 => !client.skip_pager, _ => client.skip_pager,
                                };
                            } else if atom == self.ctx.atoms._NET_WM_STATE_SHADED {
                                client.is_shaded = match action {
                                    0 => false, 1 => true, 2 => !client.is_shaded, _ => client.is_shaded,
                                };
                                // TODO: shading implementation
                            } else if atom == self.ctx.atoms._NET_WM_STATE_ABOVE {
                                client.is_above = match action {
                                    0 => false, 1 => true, 2 => !client.is_above, _ => client.is_above,
                                };
                                if client.is_above { client.is_below = false; client.layer = crate::window::LAYER_ONTOP; }
                                else { client.layer = crate::window::LAYER_NORMAL; }
                            } else if atom == self.ctx.atoms._NET_WM_STATE_BELOW {
                                client.is_below = match action {
                                    0 => false, 1 => true, 2 => !client.is_below, _ => client.is_below,
                                };
                                if client.is_below { client.is_above = false; client.layer = crate::window::LAYER_BELOW; }
                                else { client.layer = crate::window::LAYER_NORMAL; }
                            }
                        }
                        
                        if toggle_fs { let _ = self.toggle_fullscreen(event.window); }
                        if toggle_max { let _ = self.toggle_maximize(event.window); }
                        let _ = self.update_net_wm_state(event.window);
                    }
                    needs_paint = true;


                 } else if event.type_ == self.ctx.atoms._NET_WM_MOVERESIZE {
                     let data = event.data.as_data32();
                     let x = data[0] as i16;
                     let y = data[1] as i16;
                     let direction = data[2];
                     
                     if let Some(client) = self.clients.get(&event.window) {
                         if let Some(frame) = client.frame {
                             if direction == 8 { // _NET_WM_MOVERESIZE_MOVE
                                 let frame_geom = self.ctx.conn.get_geometry(frame)?.reply()?;
                                 self.drag_state = DragState::Moving {
                                     window: event.window,
                                     start_pointer_x: x,
                                     start_pointer_y: y,
                                     start_frame_x: frame_geom.x,
                                     start_frame_y: frame_geom.y,
                                     snap: SnapZone::None,
                                 };
                                 // Grab pointer to receive motion events
                                 self.ctx.conn.grab_pointer(
                                     false,
                                     self.ctx.root_window,
                                     EventMask::POINTER_MOTION | EventMask::BUTTON_RELEASE,
                                     x11rb::protocol::xproto::GrabMode::ASYNC,
                                     x11rb::protocol::xproto::GrabMode::ASYNC,
                                     x11rb::NONE,
                                     self.cursors.normal,
                                     x11rb::CURRENT_TIME,
                                 )?;
                                 info!("Started MOVERESIZE_MOVE for window {}", event.window);
                             }
                         }
                     }
                 }
            }
            Event::KeyPress(event) => {
                 debug!("⌨️ KeyPress: detail={}, state={:?}, window={}", event.detail, event.state, event.event);
            }
            Event::ButtonPress(event) => {
                debug!("🎯 ButtonPress: window={}, root=({}, {}), event=({}, {}), detail={}", event.event, event.root_x, event.root_y, event.event_x, event.event_y, event.detail);
                let mut client_window = None;
                let mut frame_window = None;
                let mut is_client_click = false;

                if let Some(c) = self.clients.get(&event.event) {
                    client_window = Some(event.event);
                    frame_window = c.frame;
                    is_client_click = true;
                    info!("🖱️ Client click detected on win {} (frame {:?})", event.event, frame_window);
                } else if let Some(c) = self.clients.values().find(|c| c.frame == Some(event.event)) {
                    client_window = Some(c.window);
                    frame_window = Some(event.event);
                    info!("🖱️ Frame click detected on frame {} (win {})", event.event, c.window);
                }

                if let (Some(win), Some(frame)) = (client_window, frame_window) {
                    if let Some(c) = self.clients.get(&win) {
                        if !c.is_desktop {
                            let _ = self.ctx.conn.configure_window(frame, &x11rb::protocol::xproto::ConfigureWindowAux::new().stack_mode(x11rb::protocol::xproto::StackMode::ABOVE));
                        }
                    }
                    let _ = self.focus_window(win);
                    needs_paint = true;

                    if is_client_click {
                        use x11rb::protocol::xproto::Allow;
                        if let Err(e) = self.ctx.conn.allow_events(Allow::REPLAY_POINTER, x11rb::CURRENT_TIME) {
                            warn!("Failed to replay pointer: {}", e);
                        } else {
                            debug!("✓ Replayed pointer to client {}", win);
                        }
                    } else if event.detail == 1 {
                        let geom_data = self.ctx.conn.get_geometry(frame).ok().and_then(|c| c.reply().ok());
                        if let Some(geom) = geom_data {
                            let part = FrameGeometry::hit_test(geom.width, geom.height, event.event_x, event.event_y);
                            let cursor = self.get_cursor_for_part(part);
                            let grab_ok = self.ctx.conn.grab_pointer(false, self.ctx.root_window, EventMask::BUTTON_RELEASE | EventMask::POINTER_MOTION, x11rb::protocol::xproto::GrabMode::ASYNC, x11rb::protocol::xproto::GrabMode::ASYNC, x11rb::NONE, cursor, x11rb::CURRENT_TIME).ok().and_then(|c| c.reply().ok());
                            if let Some(reply) = grab_ok {
                                if reply.status == x11rb::protocol::xproto::GrabStatus::SUCCESS {
                                    let is_double_click = (win == self.last_click_window) && (event.time.wrapping_sub(self.last_click_time) < 400);
                                    if !is_double_click { self.last_click_time = event.time; self.last_click_window = win; }
                                    let should_maximize = self.settings_manager.current.double_click_action == "maximize";
                                    match part {
                                        FramePart::TitleBar => {
                                            if is_double_click {
                                                if should_maximize { let _ = self.toggle_maximize(win); }
                                                let _ = self.ctx.conn.ungrab_pointer(x11rb::CURRENT_TIME);
                                                self.drag_state = DragState::None;
                                            } else {
                                                self.drag_state = DragState::Moving { window: win, start_pointer_x: event.root_x, start_pointer_y: event.root_y, start_frame_x: geom.x, start_frame_y: geom.y, snap: SnapZone::None };
                                            }
                                        }
                                        FramePart::CornerBottomRight => { self.drag_state = DragState::Resizing { window: win, start_pointer_x: event.root_x, start_pointer_y: event.root_y, start_width: geom.width, start_height: geom.height }; }
                                        FramePart::CloseButton => { let _ = self.send_delete_window(win); let _ = self.ctx.conn.ungrab_pointer(x11rb::CURRENT_TIME); }
                                        FramePart::MaximizeButton => { let _ = self.toggle_maximize(win); let _ = self.ctx.conn.ungrab_pointer(x11rb::CURRENT_TIME); }
                                        FramePart::MinimizeButton => { let _ = self.toggle_minimize(win); let _ = self.ctx.conn.ungrab_pointer(x11rb::CURRENT_TIME); }
                                        _ => { let _ = self.ctx.conn.ungrab_pointer(x11rb::CURRENT_TIME); }
                                    }
                                }
                            }
                        }
                    } else if event.detail == 3 {
                        info!("🖱️ Right click on frame (button 3) for window {} - Menu not implemented yet", win);
                    }
                }
            }
            Event::MotionNotify(event) => {
                 let mut next_snap = None;
                 let mut ns_val = None;
                 match self.drag_state {
                     DragState::Moving { window, start_pointer_x, start_pointer_y, start_frame_x, start_frame_y, snap } => {
                          let dx = event.root_x - start_pointer_x; let dy = event.root_y - start_pointer_y;
                          let screen_w = self.ctx.screen_width as i16;
                          let screen_h = self.ctx.screen_height as i16;
                          let ns = if event.root_x <= 0 { SnapZone::Left }
                                   else if event.root_x >= screen_w - 1 { SnapZone::Right }
                                   else if event.root_y <= 0 { SnapZone::Top }
                                   else if event.root_y >= screen_h - 1 { SnapZone::None }
                                   else { SnapZone::None };
                           if ns != snap { next_snap = Some(ns); ns_val = Some(window); }
                           
                           let new_x = start_frame_x + dx;
                           let new_y = start_frame_y + dy;
                           
                           if let Some(client) = self.clients.get_mut(&window) {
                               if let Some(frame) = client.frame {
                                   let _ = self.ctx.conn.configure_window(frame, &x11rb::protocol::xproto::ConfigureWindowAux::new().x(Some(new_x as i32)).y(Some(new_y as i32)));
                               }
                               client.x = new_x;
                               client.y = new_y;
                           }
                           needs_paint = true;
                     }
                     DragState::Resizing { window, start_pointer_x, start_pointer_y, start_width, start_height } => {
                           let dx = event.root_x - start_pointer_x; let dy = event.root_y - start_pointer_y;
                           let new_w = (start_width as i16 + dx).max(100) as u16; 
                           let new_h = (start_height as i16 + dy).max(50) as u16;
                           
                           if let Some(client) = self.clients.get_mut(&window) {
                               client.width = new_w;
                               client.height = new_h;
                               if let Some(frame) = client.frame {
                                   let (border, title) = if client.is_fullscreen || client.is_desktop || client.is_dock { (0, 0) } else { (BORDER_WIDTH, TITLE_HEIGHT) };
                                   let frame_w = new_w as u32 + (2 * border) as u32;
                                   let frame_h = new_h as u32 + title as u32 + (2 * border) as u32;
                                   
                                   let _ = self.ctx.conn.configure_window(frame, &x11rb::protocol::xproto::ConfigureWindowAux::new().width(Some(frame_w)).height(Some(frame_h)));
                                   let _ = self.ctx.conn.configure_window(window, &x11rb::protocol::xproto::ConfigureWindowAux::new().width(Some(new_w as u32)).height(Some(new_h as u32)));
                                   let _ = draw_decoration(&self.ctx, frame, &client.name, new_w + 2*border, new_h + title + 2*border, title);
                                   let _ = self.update_window_shape(window);
                               }
                               self.client_xsync_request(window);
                           }
                           needs_paint = true;
                     }
                     _ => {}
                 }
                 if let (Some(ns), Some(_win)) = (next_snap, ns_val) {
                      if let DragState::Moving { ref mut snap, .. } = self.drag_state { *snap = ns; }
                 }
            }
            Event::ButtonRelease(event) => {
                 if event.detail == 1 {
                     if let DragState::Moving { window, snap, .. } = self.drag_state {
                         if snap != SnapZone::None { let _ = self.apply_snap(window, snap); }
                     }
                     if !matches!(self.drag_state, DragState::None) { 
                         let _ = self.ctx.conn.ungrab_pointer(x11rb::CURRENT_TIME); 
                         self.drag_state = DragState::None; 
                         needs_paint = true;
                     } 
                 }
            }
            _ => {}
        }
        Ok(needs_paint)
    }

    fn place_window(&self, width: u16, height: u16) -> (i16, i16) {
        let (wx, wy, ww, wh) = self.calculate_workarea();
        let existing: Vec<(i16, i16)> = self.clients.values()
            .filter(|c| c.workspace == self.current_workspace)
            .map(|c| (c.x, c.y))
            .collect();
        
        let (x, y) = cascade_placement(ww, wh, width, height, &existing);
        (x + wx, y + wy)
    }

    fn client_xsync_request(&mut self, window: Window) {
        if let Some(client) = self.clients.get_mut(&window) {
            if client.sync_waiting { return; }
            if let Some(_counter) = client.sync_counter {
                client.sync_next_value += 1;
                let data = [
                    self.ctx.atoms._NET_WM_SYNC_REQUEST.into(),
                    x11rb::CURRENT_TIME,
                    (client.sync_next_value & 0xFFFFFFFF) as u32,
                    (client.sync_next_value >> 32) as u32,
                    0,
                ];
                let event = x11rb::protocol::xproto::ClientMessageEvent {
                    response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
                    format: 32,
                    window,
                    type_: self.ctx.atoms.WM_PROTOCOLS,
                    data: x11rb::protocol::xproto::ClientMessageData::from(data),
                    sequence: 0,
                };
                let _ = self.ctx.conn.send_event(false, window, EventMask::NO_EVENT, event);
                client.sync_waiting = true;
            }
        }
    }

    fn client_create_xsync_alarm(&mut self, window: Window) -> Result<()> {
        use x11rb::protocol::sync::{CreateAlarmAux, Trigger, TESTTYPE, VALUETYPE, Int64};
        
        if let Some(client) = self.clients.get_mut(&window) {
            if let Some(counter) = client.sync_counter {
                let alarm = self.ctx.conn.generate_id()?;
                let trigger = Trigger {
                    counter,
                    wait_type: VALUETYPE::RELATIVE,
                    wait_value: Int64 { hi: 0, lo: 1 },
                    test_type: TESTTYPE::POSITIVE_COMPARISON,
                };
                let aux = CreateAlarmAux::new()
                    .counter(Some(trigger.counter))
                    .value_type(Some(trigger.wait_type))
                    .value(Some(trigger.wait_value))
                    .test_type(Some(trigger.test_type))
                    .delta(Some(Int64 { hi: 0, lo: 1 }))
                    .events(1u32);
                
                SyncExt::sync_create_alarm(&self.ctx.conn, alarm, &aux)?;
                client.sync_alarm = Some(alarm);
                debug!("Created Pulse alarm {} for window {}", alarm, window);
            }
        }
        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        if let Err(e) = self.paint() { warn!("Initial paint failed: {}", e); }
        let _ = self.update_net_workarea();
        loop {
            self.ctx.conn.flush()?;
            let mut needs_paint = false;
            
            // Wait for at least one event
            match self.ctx.conn.wait_for_event() {
                Ok(event) => {
                    needs_paint |= self.handle_event(event)?;
                    
                    // Drain all other pending events before painting to avoid flooding
                    while let Some(event) = self.ctx.conn.poll_for_event()? {
                        needs_paint |= self.handle_event(event)?;
                    }
                }
                Err(e) => {
                    error!("X11 server connection closed or error: {}", e);
                    break;
                }
            }
            
            if needs_paint {
                if let Err(e) = self.paint() {
                    self.error_tracker.record_compositor_error("paint loop", e);
                }
            }

            // Periodic health check
            let health = self.error_tracker.health_check();
            if !health.is_healthy {
                warn!("System health degraded: X11 errors: {}, Compositor errors: {}, Window errors: {}", 
                    health.x11_errors, health.compositor_errors, health.window_errors);
            }
        }
        Ok(())
    }

    fn update_window_shape(&self, window: Window) -> Result<()> {
        let client = if let Some(c) = self.clients.get(&window) { c } else { return Ok(()); };
        let frame = if let Some(f) = client.frame { f } else { return Ok(()); };
        
        let (border, title) = if client.is_fullscreen || client.is_desktop || client.is_dock || client.is_csd { 
            (0, 0) 
        } else { 
            (crate::window::frame::BORDER_WIDTH, crate::window::frame::TITLE_HEIGHT) 
        };

        // Set Input shape when using XShape extension
        // 1. Start with frame's own bounding shape as the input shape
        self.ctx.conn.shape_combine(SO::SET, SK::INPUT, SK::BOUNDING, frame, 0, 0, frame)?;
        
        // 2. Subtract the area where the client window is (bounding shape)
        self.ctx.conn.shape_combine(SO::SUBTRACT, SK::INPUT, SK::BOUNDING, frame, border as i16, (title + border) as i16, window)?;
        
        // 3. Union the client window's own input shape back in
        self.ctx.conn.shape_combine(SO::UNION, SK::INPUT, SK::INPUT, frame, border as i16, (title + border) as i16, window)?;
        
        Ok(())
    }

    fn read_startup_id(&self, window: Window) -> Option<String> {
        if let Ok(cookie) = self.ctx.conn.get_property(false, window, self.ctx.atoms._NET_STARTUP_ID, self.ctx.atoms.UTF8_STRING, 0, 1024) {
             if let Ok(reply) = cookie.reply() {
                 return String::from_utf8(reply.value).ok();
             }
        }
        None
    }
}


