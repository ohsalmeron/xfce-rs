// MPRIS2 integration module for media player control
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, debug, warn};
use zbus::{Connection, Proxy, names::OwnedWellKnownName};
use once_cell::sync::Lazy;

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
const MPRIS_PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Debug, Clone)]
pub struct PlayerInfo {
    #[allow(dead_code)]
    pub dbus_name: String,
    pub player_name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_art: Option<String>,
    pub playing: bool,
    pub position: u64,
    pub length: u64,
}

pub struct MprisManager {
    connection: Arc<Mutex<Option<Connection>>>,
    players: Arc<Mutex<HashMap<String, PlayerInfo>>>,
    active_player: Arc<Mutex<Option<String>>>,
}

impl MprisManager {
    pub fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(None)),
            players: Arc::new(Mutex::new(HashMap::new())),
            active_player: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to D-Bus session bus for MPRIS2");

        let connection = Connection::session()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to D-Bus: {}", e))?;

        *self.connection.lock().await = Some(connection.clone());

        // Discover initial players
        self.discover_players().await?;

        info!("MPRIS2 connection established");
        Ok(())
    }

    async fn discover_players(&self) -> Result<()> {
        let connection_guard = self.connection.lock().await;
        let connection = connection_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        // List all D-Bus names
        let dbus_bus_name: OwnedWellKnownName = OwnedWellKnownName::try_from("org.freedesktop.DBus")
            .map_err(|_| anyhow::anyhow!("Invalid bus name"))?;
        let proxy = Proxy::new(
            connection,
            &dbus_bus_name,
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        ).await?;

        let names_result = proxy.call_method("ListNames", &()).await?;
        let names: Vec<String> = names_result.body().deserialize()?;

        let mut found_players = Vec::new();

        // Filter for MPRIS2 players
        for name in names {
            if name.starts_with(MPRIS_PREFIX) {
                let player_name = name.strip_prefix(MPRIS_PREFIX)
                    .unwrap_or(&name)
                    .to_string();
                found_players.push((name, player_name));
            }
        }

        // Update players map
        let mut players = self.players.lock().await;
        players.clear();
        
        // Use mpris crate to get actual player identities (collect identities first, then match)
        let player_identities: Vec<String> = tokio::task::spawn_blocking(|| {
            use mpris::PlayerFinder;
            let finder = PlayerFinder::new().ok()?;
            let all_players = finder.find_all().ok()?;
            Some(all_players.iter().map(|p| p.identity().to_string()).collect())
        }).await.ok().flatten().unwrap_or_default();
        
        if !player_identities.is_empty() {
            debug!("Found {} MPRIS players via crate: {:?}", player_identities.len(), player_identities);
            for identity in player_identities {
                // Find matching D-Bus name from our discovered list
                let dbus_name_opt = found_players.iter()
                    .find(|(_, player_name)| {
                        identity == *player_name || 
                        identity.contains(player_name) ||
                        player_name.contains(&identity)
                    })
                    .map(|(dbus_name, _)| dbus_name.clone());
                
                let dbus_name = dbus_name_opt.unwrap_or_else(|| format!("{}{}", MPRIS_PREFIX, identity));
                
                if let Ok(player_info) = self.get_player_info(connection, &dbus_name, &identity).await {
                    players.insert(dbus_name.clone(), player_info);
                    
                    // Set first player as active if none is set
                    if self.active_player.lock().await.is_none() {
                        *self.active_player.lock().await = Some(dbus_name);
                    }
                }
            }
        } else {
            // Fallback to old method if mpris crate fails
            debug!("MPRIS crate failed, using D-Bus discovery fallback");
            for (dbus_name, player_name) in found_players {
                if let Ok(player_info) = self.get_player_info(connection, &dbus_name, &player_name).await {
                    players.insert(dbus_name.clone(), player_info);
                    
                    // Set first player as active if none is set
                    if self.active_player.lock().await.is_none() {
                        *self.active_player.lock().await = Some(dbus_name);
                    }
                }
            }
        }
        Ok(())
    }

    async fn get_player_info(&self, _connection: &Connection, dbus_name: &str, player_name: &str) -> Result<PlayerInfo> {
        // Use mpris crate to get metadata properly - run in blocking task
        use mpris::PlayerFinder;
        
        let dbus_name_clone = dbus_name.to_string();
        let player_name_clone = player_name.to_string();
        
        let (playing, position, title, artist, album, album_art, length) = tokio::task::spawn_blocking(move || {
            let finder = match PlayerFinder::new() {
                Ok(f) => f,
                Err(e) => {
                    warn!("Failed to create PlayerFinder: {}", e);
                    return (false, 0u64, format!("Playing from {}", player_name_clone), "Unknown Artist".to_string(), "Unknown Album".to_string(), None, 0u64);
                }
            };
            
            // List all available players and find the right one by identity
            let player_result = if let Ok(all_players) = finder.find_all() {
                let player_names: Vec<String> = all_players.iter().map(|p| p.identity().to_string()).collect();
                debug!("Available MPRIS2 players: {:?}", player_names);
                
                // Special handling for VLC - try multiple patterns
                let is_vlc = player_name_clone.to_lowercase().contains("vlc") || 
                             dbus_name_clone.to_lowercase().contains("vlc");
                
                // Try to find by matching the identity directly (this is what mpris crate uses)
                // player_name_clone should be the identity from discover_players
                if is_vlc {
                    // Special VLC discovery with multiple patterns
                    debug!("Detected VLC player, trying multiple identity patterns");
                    let vlc_patterns = vec![
                        "VLC media player",
                        "vlc",
                        "VLC",
                        "org.mpris.MediaPlayer2.vlc",
                    ];
                    
                    // Try each pattern
                    let mut found_player: Option<mpris::Player> = None;
                    for pattern in &vlc_patterns {
                        // Try to find in all_players list first
                        if let Some(player_ref) = all_players.iter()
                            .find(|p| {
                                let identity = p.identity().to_lowercase();
                                identity == pattern.to_lowercase() ||
                                identity.contains(&pattern.to_lowercase()) ||
                                pattern.to_lowercase().contains(&identity)
                            }) {
                            let identity = player_ref.identity().to_string();
                            debug!("Found VLC player using pattern '{}': {}", pattern, identity);
                            // Recreate player using find_by_name
                            if let Ok(player) = finder.find_by_name(&identity) {
                                found_player = Some(player);
                                break;
                            }
                        }
                    }
                    
                    // If not found in list, try find_by_name with VLC patterns
                    if found_player.is_none() {
                        for pattern in &vlc_patterns {
                            if let Ok(player) = finder.find_by_name(pattern) {
                                debug!("Found VLC player via find_by_name('{}'): {}", pattern, player.identity());
                                found_player = Some(player);
                                break;
                            }
                        }
                    }
                    
                    // Use found VLC player or fall back to normal matching
                    found_player.map(Ok).unwrap_or_else(|| {
                        // Fallback to normal matching
                        match all_players.into_iter()
                            .find(|p| {
                                let identity = p.identity();
                                identity == player_name_clone ||
                                identity.contains(&player_name_clone) ||
                                player_name_clone.contains(identity)
                            }) {
                            Some(p) => Ok(p),
                            None => {
                                finder.find_by_name(&player_name_clone)
                                    .or_else(|_| {
                                        let base_name = player_name_clone.split('.').next().unwrap_or(&player_name_clone);
                                        finder.find_by_name(base_name)
                                    })
                            }
                        }
                    })
                } else {
                    // Normal matching for non-VLC players
                    match all_players.into_iter()
                        .find(|p| {
                            let identity = p.identity();
                            identity == player_name_clone ||
                            identity.contains(&player_name_clone) ||
                            player_name_clone.contains(identity)
                        }) {
                        Some(p) => Ok(p),
                        None => {
                            finder.find_by_name(&player_name_clone)
                                .or_else(|_| {
                                    let base_name = player_name_clone.split('.').next().unwrap_or(&player_name_clone);
                                    finder.find_by_name(base_name)
                                })
                        }
                    }
                }
            } else {
                // Fallback: try find_by_name
                // Special VLC handling even in fallback
                let is_vlc = player_name_clone.to_lowercase().contains("vlc");
                if is_vlc {
                    let vlc_patterns = vec!["vlc", "VLC", "VLC media player"];
                    let mut found = None;
                    for pattern in &vlc_patterns {
                        if let Ok(player) = finder.find_by_name(pattern) {
                            debug!("Found VLC player via fallback find_by_name('{}'): {}", pattern, player.identity());
                            found = Some(Ok(player));
                            break;
                        }
                    }
                    found.unwrap_or_else(|| {
                        finder.find_by_name(&player_name_clone)
                            .or_else(|_| {
                                let base_name = player_name_clone.split('.').next().unwrap_or(&player_name_clone);
                                finder.find_by_name(base_name)
                            })
                    })
                } else {
                    finder.find_by_name(&player_name_clone)
                        .or_else(|_| {
                            let base_name = player_name_clone.split('.').next().unwrap_or(&player_name_clone);
                            finder.find_by_name(base_name)
                        })
                }
            };
            
            let player = player_result;
            
            let player = match player {
                Ok(p) => {
                    let identity = p.identity();
                    info!("Found player: {} (dbus: {}, search_name: {})", identity, dbus_name_clone, player_name_clone);
                    if identity.to_lowercase().contains("vlc") {
                        info!("VLC player found! Identity: {}", identity);
                    }
                    p
                }
                Err(e) => {
                    warn!("Failed to find player '{}' or '{}': {:?}", dbus_name_clone, player_name_clone, e);
                    return (false, 0u64, format!("Playing from {}", player_name_clone), "Unknown Artist".to_string(), "Unknown Album".to_string(), None, 0u64);
                }
            };
            
            let playing = player.get_playback_status()
                .map(|s| s == mpris::PlaybackStatus::Playing)
                .unwrap_or(false);
            
            let position = player.get_position()
                .map(|d| d.as_secs())
                .unwrap_or(0);
            
            let mut title = format!("Playing from {}", player_name_clone);
            let mut artist = "Unknown Artist".to_string();
            let mut album = "Unknown Album".to_string();
            let mut album_art: Option<String> = None;
            let mut length = 0u64;
            
            let is_vlc_player = player.identity().to_lowercase().contains("vlc");
            match player.get_metadata() {
                Ok(metadata) => {
                    // Log all available metadata fields for VLC debugging
                    if is_vlc_player {
                        info!("VLC metadata retrieved - checking fields...");
                        // Try to get raw metadata map for debugging
                        debug!("Metadata fields available for VLC player");
                    }
                    
                    if let Some(t) = metadata.title() {
                        title = t.to_string();
                        if is_vlc_player {
                            info!("VLC title found: '{}'", title);
                        }
                    } else if is_vlc_player {
                        warn!("VLC metadata has no title field");
                    }
                    
                    if let Some(artists) = metadata.artists() {
                        if let Some(first) = artists.first() {
                            artist = first.to_string();
                            if is_vlc_player {
                                info!("VLC artist found: '{}'", artist);
                            }
                        } else if is_vlc_player {
                            warn!("VLC metadata has artists() but list is empty");
                        }
                    } else if is_vlc_player {
                        warn!("VLC metadata has no artists() field");
                    }
                    
                    if let Some(a) = metadata.album_name() {
                        album = a.to_string();
                        if is_vlc_player {
                            info!("VLC album found: '{}'", album);
                        }
                    } else if is_vlc_player {
                        debug!("VLC metadata has no album_name field");
                    }
                    
                    if let Some(url) = metadata.art_url() {
                        album_art = Some(url.to_string());
                    }
                    if let Some(len) = metadata.length() {
                        length = len.as_secs();
                    }
                    
                    if is_vlc_player {
                        info!("VLC final metadata: title='{}', artist='{}', album='{}'", title, artist, album);
                    } else {
                        info!("Successfully extracted metadata: title='{}', artist='{}', album='{}'", title, artist, album);
                    }
                }
                Err(e) => {
                    if is_vlc_player {
                        warn!("Failed to get metadata from VLC player: {}", e);
                    } else {
                        debug!("Failed to get metadata from player: {}", e);
                    }
                }
            }
            
            (playing, position, title, artist, album, album_art, length)
        }).await.map_err(|e| anyhow::anyhow!("Task error: {}", e))?;
        
        info!("Parsed metadata: title='{}', artist='{}', album='{}'", title, artist, album);

        Ok(PlayerInfo {
            dbus_name: dbus_name.to_string(),
            player_name: player_name.to_string(),
            title,
            artist,
            album,
            album_art,
            playing,
            position,
            length,
        })
    }

    async fn get_active_player(&self) -> Result<String> {
        let active = self.active_player.lock().await.clone();
        active.ok_or_else(|| anyhow::anyhow!("No active MPRIS2 player"))
    }

    pub async fn refresh_player_info(&self, dbus_name: &str) -> Result<()> {
        let connection_guard = self.connection.lock().await;
        let connection = connection_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        let player_name = dbus_name.strip_prefix(MPRIS_PREFIX)
            .unwrap_or(dbus_name)
            .to_string();

        if let Ok(player_info) = self.get_player_info(connection, dbus_name, &player_name).await {
            let mut players = self.players.lock().await;
            players.insert(dbus_name.to_string(), player_info);
        }
        Ok(())
    }

    pub async fn play_pause(&self) -> Result<()> {
        let connection_guard = self.connection.lock().await;
        let connection = connection_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        let dbus_name_str = self.get_active_player().await?;
        let bus_name: OwnedWellKnownName = OwnedWellKnownName::try_from(dbus_name_str.as_str())
            .map_err(|_| anyhow::anyhow!("Invalid bus name: {}", dbus_name_str))?;

        let proxy = Proxy::new(
            connection,
            &bus_name,
            MPRIS_OBJECT_PATH,
            MPRIS_PLAYER,
        ).await?;

        proxy.call_method("PlayPause", &()).await?;
        Ok(())
    }

    pub async fn previous(&self) -> Result<()> {
        let connection_guard = self.connection.lock().await;
        let connection = connection_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        let dbus_name_str = self.get_active_player().await?;
        let bus_name: OwnedWellKnownName = OwnedWellKnownName::try_from(dbus_name_str.as_str())
            .map_err(|_| anyhow::anyhow!("Invalid bus name: {}", dbus_name_str))?;

        let proxy = Proxy::new(
            connection,
            &bus_name,
            MPRIS_OBJECT_PATH,
            MPRIS_PLAYER,
        ).await?;

        proxy.call_method("Previous", &()).await?;
        Ok(())
    }

    pub async fn next(&self) -> Result<()> {
        let connection_guard = self.connection.lock().await;
        let connection = connection_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        let dbus_name_str = self.get_active_player().await?;
        let bus_name: OwnedWellKnownName = OwnedWellKnownName::try_from(dbus_name_str.as_str())
            .map_err(|_| anyhow::anyhow!("Invalid bus name: {}", dbus_name_str))?;

        let proxy = Proxy::new(
            connection,
            &bus_name,
            MPRIS_OBJECT_PATH,
            MPRIS_PLAYER,
        ).await?;

        proxy.call_method("Next", &()).await?;
        Ok(())
    }

    pub async fn seek(&self, position: u64) -> Result<()> {
        let connection_guard = self.connection.lock().await;
        let connection = connection_guard.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        let dbus_name_str = self.get_active_player().await?;
        let bus_name: OwnedWellKnownName = OwnedWellKnownName::try_from(dbus_name_str.as_str())
            .map_err(|_| anyhow::anyhow!("Invalid bus name: {}", dbus_name_str))?;

        let proxy = Proxy::new(
            connection,
            &bus_name,
            MPRIS_OBJECT_PATH,
            MPRIS_PLAYER,
        ).await?;

        proxy.call_method("Seek", &(position as i64)).await?;
        Ok(())
    }

    pub async fn get_now_playing(&self) -> Result<Option<crate::NowPlaying>> {
        let dbus_name = match self.get_active_player().await {
            Ok(name) => name,
            Err(_) => return Ok(None),
        };

        // Refresh player info to get latest metadata
        if let Err(e) = self.refresh_player_info(&dbus_name).await {
            debug!("Failed to refresh player info: {}", e);
        }

        let players = self.players.lock().await;
        if let Some(player_info) = players.get(&dbus_name) {
            Ok(Some(crate::NowPlaying {
                title: player_info.title.clone(),
                artist: player_info.artist.clone(),
                album: player_info.album.clone(),
                album_art: player_info.album_art.clone(),
                position: player_info.position,
                length: player_info.length,
                playing: player_info.playing,
                player_name: player_info.player_name.clone(),
            }))
        } else {
            Ok(None)
        }
    }
}

// Global manager instance
static MANAGER: Lazy<Arc<Mutex<MprisManager>>> = Lazy::new(|| {
    Arc::new(Mutex::new(MprisManager::new()))
});

// Public API functions
pub async fn init() -> Result<()> {
    info!("Initializing MPRIS2 connection");
    let manager = MANAGER.lock().await;
    manager.connect().await?;
    Ok(())
}

pub async fn play_pause() -> Result<()> {
    let manager = MANAGER.lock().await;
    manager.play_pause().await
}

pub async fn previous() -> Result<()> {
    let manager = MANAGER.lock().await;
    manager.previous().await
}

pub async fn next() -> Result<()> {
    let manager = MANAGER.lock().await;
    manager.next().await
}

pub async fn seek(position: u64) -> Result<()> {
    let manager = MANAGER.lock().await;
    manager.seek(position).await
}

pub async fn get_now_playing() -> Result<Option<crate::NowPlaying>> {
    let manager = MANAGER.lock().await;
    manager.get_now_playing().await
}
