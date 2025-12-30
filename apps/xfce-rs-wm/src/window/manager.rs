use std::collections::HashMap;
use anyhow::Result;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Window, ConnectionExt, CreateWindowAux, WindowClass, EventMask, AtomEnum, PropMode, MapState, SubwindowMode, ConfigWindow, ConfigureWindowAux};
use x11rb::protocol::composite::ConnectionExt as CompositeExt;
use x11rb::protocol::damage::{ConnectionExt as DamageExt, ReportLevel, Damage};
use x11rb::protocol::render::{ConnectionExt as RenderExt, CreatePictureAux, Picture};
use x11rb::protocol::xfixes::ConnectionExt as XFixesExt;
use x11rb::protocol::shape::ConnectionExt as ShapeExt;
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


#[derive(Debug, Clone, Copy)]
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
        
        // Fetch Window Type for Placement
        let mut is_dialog = false;
        let mut window_types = Vec::new();
        let type_reply = self.ctx.conn.get_property(false, win, self.ctx.atoms._NET_WM_WINDOW_TYPE, AtomEnum::ATOM, 0, 1024)?.reply();
        if let Ok(prop) = type_reply {
            if prop.type_ == u32::from(AtomEnum::ATOM) && prop.format == 32 {
                for atom in prop.value32().unwrap() {
                    window_types.push(atom);
                    if atom == self.ctx.atoms._NET_WM_WINDOW_TYPE_DIALOG {
                        is_dialog = true;
                    }
                }
            }
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
        
        let is_dock = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_DOCK);
        let is_desktop = window_types.contains(&self.ctx.atoms._NET_WM_WINDOW_TYPE_DESKTOP);
        let is_fullscreen = window_types.contains(&self.ctx.atoms._NET_WM_STATE_FULLSCREEN);
        


        let is_csd = self.has_csd(win);
        
        let (motif_decor, motif_title) = self.read_motif_hints(win);
        
        let (border_width, title_height) = if is_dock || is_fullscreen || is_desktop || !motif_decor || is_csd {
            (0, 0)
        } else if !motif_title {
            (BORDER_WIDTH, 0)
        } else {
            (BORDER_WIDTH, TITLE_HEIGHT)
        };
        
        use crate::window::{LAYER_DOCK, LAYER_NORMAL, LAYER_FULLSCREEN, LAYER_DESKTOP};
        let layer = if is_desktop {
            LAYER_DESKTOP
        } else if is_dock {
            LAYER_DOCK
        } else if is_fullscreen {
            LAYER_FULLSCREEN
        } else {
            LAYER_NORMAL
        };

        if (x <= 1 || y <= 1) && !is_dock && !is_desktop {
            let screen = &self.ctx.conn.setup().roots[self.ctx.screen_num];
            
            if is_dialog {
                let (nx, ny) = center_window(screen.width_in_pixels, screen.height_in_pixels, geom.width, geom.height);
                x = nx;
                y = ny;
            } else {
                 let origins: Vec<(i16, i16)> = self.clients.values().map(|c| (c.x, c.y)).collect();
                 let (nx, ny) = cascade_placement(screen.width_in_pixels, screen.height_in_pixels, geom.width, geom.height, &origins);
                 x = nx;
                 y = ny;
            }
        }
        
        // Force Desktops to 0,0 and screen size if they are intended to be background
        let (fix_x, fix_y, fix_w, fix_h) = if is_desktop {
            (0, 0, self.ctx.screen_width, self.ctx.screen_height)
        } else {
            (x, y, geom.width, geom.height)
        };

        let frame_geom = FrameGeometry::from_client(fix_x, fix_y, fix_w, fix_h, border_width, title_height);
        debug!("Frame geometry for window {}: {:?}", win, frame_geom);
        let frame_win = self.ctx.conn.generate_id()?;
        
        // Listen for frame events (decorations) and motion
        let values = CreateWindowAux::new()
            .event_mask(EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT | EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE)
            .background_pixel(0x333333)
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
        
        // Initial stacking order: Desktops stay at the bottom, others go to top
        let mut aux = x11rb::protocol::xproto::ConfigureWindowAux::new();
        if is_desktop {
            aux = aux.stack_mode(x11rb::protocol::xproto::StackMode::BELOW);
        } else {
            aux = aux.stack_mode(x11rb::protocol::xproto::StackMode::ABOVE);
        }
        let _ = self.ctx.conn.configure_window(frame_win, &aux);
        debug!("Created frame window {} for client {} (stacking: {:?})", frame_win, win, if is_desktop { "BELOW" } else { "ABOVE" });
        
        if workspace == self.current_workspace || workspace == 0xFFFFFFFF {
             self.ctx.conn.map_window(frame_win)?;
             self.ctx.conn.map_window(win)?;
        }
        
        let mut client = Client::new(
            win,
            frame_geom.x,
            frame_geom.y,
            fix_w,
            fix_h
        );
        client.frame = Some(frame_win);
        client.name = name;
        client.workspace = workspace;
        client.window_type = window_types;
        client.transient_for = transient_for;
        client.layer = layer;
        client.is_desktop = is_desktop;
        client.is_dock = is_dock;
        client.is_fullscreen = is_fullscreen;
        
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


        let width = geom.width + (2 * border_width);
        let height = geom.height + title_height + (2 * border_width);
        debug!("Drawing decoration for frame {} (title: {})", frame_win, client.name);
        let _ = self.error_tracker.warn_if_failed(
            draw_decoration(&self.ctx, frame_win, &client.name, width, height, title_height),
            "draw initial decoration",
            crate::window::error::ErrorCategory::Window
        );
        
        self.clients.insert(win, client);
        self.mru_stack.retain(|&w| w != win);
        self.mru_stack.insert(0, win);
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
                   return Some((client.picture, content_pic, client.x, client.y, w, h, b, t, client.width, client.height));
                }
            }
            None
        });

        let unmanaged_list = self.unmanaged_windows.values().map(|u| {
            (None, u.picture, u.x, u.y, u.width, u.height, 0, 0, u.width, u.height)
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
        let workarea = [x as u32, y as u32, w as u32, h as u32];
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
        
        if let Some(client) = self.clients.get(&window) {
            info!("🎯 FOCUS: Window {} exists, name='{}', frame={:?}", 
                  window, client.name, client.frame);
                  
            if self.is_protocol_supported(window, self.ctx.atoms.WM_TAKE_FOCUS) {
                 let event = ClientMessageEvent {
                    response_type: x11rb::protocol::xproto::CLIENT_MESSAGE_EVENT,
                    format: 32,
                    window,
                    type_: self.ctx.atoms.WM_PROTOCOLS,
                    data: ClientMessageData::from([self.ctx.atoms.WM_TAKE_FOCUS, x11rb::CURRENT_TIME, 0, 0, 0]),
                    sequence: 0,
                };
                match self.ctx.conn.send_event(false, window, EventMask::NO_EVENT, event) {
                    Ok(_) => info!("✓ FOCUS: Sent WM_TAKE_FOCUS to window {}", window),
                    Err(e) => warn!("Failed to send WM_TAKE_FOCUS to window {}: {}", window, e),
                }
            } else {
                info!("ℹ️ FOCUS: WM_TAKE_FOCUS not supported by window {}", window);
            }

            // Set input focus to the client window (not the frame!)
            match self.ctx.conn.set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME) {
                Ok(_) => {
                    info!("✓ FOCUS: Successfully set input focus to window {}", window);
                    
                    // Update _NET_ACTIVE_WINDOW
                    if let Err(e) = self.ctx.conn.change_property32(PropMode::REPLACE, self.ctx.root_window, self.ctx.atoms._NET_ACTIVE_WINDOW, AtomEnum::WINDOW, &[window]) {
                        warn!("Failed to update _NET_ACTIVE_WINDOW: {}", e);
                    } else {
                        info!("✓ FOCUS: Updated _NET_ACTIVE_WINDOW property");
                    }
                },
                Err(e) => {
                    error!("❌ FOCUS: Failed to set input focus to window {}: {}", window, e);
                }
            }
        } else {
            warn!("⚠ FOCUS: Window {} not found in clients", window);
        }
        self.mru_stack.retain(|&w| w != window);
        self.mru_stack.insert(0, window);
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

    fn has_csd(&self, window: Window) -> bool {
        // 1. Check for _GTK_FRAME_EXTENTS
        if let Ok(cookie) = self.ctx.conn.get_property(
            false,
            window,
            self.ctx.atoms._GTK_FRAME_EXTENTS,
            AtomEnum::CARDINAL,
            0,
            4
        ) {
            if let Ok(reply) = cookie.reply() {
                if !reply.value.is_empty() {
                    debug!("Window {} has _GTK_FRAME_EXTENTS - treating as CSD", window);
                    return true;
                }
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
                if let Some(client) = self.clients.get_mut(&event.window) {
                    let mut frame_aux = ConfigureWindowAux::new();
                    if event.value_mask.contains(ConfigWindow::X) { client.x = event.x; frame_aux = frame_aux.x(Some(event.x as i32)); }
                    if event.value_mask.contains(ConfigWindow::Y) { client.y = event.y; frame_aux = frame_aux.y(Some(event.y as i32)); }
                    if event.value_mask.contains(ConfigWindow::WIDTH) { client.width = event.width; frame_aux = frame_aux.width(Some(event.width as u32)); }
                    if event.value_mask.contains(ConfigWindow::HEIGHT) { client.height = event.height; frame_aux = frame_aux.height(Some(event.height as u32)); }
                    
                    if let Some(frame) = client.frame {
                         let _ = self.ctx.conn.configure_window(frame, &frame_aux);
                         let (border, title) = if client.is_fullscreen || client.is_desktop || client.is_dock { (0, 0) } else { (BORDER_WIDTH, TITLE_HEIGHT) };
                         let _ = self.ctx.conn.configure_window(event.window, &ConfigureWindowAux::new().width(Some(client.width as u32)).height(Some(client.height as u32)));
                         if event.value_mask.intersects(ConfigWindow::WIDTH | ConfigWindow::HEIGHT) {
                             let _ = draw_decoration(&self.ctx, frame, &client.name, client.width + 2*border, client.height + title + 2*border, title);
                         }
                    }
                } else {
                    let mut aux = ConfigureWindowAux::new();
                    if event.value_mask.contains(ConfigWindow::X) { aux = aux.x(Some(event.x as i32)); }
                    if event.value_mask.contains(ConfigWindow::Y) { aux = aux.y(Some(event.y as i32)); }
                    if event.value_mask.contains(ConfigWindow::WIDTH) { aux = aux.width(Some(event.width as u32)); }
                    if event.value_mask.contains(ConfigWindow::HEIGHT) { aux = aux.height(Some(event.height as u32)); }
                    if event.value_mask.contains(ConfigWindow::STACK_MODE) { aux = aux.stack_mode(Some(event.stack_mode)); }
                    let _ = self.ctx.conn.configure_window(event.window, &aux);
                }
                needs_paint = true;
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
                let _ = self.ctx.conn.damage_subtract(event.damage, x11rb::NONE, x11rb::NONE); 
                
                // Prevent infinite loops: ignore damage on our own overlay or the root window
                if event.drawable != self.compositor.overlay_window && event.drawable != self.ctx.root_window {
                    debug!("DamageNotify for window {}", event.drawable);
                    needs_paint = true; 
                }
            }
            Event::PropertyNotify(event) => {
                 if event.atom == self.ctx.atoms._NET_WM_STRUT || event.atom == self.ctx.atoms._NET_WM_STRUT_PARTIAL {
                      if let Ok(strut) = self.read_strut_property(event.window) {
                          if let Some(client) = self.clients.get_mut(&event.window) {
                               client.strut = strut;
                               let _ = self.update_net_workarea();
                          }
                      }
                 } else if event.atom == self.ctx.atoms._NET_WM_NAME {
                      if let Some(client) = self.clients.get_mut(&event.window) {
                          let name_reply = self.ctx.conn.get_property(false, event.window, self.ctx.atoms._NET_WM_NAME, self.ctx.atoms.UTF8_STRING, 0, 1024)?.reply();
                          if let Ok(prop) = name_reply {
                              if let Ok(name) = String::from_utf8(prop.value) { client.name = name;
                                  if let Some(frame) = client.frame {
                                      let _ = self.ctx.conn.send_event(false, frame, EventMask::EXPOSURE, x11rb::protocol::xproto::ExposeEvent { response_type: x11rb::protocol::xproto::EXPOSE_EVENT, sequence: 0, window: frame, x: 0, y: 0, width: 0, height: 0, count: 0 });
                                  }
                              }
                          }
                      }
                 }
            }
            Event::Expose(event) => {
                if event.count == 0 {
                    if let Some(client) = self.find_client_by_frame(event.window) {
                        let (border, title) = if client.is_fullscreen || client.is_desktop || client.is_dock { (0, 0) } else { (BORDER_WIDTH, TITLE_HEIGHT) };
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
                 } else if event.type_ == self.ctx.atoms._NET_WM_STATE {
                     let data = event.data.as_data32();
                     if data[1] == self.ctx.atoms._NET_WM_STATE_MAXIMIZED_VERT || data[1] == self.ctx.atoms._NET_WM_STATE_MAXIMIZED_HORZ { let _ = self.toggle_maximize(event.window); needs_paint = true; }
                     if data[1] == self.ctx.atoms._NET_WM_STATE_FULLSCREEN { let _ = self.toggle_fullscreen(event.window); needs_paint = true; }
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
                               }
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
}


