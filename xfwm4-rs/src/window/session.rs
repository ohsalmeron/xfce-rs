use zbus::{proxy, Connection};
use anyhow::Result;
use tracing::{info, warn};
use std::collections::HashMap;
use futures_util::StreamExt;

#[proxy(
    interface = "org.xfce.Session.Manager",
    default_service = "org.xfce.SessionManager",
    default_path = "/org/xfce/SessionManager"
)]
trait Session {
    fn register_client(&self, app_id: &str, client_startup_id: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

#[proxy(
    interface = "org.xfce.Session.Client",
    default_service = "org.xfce.SessionManager"
)]
trait SessionClient {
    fn set_sm_properties(&self, properties: HashMap<&str, zbus::zvariant::Value<'_>>) -> zbus::Result<()>;
    fn end_session_response(&self, is_ok: bool, reason: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn query_end_session(&self, flags: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn end_session(&self, flags: u32) -> zbus::Result<()>;
}

pub struct SessionManager {
    client_path: Option<zbus::zvariant::OwnedObjectPath>,
}

impl SessionManager {
    pub async fn new() -> Result<Self> {
        Ok(Self { client_path: None })
    }

    pub async fn register(&mut self, sm_client_id: Option<&str>) -> Result<()> {
        let conn = Connection::session().await?;
        let proxy = SessionProxy::new(&conn).await?;
        
        let app_id = "xfwm4-rs";
        let startup_id = sm_client_id
            .map(|s| s.to_string())
            .or_else(|| std::env::var("DESKTOP_STARTUP_ID").ok())
            .unwrap_or_default();
        
        match proxy.register_client(app_id, &startup_id).await {
            Ok(path) => {
                info!("Registered with XFCE Session Manager: {}", path);
                self.client_path = Some(path.clone());
                
                let path_clone = path.clone();
                let conn_clone = conn.clone();
                
                // Spawn signal listener
                tokio::spawn(async move {
                    if let Ok(client_proxy) = SessionClientProxy::builder(&conn_clone)
                        .path(path_clone)?
                        .build()
                        .await 
                    {
                        // Set initial properties
                        let mut props = HashMap::new();
                        props.insert("SmProgram", zbus::zvariant::Value::from("xfwm4-rs"));
                        let restart_cmd = vec!["xfwm4-rs", "--replace"];
                        props.insert("SmRestartCommand", zbus::zvariant::Value::from(restart_cmd));
                        
                        let _ = client_proxy.set_sm_properties(props).await;

                        // Listen for signals
                        let mut query_end = match client_proxy.receive_query_end_session().await {
                            Ok(s) => s,
                            Err(e) => { warn!("Failed to listen for QueryEndSession: {}", e); return Ok(()); }
                        };
                        let mut end_session = match client_proxy.receive_end_session().await {
                            Ok(s) => s,
                            Err(e) => { warn!("Failed to listen for EndSession: {}", e); return Ok(()); }
                        };

                        loop {
                            tokio::select! {
                                Some(sig) = query_end.next() => {
                                    if let Ok(args) = sig.args() {
                                        info!("Received QueryEndSession: flags={}", args.flags);
                                        // Respond OK
                                        let _ = client_proxy.end_session_response(true, "").await;
                                    }
                                }
                                Some(sig) = end_session.next() => {
                                    if let Ok(args) = sig.args() {
                                        info!("Received EndSession: flags={}", args.flags);
                                        // Graceful exit
                                        std::process::exit(0);
                                    }
                                }
                            }
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                });
            }
            Err(e) => {
                warn!("Could not register with XFCE Session Manager: {}", e);
            }
        }
        
        Ok(())
    }
}
