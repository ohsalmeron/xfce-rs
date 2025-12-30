use iced::widget::{container, row, mouse_area, button, text, column};
use iced::{Alignment, Element, Length, Task, Theme, Point};
use tracing::{info, warn};
use xfce_rs_ui::styles;

mod plugin_manager;
mod plugin_slot;
mod settings;
mod settings_app;

use plugin_manager::PluginManager;
use plugin_slot::PluginSlot;
use settings::PanelSettings;
use settings_app::SettingsApp;

pub fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    info!("XFCE.rs Panel starting");
    
    iced::application(PanelApp::new, PanelApp::update, PanelApp::view)
        .title(PanelApp::title)
        .theme(PanelApp::theme)
        .style(PanelApp::style)
        .window({
            let settings = PanelSettings::load();
            let (width, height) = settings.get_window_size(1920.0, 1080.0);
            let (x, y) = settings.get_window_position(1920.0, 1080.0);
            iced::window::Settings {
                size: iced::Size::new(width, height),
                position: iced::window::Position::Specific(iced::Point::new(x, y)),
                transparent: true,
                decorations: false,
                resizable: false,
                ..Default::default()
            }
        })
        .subscription(|app: &PanelApp| {
            // Only poll for settings changes if settings panel is not open
            // (to avoid conflicts with live editing)
            if !app.show_settings {
                iced::time::every(std::time::Duration::from_secs(2))
                    .map(|_| Message::ReloadSettings)
            } else {
                iced::Subscription::none()
            }
        })
        .run()
}

struct PanelApp {
    plugin_manager: PluginManager,
    plugins: Vec<PluginSlot>,
    settings: PanelSettings,
    context_menu: Option<ContextMenu>,
    mouse_pos: Point,
    show_settings: bool,
    settings_app: Option<SettingsApp>,
}

#[derive(Debug, Clone)]
struct ContextMenu {
    position: Point,
}

#[derive(Debug, Clone)]
enum Message {
    #[allow(dead_code)] // Will be used for future plugin management
    PluginLoaded(String),
    #[allow(dead_code)] // Will be used for future plugin management
    PluginUnloaded(String),
    Refresh,
    RightClick(Point),
    CloseContextMenu,
    OpenSettings,
    CloseSettings,
    SettingsChanged(settings_app::Message),
    MouseMoved(Point),
    ReloadSettings,
}

impl PanelApp {
    fn new() -> (Self, Task<Message>) {
        let plugin_manager = PluginManager::new();
        let settings = PanelSettings::load();
        
        // Discover and load plugins
        let plugins = plugin_manager.discover_plugins();
        info!("Discovered {} plugins", plugins.len());
        
        let app = Self {
            plugin_manager,
            plugins: plugins.into_iter().map(|p| PluginSlot::new(p)).collect(),
            settings,
            context_menu: None,
            mouse_pos: Point::ORIGIN,
            show_settings: false,
            settings_app: None,
        };
        
        (
            app,
            Task::perform(async move {
                // Small delay to let panel initialize
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                Message::Refresh
            }, |_| Message::Refresh),
        )
    }

    fn title(&self) -> String {
        String::from("XFCE.rs Panel")
    }

    fn theme(&self) -> Theme {
        if self.settings.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    fn style(&self, theme: &Theme) -> iced::theme::Style {
        iced::theme::Style {
            background_color: iced::Color::TRANSPARENT,
            text_color: theme.palette().text,
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RightClick(pos) => {
                self.context_menu = Some(ContextMenu { position: pos });
                Task::none()
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
                Task::none()
            }
            Message::OpenSettings => {
                self.context_menu = None;
                self.show_settings = true;
                let (settings_app, _) = SettingsApp::new(self.settings.clone());
                self.settings_app = Some(settings_app);
                Task::none()
            }
            Message::CloseSettings => {
                self.show_settings = false;
                // Reload settings from file (in case they were saved)
                let saved_settings = PanelSettings::load();
                if saved_settings != self.settings {
                    self.settings = saved_settings;
                    // Trigger reload to apply changes
                    return Task::perform(async {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        Message::ReloadSettings
                    }, |_| Message::ReloadSettings);
                }
                self.settings_app = None;
                Task::none()
            }
            Message::SettingsChanged(msg) => {
                if let Some(ref mut settings_app) = self.settings_app {
                    let _ = settings_app.update(msg);
                }
                Task::none()
            }
            Message::ReloadSettings => {
                // Check if settings file changed
                let new_settings = PanelSettings::load();
                let size_changed = new_settings.size != self.settings.size;
                let position_changed = new_settings.position != self.settings.position;
                let mode_changed = new_settings.mode != self.settings.mode;
                let dark_mode_changed = new_settings.dark_mode != self.settings.dark_mode;
                
                if size_changed || position_changed || mode_changed || dark_mode_changed {
                    info!("Settings changed, applying: size={}, position={:?}, mode={:?}", 
                        new_settings.size, new_settings.position, new_settings.mode);
                    
                    self.settings = new_settings.clone();
                    
                    // Update settings app if it's open
                    if let Some(ref mut settings_app) = self.settings_app {
                        let (new_app, _) = SettingsApp::new(self.settings.clone());
                        *settings_app = new_app;
                    }
                    
                    // Apply window size/position changes
                    // Note: Window size/position changes require restarting the panel
                    // This is common in desktop environments (xfce4-panel also requires restart for some changes)
                    if size_changed || position_changed || mode_changed {
                        let (width, height) = self.settings.get_window_size(1920.0, 1080.0);
                        let (x, y) = self.settings.get_window_position(1920.0, 1080.0);
                        info!("Window settings changed - size: {}x{}, position: ({}, {}). Restart panel to apply.", width, height, x, y);
                    }
                    
                    // Apply theme change
                    if dark_mode_changed {
                        info!("Dark mode changed to {}", self.settings.dark_mode);
                        // Theme is applied via the theme() method, which is called on each render
                    }
                }
                Task::none()
            }
            Message::MouseMoved(pos) => {
                self.mouse_pos = pos;
                Task::none()
            }
            Message::PluginLoaded(name) => {
                info!("Plugin loaded: {}", name);
                // Start the plugin
                if let Some(plugin_info) = self.plugins.iter().find(|p| p.plugin_name() == &name)
                    .map(|p| p.plugin_info().clone()) {
                    if let Err(e) = self.plugin_manager.start_plugin(&plugin_info) {
                        warn!("Failed to start plugin {}: {}", name, e);
                    } else {
                        // Update plugin slot status
                        if let Some(slot) = self.plugins.iter_mut().find(|p| p.plugin_name() == &name) {
                            slot.set_running(true);
                        }
                    }
                }
                Task::none()
            }
            Message::PluginUnloaded(name) => {
                info!("Plugin unloaded: {}", name);
                if let Err(e) = self.plugin_manager.stop_plugin(&name) {
                    warn!("Failed to stop plugin {}: {}", name, e);
                } else {
                    // Update plugin slot status
                    if let Some(slot) = self.plugins.iter_mut().find(|p| p.plugin_name() == &name) {
                        slot.set_running(false);
                    }
                }
                Task::none()
            }
            Message::Refresh => {
                // Reload plugins
                let plugins = self.plugin_manager.discover_plugins();
                let plugin_infos: Vec<_> = plugins.iter().cloned().collect();
                self.plugins = plugins.into_iter().map(|p| PluginSlot::new(p)).collect();
                // Auto-start all discovered plugins
                for plugin_info in plugin_infos {
                    if let Err(e) = self.plugin_manager.start_plugin(&plugin_info) {
                        warn!("Failed to start plugin {}: {}", plugin_info.name, e);
                    } else {
                        if let Some(slot) = self.plugins.iter_mut()
                            .find(|p| p.plugin_name() == plugin_info.name.as_str()) {
                            slot.set_running(true);
                        }
                    }
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // Create plugin slots in a row
        let plugin_elements: Vec<Element<'_, Message>> = self.plugins.iter().map(|slot| slot.view()).collect();
        let plugin_row = row(plugin_elements)
            .spacing(4)
            .align_y(Alignment::Center)
            .padding(4);

        let panel_content = mouse_area(
            container(plugin_row)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme| styles::glass_base(theme))
        )
        .on_right_press(Message::RightClick(self.mouse_pos))
        .on_move(Message::MouseMoved);

        // Build layers
        let mut layers = vec![panel_content.into()];
        
        // Context menu layer
        if let Some(menu) = &self.context_menu {
            let menu_content = container(
                column![
                    button(text("Settings").size(14))
                        .on_press(Message::OpenSettings)
                        .width(Length::Fill)
                        .padding(10)
                        .style(|theme, status| styles::app_card(theme, status)),
                    button(text("Close").size(14))
                        .on_press(Message::CloseContextMenu)
                        .width(Length::Fill)
                        .padding(10)
                        .style(|theme, status| styles::app_card(theme, status)),
                ]
                .spacing(5)
            )
            .width(150)
            .padding(5)
            .style(|theme| styles::glass_base(theme));

            layers.push(
                mouse_area(
                    container(
                        container(menu_content)
                            .padding(iced::Padding {
                                top: menu.position.y.max(0.0),
                                left: menu.position.x.max(0.0),
                                right: 0.0,
                                bottom: 0.0,
                            })
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                )
                .on_press(Message::CloseContextMenu)
                .on_right_press(Message::CloseContextMenu)
                .into()
            );
        }

        // Settings overlay layer
        if self.show_settings {
            if let Some(ref settings_app) = self.settings_app {
                let settings_view = settings_app.view()
                    .map(Message::SettingsChanged);
                
                layers.push(
                    mouse_area(
                        container(
                            container(settings_view)
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .align_x(Alignment::Center)
                                .align_y(Alignment::Center)
                                .padding(50)
                        )
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .style(|_theme| {
                            iced::widget::container::Style {
                                background: Some(iced::Background::Color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.7))),
                                ..Default::default()
                            }
                        })
                    )
                    .on_press(Message::CloseSettings)
                    .on_right_press(Message::CloseSettings)
                    .into()
                );
            }
        }

        iced::widget::Stack::with_children(layers).into()
    }
}
