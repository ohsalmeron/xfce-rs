mod core;
mod window;
mod ewmh;
mod utils;

use tracing::{info, error, warn};
use crate::core::context::Context;
use crate::window::manager::WindowManager;

use clap::Parser;
use x11rb::protocol::xproto::{ConnectionExt, WindowClass, CreateWindowAux, EventMask};
use x11rb::connection::Connection;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Replace the existing window manager
    #[arg(long)]
    replace: bool,

    /// Session management client ID
    #[arg(long = "sm-client-id")]
    sm_client_id: Option<String>,
}

fn acquire_wm_selection(ctx: &Context, replace: bool) -> anyhow::Result<()> {
    // ICCCM 2.8: Manager Selection
    // Atom: WM_S{screen_num}
    let atom_name = format!("WM_S{}", ctx.screen_num);
    let wm_sn_atom = ctx.conn.intern_atom(false, atom_name.as_bytes())?.reply()?.atom;
    
    // Check if another WM owns it
    let owner = ctx.conn.get_selection_owner(wm_sn_atom)?.reply()?.owner;
    if owner != x11rb::NONE {
        if !replace {
             return Err(anyhow::anyhow!("Another window manager is already running on screen {}. Use --replace to replace it.", ctx.screen_num));
        }
        info!("Another WM is running (Window {}). replacing...", owner);
        // We don't need to explicitly kill it? 
        // Standard says: "If the selection is owned, the client should wait for the owner to release it if it wants to replace."
        // But usually we just Take it.
    }

    // Capture selection
    // We need a window to own the selection. We can use a dummy window or the root? 
    // Usually a separate unmapped window is safer.
    let selection_win = ctx.conn.generate_id()?;
    ctx.conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        selection_win,
        ctx.root_window,
        -1, -1, 1, 1, 0,
        WindowClass::INPUT_ONLY,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new().event_mask(EventMask::STRUCTURE_NOTIFY)
    )?;
    
    ctx.conn.set_selection_owner(selection_win, wm_sn_atom, x11rb::CURRENT_TIME)?;
    
    // Check if we got it
    let new_owner = ctx.conn.get_selection_owner(wm_sn_atom)?.reply()?.owner;
    if new_owner != selection_win {
        return Err(anyhow::anyhow!("Failed to acquire WM selection."));
    }
    
    // Announce we are here (ClientMessage to Root) - Optional but good practice
    // MANAGER ClientMessage
    
    info!("Acquired WM selection: {}", atom_name);
    Ok(())
}

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    
    let args = Args::parse();
    
    info!("Starting xfwm4-rs...");

    match Context::new() {
        Ok(ctx) => {
            info!("Successfully connected to X11 server.");
            info!("Screen: {}, Root Window: {}", ctx.screen_num, ctx.root_window);
            
            // Check replacement
            if let Err(e) = acquire_wm_selection(&ctx, args.replace) {
                 error!("{}", e);
                 return Err(e);
            }
            
            crate::ewmh::setup::setup_hints(&ctx)?;
            
            // Initialize Settings
            let settings_manager = crate::window::settings::SettingsManager::new().await?;
            
            // Initialize Session
            let mut session_manager = crate::window::session::SessionManager::new().await?;
            if let Err(e) = session_manager.register(args.sm_client_id.as_deref()).await {
                warn!("Session registration failed: {}", e);
            }
            
            let mut wm = WindowManager::new(ctx, settings_manager)?;
            wm.scan_windows()?;
            
            // Run with error handling - don't let X11 errors crash us
            loop {
                match wm.run() {
                    Ok(_) => break, // Normal exit
                    Err(e) => {
                        // Check if it's a fatal error or recoverable
                        let error_msg = format!("{}", e);
                        if error_msg.contains("closed the connection") || 
                           error_msg.contains("broken pipe") ||
                           error_msg.contains("I/O error") {
                            error!("Fatal X11 error - server disconnected: {}", e);
                            break;
                        } else {
                            // Log but try to continue for other errors
                            error!("X11 error (continuing): {}", e);
                            std::thread::sleep(std::time::Duration::from_millis(100));
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to connect to X11 server: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
