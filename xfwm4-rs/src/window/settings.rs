use zbus::Connection;
use anyhow::Result;
use tracing::{debug, warn};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Settings {
    pub double_click_action: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            double_click_action: "maximize".to_string(),
        }
    }
}

pub struct SettingsManager {
    pub current: Settings,
}

impl SettingsManager {
    pub async fn new() -> Result<Self> {
        let mut manager = Self {
            current: Settings::default(),
        };
        
        // Try to load from Xfconf if available
        if let Err(e) = manager.load_xfconf().await {
            warn!("Failed to load Xfconf settings, using defaults: {}", e);
        }
        
        Ok(manager)
    }

    async fn load_xfconf(&mut self) -> Result<()> {
        let conn = Connection::session().await?;
        
        // org.xfce.Xfconf /org/xfce/Xfconf org.xfce.Xfconf
        // Method: GetProperties(s channel, s property_base) -> a{sv}
        
        let reply: HashMap<String, zbus::zvariant::OwnedValue> = conn.call_method(
            Some("org.xfce.Xfconf"),
            "/org/xfce/Xfconf",
            Some("org.xfce.Xfconf"),
            "GetAllProperties",
            &("xfwm4", "/"),
        ).await?.body().deserialize()?;

        debug!("Loaded {} properties from Xfconf", reply.len());

        if let Some(val) = reply.get("/general/double_click_action") {
            if let Ok(s) = val.downcast_ref::<&str>() {
                self.current.double_click_action = s.to_string();
            }
        }
        
        Ok(())
    }
}
